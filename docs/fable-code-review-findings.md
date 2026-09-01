# Code review findings

> ***Notes:*** I have reviewed and I concur with all code error/gap fixes prefixed with a bold-italic ***Fix***. See also my strike-through edits, which are deletions from your proposed actions, as well as my additional instructions or questions, which are enclosed in <@ ... @> pairs. I also concur with all proposed comment fixes. Before making with any code changes, answer my questions, ask any questions you may have about what action should be taken on any of the findings, and then ask me for approval to proceed with the fixes.

Reviewed 2026-08-31 by Claude (Fable 5). Scope: all of `src/` (library and binaries), `tests/`,
`examples/`, `build.rs`. Method: five parallel reviewers, one per module slice; every finding
below marked CONFIRMED was then re-verified against the code by the compiling reviewer. No file
was changed; this document only proposes.

Baseline: `cargo check --all-targets`, clippy, `cargo test`, and the intra-doc-link check are all
green in both feature combinations. The one warning is finding C20.

Confidence marks: **CONFIRMED** = the failure was traced through the code. **PLAUSIBLE** =
suspicious but not fully traced (usually depends on library behaviour or data not exercised).

---

## Part 1 — Code errors and gaps

### High: silent data loss

**C1. `ev_csv_to_xlsx` silently overwrites an existing workbook.** CONFIRMED.
`src/bin/ev_csv_to_xlsx.rs:32` calls `session::session_csv_to_xlsx`, which writes
unconditionally (`src/session/excel.rs:187` — no exists-check anywhere on that path). Every
other producer of a workbook protects it: `gb_peak_values` refuses when the target exists
("a silent overwrite is how that work gets lost"), and both GUIs ask first.
*Failure:* `ev_csv_to_xlsx report.csv` with a hand-annotated `report.xlsx` beside it destroys
the annotations without a word.
***Fix:*** refuse when the target exists, matching `gb_peak_values` (optionally with a `--replace`
flag). <@ I don't agree with "The check belongs in or beside `session_csv_to_xlsx` so the GUI convert path and the CLI share it." You are being too loose when talking about the GUI. This code is gated by feature "historic" and code not gated by "historic" should obviously not be using this shared function. @>

**C2. The input==output guard is byte-wise; a case-differing extension bypasses it.** CONFIRMED
(guard bypass traced; the damage is platform-dependent).
`src/api/io.rs:618` (`checked_workbook_path`) compares `output == input` as paths. An input named
`Book.XLSX` yields output `Book.xlsx` — unequal as bytes, the same file on Windows and macOS,
and Windows is a shipped target. Under `OnExistingWorkbook::Replace` the exists-check is waived,
so the conversion truncates its own input — the exact event `OutputWouldBeInput` exists to
prevent ("there is no reading a file that the writer has already truncated").
The same pattern is in `gb_peak_values.rs:118-124`, where on a case-insensitive filesystem the
error then tells the user to "move or delete" what is actually their input file.
(`ev_peak_cli.rs:119` already compares the extension case-insensitively.)
***Fix:*** refuse when the input's extension is `xlsx` under `eq_ignore_ascii_case`, in both places.

### Medium: wrong or missing output

**C3. A session end falling in the DST gap is resolved silently, with no anomaly.** CONFIRMED.
`src/session/csv.rs:695`: `resolve_end`'s `Gap` arm applies `compatible()` and records nothing.
The identical fault on the *start* raises `AnomalyKind::DstGapShifted` (csv.rs:601, with the
comment "the shift is reported rather than silently applied"), and the module doc promises
"every judgement call is recorded as an `AnomalyKind`".
*Failure:* a row whose `Conn_DateTime_End` is a wall time that never occurred (e.g. 02:30 on a
spring-forward day) is shifted, passes `duration_is_consistent`, and is accepted with no flag;
the same value in the start column would be flagged.
***Fix:*** have `resolve_end` report the gap case back to `resolve` so `DstGapShifted` (or a
dedicated kind) lands in the row's anomalies. <@ In addition, `DstGapShifted` should be renamed `FellInDstGap` any session with that anomaly must be excluded, just like with `InconsistentDuration`. @>

