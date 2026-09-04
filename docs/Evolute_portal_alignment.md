# Evolute portal alignment

Changes to the crate due to newly confirmed information after gaining access to the Evolute portal.

## Changes

### Disposition of "historic" code

To facilitate the required changes, we will remove all code gated by / dependent on the "historic" feature.

### Session start and end time precision and time zone

Session start and end dates are now reported with seconds precision, not minutes. In addition, the invariant `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End` should hold. Furthermore, it has been confirmed that `Conn_DateTime_Start` and `Conn_DateTime_End` are reported in the EST time zone, not ET as we had previously been told.  

Implications:
- All time padding is no longer appropriate.
- `adj_*` fields are no longer necessary and all logic should be based on the corresponding fields without adjustment.
- TIME_GRID_STEP will have little if any utility.
- References to TIME_GRID_STEP in most (perhaps all) documentation should be removed.
- Bracketing of values is no longer appropriate. The Bracket type should be removed.
- Most logic related to the DST fold and gap should be removed. Only logic related to the display of data in local time (where required) would likely survive.
- The `time-reporting-uncertainty.md` document becomes obsolete. The invariant `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End` or its UTC equivalent should be used to judge start-end time consistency.
- Anomalies related to the above that are no longer needed should be removed.
- Comments and documents must be updated accordingly.

### Session report file scope and name

The session report is not restricted to a calendar month -- it can have any start date and a subsequent end date. The naming convention is exemplified by `Session_Report_August_28_2026-September_1_2026.csv`.

Implications:
- All logic involving the reading of session reports must be modified to reflect this reality.
- Comments and documents must be updated accordingly.

### Charges report file scope and name

The charges report is not restricted to a single calendar month -- it can span any number of contiguous calendar months. The naming convention is exemplified by: `123 Foo Bar Road_Charges_August 2026-August 2026.csv` and `123 Foo Bar Road_Charges_November 2026-January 2027.csv`.

Implications:
- All logic involving the reading of charges reports must be modified to reflect this reality.
- Until further notice, we will only accept charges reports spanning exactly one month, i.e., our charges report reading function must require that the start and end months in the file name be the same.
- Comments and documents must be updated accordingly.

## Phase of work

I want to implement the code and documentation changs in phases.

### Phase 1

Impact of session  start and end time precision.

### Phase 2

Impact of session start and end times  being reported in EST.

### Phase 3

Impact of changes to session and charges report files scope and name.

- **CSV file name parsing functions** -- I want the parsing of session and charges report file name parsing to be encapsulated in two functions:

  ```
  fn parse_session_report_name(name: &str) -> Result<(Timestamp, Timestamp), SessionReportNameError>
  ```

  that returns the start and end dates from the session report file name, and a function

  ```
  fn parse_charges_report_name(name: &str) -> Result<(Timestamp, Timestamp), ChargesReportNameError>
  ```

  that returns the start and end dates from the charges report file name.

  In both cases, the second returned `Timestamp` should be the last day of the date range that the file name specifies. For example, the second `Timestamp` returned by

  ```
  parse_charges_report_name("123 Foo Bar Road_Charges_November 2026-January 2027.csv")
  ```

  should correspond to 2027-01-31.

- **File picking in GUI** -- The GUI cost recovery tab, which currently requires two calendar-month-aligned session reports, should allow one or two session reports to be provided. If the first one covers the billing period, then the second one is not required.

- Other code and documentation changes resulting from the changes to session and charges report files scope and name.