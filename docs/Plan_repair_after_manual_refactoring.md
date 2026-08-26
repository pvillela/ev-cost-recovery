# Repair plan for the 3-commit API-focused refactoring

Written 2026-08-26, from the review asked for in
[`Prompt_Learn_from_manual_refactoring.md`](Prompt_Learn_from_manual_refactoring.md). The brief the
refactoring was done against is
[`archive/API_focused_refactoring.md`](archive/API_focused_refactoring.md).

Nothing in this plan has been applied yet.

## Context

Commits `f55c05f`, `e2c3c17`, `0b4f33e` (2026-08-26) reshaped the library around the `api`
module and the `ev_cost_recovery` GUI. The refactoring was done by hand. It left the default
build broken, two unit tests failing, 29 broken rustdoc links, and five documentation files
describing code that no longer exists.

Drawing the legacy boundary also exposed a defect that predates it: three integration tests whose
opening lines claim they exercise the public API go through the workbook round-trip instead
(Part 2F). That is not a consequence of the refactoring. Marking the legacy path is simply what
made the mislabelling visible, which is the boundary earning its keep.

This plan fixes all of that and closes the gaps between what the refactoring set out to do and
what it did.

---

## Part 1 — The goal, as the reviewer read it

The crate had one public surface: everything. The refactoring gives it **one narrow supported
surface — `api` plus the `ev_cost_recovery` GUI — and quarantines the rest.** Five moves serve
that:

1. **Narrow the public API.** `pub mod excel` → `mod excel` with named re-exports;
   `pub use espi::*` → `pub use espi::Feed`. A caller can no longer reach the internals of the
   session workbook writer or the ESPI parser.

2. **Split pure from I/O, and say which is which in the name.** I/O functions became verb-first
   or `X_to_Y`: `parse`→`parse_espi_xml`, `period_values_xml`→`read_gb_for_billing_period`,
   `write_workbook`→`write_gb_workbook`, `session_list`→`csv_sessions` / `xlsx_to_sessions`,
   `interval_estimates`→`xlsx_to_interval_estimates`, `estimates_from_report`→
   `estimates_from_sessions`. `peaks_io.rs`→`read_xml.rs`. The file name now says the direction.

3. **One error vocabulary, owned by the crate rather than by the API.** `ReadError` and
   `ConversionError` moved from `api::io` to a new `src/error.rs`, so a library module raises the
   typed error at its source instead of returning `Box<dyn Error>` for `api::io` to re-wrap.

4. **Quarantine the legacy path behind `historic`.** The xlsx round-trip
   (workbook → sessions → interval estimates) serves only `ev_peak_gui` and `ev_peak_cli`, not the
   API. It is now `session::excel::historic`, and those two binaries carry
   `required-features = ["historic"]`.

5. **Disambiguate names that only read clearly in their own file.** `Kind`→`ColKind`,
   `WriteReport`→`GbWriteReport`, `ConversionReport`→`SessionWriteReport`. `GbWriteReport` also
   absorbed the output path, which retired the `GbConversionReport` wrapper.

Every one of those is a real improvement and none of them should be undone.

---

## Part 2 — What is broken

### A. The default build does not compile

`cargo check --all-targets` (no features) fails in three places. `cargo build --release` is fine,
which is why this was easy to miss.

| Where | Why |
|---|---|
| `src/session/excel.rs:1054` | `mod test_historic` is `#[cfg(test)]` only; its `use super::historic::*` needs the feature too. |
| `tests/session/{consistency_band,report_rendering,segment_tiling}.rs` | Import `xlsx_to_sessions` / `xlsx_to_interval_estimates`. `tests/session/mod.rs` declares all three unconditionally, and they share one `tests/integration.rs` binary, so this breaks the whole integration target. |
| `examples/sessions.rs` | Imports `xlsx_to_sessions`. Cargo.toml has `[[bin]]` `required-features` for the two binaries but no `[[example]]` entry. |

**Consequence:** `.github/workflows/release-build.yaml:40` runs `cargo test --verbose`. The next
tag push fails before it builds anything.

### B. Two unit tests fail — the shipped behaviour changed, the tests did not

```
green_button::read_xml::test::a_missing_file_is_named
green_button::read_xml::test::a_closing_day_that_would_start_a_period_on_a_missing_date_is_refused
```

