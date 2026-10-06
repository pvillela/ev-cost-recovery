# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 1.0.1 - 2026-10-06

### Changed

- The refusal for session reports that leave part of a billing period or month uncovered reads
  `the session report does not cover the …` when one report is given. With two or more it reads
  `the session reports do not cover the …`.
- Fixed change log for v1.0.0.

## 1.0.0 - 2026-10-04

### Added

- The Cost recovery and Evolute reimbursement reports end with a **Source Data** section. Under
  *Session data* it counts the records dropped as identical copies, then lists *Sessions left out*,
  *Sessions needing a look* and *Overall anomalies*. Overall anomalies tables every anomaly in each
  session report by kind, explains each kind counted, and suggests converting the file to see each
  row.
- Both conversions on the Convert to workbook tab produce a report of what they found. The tab
  shows it with Copy and Save…, offered as `<workbook name>.conversion.report.md`, and the
  conversion command-line tools print it. The session conversion report lists the rows that needed
  a judgement call in a Row / Session / Anomaly table, with each kind explained once beneath it. The
  Green Button conversion report groups anomalies by kind, with example hours and a description of
  each kind.

### Changed

- When the EV cost-recovery rates change during a billing period, the table at the top of the
  Cost recovery report's *EV Cost Recovery* section has a kWh column, giving the energy priced at
  each set of rates and in total. Its *Amount* column is headed *Recovery*, and its total line,
  *Cost recovery*, is named *Billing period total*.
- The app and the command-line tools write no `.log` files. What the logs held is in the reports
  described above.
- The Convert to workbook tab shows the workbook's path once, on the report's `Workbook:` line.
- The app window opens at 1000 × 700, which fits a 1024 × 768 screen, and cannot be made narrower
  than 900, the width its tab bar needs.
- **Breaking, `api`:** `SessionWriteReport` and `GbWriteReport` have no `log` field, and gain
  `to_markdown()`, which renders the conversion report. The session results' `notes` carry no logs:
  `SessionNotes::write_logs` and the `logs` fields are removed, and `Sessions::from_session_lists`
  takes no logs argument.

### Fixed

- The About window fits inside the app window. A smaller app window cut off its edges. Its notices
  scroll sideways as well as down, so a long line can be read.
- In a window narrower than its tab bar, the buttons at the end of the bar were drawn over the last
  tab.

## 0.3.1 - 2026-09-27

### Changed

- The command-line tool `ev_csv_to_xlsx` is renamed `session_csv_to_xlsx_cli`, after the function
  it calls and with the same suffix as the other command-line tools.

## 0.3.0 - 2026-09-25

### Added

- The EV cost-recovery rates are read from a rates workbook: an `.xlsx` file with a sheet named
  `rates` (or `Sheet1`), columns `effective_date`, `on_peak`, `mid_peak` and `off_peak`, and one row
  for each date the rates change. `docs/rates/README.md` describes it in full. The Cost recovery and
  Evolute reimbursement tabs share one workbook: choosing it on one tab chooses it on both. It is
  read on every run, so a change saved in the spreadsheet is used the next time the figures are
  worked out. The reports name the workbook they used.
- Each problem a rates workbook can have gets its own error message, listed in `docs/ERRORS.md`.
- Touchpad scrolling on a Linux Wayland desktop, such as KDE Plasma, is accelerated to the rate X11
  gives, and faster finger movement scrolls further. Mouse wheels and all input on Windows are not
  affected.

### Changed

- The rates are no longer typed on the tabs or given on the command line. `cost_recovery_cli` and
  `cost_recovery_surplus_cli` take the rates workbook's path in their place, for example
  `cost_recovery_cli <YYYY-MM-DD> <RATES.xlsx> <SESSIONS.csv>...`.
- How the workbook's rows are applied: Cost recovery prices a billing period at the rates in effect
  on its first day. If a row's effective date falls within the period, the rest of the period, from
  local midnight at the start of that date, is priced at that row's rates. More than one such row
  in a billing period is refused. Evolute reimbursement prices a month at the rates in effect on
  the 1st, and refuses a row that falls within the month after the 1st.
