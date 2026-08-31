# Code Review Findings

Reviewed 2026-08-31. Scope: `src/` (17,551 lines of Rust), plus a read of the relevant `docs/`
for the invariants the code claims.

Baseline before review:

- `cargo check --all-targets` and `--features historic` — clean.
- `cargo test --features historic` — all pass (3 integration tests `ignored`, data-gated).
- `cargo clippy --all-targets --features historic` — one warning (`espi.rs:107`, see §Clippy).

So everything below is beyond what the compiler and the existing tests catch.

---

## Bugs (code errors)

### 1. HIGH — `collect_session_anomalies` panics on an inverted session that shares an id

`src/session/peak.rs:254` · `src/session/common.rs:434-448, 1058-1088`

`collect_session_anomalies` chains the report-level anomalies (the `DuplicateId` set) through the
strict `Session::intersects`:

```rust
.filter(|a| a.session.intersects(interval))
```

`Session::intersects` panics when a session's adjusted span is inverted (`adj_conn_end <
conn_start`) — its own doc (`common.rs:199-216`) states this is deliberate, on the invariant that
an inverted record is flagged `InconsistentDuration` and sorted into `Sessions::excluded`, so it
"never reaches the estimating logic at all".

That invariant is broken by the `DuplicateId` path:

1. `MergedSessions::merge_sessions` runs `duplicate_id_anomalies` over the **pre-bucketing** merged
   list (`common.rs:420`), so an inverted record that shares a `Charge_Session_ID` with another
   record gets a `DuplicateId` anomaly.
2. `Sessions::from_session_lists` stores those anomalies verbatim in `Sessions::anomalies`
   (`common.rs:1071`) — it does not filter out sessions it then buckets into `excluded`.
3. `estimates_from_sessions` passes `sessions.anomalies` to `collect_session_anomalies`, which calls
   the strict `intersects` on them.

**Trigger:** two records sharing a `Charge_Session_ID` (Evolute reuses ids — the June 2026 report
carries `S37487` twice, per the code's own docs) where one of them is inverted. The whole estimate
crashes in `Interval::from_start_end` (`time/base.rs`).

**Fix:** filter `InconsistentDuration` sessions out of the report-anomaly chain in
`collect_session_anomalies`, or call `Session::lenient_intersects` there — the method the doc at
`common.rs:222-235` says exists precisely for the report's "excluded session" case.

**Stale comments caused by this bug** (fixing the bug restores them):
- `peak.rs:35` — "Sessions excluded outright are *not* here" is false: an excluded session can
  arrive via its `DuplicateId` anomaly.
- `common.rs:205-206` — "so it never reaches the estimating logic at all" — same reason.

### 2. MEDIUM — historic workbook reader does not re-check duration consistency

`src/session/excel.rs:595-655` (`read_sessions`, behind the `historic` feature)

The CSV path re-derives `InconsistentDuration` via `duration_is_consistent` and buckets the record
into `Sessions::excluded`. The workbook read-back path does not: it reads the stored `anomalies`
cell verbatim (`excel.rs:619`), so a workbook edited to put `conn_end` before `conn_start` (or with
a cleared anomalies cell) lands in `Sessions::sessions`, and `Session::intersects` panics downstream
the same way as bug 1.

`check_stored_columns` (`excel.rs:632`) only flags stored-vs-recomputed *derived* columns as
`WorkbookDiscrepancy`; it does not re-run the consistency check or re-bucket.

**Fix:** mirror the CSV path — run `duration_is_consistent` on every workbook row and bucket the
failures into `excluded` before constructing the `Sessions`.

### 3. MEDIUM — Charges Report kWh column does not strip thousands separators

`src/charges_report.rs:384, 435-450, 456-464`

`total_kwh` is parsed through `number()`, which is a bare `f64::parse()`. The `Cost` column goes
through `money()`, which strips `$` and `,` first — and whose doc notes the comma-stripping exists
"since a busy month could reach four figures". kWh is *more* likely than dollars to reach four
figures: a charger's month easily tops 1,000 kWh.

**Trigger:** a kWh cell written `12,345.6` (a plausible export for a busy month) rejects the whole
file with `BadValue`, where the same input would pass if the parser matched `money()`'s behaviour.

**Fix:** route kWh through the same comma-stripping as `money()` (a shared `strip_separators`
helper, or `money`-style cleaning).

---

## Gaps (missing validation / unhandled edge cases)

### 4. MEDIUM — reimbursement `verdict`/`remittance_verdict` narrate NaN as a definite result

`src/api/pure/reimbursement.rs:447-458, 461-472`

Both verdict functions pick their sentence with `variance.abs() < 0.005` then `variance > 0.0`. For
`variance = NaN` both comparisons are false, so the `else` branch fires and the report claims
"Evolute reimbursed/sent less than … by the amount above" while the column prints `NaN`. `+inf`
takes the "more than" branch.

The sibling `recovery.rs:736-747` guards this with `!surplus.is_finite()` and documents why. The
reimbursement side lacks the guard.

**Trigger:** reachable from the shipped GUI — `ReimbursementForm::number`
(`src/bin/ev_cost_recovery/state.rs:507`) accepts `"NaN"`/`"inf"`.

**Fix:** at the top of both functions, return a neutral sentence when `!variance.is_finite()`,
mirroring `recovery.rs`.

### 5. MEDIUM — a blank line aborts the whole Charges/Session CSV read

`src/session/csv.rs:267-273`

The `csv` crate yields a blank line as a single-empty-field record, which fails field parsing and
returns `BadValue` on `Conn_DateTime_Start`. One trailing blank line (e.g. added by an editor)
rejects the entire file.

**Fix:** skip records whose fields are all empty before parsing.

### 6. MEDIUM — an end inside the spring-forward gap shifts the session silently

`src/session/csv.rs:696`

`resolve_end`'s gap arm calls `ambiguous.compatible()` with **no** anomaly, while the start-side arm
(`csv.rs:599-603`) raises `DstGapShifted`. A session ending inside the DST gap (e.g. 02:30 on a
spring-forward day) that still passes `duration_is_consistent` is shifted an hour later with zero
flags.

**Fix:** raise the same anomaly the start-side arm does.

### 7. LOW-MEDIUM — cost-recovery rates accept negative / NaN / infinite values

`src/bin/ev_cost_recovery/state.rs:177-194` (`RatesForm::parse`) ·
`src/bin/cost_recovery_cli.rs:111-137` · `src/bin/cost_recovery_surplus_cli.rs` (same `parse_rates`)

Both parsers accept any `f64::from_str`-parseable token. A rate of `-0.11`, `NaN`, or `inf` is
accepted silently and flows into the report, producing wrong-signed or NaN-poisoned figures. Rates
are dollars per kWh; a negative or non-finite value is never meaningful.

**Fix:** reject `!value.is_finite() || value < 0.0`, naming the band (as the blank-field error
already does).

### 8. LOW — `reading_period(...).expect("just matched")` panics on layout drift

`src/hydro_bill/bill_pdf.rs:429`

The meter-reading row is found by `.find(|line| line.fragments.iter().any(|f| reading_period(&f.text).is_some()))`,
but the code then destructures `fragments` by position and assumes the matched fragment is
`fragments[1]`. If the bill generator ever moves the reading period to a different column (or the
fragment count is still 7 but the period sits elsewhere), the `expect` panics on a user bill rather
than returning a layout error. The 7-fragment count *is* validated (`bill_pdf.rs:422-428`), but the
matched-fragment-at-index-1 assumption is not.

**Fix:** re-check `reading_period(&period.text).is_some()` and return `shape(...)` otherwise, or
match by label rather than position.

### 9. LOW — `parse_duration` can overflow on absurd hour fields

`src/session/csv.rs:446`

`h*3600 + m*60 + sec` is computed on unbounded `u64` hours. Overflow panics in debug builds and
wraps silently in release for an absurd `Conn_Duration` hour value.

**Fix:** checked arithmetic, or an early bound on the hour field.

### 10. LOW — duplicate CSV headers resolve last-wins, the workbook reader first-wins

`src/session/csv.rs:359-365`

The CSV header map is built with `collect`, so a repeated header silently keeps the last column. The
workbook reader (`excel.rs:792-793`) documents first-wins. The two paths should agree.

**Fix:** reject duplicate headers, or match the workbook reader's first-wins behaviour.

### 11. LOW — Charges Report money parser does not handle parentheses for negatives

`src/charges_report.rs:456-464`

`money()` strips `$` and `,` but not parentheses. The doc asserts the sign is outside the `$`
(`-$1.00`), which holds for the data seen — but a single cell exported with accounting-style
`(1.00)` rejects the month's file. Defensive; not seen in the wild.

### 12. LOW — PDF CMap `bfrange` list form does not verify list length

`src/hydro_bill/pdf_text.rs:324-335` (`read_range`)

The list form `<lo> <hi> [<a> <b> …]` must carry exactly `hi - lo + 1` targets. The code maps with
`(lo..=hi).zip(&tail[..end])`, which silently truncates to the shorter side: a short list leaves
the high codes unmapped (rendering as U+FFFD), and a long list drops the extras — no error either
way. The sibling "counts up" arm (`pdf_text.rs:311-322`) *is* defensive (`too long`/`overflows`/
`empty`); only the list arm trusts its input's shape. Low because the CMap is generator-embedded
font data, and the consequence is a visibly-wrong character rather than a silently-wrong figure.

**Fix:** verify `end == hi - lo + 1` and return an error otherwise.

### 13. LOW — `Session::avg_kw` never validates `energy_use` sign/finiteness

`src/session/common.rs:272-284` · energy parsed at `src/session/csv.rs:527`

`csv.rs` parses `Energy_Use` as any `f64` (including `-5`, `nan`, `inf`). Negative energy gives a
negative `avg_kw`, which scales negative into `load_over_panels` (`common.rs:698`) where `floor()`
yields a negative panel count and produces a negative `energy_based_kw`. NaN energy is silently
substituted as `BREAKER_RATING_KW`, and the `ExcessiveAvgKw` check never fires (`NaN > x == false`).

**Fix:** reject non-finite/negative energy at parse, and/or assert non-negative scaling in
`load_over_panels`.

---

## Clippy

### 14. LOW — `from_source` uses the `from_*` convention while taking `self`

`src/green_button/espi.rs:107`

```rust
pub fn from_source(mut self, path: &Path) -> Self { … }
```

This is a builder setter, not a `from_*` constructor. 

**Fix:** Rename to `with_source`. Safe: the only call site is `read_xml.rs:150`; the other references are comments.

---

## Comment issues

Comments that are unclear, over-verbose, or drifted from the code. Suggested replacements follow
each.

1. **`src/error.rs:20-26`** — the `ConversionError` doc repeats the module doc's "who raises it"
   and "`ReadError` went back" reasoning nearly verbatim. Trim to:
   > A workbook could not be produced from the file it was to be converted from.
   > Lives here rather than in `api::error` for the reason the module states: it is raised by the
   > two workbook writers, which do not depend on the API.

2. **`src/time/tou.rs:190-194`** — `is_off_peak`'s doc says it answers "entirely outside Toronto
   Hydro's `[07:00, 19:00)` demand window", but on weekends/holidays the whole day is off-peak, so
   hours inside 07:00-19:00 still return `true`. The replacement must describe off-peak itself, not
   name the `_nop` columns (which live in `green_button::peaks`, not here). Replace with:
   > Whether `interval` lies entirely in the off-peak period: weekdays 19:00-07:00, and weekends
   > and holidays all day.
   
3. **`src/green_button/peaks.rs:39-40`** — `max_kw_nop`'s `None` trigger is understated ("only for
   a period truncated to a weekend"). The real condition is any period with no interval outside
   off-peak. Replace "which happens only for a period truncated to a weekend" with "e.g. a period
   truncated to a weekend".

4. **`src/green_button/read_xml.rs:152-154`** — three sentences belabour one point. Replace with:
   > // `period_values` already computes the period's peaks and anomaly counts; picking the row is
   > // the whole of the work.

5. **`src/bin/ev_cost_recovery/theme.rs:3`** — "Kept identical" is false: `ev_peak_gui/theme.rs`
   added a `ceiling` accent this file lacks. Replace with "Kept close so the two apps look alike,
   though each carries accents the other does not (e.g. `ev_peak_gui`'s `ceiling`)."

6. **`src/api/pure/peak_power.rs:269-271`** — "Not merged: which records describe one session is
   decided here" is stale; merging happens in `Sessions::merge` (called by `api::io`), and
   `peak_power` takes an already-merged `&Sessions`. Replace with "already merged and bucketed into
   `Sessions`; see 'Sessions the reports share' below."

7. **`src/session/mod.rs:21-25`** — "Its one entry point, `csv_sessions`, is called from … `excel`"
   is wrong: `excel` calls `csv_session_rows`, not `csv_sessions`. Both readers return
   `SessionCsvError`. Reword to name the two entry points and their callers.

8. **`src/api/pure/reimbursement.rs:448`** — "Half a cent, for the reason `verdict` uses the same
   threshold" is circular. Replace with: "Half a cent, where the printed figure stops; see `verdict`
   below."

9. **`src/api/pure/energy.rs:279-283`** — repeats the function doc and the `$0.006000`/`$0.006177`
   reference-bill figures (also in the test doc). Replace with:
   > // Levied over `adjusted_kwh_used` -- the base the bill's three TOU consumption lines sum to --
   > // so the EV share is the EV adjusted kWh, not the metered figure.

10. **`src/bin/ev_cost_recovery/state.rs:826-829`** — "which is how every report looked to this
    function before top-level titles were recognised" is change-history. Drop it.

11. **`src/session/common.rs:464`** — "Instantiate `Self``." (with a stray backtick) is content-free.
    Replace with: "A bracket from `min` and `max`; panics unless `min <= max`."