These tests are pre-existing and every assertion in them is byte-identical to `f8a2537`. The only
edit they took across the three commits is the call-site rename `period_values_xml` →
`read_gb_for_billing_period`. They passed before and fail now because the behaviour they watch
changed underneath them.

The old `peaks_io.rs` wrote the path into every error it raised:

```rust
let xml = fs::read_to_string(xml_path).map_err(|e| format!("{}: {e}", xml_path.display()))?;
let feed = parse(&xml).map_err(|e| format!("{}: {e}", xml_path.display()))?;
```

The new `read_gb_feed` (`src/green_button/read_xml.rs:67`) boxes the raw `io::Error` instead. And
`ReadError`'s `Display` (`src/error.rs:48`) deliberately drops `path`, on the premise — stated in
its own doc comment — that "both readers name the file they concern". That premise no longer holds.
A user who mistypes a Green Button path now gets `No such file or directory (os error 2)` and
nothing else.

That old string prefix is **not** the fix to restore; see Step 2.

### C. 29 broken rustdoc intra-doc links

Full list from `cargo doc --no-deps --features historic`. All are the renames above:

- `csv::session_list` ×12 — `src/api/io.rs:77,129,184,226,279,329,401`, `src/session/common.rs:920,921`, `src/session/csv.rs:9,16,64`, `src/session/excel.rs:141,150,558,566`
- `ConversionReport` ×3 — `src/api/io.rs:475`, `src/session/common.rs:841,923`, `src/session/excel.rs:157`
- `crate::session::excel::*` ×2 — `src/api/io.rs:459,471` (module is private now)
- `green_button::write_workbook` — `src/api/io.rs:497`
- `crate::error::ApiError` — `src/api/pure/recovery.rs:185` (it is `crate::api::error::ApiError`)
- `OnExistingWorkbook::Replace` — `src/error.rs:86` (moved away from where that type lives)
- `crate::session::interval_estimates` — `src/session/common.rs:53`
- `crate::green_button::Readings` — `src/session/common.rs:913` (no longer public)
- `Sessions`, `Sessions::logs` — `src/session/excel.rs:33,42`

### D. Stale prose in source

- `src/error.rs:12` — "One of the two error kinds **this module** raises on its own." Copied
  verbatim from `api::io`. `error.rs` raises nothing.
- `src/error.rs:16` — the "both readers name the file" claim, now false (see B).
- `src/api/io.rs:17` — "How this file is laid out … then the two error enums". They left.
- `src/api/io.rs:20` — "`session::excel` arrives as a module rather than as its two functions".
  The import is now `session::{self, ...}`.
- `src/api/io.rs:487` — "derives the same one from the same function". There is no shared function
  any more (see Part 3, item 3).
- `src/api/io.rs:533` — ``/// - `gb_xml` - ...`` but the parameter is now `xml_path`.
- `src/green_button/read_xml.rs:114` — `// cargo test --lib -- green_button::peaks_io::test`.
- `src/session/excel.rs:3-5` — module doc presents write and read as a symmetric pair. The read
  half only exists under `historic`.
- `src/session/csv.rs:158` — "Shared by [`session_list`] and ...".
- `src/bin/ev_peak_cli.rs:92` — comment naming `session::excel::session_list`.
- `src/api/io.rs:679` — test doc comment: "`workbook_path` decides and returns".
- `tests/green_button/peaks_io.rs` — file still named after the module that was renamed.

### E. Documentation files

**`docs/api-conversions.md`** — 7 of ~10 identifier references are dead, and two whole sections now
assert the opposite of the truth:
- L10 `ConversionReport`→`SessionWriteReport`; L12 `GbConversionReport`→`GbWriteReport`
- L15 — `GbConversionReport` pairing premise is gone entirely
- L22-23 — `ReadError`/`ConversionError` are no longer spellable as `api::io::*`
- L29 `session::csv::session_rows`→`csv_session_rows`, **and it is `pub(super)` now**, so the
  sentence's argument ("already reachable on its own") is false
- L30 `green_button::parse`→`parse_espi_xml`; L35 `write_workbook`→`write_gb_workbook`
- **L48-53, "`session::excel::workbook_path` is now public"** — obsolete. The function is gone, its
  guarding test is gone, and the name is now derived in three places.

