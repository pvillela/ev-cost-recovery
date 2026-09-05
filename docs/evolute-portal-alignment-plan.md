# Evolute portal alignment — implementation plan

## Context

Access to the Evolute portal confirmed three facts that invalidate assumptions the crate was
built on:

1. **Session times carry seconds**, not minutes, and `Conn_DateTime_Start + Conn_Duration ==
   Conn_DateTime_End` holds. The padding, bracketing and grid machinery built to cope with
   minute-truncated times is no longer needed.
2. **Session times are stated in EST**, a fixed offset, not prevailing ET. The DST fold and gap
   inference in the session reader has nothing left to infer.
3. **Report files are not calendar-month-bound.** A session report covers any date range; a
   charges report spans any number of contiguous months, and both state that range in the file
   name.

The intended outcome is a crate that trusts the portal's times, reads either report by the range
its name states, and no longer carries the uncertainty machinery.

Two facts established during planning shape the work:

- **The invariant is not exact.** In the real export
  `data/evolute/Session_Report_August_1_2026-September_4_2026.csv`, four of five rows satisfy
  `start + duration == end` and one is off by one second. `Active_Charge_Time` shows the same
  second-level jitter. The consistency test therefore gets a ±1s tolerance, not exact equality.
- **Removing the padding moves money.** `tou_kwh` (`src/session/energy.rs:69-74`) spreads a
  session's energy across Time-of-Use periods over the *adjusted* span, so every session
  currently occupies one minute more than reported. Dropping the padding shifts the kWh split for
  every session straddling a TOU boundary.

---

## Before starting: capture a baseline

Run `cost_recovery_surplus_cli` on the June inputs and save the report under `data/`. Diff
against it after every phase.

- Phases 0 and 3 must produce a **byte-identical** report. A difference means something is wrong.
- Phases 1 and 2 will differ. Inspect the delta rather than assume it.

This is the only end-to-end check available, because a bill PDF and Green Button XML are needed
and neither may be committed.

---

## Phase 0 — Remove the `historic` feature

No behaviour change. The report must diff clean.

**Delete**

- `src/session/ioi.rs` entire; `session::excel::historic` (`src/session/excel.rs:569-920`) and its
  test module (`:1167-1787`).
- `map_local` / `TzLocalMapping` (`src/time/dst.rs:75-97`), `time::excel`'s
  `instant_of_serial` / `duration_of_serial`.
- Targets `ev_peak_cli`, `ev_peak_gui/`, `examples/sessions.rs`, and their `[[bin]]` /
  `[[example]]` entries in `Cargo.toml`.
- `AnomalyKind::WorkbookDiscrepancy` (`src/session/common.rs:863`) with its `as_str`, `Display`
  and doc entries; `AnomalyKind::from_token` (`:966`) and its round-trip test, whose only
  production caller was the workbook reader.
- `IntervalEstimates::write_logs` (`src/session/peak.rs:66`), stranded when `ev_peak_cli` goes.
- The `historic` feature in `Cargo.toml:29-30` and the second `cargo test` line in
  `.github/workflows/release-build.yaml:74-76`.

**Keep.** `TZ_OFFSETS` and `TIME_ZONE_NAME` (`src/time/base.rs:16-22`) are declared
unconditionally and are load-bearing in every build — `time_zone()` resolves the name and
`BILLING_OFFSET` is a `TZ_OFFSETS` entry. Only their gated `use` lines in `src/time/mod.rs:63-65`
go.

**Docs.** Archive `docs/historic-feature.md`. Drop the `--features historic` lines from
`README.md` (~247–257), `CLAUDE.md:13,27` and `docs/maintenance-manual.md`.

**Free win.** This deletes `ev_peak_gui`'s `interval_heading`
(`src/bin/ev_peak_gui/state.rs:382-399`), a second implementation of zoned rendering that looks
the abbreviation up in `TZ_OFFSETS` where `interval_line` uses jiff's `%Z`. Phase 1 inherits one
implementation, not two.

---

## Phase 1 — EST

Session report wall times are read at a fixed −5 offset. Everything else stays on prevailing ET.

**Add.** `SESSION_OFFSET` and `session_zone()` in `src/time/base.rs`, modelled on
`BILLING_OFFSET` / `billing_zone()` (`:82-96`). A **separate** constant, not a reuse of
`BILLING_OFFSET`: the values coincide but the reasons do not, and sharing would make session
times move if Toronto Hydro ever changed how it cuts periods.