12. **`src/session/common.rs:271`** — `avg_kw`'s doc omits the spike-substitution contract that
    `peak.rs` relies on. Append: "For a spike (zero charge time) the division is not real; it
    substitutes `0.0` for zero energy and `BREAKER_RATING_KW` otherwise, so a spike can still be
    listed."

13. **`src/session/excel.rs:578-580`** — "A `<stem>.session.xlsx.read.log` is written beside the
    workbook" is false: the reader returns it unwritten on `Sessions::logs`; only a binary writes
    it. Replace "is written beside the workbook" with "is returned, unwritten, on `Sessions::logs`".

14. **`src/session/excel.rs:588`** — "treated as trailing blanks" is misleading: the reader skips
    any row with an empty id, at any position. Replace with "Rows with no `Charge_Session_ID` at
    all are ignored."

15. **Run-log rationale repeated at ~11 sites** (`cost_recovery_cli.rs`, `cost_recovery_surplus_cli.rs`,
    `energy_cli.rs`, `energy_cost_cli.rs`, `peak_power_cli.rs`, `peak_power_cost_cli.rs`,
    `ev_csv_to_xlsx.rs`, `gb_peak_values.rs`, `ev_cost_recovery/state.rs` ×4, `ev_peak_gui/state.rs`)
    — the same "the library returns logs unwritten; a binary is where they land" rationale is
    restated near-verbatim. Keep the full rationale once and shorten the repeats to a pointer.

16. **`src/session/common.rs:104-110` vs `732-735`** — the same three-check consistency derivation
    is stated twice. Keep one copy and defer the other to it.

---

## Minor note

The global `~/.claude/CLAUDE.md` (loaded as this project's action rules) points at
`docs/action-rules.md`, which does not exist in this repository. Either the file should be added or
the reference updated.
