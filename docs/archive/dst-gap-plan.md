# Fix `FellInDstGap`, and move DST logic into `time`

## Context

A reported session time that falls in the DST gap names an instant that never occurred. The code
today shifts it forward with jiff's `compatible()` (`src/session/csv.rs:517-523, 630`), flags
`FellInDstGap`, and excludes the session. The shift is a guess, and every document describing it —
`docs/time/README.md:131-139`, `docs/session/README.md:162`, the `AnomalyKind` rustdoc and its
`Display` prose — states the guess as though it were a reading.

The fix is to assign no instant at all. A gap session gets two sentinel timestamps that cannot be
mistaken for data, is excluded, and takes no further part in anything.

Two things ride along, both settled during design:

- The fold path is reformulated so that **all time arithmetic after the gap/fold decision is on UTC
  values**. Today the fold discriminator compares *local wall times*
  (`src/session/csv.rs:609-613`), which is a second spelling of a test the crate already owns.
- The DST mechanism moves out of `session::csv` into `time`, joining the other resolver,
  `map_local`, which is currently stranded in `session::ioi`.

## The design

### Gap

Both reported wall times are tested for the gap **first**. If either falls in it:

- `conn_start = time::UNPLACEABLE_START` (`Timestamp::MAX`), `conn_end = time::UNPLACEABLE_END`
  (`Timestamp::MIN`).
- `FellInDstGap` is raised once, whichever end it came from.
- **Every time-derived check is skipped** — no `duration_is_consistent`, no `is_on_grid`. Both
  would report artefacts of the sentinels. Non-time anomalies (`ZeroActiveChargeTime`,
  `ExcessiveAvgKw`) are unaffected.

Gap and fold cannot co-occur: Ontario's gap is 02:00–03:00 in March, its fold 01:00–02:00 in
November.

### Fold

Each reported wall time yields one instant, or two if it is in the fold. Form the start-end
combinations and keep those passing `duration_is_consistent(start, end, conn_duration)`
(`src/session/common.rs:121-130`) — the crate's one statement of the test, on UTC instants,
unchanged. Then count:

- **1** — that combination is the session.
- **2** — only reachable when both wall times are in the fold; the hour cancels on both sides, so
  the EDT/EDT and EST/EST combinations pass or fail together. Duplicate the record,
  `DstAmbiguousDuplicated`, ids suffixed `-EDT` / `-EST`.
- **0** — the sentinels, plus `DstUnresolvable` and `InconsistentDuration`.

A mismatched combination (EDT start, EST end) fails on its own, because it implies a duration an
hour out. No special case.

`SLACK_EARLY`, `SLACK_LATE` (`src/session/csv.rs:54-55`), `reproduces_reported_end`
(`csv.rs:609-613`) and `resolve_end`'s nearest-candidate tie-break (`csv.rs:631-642`) are all
**deleted**. `time::wall_clock_instant` leaves the DST path and serves Excel serials alone.

`DstUnresolvable` stays a separate kind: it says *why* no instant could be assigned.

### The sentinels, and what may touch them

`UNPLACEABLE_START` / `UNPLACEABLE_END` are `pub` in `time`. Any span built from them is inverted,
which is the point — code that tries to place such a session on a timeline should crash rather than
return a plausible answer.

Three functions panic on an inverted span and **stay that way**, as guards:

| | |
|---|---|
| `Session::adj_duration()` | `src/session/common.rs:265-267` |
| `SessionOverlap::duration()` | `src/session/common.rs:366-374` |
| `Session::intersects()` | `src/session/common.rs:215` |

Verified safe on the sentinels, needing no change: `adj_conn_start()`, `adj_conn_end()`, the four
`*_local()` accessors, `avg_kw()`, `is_inconsistent_duplicate()`. `adj_conn_end()` survives because
`Timestamp::MIN + 1s` already sits on the minute grid.

Only the callers that legitimately hold excluded sessions are changed — the report and the two
workbook halves, below.

## Changes by file

### New: `src/time/dst.rs`

Private `mod dst` in `time/`, re-exported through `time/mod.rs`'s existing three tiers so no new
module segment appears in any path. Holds:

- gap/fold classification of a wall time, and fold-candidate enumeration;
- `UNPLACEABLE_START` / `UNPLACEABLE_END`;
- `map_local` and `TzLocalMapping`, **moved verbatim** from `src/session/ioi.rs:79-112`. The items
  are ungated; only the `pub use` carries `#[cfg(feature = "historic")]`, matching how
  `time/mod.rs` already handles `TZ_OFFSETS`.

`docs/time/README.md:164-178` ("Two resolvers, deliberately") then describes two functions that
actually sit side by side.

### `src/session/csv.rs`

Rewrite `CsvSession::resolve` (`:479-597`) to the gap-first / combination-counting shape above.
Delete `resolve_end`, `reproduces_reported_end`, `SLACK_EARLY`, `SLACK_LATE`.
`SessionCsvError::Unresolvable` stays — `unambiguous()`, `earlier()` and `later()` still return
`Result`.

### `src/session/report.rs`

`push_excluded` (`:458-511`) branches on `FellInDstGap` and prints `—` in **From**, **To** and
**In interval**, leaving the other columns alone. That also keeps it out of `lenient_intersects`
(`common.rs:234-242`), which would otherwise swap the sentinels into a span covering all of time.