**C4. The Detail tab shows a failed save for one frame only.** CONFIRMED.
`src/bin/ev_cost_recovery/detail.rs:92-94`: the save error is drawn via `widgets::error_block`
inside the `clicked()` branch, which in immediate mode runs only on the click frame. The other
two tabs persist the message in `state.error` and re-draw it every frame
(`surplus.rs:191-193`, `reimbursement.rs:264-267`); the Detail tab neither stores it nor renders
`state.error` at all.
*Failure:* saving the detail report to an unwritable path produces no file and no visible error.
***Fix:*** store the message in `state.error` and render it from `detail::ui` as `surplus::ui` does.

**C5. Three CLIs never write the meter run log the GUI writes.** CONFIRMED.
`cost_recovery_surplus_cli.rs:110`, `peak_power_cost_cli.rs:70`, and `peak_power_cli.rs:75`
write only `notes.write_logs()`. The same results carry `meter: MeterNotes`
(`src/api/pure/peak_power.rs:142`, `recovery.rs:169`), whose `write_log` doc says "For a
binary"; the GUI calls it (`ev_cost_recovery/state.rs:340`), and `docs/app-cheat-sheet.md`
documents `*.meter.xml.read.log` beside the export.
*Failure:* the same inputs leave different artifacts depending on whether the app or the CLI
ran; the CLI run records no meter-side anomalies for the priced period.
***Fix:*** call `meter.write_log()` beside `notes.write_logs()` in all three.

**C6. Rate and amount fields accept `nan`, `inf`, and negatives.** CONFIRMED (the parse
behaviour is a language fact; the downstream NaN report was not run).
`"nan".parse::<f64>()` succeeds, so `RatesForm::parse` (`ev_cost_recovery/state.rs:177-194`),
`ReimbursementState::number` (state.rs:494-501), and the duplicated `parse_rates` in
`cost_recovery_cli.rs:110` / `cost_recovery_surplus_cli.rs:120` all pass them through. A blank
rate is carefully refused precisely because it "would price that band's energy at nothing and
still produce a report" — `nan` produces that report with NaN totals.
***Fix:*** reject non-finite ~~(~~and ~~arguably~~ negative~~)~~ values, naming the field.

### Low: contract violations, latent bugs, test gaps

**C7. Identical duplicate rows within one file are silently collapsed, contradicting
"detection alone".** CONFIRMED (code/comment disagreement; the behaviour itself may be right).
`src/session/common.rs:405-419` collapses a consistent duplicate regardless of which list it
came from, while the doc directly above (common.rs:403-404) says "One list in means there is
nothing to collapse across files, and this is detection alone" — repeated at `csv.rs:198-200`
and in `session/mod.rs`. A CSV containing the same row twice loses one copy with no anomaly and
no log line, and `merging_one_list_leaves_it_alone` (common.rs:1316) tests only distinct
sessions, so nothing pins either reading.
***Fix:*** ~~decide which is right — restrict collapsing to cross-file pairs, or~~ fix the three
comments and flag/log the within-file collapse — and pin it with a test either way.

**C8. The charges-report cross-check test breaks on the very values the reader was written to
handle.** CONFIRMED by inspection (no fixture currently trips it).
`tests/charges_report/real_reports.rs:119`: `totals_by_hand` splits lines naively on `,`.
`charges_report::number` strips thousands separators because "a busy month's kWh easily reaches
four figures" — and a CSV cell holding one is quoted (`"1,234.5"`), so the naive split yields
`"1` for the kWh cell (parse panic) and shifts every later column.
*Failure:* the first real report with a four-figure kWh makes the independent check fail on its
own parsing, not on a discrepancy.
***Fix:*** make the splitter quote-aware~~, or panic loudly (naming the limitation) when a line
contains a `"`~~.