**`docs/session/README.md`** — L206 "Both `session_list` functions"; and the whole two-step workflow
it is built around (L21-53) is now the `historic` path. The feature is never mentioned.

**`README.md`** — L166 `cargo test # everything` does not compile. L167
`cargo run --example sessions` does not compile. L170 lists `ev_peak_cli` without saying it needs
the feature. L165 `cargo build --release` no longer builds `ev_peak_gui`.
*(Pre-existing: L118 and L45-93 describe two GUI tabs; there are four.)*

**`docs/maintenance-manual.md`** — L275 `SessionReport::new`→`Sessions::from_session_lists`;
L298 `SessionReport::excluded`→`Sessions::excluded`; L54, L74, L171-177, L391 prescribe `cargo test`
invocations that no longer compile.

**`docs/green_button/README.md`** — L128 same `cargo test` problem. Layout table (L133-145) has no
row for `read_xml.rs`, the module's read entry point.

**Nothing anywhere documents the `historic` feature.**

### F. Three tests claim to exercise the public API and do not

This is a pre-existing defect that the `historic` gate exposed. It is not a consequence of the
refactoring; drawing the legacy boundary is simply what made it visible.

`tests/session/consistency_band.rs` opens *"exercised end to end through the public API"*.
`tests/session/segment_tiling.rs` opens *"Driven through the public API only — a CSV in, an
`IntervalEstimates` out — so what is pinned here is behaviour a caller can actually depend on"*.
`tests/session/report_rendering.rs` says its cases are *"reached through the real path, from a
CSV"*.

None of the three is true. All three go CSV → `session_csv_to_xlsx` → `xlsx_to_sessions` /
`xlsx_to_interval_estimates`, which is the workbook round-trip — the very thing now marked
`historic`.

A public route exists and always did: `IntervalEstimates` is an API output type, returned inside
[`PowerEstimates`](../src/api/pure/peak_power.rs)'s `kw_estimates` and `kva_estimates` fields by
`api::peak_power`. The tests took the shorter route instead and then described it as the API one.

So the finding is not that coverage moved. It is that these tests never pinned what their doc
comments claim, and the gate is what surfaced it. Step 3 fixes the tests.

`report_rendering.rs` carries a second, unrelated problem: it also pins the **site-load table**
golden, which has nothing to do with sessions, workbooks or `historic`.

---

## Part 3 — Where the refactoring fell short of its own goals

Ordered by how much it matters.

**1. The default build was never checked.** Every failure in Part 2A is caught by one
`cargo check --all-targets`. `cargo build --release` passing is not evidence — the feature gating
is precisely what makes those two differ now.

**2. Two safe-looking edits combined to drop the filename from an error, and the broken build hid
it.** Moving `ReadError` to `src/error.rs` changed no behaviour; that part was a clean relocation.
What lost the path was the pair of edits below, neither of which is wrong on its own:

- Extracting `read_gb_feed` out of `period_values_xml` boxed the raw `io::Error` instead of
  prefixing it with the path, as the inline version had.
- The return type became `ReadError`, whose `Display` deliberately ignores its own `path` field
  (`src/error.rs:48`).

If `Display` printed `path`, the first edit would not matter. If the reader still prefixed, the
second would not matter. Together, nobody prints the path — and nothing type-checks that, because
`path` is still populated, merely never read.

It went unseen because item 1 hid it: `cargo test` with no features does not compile, so the two
failing assertions only appear under `cargo test --features historic`. A broken default build does
not just stop the build; it conceals every failure downstream of it.

**3. A comment claims a single definition of the workbook name that does not exist.**

`with_extension("xlsx")` is written out in seven places, across both conversions and three
binaries:

| | Session | Green Button |
|---|---|---|
| library | `src/session/excel.rs:170` | — |
| api | `src/api/io.rs:486` | `src/api/io.rs:525` |
| `ev_cost_recovery` | `src/bin/ev_cost_recovery/state.rs:602` | `src/bin/ev_cost_recovery/state.rs:631` |
| other binaries | `src/bin/ev_peak_gui/state.rs:98` | `src/bin/gb_peak_values.rs:114` |

