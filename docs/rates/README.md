# The rates workbook

The EV cost-recovery rates are read from an Excel workbook (`.xlsx`), which can have any name. It
is read each time the figures are worked out, so a change saved in the spreadsheet is used on the
next run.

`src/rates/` holds the code: `pure.rs` for the rates as the calculations take them, and `excel/`
for reading them from the workbook.

## The sheet

The rates are on the sheet named `rates`. If there is none, the sheet named `Sheet1` is used.
Capitals and surrounding spaces in the sheet name do not matter. Other sheets are ignored.

## The columns

Row 1 names the columns. It must hold these four names, spelled exactly as shown, in any order:

| Column           | Contents of the rows below                                   |
| :--------------- | :----------------------------------------------------------- |
| `effective_date` | The first day the rates on that row apply. It must be an Excel date — a date the spreadsheet shows in a date format — not text, and with no time of day. |
| `on_peak`        | The on-peak rate, in dollars per kilowatt-hour. A number greater than zero. |
| `mid_peak`       | The mid-peak rate, in dollars per kilowatt-hour. A number greater than zero. |
| `off_peak`       | The off-peak rate, in dollars per kilowatt-hour. A number greater than zero. |

Other columns are ignored, and can hold notes.

## The rows

- The rates start on row 2 and end at the first row with an empty `effective_date`. Nothing may
  follow that row in the four columns.
- The effective dates must increase down the sheet: each one later than the one above it.
- The effective dates are checked on every run. A rate is checked only when a run uses its row, so
  an old row with a rate missing does not stop a run that does not reach it.

## Which rows are used

The rates in effect on a date are those on the last row whose `effective_date` is on or before
that date.

- **Cost recovery** uses the rates in effect on the billing period's first day. If a row's
  `effective_date` falls within the period, the period is split at local midnight at the start of
  that date, and the rest of it is priced at that row's rates. At most one row may fall within a
  billing period.
- **Evolute reimbursement** uses the rates in effect on the 1st of the month. No row may fall
  within the month after the 1st.

## An example

| effective_date | on_peak | mid_peak | off_peak |
| :------------- | ------: | -------: | -------: |
| 2026-05-01     |  0.1100 |   0.0900 |   0.0700 |
| 2026-09-01     |  0.5152 |   0.4740 |   0.4218 |

## A known limit

Dates are read in the 1900 date system, which every current version of Excel and LibreOffice uses
by default. A workbook saved in the 1904 date system, an option in old versions of Excel for Mac,
would read every date four years and one day early.