**C9. The anomaly-token round-trip test omits `ImplausibleGap`.** CONFIRMED.
`src/green_button/common.rs:218-225` lists six of the seven variants. The tokens are declared a
stable wire format; renaming `ImplausibleGap` in `as_str` without `from_token` (or vice versa)
would silently drop that anomaly on workbook read-back, and no test would notice.
***Fix:*** add `Anomaly::ImplausibleGap` to the array.

**C10. `Sessions::merge` drops `WorkbookDiscrepancy` anomalies; its doc claims re-derivation.**
CONFIRMED, latent.
`src/session/common.rs:1079`: the doc says report anomalies are "dropped rather than
concatenated: they are re-derived from the combined records", but re-derivation recovers only
`DuplicateId`. `WorkbookDiscrepancy`, which the historic workbook reader also puts on
`Sessions::anomalies` (`excel.rs:664`), would be lost. Unreachable today — the only non-test
caller merges CSV-sourced reports — but the stated invariant is false.
***Fix:*** ~~carry non-`DuplicateId` anomalies through, or~~ narrow the doc to say which kinds survive.

**C11. An Excel-saved workbook's spike row raises a spurious `avg_kw` discrepancy.** PLAUSIBLE
(`historic`).
`src/session/excel.rs:739`: `check_stored_columns` compares stored `avg_kw` against
`session.avg_kw()`, which for a zero-charge-time spike substitutes `BREAKER_RATING_KW`. The
sheet's formula for that row evaluates to `#DIV/0!` — deliberately, per the writer's own
comment ("the honest answer") — so once Excel has saved the workbook, the cached cell holds an
error token, the parse fails, and the row is flagged for the exact state the writer produces.
***Fix:*** skip the `avg_kw` comparison when `session.charge_time.is_zero()`.

**C12. A `Tf` naming a font absent from the page's resources silently drops all following
text.** PLAUSIBLE.
`src/hydro_bill/pdf_text.rs:160-165`: `encoding` becomes `None` and `decode` returns `""`, so
runs vanish without a trace — against the module's own U+FFFD "visible damage" philosophy
(line 253). A *declared* font lacking a CMap errors; an *undeclared* font name gets silence.
***Fix:*** treat an unresolvable `Tf` as an error, or emit U+FFFD fragments. <@ I don't care about the detailed approach, provided that the file read is rejected with an appropriate reason displayed. @>

**C13. `BillingPeriod::ending_on` never checks `bill_end_day` against `MAX_BILL_END_DAY`.**
CONFIRMED (panic-message quality, not wrong results).
`src/hydro_bill/billing_period.rs:111-127` asserts `ending.day() == bill_end_day`, but a day of
28-31 reaches jiff's `date()` and panics inside the library for short months, with a message
naming neither the rule nor the constant. The only guard lives in one caller
(`green_button/read_xml.rs:189`).
***Fix:*** assert `(1..=MAX_BILL_END_DAY).contains(&bill_end_day)` in `ending_on`, which
`containing` funnels through. <@ Shouldn't this assertion be in addition to the existing one? @>

**C14. Comma-stripping accepts misplaced separators.** CONFIRMED.
`src/charges_report.rs:441-442` (`number`) deletes every comma before parsing, so `1,2` reads
as 12 — a malformed cell silently becomes a plausible figure, in a reader whose stated posture
is "an error rather than a partial sum". Same in `bill_pdf.rs:536-542` (`money`).
***Fix** ~~(if wanted)~~:* accept commas only as digit-triple grouping; otherwise `BadValue`.