**Replace.** `CsvSession::resolve` (`src/session/csv.rs:454-540`) collapses from a
probe-and-choose over both offsets to one fixed-offset conversion.

**Delete.** `FellInDstGap`, `DstUnresolvable`, `DstAmbiguousDuplicated`; `UNPLACEABLE_START` /
`UNPLACEABLE_END` and `Session::is_placeable` (`common.rs:200-202`); `AnomalyKind::leaves_no_instant`
(`:909`); `local_readings` and `falls_in_gap` (`src/time/dst.rs:45-66`) — likely the whole module.
`excludes_session` (`:929-931`) reduces to `InconsistentDuration` alone.

**What does not change.** Prevailing ET remains the zone for display, Time-of-Use periods, the
07:00–19:00 demand window and the holiday calendar (`src/hydro_bill/billing_period.rs:27`,
`src/time/base.rs:73-74`). `truncate_to`, `local_date`, `local_hour`, `local_midnight` all stay.

### Zone labelling

Reading EST and displaying ET means rendered times sit an hour later than the portal screen all
summer. Every displayed time therefore names its offset — `EDT` in summer, `EST` in winter — and
one report may legitimately mix the two when, say, kW and kVA peaks fall either side of the
transition.

Add **`time::format`** with two functions, one for a zoned instant and one for a zoned range,
using jiff's `%Z` — it cannot disagree with the instant it prints, whereas a `TZ_OFFSETS` lookup
is a second copy of the zone's rules. There is no formatting function anywhere in `src/time/`
today, only conversions, and the survey found five distinct spellings of the same instant.

Route these through it:

| Site | What it renders |
|---|---|
| `src/session/report.rs:595-618` `interval_line` | already zoned; becomes the first caller |
| `src/bin/ev_cost_recovery/detail.rs:60-71` | priced-interval heading — the conspicuous gap |
| `src/session/report.rs:477-478` | Excluded sessions `From`/`To` columns |
| `src/green_button/peaks.rs:180-186` | meter anomaly table `Hour` column |
| `src/green_button/common.rs:191-198` | the same hours in the run log |

**Not labelled.** The bare `%H:%M` segment names (`report.rs:323, 405, 447`) — the `Interval`
header above them already states the zone. **Neither workbook**: a serial cell with a zone suffix
stops being a date to Excel, breaking the user's own sorting and arithmetic. The Green Button
workbook's stated policy at `src/green_button/excel.rs:236-237` stands.

Reword the prose at `report.rs:421` and `:506`, both of which say "Times are local (ET)".

**After this phase** `TZ_OFFSETS` has exactly one caller left, `BILLING_OFFSET`. Fold it into
`base.rs`'s standard-time section rather than leaving a two-element table with one user.

---

## Phase 2 — Precision

**Delete.** `adj_conn_start_of` / `adj_conn_end_of` (`src/session/common.rs:87-99`) and the
`Session` accessors at `:204-283`; `Bracket` (`:496-599`); `TIME_GRID_STEP` (`:50`); `is_on_grid`,
`note_off_grid_rows` (`csv.rs:250-289`) and `AnomalyKind::OffGridTimes`.

`truncate_to` (`src/time/base.rs:126`) **survives** — Green Button's `METER_INTERVAL` still uses
it. Only the session-side step goes.

Note the naming trap: `adj_*` in `src/hydro_bill/bill.rs:52-57` and `bill_pdf.rs:110-115` are
Toronto Hydro's *adjusted demand* figures — loss factor and days/30 proration. Untouched.

**Rewrite.** `duration_is_consistent` (`common.rs:121-130`) becomes
`|start + duration − end| <= DURATION_TOLERANCE`, with

```rust
/// Slack allowed when checking `Conn_DateTime_Start + Conn_Duration` against
/// `Conn_DateTime_End`. The portal rounds at second level: in
/// `Session_Report_August_1_2026-September_4_2026.csv`, four of five rows agree exactly and
/// `2026-08-30 16:57:00 + 2:03:50` is reported as ending at `19:00:49`, a second early.
const DURATION_TOLERANCE: Duration = Duration::from_secs(1);
```

The old check 1 (`rep_start <= rep_end`) can go: `parse_duration` rejects negatives
(`csv.rs:330-350`), so the equality implies it.