Seven copies of `with_extension("xlsx")` is not a defect worth a utility. The expression is
self-evident and cannot drift into meaning something else. The deleted `excel::workbook_path`, and
the test that pinned it, were ceremony around a one-liner — and *session-scoped* ceremony at that,
which covered four of the seven sites and left the Green Button half deriving its own name. A
utility that half the callers ignore is two definitions wearing the costume of one; deleting it
was right.

The defect is `src/api/io.rs:487`, which still tells a reader the callee "derives the same one from
the same function". There is no such function. Either every one of the seven sites calls one
utility, or none does and the comment goes. Step 4 takes the second.

**4. The pure/I-O split is not clean.** `src/session/peak.rs` is otherwise pure and now hosts
`xlsx_to_interval_estimates`, which opens a file. This is goal 2's own rule broken by goal 4's
change.

**5. `green_button::for_test` ships a hole in the wall goal 1 just built.** `espi` was made private;
`pub mod for_test { pub use super::espi::*; }` re-exports all of it publicly, so four integration
tests can reach `parse_espi_xml`. It is in the rendered docs and in the public API.

**6. The deletion pass in the brief was not done.** The brief said code supporting neither API
nor binary "should be marked for deletion". Nothing is marked. `MergedSessions` was privatised,
which is adjacent, but no deletion candidates are recorded anywhere.

**7. Segregation of historic targets is half-finished.** `[[bin]]` entries exist; `[[example]]` does
not; CI has no historic job; no doc says how to build or test either half.

**8. Small residue.** `src/api/io.rs:486-489` computes `output_path` only to clone it into a
discarded call, then does `let ret = ...; Ok(ret)`. `src/bin/ev_cost_recovery/convert.rs:222` binds
`let report = &outcome;` on an already-borrowed value. `src/session/peak.rs:100` puts
`#[cfg(...)]` above the doc comment rather than below it. `MergedSessions` is private but its fields
are still `pub`.

### Which step addresses which item

| Item | Step |
|---|---|
| 1. Default build never checked | **Step 1** fixes the three breaks and makes CI run both configurations. **Step 11** adds the check to the standing list, so it is caught next time rather than found by review. |
| 2. Filename dropped from an error | **Step 2** — structured `GbReadError`, path in the variant. **Step 1** is what makes the two failing tests visible at all. |
| 3. A comment claims a single definition that does not exist | **Step 4** — delete the claim. No utility is restored: one that every caller uses, or none. |
| 4. Pure/I-O split not clean | **Step 5** — `xlsx_to_interval_estimates` moves into `session::excel::historic`, leaving `peak.rs` pure. |
| 5. `for_test` is public API | **Step 6** — `#[doc(hidden)]` plus a comment. **Step 10** asks the further question of whether it should exist. |
| 6. Deletion pass not done | **Step 10** — a survey producing `docs/deletion-candidates.md`. Deliberately a list, not deletions. |
| 7. Historic segregation half-finished | **Step 1** items 3 and 4 (`[[example]]`, CI) and **Step 8** item 1 (`docs/historic-feature.md`). |
| 8. Small residue | **Step 9**, except the `api::io` lines, which **Step 4** takes, and the misplaced `#[cfg]`, which **Step 5** dissolves. |

Part 2F — the three mislabelled tests — is not in this list because it is not a shortfall of the
refactoring. **Step 3** addresses it.

---

## Part 4 — The fix

### Step 1 — Make the default build compile

1. `src/session/excel.rs:1053` — add `#[cfg(feature = "historic")]` beside the existing
   `#[cfg(test)]` on `mod test_historic`.
2. `tests/session/mod.rs` — put `#[cfg(feature = "historic")]` on all three `mod` lines, as a
   stopgap so the build is green. Step 3 removes every one of those gates again by fixing the
   tests; do not leave them in place as the answer.
3. `Cargo.toml` — add, beside the two `[[bin]]` blocks:
   ```toml
   [[example]]
   name = "sessions"
   required-features = ["historic"]
   ```
4. `.github/workflows/release-build.yaml:40` — run **both** configurations, not one:
   ```yaml
   run: |
     cargo test --verbose
     cargo test --verbose --features historic
   ```
   One command cannot cover both halves: with the feature on, the default build is never
   exercised, which is exactly the blind spot that let Part 2A through. The release build itself
   stays featureless, which is the segregation working.

**Gate:** `cargo check --all-targets` and `cargo check --all-targets --features historic` both clean.