**C15. Two places still format a path into a message string, against the repo's own rule.**
CONFIRMED.
`src/hydro_bill/pdf_text.rs:111,115` and (behind `historic`) `src/session/excel.rs:593` do
`format!("{}: {e}", path.display())` into a `Box<dyn Error>`. CLAUDE.md's claim that "there is
no remaining place in the crate where a path reaches a message by string formatting" is
therefore inaccurate. In `pdf_text` the invariant that `BillError::Unreadable` may ignore its
own `path` field rests on this string (bill_pdf.rs:185-190).
***Fix:*** a typed `pdf_text` error (`path`, optional `page`, `cause`) formatted at `Display`; for
the historic reader, ~~either convert or~~ comment it as legacy exempt from the rule — and adjust
the CLAUDE.md sentence either way.

**C16-C19, briefly (all CONFIRMED unless noted):**

- **C16** `ev_peak_gui/state.rs:122-126` (`historic`): a failed *log write* after a successful
  conversion is reported as a total failure, dropping the outcome; the newer app carries it as
  `log_failure` (`ev_cost_recovery/state.rs:600-606`). ~~Mirror that, or~~ leave — the app is
  slated for retirement.
- **C17** `ev_peak_gui/state.rs:195` (`historic`): the date picker defaults from the *system*
  zone; the newer app deliberately converts to `time_zone()` (`ev_cost_recovery/state.rs:198-202`).
  On a machine outside Eastern time the picker can open a day off. <@ If this only impacts legacy, leave it. Otherwise, consult me. @>
- **C18** `src/api/pure/peak_power.rs:42`: `PowerEstimates` is the one public API result type
  without `#[derive(Debug)]`; all its fields are `Debug`. ***Fix:*** Add the derive.
- **C19** `src/api/io.rs:680-706`: the "source file is named exactly once" regression test
  covers the Green Button and session paths but not `ReadError::Bill` or
  `ReadError::ChargesReport`, where the same double-naming is equally possible. ***Fix:*** Extend it.

**C20.** `src/session/excel.rs:17`: unused import `duration_is_consistent` — the build's one
warning. ***Fix:*** Remove it.

**Minor notes** (each a sentence; act or ignore):

- `examples/sessions.rs:18-19`: USAGE promises "A .xlsx.read.log is written beside the
  workbook", but the example never calls `report.write_logs()` — ~~write it or~~ drop the claim.
- `src/session/test_support.rs:32`: `minutes: i64` then `minutes as u64 * 60` — a negative
  wraps into a ~584-billion-year duration; ***Fix:*** take `u64`.
- `src/time/base.rs:369-378`: `Interval::intersection` on disjoint inputs returns an empty
  interval whose `start` is not meaningful; ***Fix:*** one doc line would settle the contract.
- `src/session/report.rs:217,229`: the source table is keyed by bare file name; two same-named
  files in different directories would share a cell. Worth a one-line comment on the assumption. <@ Why not use full absolute path as the key? @>
- `src/green_button/espi.rs:289-312`: gap-fill placeholders inherit a misaligned reading's
  off-grid phase and are never themselves flagged; cosmetic (empty rows cannot become peaks). <@ Leave it. @>
- `src/green_button/espi.rs:227`: when a feed duplicates an interval start, the later value
  silently wins into totals and peaks; the rule is an accident of feed order — document it on
  `Series::duplicates` ~~or keep the first deliberately~~.
- `parse_rates` is duplicated verbatim in `cost_recovery_cli.rs:110-136` and
  `cost_recovery_surplus_cli.rs:120-146`; it parses into the API's own `CostRecoveryRates` and
  could live in the library. Drift hazard, not a defect. <@ Add comment to both indicating the duplication. @>
- `src/log.rs:148-153`: the `log_path` fallback stem is `"session_report"`, but the module now
  serves meter and charges logs too; unreachable in practice, but a neutral stem would stop the
  name lying. <@ Why do we need a fallback stem? @>

---

## Part 2 — Comments: wrong, unclear, or too verbose

### Comments that contradict the code

