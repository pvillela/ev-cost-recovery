# Evolute portal alignment

Changes to the crate due to newly confirmed information after gaining access to the Evolute portal.

**Status: done.** Every change below is on branch `evolute-portal-align`. What was built differs from what
was first written here in a few places; each difference is noted where it arises, and
[As built](#as-built) summarises the outcome.

## Changes

### Disposition of "historic" code

To facilitate the required changes, we will remove all code gated by / dependent on the "historic"
feature.

> It served three targets, not two: `ev_peak_cli`, `ev_peak_gui` and `examples/sessions.rs`.

### Session start and end time precision and time zone

Session start and end dates are now reported with seconds precision, not minutes. In addition, the
invariant `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End` should hold. Furthermore, it
has been confirmed that `Conn_DateTime_Start` and `Conn_DateTime_End` are reported in the EST time
zone, not ET as we had previously been told.

> **The invariant does not hold exactly.** In the first real portal export,
> `Session_Report_August_1_2026-September_4_2026.csv`, four of the five rows satisfy it and one does
> not: `2026-08-30 16:57:00 + 2:03:50` is reported as ending at `19:00:49`, a second early.
> `Active_Charge_Time` sits a second under `Conn_Duration` on three of the five. Something in the
> source rounds at second level, so the check allows one second of slack — `DURATION_TOLERANCE`,
> whose doc comment carries this evidence. Exact equality would exclude a fifth of the only genuine
> export there is.

Implications:
- All time padding is no longer appropriate.
- The session `adj_*` fields are no longer necessary and all logic should be based on the
  corresponding fields without adjustment. **The session ones only:** `adj_conn_start_of` and
  `adj_conn_end_of` in `src/session/common.rs`. Toronto Hydro's `adj_*` figures in
  `src/hydro_bill/` are its adjusted demand — loss factor and days/30 proration — and are untouched.
- TIME_GRID_STEP will have little if any utility.
- References to TIME_GRID_STEP in most (perhaps all) documentation should be removed.
- Bracketing of values is no longer appropriate. The Bracket type should be removed.
- Most logic related to the DST fold and gap should be removed. Only logic related to the display of
  data in local time (where required) would likely survive.
- The `docs/session/time-reporting-uncertainty.md` document becomes obsolete. The invariant
  `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End` or its UTC equivalent should be used to
  judge start-end time consistency.
- Anomalies related to the above that are no longer needed should be removed.
- Comments and documents must be updated accordingly.

> Seconds already parsed before any of this: `parse_local` tried `%Y-%m-%d %H:%M:%S` and fell back
> to `%H:%M`. What changed is not the parsing but what the code downstream is allowed to trust.

### Session report file scope and name

The session report is not restricted to a calendar month -- it can have any start date and a
subsequent end date. The naming convention is exemplified by
`Session_Report_August_28_2026-September_1_2026.csv`.

Implications:
- All logic involving the reading of session reports must be modified to reflect this reality.
- Comments and documents must be updated accordingly.

### Charges report file scope and name

The charges report is not restricted to a single calendar month -- it can span any number of
contiguous calendar months. The naming convention is exemplified by:
`123 Foo Bar Road_Charges_August 2026-August 2026.csv` and
`123 Foo Bar Road_Charges_November 2026-January 2027.csv`.

Implications:
- All logic involving the reading of charges reports must be modified to reflect this reality.
- Until further notice, we will only accept charges reports spanning exactly one month, i.e., our
  charges report reading function must require that the start and end months in the file name be the
  same.
- Comments and documents must be updated accordingly.

> **This is a replacement, not a tweak.** The old name was `<building>_charges_<ISO timestamp>.csv`,
> parsed by splitting on a lowercase `_charges_` marker and discarding everything after the `T`. The
> new form shares no structure with it: capital `C`, spaces, month names, a range. The old form is
> no longer read at all.
>
> The one-month restriction is the *reader's*, not the parser's.
> `parse_charges_report_name` returns whatever range the name states and `charges_report` refuses a
> longer one, so the parser stays reusable when the restriction lifts.

## Phases of work

The phases below are numbered as first written. **They were implemented in the order 0, 2, 1, 3.**

`historic` came out first, as its own phase 0, because deleting code and changing behaviour in one
commit makes both unreviewable. EST was done before precision because the DST fold resolver reads
its tolerance windows from `TIME_GRID_STEP`: doing precision first would have meant rewriting a
resolver that the EST change then deletes.

### Phase 1

Impact of session start and end time precision.

### Phase 2

Impact of session start and end times being reported in EST.

### Phase 3

Impact of changes to session and charges report files scope and name.

- **CSV file name parsing functions** -- I want the parsing of session and charges report file name
  parsing to be encapsulated in two functions:

  ```rust
  fn parse_session_report_name(name: &str) -> Result<(Date, Date), SessionReportNameError>
  fn parse_charges_report_name(name: &str) -> Result<(Date, Date), ChargesReportNameError>
  ```

  that return the start and end dates from the file name.

  In both cases, the second returned date should be the last day of the date range that the file
  name specifies. For example, the second date returned by

  ```
  parse_charges_report_name("123 Foo Bar Road_Charges_November 2026-January 2027.csv")
  ```

  should correspond to 2027-01-31.

  > **`jiff::civil::Date`, not `Timestamp`.** A file name states calendar days, not instants, and an
  > instant needs a zone the name does not contain. Every file-name and month path in the crate
  > already used `Date`; `Timestamp` is reserved for session and meter instants.

- **File picking in GUI** -- The GUI cost recovery tab should allow one or two session reports to be
  provided. If the first one covers the billing period, then the second one is not required.

  > The tab **did not** require calendar-month-aligned reports, as first written here. It required
  > all four input slots to be filled, and checked only that a name could be parsed at all; month
  > alignment was enforced on the *Reimbursement* tab. So this item was about relaxing an arity
  > requirement, not an alignment one — and the arity was relaxed all the way down, not only in the
  > GUI.

- Other code and documentation changes resulting from the changes to session and charges report
  files scope and name.

## As built

**Phase 0 — remove `historic`.** No behaviour change: the surplus report for the period ending
2026-06-23 is byte-identical before and after. `session::ioi`, `session::excel::historic`, the DST
mapping helpers, the three targets, `AnomalyKind::WorkbookDiscrepancy` and
`IntervalEstimates::write_logs` all went. `TZ_OFFSETS` and `TIME_ZONE_NAME` stayed: both are
load-bearing in every build, and only their gated re-exports were removed.

**Phase 1 (was 2) — EST.** `SESSION_OFFSET` and `session_instant` replace the offset probe.
`FellInDstGap`, `DstUnresolvable` and `DstAmbiguousDuplicated` are gone, with the unplaceable
sentinels and `Session::is_placeable`; `excludes_session` now names `InconsistentDuration` alone.

*This moved money.* The total kWh is unchanged, but sessions shifted an hour against Time-of-Use
boundaries that stay on prevailing local time, so on-peak drained into mid- and off-peak:

| | before | after |
|---|---:|---:|
| On-peak | 454.072 | 379.441 |
| Mid-peak | 229.110 | 243.680 |
| Off-peak | 678.823 | 738.884 |

*And it made a discrepancy visible.* Reading EST while displaying prevailing local time means a
summer session shows an hour later than the portal states it. Every rendered time now names its
offset through the new `time::format`, and one report can carry both when two peaks fall either side
of a transition. The workbooks carry no labels and are on one clock throughout, because a serial
cell with a zone suffix stops being a date to Excel.

**Phase 2 (was 1) — precision.** The padding, `Bracket`, `TIME_GRID_STEP`, `OffGridTimes` and
`truncate_to` are gone; `duration_is_consistent` is one check with one second of slack. The workbook
lost four columns that had become exact duplicates of four others.

Two consequences worth knowing. A session reported to start and end at the same instant has no span,
so its energy is filed under the band that instant falls in rather than divided by zero. And the
minute-precision reports in `data/evolute` no longer read: almost every row misses the invariant,
which is those files being pre-portal rather than a regression. `data/baseline/README.md` records
the figures and how the `-seconds` copies were made.

**Phase 3 — names and scope.** Both parsers with typed errors, `&[&Path]` through six API entry
points and six CLIs, and the GUI's optional second slot. `report_month` and `check_same_month` are
gone: the reconciliation takes its month from the charges report and checks the session reports
cover it, which is a question a user can always satisfy — a month-aligned session report is one they
may never have.

Verified end to end on the June period: two reports give the phase 2 figures exactly, one report
covering the whole period gives the same figures, a redundant third changes nothing, and a set
leaving a day uncovered is refused by name.