### Step 2 — Give the Green Button reader a structured error

**Rule: information that reaches a message lives in a field of the error variant. No path is ever
pre-baked into a string.** `format!("{}: {e}", path.display())` is out — including where the old
`peaks_io.rs` did it, which is not a precedent to restore.

`BillError` (`src/hydro_bill/bill_pdf.rs:182-198`) is the in-repo model: `path` is a field on every
variant and `Display` formats it from that field. The other three causes `ReadError` wraps should
match it. `ReadError::Display` then keeps delegating to the cause, and `src/error.rs:16` becomes
true because every cause really does name its own file.

Where the four stand:

| Cause | Structured enum? | Path in the variant? |
|---|---|---|
| `BillError` | Yes | **Yes — nothing to do** |
| `ChargesReportError` | Yes | No, in any of its five variants |
| Green Button | No — `Box<dyn Error>` | No. **This is the regression.** |
| Session CSV (`csv_sessions`) | No — `Box<dyn Error>` | No; string-baked at `src/session/csv.rs:94` |

**2a. Green Button.** Replace the `Box<dyn Error>` out of `read_xml.rs` with an enum in that file:

```rust
/// Why a Green Button export could not be read.
#[derive(Debug)]
pub enum GbReadError {
    /// The file could not be opened or read.
    Unreadable { path: PathBuf, cause: io::Error },
    /// The bytes were read but are not a well-formed ESPI feed.
    Malformed { path: PathBuf, cause: Box<dyn Error> },
    /// The feed carries no reading in the billing period asked for.
    PeriodNotCovered {
        path: PathBuf,
        period_ending: Date,
        /// The span the feed does cover, `None` when it carries no readings at all.
        covers: Option<(Date, Date)>,
    },
    /// The closing date and the closing day describe different calendars. No `path`: this is
    /// settled before the file is opened.
    Calendar(CalendarProblem),
}
```

`Display` writes `path` from the field. `PeriodNotCovered` in particular retires the private
`coverage(&Readings) -> String` helper (`read_xml.rs:100`), which pre-formats a whole sentence: the
two dates belong in the variant and the sentence belongs in `Display`.

`read_gb_feed` and `read_gb_for_billing_period` return `GbReadError`; `api::io` boxes it into
`ReadError::GreenButton { path, cause }` as now. `gb_peak_values` (`src/bin/gb_peak_values.rs:85`)
calls `read_gb_feed` directly and needs no change — it prints the error, which now names the file.

Give `read_gb_feed` a doc comment while there; it is new public API and has none.

**2b. `ChargesReportError`.** Add `path: PathBuf` to all five variants and write it from the field
in `Display` (`src/charges_report.rs:68-133`). Contained. Direct callers are `api::io.rs:416` and
`tests/charges_report/real_reports.rs:45` — the latter currently adds the path by hand in its
`panic!` and can stop.

**2c. `csv_sessions` — named follow-up, not this pass.** Its `Box<dyn Error>` is assembled from
ad-hoc errors raised throughout the row parsing in `src/session/csv.rs`; giving it a structured
enum is a refactor of that file rather than a patch, and it is pre-existing debt rather than
something these three commits broke. Its only non-test direct caller is `api::io.rs:623`, so the
blast radius is small when it is done.

**Gate:** the two `green_button::read_xml::test` failures pass **with no edit to either test** —
their assertions are `err.contains("/nonexistent/feed.XML")`, which a `Display` that writes the
field satisfies exactly as the old string-prefix did.

### Step 3 — Make the three session tests test what they say they test

This repairs Part 2F. **No new public API.** `IntervalEstimates` is already an API output type —
`api::peak_power` returns it in `PowerEstimates.kw_estimates` and `.kva_estimates`
(`src/api/pure/peak_power.rs:46-48`). The tests did not lack a route; they took a shortcut and
then described the shortcut as the route.

Each of the three has a different honest home. Decide per test, by what it actually pins:

**`consistency_band.rs` — make it a unit test.** What it pins is `duration_is_consistent`'s
consequence: which sessions `Sessions::from_session_lists` puts in which bucket, and which reach
`estimates_from_sessions`. That is internal pure logic, not API surface. Move it into
`src/session/` as a `#[cfg(test)]` module calling `csv_sessions` and `estimates_from_sessions`
directly, and rewrite the opening line to say so. Its CSV fixtures are reachable from `src/` by
the pattern `src/golden.rs` already uses. Runs ungated.