- **`src/api/pure/peak_power.rs:336-339`** — "There is no `io` counterpart yet, so a caller
  reads the bill with `hydro_bill_from_pdf`…" is stale: `api::io::peak_power_cost` exists
  (`io.rs:150`) and does exactly that. Replace with the phrasing `fn@peak_power` uses at
  248-250: "The reading half of the same call is [`api::peak_power_cost`], which is where these
  arguments come from."
- **`src/session/common.rs:403-404`** (plus `csv.rs:198-200` and `session/mod.rs`) — "detection
  alone" / "nothing to collapse across files"; see C7 — the code collapses within one file too.
- **`src/session/common.rs:983`** — `Sessions::anomalies`: "Currently
  [`AnomalyKind::DuplicateId`] only" is false with `--features historic`: `excel.rs:664` adds
  `WorkbookDiscrepancy`, whose own doc says it lives here. Say: "Currently `DuplicateId`, plus
  `WorkbookDiscrepancy` when the `historic` workbook reader produced this value."
- **`src/session/common.rs:203`** — `Session::intersects` panic doc: the panic fires when
  `adj_conn_end` precedes **`adj_conn_start`** (the truncated start), not `conn_start` as
  stated. Name the right field.
- **`src/session/excel.rs:591-593`** (`historic`) — "See `csv::csv_sessions` for why it is done
  here rather than at each site": `csv_sessions` no longer string-formats; it returns the
  structured `SessionCsvError`. The pointer leads nowhere. Delete the sentence (and see C15).
- **`src/api/error.rs:38-44`** — "All four causes are structured types that name their own
  file, each from a `path` field of its own" overreaches: `GbReadError`'s
  `NotABillingCalendar` and `BillEndDayOutOfRange` carry no path — which `io::gb_read_error`'s
  own doc states (`io.rs:631-634`). Soften to "each cause that concerns a file names it from a
  `path` field of its own".
- **`src/time/base.rs:14-15`** — `TIME_ZONE_NAME`: "Public because both binaries and several
  doc comments name it" — the const is unreachable outside the crate (module private,
  re-exported `pub(crate)` behind `historic`) and no binary names it; `base.rs:18` and
  `session/ioi.rs:129` both call it crate-private. Replace with: "Referenced by `session::ioi`
  and several doc comments; a reader who finds 'in local time' in a message needs somewhere to
  learn which zone that is. Not exported from the crate."
- **`tests/green_button/fixtures_golden.rs:66-67`** — "the writer refuses to overwrite" is
  false: `write_gb_workbook` has no exists-check (its doc puts that on the caller; the refusal
  lives in `gb_peak_values.rs:77`). Replace with: "A scratch directory per fixture, since tests
  run in parallel. A normal test run never writes into tests/fixtures."
- **`src/charges_report.rs:228`** — `row_list`'s doc examples don't match its output (the code
  emits `rows 2, 3, 4` with no "and", and `rows 2, 3, 4, 5, and 12 more` past the fourth).
  Correct the examples.
- **`src/charges_report.rs:35`** — "A two-digit year, which `%y` reads as 20xx" is false: jiff
  maps 00-68 → 20xx and 69-99 → 19xx. Say so, and keep the point that every real value here
  lands in 20xx.
- **`src/bin/ev_peak_cli.rs:113`** — the `workbook_fault` doc's example command reads
  `estimates "…" 1h`; the binary is `ev_peak_cli` (the test at line 195 has it right).
- **`src/bin/ev_cost_recovery/widgets.rs:1`** — "the two tabs share" is stale; the app has four
  tabs. "Small pieces of chrome the tabs share."

### Comments that are unclear, garbled, or misplaced

- **`src/api/pure/recovery.rs:172-178`** — the "No per-band recovery for the whole period…
  [`Self::kwh`] and [`Self::cost_recovery`]…" block sits after `CostRecoverySurplus`, which has
  neither field; the fields belong to `CostRecovery` (recovery.rs:116,119). Move the block
  under `CostRecovery` (or fold it into the `kwh` field doc, which carries half of it).