**Collapse `Bracket`'s dependants.** `EstimateSet`'s four fields (`src/session/peak.rs:79-82`) and
`EstimateSet::values()` (`:87`) become plain `f64`; `Segment::agg_count`, `agg_kw`,
`count_based_load`, `energy_based_load` (`common.rs:663-686`) return bare values. The report's
Min/Max columns (`report.rs:264, 317-323`) collapse to one. `api::pure::peak_power` already
reduces every bracket with `.mid()` (`:658, 996, 1005, 1250`), so **no API number changes from
this** — the money movement in Phase 2 comes from the padding, not from `Bracket`.

**Docs.** Archive `docs/session/time-reporting-uncertainty.md` and remove its five code citations
(`common.rs:26, 80, 771`, `csv.rs:1049`, `time/base.rs:135`). See the doc table below.

---

## Phase 3 — File names and scope

### Two parsers, typed errors

Replace `report_coverage`'s `Option` (`src/session/file_name.rs:35-48`) and `charges_month`'s
(`src/charges_report.rs:68-83`) with typed errors — three callers currently write their own
message and duplicate the expected-form string verbatim (`api/pure/coverage.rs:64`,
`reimbursement.rs:103`, `state.rs:271-274`).

```rust
fn parse_session_report_name(name: &str) -> Result<(Date, Date), SessionReportNameError>
fn parse_charges_report_name(name: &str) -> Result<(Date, Date), ChargesReportNameError>
```

**`civil::Date`, not `Timestamp`** — a file name states calendar days, not instants, and every
name and month path in the crate already uses `Date` (`file_name.rs:17`, `charges_report.rs:26`,
`api/pure/coverage.rs:16`). Any later conversion is not the parser's business. Second value is
the last day of the range, inclusive, matching `SessionReportCoverage.to`.

Variants for both: `NotAReport`, `MissingRange`, `BadDate { text }`, `Inverted { from, to }`.
Both `pub` — these parsers are public, unlike `SessionCsvError`.

**Charges name.** `123 Foo Bar Road_Charges_November 2026-January 2027.csv` replaces
`<building>_charges_<ISO timestamp>.csv`. Split on the **last** `_Charges_` with `rsplit_once`,
match the marker case-insensitively, then split the remainder on `-` requiring exactly two parts —
so an address containing a hyphen or a space cannot break it. The old ISO form is **not**
accepted; delete that parser rather than carry two formats.

**The parsers only report.** Rules live above them: the reader requires the charges range to be a
single month (name *and* content — a hand-renamed file must not misattribute charges), and
`ChargesReport::month` stays a single `Date` while that restriction holds.

### Arity

`reports_cover` (`file_name.rs:96-113`) already accepts any number of reports with overlap. The
"exactly two" lives only in signatures:

- Six API entry points taking two `&Path` (`src/api/io.rs:105, 165, 207, 256, 303, 360`) → `&[&Path]`.
- Six CLIs pattern-matching two positional paths (`energy_cli.rs:39-42`,
  `cost_recovery_cli.rs:52-58`, and four more) → variadic, usage `<SESSIONS.csv>...`.
- GUI cost recovery tab: keep both slots visible, drop `Sessions2` from `can_run`
  (`state.rs:307-309`), and gate running on `reports_cover` over whatever is filled. That replaces
  a count with the question actually being asked — and catches two files that leave a gap, which
  the present check misses.

Note the document's line 81 is wrong: the cost recovery tab does **not** check month alignment
today (`state.rs:266-288`). It requires four filled slots. Alignment is enforced only on the
reimbursement tab (`:496`).

### Reimbursement period

Delete `report_month` (`file_name.rs:59-63`) and `check_same_month`
(`api/pure/reimbursement.rs:254-270`). With arbitrary session ranges a user may never hold a
whole-month report, so the requirement becomes unsatisfiable. Take the reconciliation period from
the charges report's month and require the session reports to *cover* it via `reports_cover`. The
reimbursement tab gains the same one-or-two flexibility.

---

## Test data and fixtures

Only `data/evolute` may be committed — it is anonymised. `data/hydro_bills` and
`data/green_button` stay out, so end-to-end tests needing a bill or meter export remain local and
`#[ignore]`d.