Rewrite the two hardcoded passages that describe only `InconsistentDuration`: `:175-178` and
`:372-373`.

### `src/session/excel.rs` — writer

Leave empty, for a `FellInDstGap` row: `conn_start_utc`, `conn_end_utc`, `adj_conn_start_utc`,
`adj_conn_end_utc`, `adj_conn_start_local`, `adj_conn_end_local`, and the `adj_conn_duration`
formula (`:290-348`). Subtracting two empty cells yields `0:00:00`, which is why the formula goes
too.

`conn_start_local` and `conn_end_local` are **still written** — they come from `Row.start_local` /
`Row.end_local` (`:291, 294`), verbatim from the CSV, and owe nothing to the sentinels.

### `src/session/excel.rs` — reader (`historic`)

`read_sessions` already parses the anomalies column first (`:626`). On `FellInDstGap`: substitute
the two sentinels and **do not read** `conn_start_utc` / `conn_end_utc` (`:627-628`) — a serial
round trip is only second-accurate, so the sentinels would not come back exact. Skip the
`duration_is_consistent` re-derivation at `:632-636`, and skip `check_stored_columns`' two
`check_instant` calls (`:729-734`) so blank cells are not reported as discrepancies.

`adj_conn_duration` needs nothing: the guard at `:740` already yields `None` for an inverted span,
so the column never enters the comparison loop at `:754`.

### Doc comments

`src/session/common.rs` — the `FellInDstGap` rustdoc (`:758-764`) and its `Display` string
(`:947-950`), both of which say "resolved forward"; and `:848-849`, `:876-877`, `:1032-1036`,
`:1081-1085`. Also `:1269` (`SessionNotes::excluded`) and `src/session/peak.rs:38-40`
(`IntervalEstimates::excluded_sessions`), which still document exclusion as
`InconsistentDuration`-only.

### Prose

| File | What |
|---|---|
| `docs/time/README.md:81-88` | Rewrite in descriptive tense; it never mentions the gap |
| `docs/time/README.md:104-139` | The derivation, restated in UTC combination-counting terms |
| `docs/session/README.md:162` | The `FellInDstGap` exclusion line |
| `docs/maintenance-manual.md:314-319` | The "Adding an `AnomalyKind`" checklist, which still says `InconsistentDuration` is the only excluding kind |
| `docs/session/time-reporting-uncertainty.md:128-129` | Same false claim |
| `docs/session/time-reporting-uncertainty.md:5-6` | Says "six" other `AnomalyKind`s; there are eight |
| `docs/fable-code-review-findings.md:44-55` | Append a one-line resolution note to C3; leave the finding itself as the record it is |

Nothing under `docs/archive/` is touched. `_todo/` is the user's to update.

### Broken links

Five references point at `docs/hydro_bill/archive/dst-energy-anomaly-pre-fix.md`; the file is at
`docs/archive/hydro_bill/...`. Fix `docs/time/README.md:45`, `docs/time/README.md:75`,
`src/time/base.rs:78`, `src/hydro_bill/billing_period.rs:25`,
`src/green_button/invoice_tests.rs:99`.

## Tests

All in `src/`, because reading a workbook back is `historic` and `grep -rn historic tests/` must
find nothing.

**Round trips** in `src/session/excel.rs`'s test module, following the existing `convert(tag, csv)`
→ `xlsx_to_sessions` pattern:

1. start in the gap · 2. end in the gap · 3. both in the gap, flagged once
4. fold resolved to EDT · 5. fold resolved to EST
6. fold ambiguous — both `-EDT` and `-EST` rows survive as distinct sessions
7. fold with no consistent combination — sentinels, `DstUnresolvable`, `InconsistentDuration`

Cases 1, 2, 3 and 7 each assert: the session lands in `Sessions::excluded`; its anomalies match
what the CSV reader produced; `conn_start == UNPLACEABLE_START` and `conn_end == UNPLACEABLE_END`
**exactly** (this is what catches a reader parsing cells instead of substituting); and
`Sessions::anomalies` carries no `WorkbookDiscrepancy`.

**Also:**

- A writer test reading the sheet directly: the six derived time cells and the duration formula are
  empty for a gap row, and `conn_start_local` / `conn_end_local` still hold the reported wall times.
- `the_two_readers_agree` gains a gap row in its fixture.
- The `Session_Report_Anomalies` golden gains a gap row, exercising the `—` cells end to end
  (`src/session/report_rendering_tests.rs:48`, `tests/session/report_rendering.rs:17`).
- `src/session/csv.rs:944-996` is rewritten, not extended — `dst_gap_resolves_forward_and_reports`
  asserts the behaviour being removed, and says so in its name.

## Verification

Per `CLAUDE.md`, every feature combination and all targets — a feature that gates whole targets
means a green default build proves nothing:

```sh
cargo check --all-targets
cargo check --all-targets --features historic
cargo test
cargo test --features historic
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps --features historic
grep -rn historic tests/          # expected to find nothing
cargo fmt --check
```

Fix the build before believing any test result. Then regenerate the two report goldens and read the
diff — the "Excluded sessions" table and its explanatory paragraph both change.