- The site-load report states the panel as "at most 10 active 40 A breakers at any time". The
  installed panel holds 20 breakers, and 10 is the limit on active ones that the site model uses.
- **Breaking, `api`:** `cost_recovery`, `cost_recovery_surplus` and
  `reconcile_evolute_reimbursement` take the rates workbook's path (`rates_xlsx: &Path`) in place of
  `CostRecoveryRates` values. `CostRecoveryRates` cannot be parsed from text: its `FromStr`,
  `RateBand` and `CostRecoveryRatesError` are removed. `ReadError` gains a `RatesWorkbook` variant.

## 0.2.0 - 2026-09-21

### Changed

- The Linux release binary is built on Ubuntu 26.04, and needs Ubuntu 26.04 or newer (or another
  distribution with a glibc at least as recent). Earlier releases ran on 22.04 and newer.
- On Linux, file dialogs are the desktop's own, opened through the XDG desktop portal, and start in
  the folder the app names when the portal is version 1.17 or newer. The binary needs no GTK
  libraries.
- When the rows in a session report's Anomalies and Excluded sessions tables come from more than
  one session report file, they are grouped under the name of the file they come from, so each Row
  number can be looked up. A report read from one file is unaffected.
- An excluded session also shows the anomalies it shares with other rows. Of two rows with the same
  session id where one is excluded, both show `DuplicateId`, where only the kept row did.
- A cost-recovery rate that cannot be read from the command line is reported with the band it is
  for and the text that failed.
- `gb_peak_values` and `ev_csv_to_xlsx` behave like the other command-line tools: errors start with
  `error:`, usage printed after a failure goes to standard error, and a log that could not be
  written does not on its own make the exit status 1, since the workbook is written and correct.
- The surplus report's note on *Basis* says that each basis has its own detail report, without
  naming the app's Peak power detail tab.
- The description of `InconsistentDuration` is reworded, and the note on the connection-span column
  in session workbooks no longer describes columns the sheet does not have.
- **Breaking, `api`:** `parse_rates` is replaced by `CostRecoveryRates`'s `FromStr`, which returns
  `CostRecoveryRatesError`. `CoverageError::PeriodNotCovered` carries a `CoveredSpan`, and
  `CoverageError` gains `NoReports`, for an empty list of session reports.

### Fixed

- An amount of zero printed as `-0.00`, for example in a month with no sessions, on a bill with no
  rebate, or for a surplus that rounds to nothing. It prints as `0.00`.
- The Evolute reimbursement tab, refusing session reports that do not cover the month, called the
  month "the billing period". It names the calendar month.
- The description of `ZeroActiveChargeTime` said the software substitutes an average power for such
  a session. It says what happens: the session's energy counts towards every estimate, spread over
  its connection span like any other session's. The estimates themselves are unaffected.
- A Toronto Hydro bill PDF that lists a font it shows no text in was refused; it is read. Text
  following a restored graphics state in a bill is decoded in the font in effect there.
- A session duration with a leading `+`, such as `+5:07:53`, or with more hours than can be counted,
  is reported as a malformed cell. The `+` form was read as a valid duration.
- The command-line tools stop with an argument error (exit status 1) on a path that is not valid
  Unicode, where they crashed.
- A save that fails on the Peak power detail tab shows its message on that tab, where it appeared
  under the Cost recovery tab's button.
- A file given as a session report, whose name is not a session report's, is named once in the
  error, where it was named twice.
- A Green Button export in which interval data cannot be matched to its meter reading is reported
  with the entry concerned.

## 0.1.0 - 2026-09-12

### Added

- Initial release: the `ev_cost_recovery` desktop app, with the Cost recovery, Peak power detail,
  Evolute reimbursement and Convert to workbook tabs, and the command-line tools
  `cost_recovery_cli`, `cost_recovery_surplus_cli`, `energy_cli`, `energy_cost_cli`,
  `ev_csv_to_xlsx`, `gb_peak_values`, `hydro_bill_dump`, `peak_power_cli` and
  `peak_power_cost_cli`.