- **`src/api/pure/energy.rs:594-596`** — the doc on
  `a_bill_stating_no_total_charges_is_refused` opens with an empty `///` and a sentence copied
  from `test_support::bill()`'s doc that describes the fixture, not this test. Delete those
  lines; keep "The energy cost divides by the bill's total charges… Zero is refused rather than
  divided by."
- **`src/session/common.rs:719-721`** — `InconsistentDuration`: "by a full one
  `TIME_GRID_STEP` or more, in one direction or the other" is ungrammatical and imprecise (the
  late side requires a step *plus a second*; `duration_is_consistent`'s doc has it right).
  Either fix the wording or defer to `duration_is_consistent`, which the next line already
  cites.
- **`src/green_button/peaks.rs:76-77`** — "Taken rather than borrowed" on a `&self` method that
  clones reads as a claim about the receiver. Meant: the *result* owns its data. "Owning clones
  rather than borrows: `PeriodValues` is consumed by the functions that price a period, and the
  notes are what survives them."
- **`src/bin/ev_peak_gui/state.rs:211-215`** — `adopt_workbook`'s doc is two docs merged: the
  first two sentences describe `select_workbook` (which has none). Move them there; keep only
  "Takes up the workbook a conversion just wrote…" here.
- **`src/api/pure/coverage.rs:32`** — `NotABillingPeriodEnding` is the one undocumented variant
  among documented siblings. One line: ``/// See [`NotABillingPeriodEnding`].``
- **`src/golden.rs:8`** — `[fixtures_dir_in](../tests/common)` renders in rustdoc as a link to
  nowhere (not caught by `broken_intra_doc_links` because it isn't one). Use a plain code span.
- **Hard-wrap artifacts** — `src/time/base.rs:18-20`, `src/time/holidays.rs:25-27`,
  `src/bin/ev_cost_recovery/state.rs:406-411` and `:420-426`: mid-sentence line breaks left by
  earlier edits. Reflow only.

### Comments that are too verbose

- **`src/log.rs:1-27`** — the module doc enumerates three *session* log suffixes and discusses
  only session concerns, but the type now serves six operations across three modules
  (`charges.csv.read`, `meter.convert`, `meter.xml.read` besides the three listed). State the
  naming scheme (`<stem>.<suffix>.log`) and that each reader declares its suffix via
  `SourceLog`; move the "Why discrepancies are not anomalies" section to the session docs that
  own the subject — the repo's own rule: a subject document describes its subject.
- **`src/api/io.rs:88-91, 141-144, 197-200, 239-242, 292-295`** — the identical three-line
  "Nothing here writes…" paragraph appears in five reader docs, and the module doc (lines 8-12)
  already states the write policy. Defensible as per-function contract; if trimmed, one
  sentence per function pointing at the module doc keeps the contract. Judgement call.
- **`src/bin/ev_cost_recovery/state.rs:420-426` / `:503-504` / `reimbursement.rs:139-143`** —
  the bank-statement/remittance rationale is stated three times. Keep the field doc as the
  home; trim `amount()`'s doc to "What Evolute actually paid; see [`Self::reimbursed`]."

---

## Verified clean, for the record

The money paths held up everywhere they were traced: the TOU/DST/holiday logic in `time/` and
its tests; the integer-until-cell-write discipline in `green_button`; the session consistency
band, fold resolution (both directions), segment tiling and `load_over_panels` boundaries; the
stretch split at `local_midnight` (exact partition); every division guarded
(`ZeroDenominator`/`NoRate`); `to_the_cent` total on non-finite input; `Charges::read`'s
duplicate-label summing; `error.rs`'s path/no-path split; `build.rs`'s staleness check; and the
state-clearing discipline in both GUIs (stale results dropped on every input edit).