**`segment_tiling.rs` — same treatment.** It pins how sessions land on quarter-hour segments,
which is `segments_for_ioi` and `estimates_from_sessions`. Same move, same rewrite of its opening
claim.

**`report_rendering.rs` — split it.**
- `the_site_load_table_matches_its_golden_file` calls `site_load_report()` and has nothing to do
  with sessions or workbooks. Move it to its own ungated `tests/session/site_load_golden.rs`.
- The two `IntervalEstimates` golden cases pin `to_markdown`'s layout. `to_markdown` *is* API, so
  this one can genuinely go through `api::peak_power` — at the cost of a Green Button fixture and
  a bill alongside each session CSV. If that is more fixture than the layout check is worth, make
  it a unit test like the other two instead. Either way its "reached through the real path"
  sentence has to become true or go.

Whichever route each takes, the goldens should not change — `estimates_from_sessions` is the same
call in every case. If `cargo test` reports a golden diff, read it before regenerating; a diff
means the workbook round-trip was altering something on the way through, which is itself worth
knowing.

**Gate:** `cargo test` with no features runs the consistency-band, tiling and site-load checks and
passes. No test file anywhere claims a route it does not take.

### Step 4 — Delete the claim, not the seven call sites

**No utility.** Either one utility that every caller uses, or none — and a `session::workbook_path`
is not the first: it reaches four of the seven sites in Part 3 item 3 and leaves the Green Button
conversion deriving its own name. Restoring it would put back the *appearance* of a single
definition while the Green Button half carried a second one, which is worse than seven honest
copies of a self-evident expression. Deleting it was right; it is not coming back.

So:

1. `src/api/io.rs:487` — delete the comment claiming the callee "derives the same one from the same
   function". Replace it with what is actually true: the api derives the output name in order to
   refuse before the conversion runs, and the callee derives it again because it will not accept
   one as an argument — a caller that could pass a path could send the workbook somewhere the
   check never looked.
2. `src/api/io.rs:679` — the test doc comment naming `workbook_path`; retarget it at
   `checked_workbook_path`, which is what that test actually exercises.
3. `docs/api-conversions.md` L48-53 — the whole "`session::excel::workbook_path` is now public"
   section goes. Covered by Step 8 item 2, which archives that file.
4. `src/api/io.rs:486-489` — drop the `output_path` binding that exists only to be cloned into a
   discarded call, and the `let ret = ...; Ok(ret)`.

Leave all seven derivation sites alone.

### Step 5 — Move `xlsx_to_interval_estimates` out of the pure module

Move it into `src/session/excel.rs`'s `historic` module, beside `xlsx_to_sessions`. Re-export from
`session` under `#[cfg(feature = "historic")]` exactly as now, so `ev_peak_cli` and `ev_peak_gui`
are untouched. `peak.rs` goes back to pure, and its `#[cfg]`-gated `use std::path::Path` goes away.

### Step 6 — Close `for_test`

> **Superseded, same day.** `#[doc(hidden)]` was done as written below, and then the follow-up
> question Step 10 raised got answered: the tests never needed the export at all, because they were
> re-implementing `read_gb_feed` by hand. `for_test` is deleted, `period_values` is `pub(crate)`,
> and the three test sites that needed it moved into `src/green_button/`. See
> [`deletion-candidates.md`](deletion-candidates.md), "`green_button::for_test`, and what came of
> it".

`src/green_button/mod.rs:21` — mark it `#[doc(hidden)]` and say in a comment that it exists for
`tests/` and is not API. Keeps the four integration tests working, keeps it out of the docs and out
of what a caller is invited to use.

Also rename `tests/green_button/peaks_io.rs` → `tests/green_button/read_xml.rs` and update
`tests/green_button/mod.rs:4`.

### Step 7 — Doc comments and source prose

Fix all 29 links from Part 2C and all the stale prose from Part 2D. The mechanical map:

| Old | New |
|---|---|
| `csv::session_list` | `csv::csv_sessions` |
| `session_list` (excel side) | `historic::xlsx_to_sessions` |
| `ConversionReport` | `SessionWriteReport` |
| `crate::session::excel::X` | `crate::session::X` |
| `green_button::write_workbook` | `green_button::write_gb_workbook` |
| `crate::error::ApiError` | `crate::api::error::ApiError` |
| `crate::green_button::Readings` | drop the link, keep the name in backticks |
| `crate::session::interval_estimates` | `crate::session::xlsx_to_interval_estimates`, and note in the prose that it is `historic` |

Rewrite by hand, not by substitution: `src/error.rs:12` ("this module raises on its own"),
`src/api/io.rs:17` (layout paragraph), `src/api/io.rs:20` (import comment), `src/session/excel.rs:3`
(module doc — say the read half is `historic`).

**Gate:** `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` clean both with and
without the feature.

### Step 8 — Documentation files

1. **New `docs/historic-feature.md`.** What `historic` means, what is behind it
   (`session::excel::historic`, `xlsx_to_sessions`, `xlsx_to_interval_estimates`, `ev_peak_cli`,
   `ev_peak_gui`, `examples/sessions.rs`), why, and the two build/test invocations. This is the doc
   the change most needs and does not have. Say plainly that nothing in `tests/` should need the
   feature: a test that does is either testing the legacy path on purpose and should say so, or is
   taking a shortcut — which is exactly what Part 2F found.
2. **`docs/api-conversions.md` — archive rather than patch.** Dated 2026-08-25, one day before the
   refactoring. It is a journal of design decisions, and its spine is a section called "Three
   adjustments this forced on existing code" — one of which the refactoring reversed, one of which
   (`workbook_path`) no longer exists, and one of which (`OnExistingWorkbook`) survives. Patching
   it means re-justifying reversed decisions in prose. Move it to `docs/archive/` under the archive
   README's existing convention, and put what is still true — the two conversions' signatures, why
   there is no pure counterpart, `OnExistingWorkbook` — into the `api::io` module doc, where it
   will be maintained. To keep it live instead: L10 `ConversionReport`→`SessionWriteReport`, L12
   and L15 for `GbWriteReport`'s own `path` field, L22-23 for `crate::error`, L29
   `csv_session_rows` (and note it is `pub(super)` now), L30 `parse_espi_xml`, L35
   `write_gb_workbook`, L48-53 deleted.
3. **`README.md`** — L165-170: `cargo test --features historic`,
   `cargo run --features historic --example sessions`, and a sentence saying `cargo build --release`
   builds the app only. Point at `docs/historic-feature.md`.
4. **`docs/session/README.md`** — L206 "both `session_list`"; a note at L21 that step 2 of the
   workflow is the historic path.
5. **`docs/maintenance-manual.md`** — L275, L298 type names; `--features historic` on L54, L74,
   L173, L391.
6. **`docs/green_button/README.md`** — L128 command; add a `read_xml.rs` row to the layout table.
7. **`docs/archive/API_focused_refactoring.md`** — untracked, not gitignored, and git holds no copy.
   It is the brief this work was done against; commit it as the record.

### Step 9 — Clear the residue

The leftovers from Part 3 item 8 that no other step picks up:

- `src/bin/ev_cost_recovery/convert.rs:222` — `let report = &outcome;` takes a second reference to
  an already-borrowed `&GbWriteReport`. It compiles through auto-deref. Use `outcome` directly.
- `src/session/common.rs:367` — `MergedSessions` is now private but its fields are still `pub`.
  Drop the `pub`, or make it `pub(super)` to match `SessionRows` beside it.
- `src/session/peak.rs:100` — `#[cfg(feature = "historic")]` sits above the doc comment rather than
  below it. Dissolved by Step 5, which moves the function out; if Step 5 is skipped, fix it here.
- `src/api/io.rs:486-489` — handled in Step 4.

None of these changes behaviour. Do them in one commit, separate from everything else, so the
review of the real changes is not diluted.

### Step 10 — The deletion survey the brief asked for

Part 3 item 6. The brief said library code supporting neither the API nor a binary "should be
marked for deletion", and nothing was marked. **This step produces a list, not deletions** —
what to remove is your call, and a survey is what makes that call possible.

Method, since `dead_code` says nothing about `pub` items:

1. For each item re-exported from `src/lib.rs`'s public modules, find its callers under `src/api/`,
   `src/bin/`, `tests/` and `examples/`.