**New fixtures.** Commit a seconds-precision session report built from the five Aug–Sep rows, and
a charges report named to the new convention (`XX-XX_Charges_June 2026-June 2026.csv`) under
`tests/fixtures/`. There is no committed charges fixture today — tests write one into
`env::temp_dir()` (`charges_report.rs:826-839`) and `tests/charges_report/real_reports.rs` is
`#[ignore]`d. Give the existing session fixtures conforming names; none of them parse today, so
the name check is bypassed entirely.

**Massage the June/May/July copies** in `data/evolute` so start and end agree with duration, under
new-convention names. Leave one row off by +1s (must pass) and one off by +5s (must raise
`InconsistentDuration`), so the file tests the tolerance boundary rather than the happy path.

**Re-author, do not just regenerate:**

| Fixture row | Today | Why it breaks | Fix |
|---|---|---|---|
| `SPIKE`, `FARSPIKE` | `16:22→16:22`, duration `0:00:30` | 29s outside the new tolerance → gains `InconsistentDuration`. Exclusion is tested **before** the spike branch (`common.rs:1174-1180`), so the row lands in `excluded` and stops exercising the spike path — while the test still passes | set duration to `0:00:00`; leave `Active_Charge_Time` zero |
| `INGAP` | `2026-03-08 02:30` | no gap exists at a fixed offset | delete |
| `Session_Report_Band.csv` (`EARLYOUT` 60s, `EARLYIN` 59s) | built around the 60-second window | tests nothing at ±1s | rebuild: one row off by 1s that passes, one off by 2s that fails. Update `src/session/consistency_band_tests.rs` |

**Goldens.** Adding `" EDT"` widens the Excluded sessions `From`/`To` columns, and
`tests/session/report_rendering.rs:104-110` asserts every table row is padded to equal width.
Regenerate with `UPDATE_GOLDEN=1` (`src/golden.rs:58-70`) and **read the diff** — it lands in the
same commit as Phase 1's behaviour change, and a wall of width-only diff is where a real change
hides.

---

## Documentation

| Doc | Action |
|---|---|
| `docs/session/time-reporting-uncertainty.md` (178) | archive — it is the record of why the padding existed |
| `docs/historic-feature.md` (146) | archive at Phase 0 |
| `docs/dst-gap-plan.md` (208), `docs/errors-plan.md` (209) | archive — completed plans; the first describes machinery Phase 1 deletes |
| `docs/time/README.md` (216) | heavy rewrite — "One probe, three outcomes", "Settling the fold", "The gap", "Two resolvers" all go |
| `docs/session/README.md` (195) | rewrite "Boundaries and the time grid", "Brackets", "Anomalies" |
| `docs/maintenance-manual.md` (719) | rewrite the grid section (~270–318) |
| `docs/session/segment-tiling.md` (137) | rewrite the bracket sections (~81–122) |
| `docs/ERRORS.md` (789) | remove the dead anomaly sections — **in lockstep** with `tests/docs_errors.rs:24-45`, which pins it |
| `docs/site-specific-constants.md` (102) | drop the `TIME_GRID_STEP` entry (~:84) |
| `README.md` (286) | drop the `historic` build lines (~247–257) and the uncertainty-doc link (~:171) |
| `docs/app-cheat-sheet.md` (191) | check for the two-session-report requirement |
| `docs/Evolute_portal_alignment.md` (114) | correct line 81, the `Timestamp` signatures (62–79), the doc path on line 22, and the exact-equality invariant. Keep the original 1/2/3 numbering with a note that implementation order is inverted |
| `docs/Questions_for_Evolute.md` (41) | **do not touch** |

`CLAUDE.md` also needs its `--features historic` commands removed (`:13, 27`) and the `tests/`
rule about the feature (`:135-138`) dropped.

---

## Verification

Per phase, in this order — a broken build hides every failure downstream of it:

```sh
cargo check --all-targets
cargo test
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps
cargo fmt --check && cargo clippy --all-targets
```

The `--features historic` variants disappear after Phase 0.

Then, each phase:

1. **Diff the baseline report.** Phases 0 and 3 byte-identical; Phases 1 and 2 inspected.
2. `grep -rn TIME_GRID_STEP\\\|Bracket\\\|historic src/ docs/ README.md tests/ .github/` — prose is
   not compiled, and the last rename left 29 broken doc links and five stale documents behind.
3. Run the GUI cost recovery tab with **one** session report covering the period, then with two,
   then with two that leave a gap. The third must be refused.
4. Confirm a rendered time names its zone, and that a report spanning the DST transition shows
   both `EST` and `EDT`.