2. Sort into: **API** (reached from `src/api/`), **binary-only** (reached from `src/bin/` but not
   `src/api/`), **test-only**, and **unreached**.
3. Binary-only items reached only by `ev_peak_cli` / `ev_peak_gui` are `historic` candidates the
   gate has not yet caught. Unreached items are deletion candidates.
4. Write the result to `docs/deletion-candidates.md` with a one-line justification each.

Two are already known and can seed the list: `session::MergedSessions` (privatised, so its old
public callers are gone) and `green_button::for_test` (Step 6 hides it; whether it should exist at
all is a deletion question).

Worth doing after Steps 1-8, not before: the `historic` gate and the test re-pointing in Step 3
both change what counts as reached.

### Step 11 — Write the learnings down

Create `CLAUDE.md` at the repository root (none exists today) with a short "after a rename or a
module move" checklist:

- **When the user is refactoring pre-existing code, the prior state is not the baseline.** A test
  that fails after their change may be pinning a kludge from before. Establish what the behaviour
  *should* be before reaching for `git show HEAD~1`. Restoring
  `format!("{}: {e}", path.display())` was proposed here purely because the tests encoded it — and
  it was the very thing being refactored out.
- **Information that reaches a message belongs in a field of the error variant, formatted at
  `Display`.** Never pre-baked into a string. `BillError` (`src/hydro_bill/bill_pdf.rs:182`) is
  this repo's model.
- Run `cargo check --all-targets` **with and without every feature**. A green
  `cargo build --release` proves nothing once features gate targets.
- Run `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` in both
  configurations. Intra-doc links are the cheapest rename detector the crate has.
- A moved doc comment is a claim about its new home. Re-read it there; `error.rs` inherited
  `api::io`'s "this module raises" and an invariant that had just been broken.
- **When a field stops being read, nothing tells you.** `ReadError::path` stayed populated while
  the last code that printed it went away, and the compiler had no opinion. Extracting a function
  and changing its return type are each safe; do both at once and check what falls between them.
- **A shared utility that only half the callers use is two definitions, not one.** Before
  extracting one, list every site that would have to call it. `excel::workbook_path` was
  session-scoped and reached four of seven derivation sites; the Green Button half always derived
  its own. One utility everybody uses, or none.
- **A test's opening claim about which route it takes is an assertion, and it must be checked
  against the imports directly below it.** Three files here said "through the public API" and went
  through the workbook round-trip. Nobody noticed for months, because prose is not compiled. When
  the shortcut is shorter, say the test is a unit test — do not describe it as the API path.
- Adding a cargo feature is a test-coverage decision. For every target pushed behind one, ask what
  a default `cargo test` stops checking — and whether it belonged there in the first place.
- `grep` the `docs/` tree and `.github/` for the old name too, not just `src/` and `tests/`.
- A public module that exists for tests (`for_test`) is public API. `#[doc(hidden)]` at minimum.

---

## Verification

```sh
# 1. Both configurations compile, all targets.
cargo check --all-targets
cargo check --all-targets --features historic

# 2. Both configurations test clean. Expect the consistency-band, segment-tiling and
#    site-load tests to run in BOTH, and the historic run to add ~5 more.
cargo test
cargo test --features historic

# 3. No broken doc links in either configuration.
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps --features historic

# 4. The regression in Part 2B, by hand.
cargo run --bin gb_peak_values -- /nonexistent/feed.XML   # must name the file

# 5. The historic binaries still build.
cargo build --features historic --bin ev_peak_cli --bin ev_peak_gui

# 6. The goldens are unchanged. If step 2 reports a diff, read it before regenerating -- a diff
#    means the workbook round-trip was altering something on the way through.
UPDATE_REPORT_GOLDEN=1 cargo test -- session::

# 6b. No test needs the feature any more. Expect no hits at all:
grep -rn 'historic' tests/          # examples/sessions.rs still needs it, and should

# 7. `for_test` is gone from the rendered docs.
grep -r for_test target/doc/ev_cost_recovery/    # expect no hits
```

Also, by eye: launch the GUI (`cargo run --bin ev_cost_recovery`), convert a session CSV and a
Green Button XML on the Convert tab, and confirm both outcomes still name the file they wrote —
`GbWriteReport::path` replaced `GbConversionReport::output_path` and that is the only thing
carrying it now.
