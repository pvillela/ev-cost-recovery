# Errors and anomalies

This document contains every error and anomaly the `ev_cost_recovery` app reports on screen or writes to a log, what each one means, and what to do about it.

To look one up, find the words you can see on screen in the contents below. Messages are quoted as
the app builds them, with `<the varying part>` in angle brackets where your own file names, dates
and figures appear.

## How this is arranged

Entries are grouped by what happened to your work, then listed alphabetically. Within a group, the
messages come first and the anomaly tokens after them.

| Group | What it means |
| --- | --- |
| [Stops the run](#stops-the-run) | The function did not produce its result. |
| [Changes the figures](#changes-the-figures) | The result is there, and something in the data moved it or was left out of it. |
| [Worth knowing](#worth-knowing) | The result stands and nothing in it moved. |

The group ranks what happened to your work, not how loud the message is. A red block on screen can
sit in *Worth knowing*.

In *Stops the run*, each entry carries one of two labels:

- **Get the data** — the files you gave do not span what was asked for. You may need a fresh export
  from Evolute, a fresh download of the Green Button export from Toronto Hydro, or a different file
  you already have.
- **Tell the maintainer** — the app could not make sense of a file. Nothing you type or re-pick will
  change that; pass the file and the message on.

An entry there with no label says what to do in its own text. Nothing in the other two groups needs
you to do anything, which is why they carry no labels.

Some messages end in wording that does not come from this software — the operating system's, or
that of the libraries that read CSV, XML and PDF files. Those entries say so and show an example.

## Contents

**Stops the run**

- [Charges Report — missing required column](#charges-report--missing-required-column)
- [Charges Report — row … cannot read](#charges-report--row--cannot-read)
- [Charges Report — the file holds no rows](#charges-report--the-file-holds-no-rows)
- [Charges Report — the file could not be read](#charges-report--the-file-could-not-be-read)
- [Charges Report — rows billed for dates outside the month](#charges-report--rows-billed-for-dates-outside-the-month)
- [does not name a billing period](#does-not-name-a-billing-period)
- [Green Button Export — no readings in the billing period](#green-button-export--no-readings-in-the-billing-period)
- [Green Button Export — the file could not be read](#green-button-export--the-file-could-not-be-read)
- [Hydro Bill — the bill has no such figure](#hydro-bill--the-bill-has-no-such-figure)
- [Hydro Bill — a value could not be read as expected](#hydro-bill--a-value-could-not-be-read-as-expected)
- [Hydro Bill — the page could not be read](#hydro-bill--the-page-could-not-be-read)
- [Hydro Bill — unrecognised charge line](#hydro-bill--unrecognised-charge-line)
- [Hydro Bill — the layout is not what was expected](#hydro-bill--the-layout-is-not-what-was-expected)
- [no consumption in a band, so no rate](#no-consumption-in-a-band-so-no-rate)
- [Rates workbook — the name does not end in .xlsx](#rates-workbook--the-name-does-not-end-in-xlsx)
- [Rates workbook — the file could not be read](#rates-workbook--the-file-could-not-be-read)
- [Rates workbook — no rates sheet](#rates-workbook--no-rates-sheet)
- [Rates workbook — row 1 does not name a column](#rates-workbook--row-1-does-not-name-a-column)
- [Rates workbook — a column named twice](#rates-workbook--a-column-named-twice)
- [Rates workbook — an effective date that is not a date](#rates-workbook--an-effective-date-that-is-not-a-date)
- [Rates workbook — an effective date with a time of day](#rates-workbook--an-effective-date-with-a-time-of-day)
- [Rates workbook — something below the last rates](#rates-workbook--something-below-the-last-rates)
- [Rates workbook — there are no rates](#rates-workbook--there-are-no-rates)
- [Rates workbook — effective dates out of order](#rates-workbook--effective-dates-out-of-order)
- [Rates workbook — no rates in effect](#rates-workbook--no-rates-in-effect)
- [Rates workbook — the rates change more than once in a billing period](#rates-workbook--the-rates-change-more-than-once-in-a-billing-period)
- [Rates workbook — the rates change within the month](#rates-workbook--the-rates-change-within-the-month)
- [Rates workbook — a rate that is not a positive number](#rates-workbook--a-rate-that-is-not-a-positive-number)
- [saving a report or a workbook failed](#saving-a-report-or-a-workbook-failed)
- [Session Report — missing required column](#session-report--missing-required-column)
- [Session Report — row … cannot read](#session-report--row--cannot-read)
- [Session Report — the file could not be read](#session-report--the-file-could-not-be-read)
- [the closing date and the calendar disagree](#the-closing-date-and-the-calendar-disagree)
- [the meter data covers only part of the period](#the-meter-data-covers-only-part-of-the-period)
- [the session reports do not cover the billing period](#the-session-reports-do-not-cover-the-billing-period)
- [there is no maximum to estimate against](#there-is-no-maximum-to-estimate-against)
- [a figure the bill states as zero](#a-figure-the-bill-states-as-zero)

**Changes the figures**

- Session report: [`DuplicateId`](#duplicateid),
  [`InconsistentDuration`](#inconsistentduration),
  [`ZeroActiveChargeTime`](#zeroactivechargetime)
- Green Button export: [`DuplicateInterval`](#duplicateinterval),
  [`ImplausibleGap`](#implausiblegap), [`MisalignedInterval`](#misalignedinterval),
  [`MissingInterval`](#missinginterval), [`MissingKva`](#missingkva), [`MissingKw`](#missingkw),
  [`MissingKwh`](#missingkwh)

**Worth knowing**

- [a breaker billed for part of the month](#a-breaker-billed-for-part-of-the-month)
- [periods that do not hold a full billing period's intervals](#periods-that-do-not-hold-a-full-billing-periods-intervals)
- [the run's log was not written](#the-runs-log-was-not-written)
- [the workbook was written, but its run log was not](#the-workbook-was-written-but-its-run-log-was-not)
- Session report: [`ExcessiveAvgKw`](#excessiveavgkw)

---

# Stops the run

The function did not produce its result. Each entry says whether the remedy is yours or the
maintainer's.

### Charges Report — missing required column

**Tell the maintainer**

> Charges Report `<file name>`: missing required column `<column name>`

**Where** Evolute reimbursement.

The file was read and does not carry a column the reconciliation needs. Usually the file is not a
Charges Report at all — a session report picked at the Charges Report slot fails exactly this way.
Check you picked the right file; if you did, Evolute's report format has changed.

`src/csv.rs`: `CsvReadError::Display`

### Charges Report — row … cannot read

**Tell the maintainer**

> Charges Report `<file name>`: row `<number>`, column `<column name>`: cannot read
> `"<the cell's contents>"`: `<why not>`

**Where** Evolute reimbursement.

One cell does not hold the kind of value its column is supposed to. The row number counts the header
row, so it is the number the spreadsheet shows. The reason at the end is the number or date reader's
own wording.

`src/csv.rs`: `CsvReadError::Display`

### Charges Report — the file holds no rows

**Tell the maintainer**

> Charges Report `<file name>`: the file holds no rows; a Charges Report carries one row per
> breaker even in a month nothing was billed for

**Where** Evolute reimbursement.

An empty report is not the same as a month with nothing in it — Evolute writes a row per breaker
either way. So this says the file is truncated or is not a Charges Report.

`src/charges_report.rs`: `ChargesReportError::Display`

### Charges Report — the file could not be read

**Tell the maintainer**

> Charges Report `<file name>`: `<why not>`

**Where** Evolute reimbursement.

Everything after the file name is the CSV library's own wording, not this software's — for
instance, `CSV error: record 4 (line: 5, byte: 210): found record with 9 fields, but the previous
record has 11 fields`. The file is damaged or is not a CSV.

`src/csv.rs`: `CsvReadError::Display`

### Charges Report — rows billed for dates outside the month

**Tell the maintainer**

> Charges Report `<file name>`: the file name says the report covers `<date>` to `<date>`, but
> these rows are billed for dates outside it
> ` `
> &nbsp;&nbsp;`<date>` to `<date>`: rows `<numbers>`

One indented line per span of dates found outside the month, with the rows carrying it.

**Where** Evolute reimbursement.

The report's file name names a month, and rows inside it are billed for dates in another. The month
in the name is what the reconciliation prices against, so the file is refused rather than half used.

`src/charges_report.rs`: `row_list`

### does not name a billing period

**Tell the maintainer**

> `<date>` does not name a billing period: one is labelled by day 23 of the month it ends in

**Where** Cost recovery.

A billing period is named by the day it closes, which for this building is the 23rd. The date came
from the bill, so a bill closing on another day — a different rate plan, or another utility's bill —
produces this.

`src/hydro_bill/billing_period.rs`: `NotABillingPeriodEnding::Display`

### Green Button Export — no readings in the billing period

**Get the data**

> Green Button Export `<file name>`: no readings in the billing period ending `<date>`. The feed
> covers `<date>` to `<date>`.

When the export holds nothing at all, the second sentence reads *The feed carries no readings at
all.* instead.

**Where** Cost recovery.

The export does not reach the period the bill is for. This is an error rather than a row of zeroes
on purpose: zeroes would read as a month with no consumption, which is a figure someone could go on
to argue a bill from.

Download an export from Toronto Hydro that spans the billing period, and check the dates in the
second sentence against the bill before running again.

`src/green_button/read_xml.rs`: `GbReadError::Display`

### Green Button Export — the file could not be read

**Tell the maintainer**

> Green Button Export `<file name>`: `<why not>`

**Where** Cost recovery, Convert to workbook.

Two different failures share this shape, and everything after the file name comes from elsewhere:

- The file could not be opened — the operating system's wording, such as
  `No such file or directory (os error 2)`. Check the file is still where you picked it from.
- The file is not an ESPI feed this can parse — the XML reader's wording. The file is damaged, or is
  not a Green Button export.

`src/green_button/read_xml.rs`: `GbReadError::Display`

### Hydro Bill — the bill has no such figure

**Tell the maintainer**

> Hydro Bill `<file name>`: the bill has no `<the thing that is missing>`

**Where** Cost recovery.

The PDF was read and a figure the calculation needs is not on it. The bill is for a different rate
plan, or Toronto Hydro has changed its layout.

`src/hydro_bill/bill_pdf.rs`: `BillError::Display`

### Hydro Bill — a value could not be read as expected

**Tell the maintainer**

> Hydro Bill `<file name>`: not `<what was expected>`: `"<the text found instead>"`

**Where** Cost recovery.

A value was where it should be and is not the kind of value expected — a date that is not a date, a
figure that is not a number. The text found is quoted so it can be compared against the bill.

`src/hydro_bill/bill_pdf.rs`: `BillError::Display`

### Hydro Bill — the page could not be read

**Tell the maintainer**

> Hydro Bill `<file name>`: page `<number>`: `<why not>`

The page number is omitted when the failure was in loading the file rather than in reading one page.

**Where** Cost recovery.

Five reasons appear at the end, and the first two are the PDF library's own wording:

- The file could not be opened, or is not a PDF this can load.
- The page's fonts or its content could not be reached.
- `font /<name>: no ToUnicode CMap: <why not>` — a font on the page carries no table saying what its
  glyphs stand for. The bills use subset fonts, where without that table the text cannot be read at
  all.
- `font /<name>: unreadable ToUnicode CMap: <why not>` — the table is there and could not be read.
- `font /<name> is used on the page but is not among its resources, so the text shown in it cannot
  be decoded` — the page uses a font it never declares. Refused rather than skipped: dropping those
  runs would leave a line looking complete while missing half of what it says.

None of these is something to change in the file. Pass the bill and the message on.

`src/hydro_bill/pdf_text.rs`: `PdfTextCause::Display`

### Hydro Bill — unrecognised charge line

**Tell the maintainer**

> Hydro Bill `<file name>`: unrecognised charge line: `<the line as it appears on the bill>`

**Where** Cost recovery.

A line in the bill's charges is one this software has no rule for. It is refused rather than ignored,
because a charge silently skipped is money missing from a figure that still looks complete. A new
charge on the bill needs a decision about whether EV charging bears any of it.

`src/hydro_bill/bill_pdf.rs`: `BillError::Display`

### Hydro Bill — the layout is not what was expected

**Tell the maintainer**

> Hydro Bill `<file name>`: `<what is wrong with the layout>`

**Where** Cost recovery.

The bill's text was read and is not arranged the way this software expects. The wording after the
file name describes the mismatch.

`src/hydro_bill/bill_pdf.rs`: `BillError::Display`

### no consumption in a band, so no rate

**Tell the maintainer**

> the bill for the billing period ending `<date>` reports no `<on-peak | mid-peak | off-peak>`
> consumption, so it states no `<band>` rate to price the EV share at

**Where** Cost recovery.

The EV share of energy is priced at the bill's own rate for each time-of-use band, and a bill states
a band's rate only where it reports consumption in it. A period with none in a band leaves nothing to
price against.

`src/api/pure/energy.rs`: `EnergyError::Display`

### Rates workbook — the name does not end in .xlsx

> rates workbook `<file name>`: the name does not end in .xlsx. The rates are read from an Excel
> workbook saved as .xlsx

**Where** Cost recovery, Evolute reimbursement.

Only the `.xlsx` format is read. A workbook in another format — `.xls`, `.ods`, `.csv` — has to be
saved as an Excel workbook (`.xlsx`) first. The format of the workbook is in
[docs/rates/README.md](rates/README.md).

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — the file could not be read

> rates workbook `<file name>`: the file could not be read: `<why not>`

**Where** Cost recovery, Evolute reimbursement.

Everything after *could not be read* is the spreadsheet library's own wording, for instance
`IoError: No such file or directory (os error 2)` for a workbook moved or deleted since it was
chosen. Otherwise the file is damaged, or is not an Excel workbook despite its name. Open it in the
spreadsheet and save it again as `.xlsx`.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — no rates sheet

> rates workbook `<file name>`: there is no sheet named "rates" or "Sheet1". Its sheets are
> `"<sheet name>"`, …

**Where** Cost recovery, Evolute reimbursement.

The rates are read from the sheet named `rates` or, if there is none, the one named `Sheet1`.
Capitals and surrounding spaces do not matter. Rename the sheet that holds the rates to `rates`.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — row 1 does not name a column

> rates workbook `<file name>`, sheet `"<sheet name>"`: row 1 does not name the column(s)
> `<column names>`. Row 1 must name effective_date, on_peak, mid_peak and off_peak, spelled exactly
> so

**Where** Cost recovery, Evolute reimbursement.

The columns are found by the names in row 1, and those names must be exactly `effective_date`,
`on_peak`, `mid_peak` and `off_peak`: all lower case, with underscores and no spaces. A capital or a
trailing space makes a different name. Correct the header cells named in the message.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — a column named twice

> rates workbook `<file name>`, sheet `"<sheet name>"`: row 1 names the column `<column name>` twice,
> in cells `<cell>` and `<cell>`

**Where** Cost recovery, Evolute reimbursement.

With two columns of the same name there is no telling which holds the rates. Rename or delete one
of them.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — an effective date that is not a date

> rates workbook `<file name>`, sheet `"<sheet name>"`: cell `<cell>` holds the text
> `"<the cell's contents>"`. An effective_date must be entered as a date, which the spreadsheet
> displays in a date format

In place of *holds the text …*, the message may say *holds the number `<number>`, not formatted as a
date*, or *holds `<number>`, which no date is stored as*.

**Where** Cost recovery, Evolute reimbursement.

A spreadsheet stores a date as a number and shows it as a date through the cell's date format. Text
that reads like a date is not one, and neither is a number in a cell formatted as a plain number:
either is more likely a slip than a date. Type the date again so the spreadsheet takes it as a date,
or give the cell a date format. A negative number, or one too large, is no date at all.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — an effective date with a time of day

> rates workbook `<file name>`, sheet `"<sheet name>"`: cell `<cell>` holds `<date>` with a time of
> day. An effective_date is a date alone, with no time

**Where** Cost recovery, Evolute reimbursement.

Rates take effect at the start of a day, so a time of day in an effective date is taken as a typing
slip rather than guessed at. Enter the date alone.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — something below the last rates

> rates workbook `<file name>`, sheet `"<sheet name>"`: cell `<cell>` holds `"<the cell's contents>"`,
> below row `<number>`, which has no effective_date. The rates end at the first row without an
> effective_date, so nothing may follow it

**Where** Cost recovery, Evolute reimbursement.

The rates end at the first row with an empty `effective_date`. Something below that row in one of
the four columns means the rates probably continue past a gap — a row whose date was deleted by
mistake — and the rows after it would be ignored without a word. Fill in the missing date, or
delete the stray cell. Notes in other columns are fine anywhere.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — there are no rates

> rates workbook `<file name>`, sheet `"<sheet name>"`: there are no rates: row 2 has no
> effective_date. The rates start on row 2, under the header

**Where** Cost recovery, Evolute reimbursement.

The sheet holds the header and nothing under it, or its first row of rates is not directly under
the header. Enter the rates starting on row 2.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — effective dates out of order

> rates workbook `<file name>`, sheet `"<sheet name>"`: the effective_date on row `<number>`,
> `<date>`, is not after the one on row `<number>`, `<date>`. The effective dates must increase down
> the sheet, with no date repeated

**Where** Cost recovery, Evolute reimbursement.

Each row's rates apply from its effective date until the next row's, so the rows must be in date
order, earliest first, with no date on two rows. Sort the rows by `effective_date`, and remove or
correct a repeated date.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — no rates in effect

> rates workbook `<file name>`, sheet `"<sheet name>"`: no rates are in effect on `<date>`: the
> earliest effective_date is `<date>`

**Where** Cost recovery, Evolute reimbursement.

The first date to be priced — a billing period's first day, or the 1st of the month being
reconciled — comes before every row of the workbook. Add a row for the rates that were in effect on
that date.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — the rates change more than once in a billing period

> rates workbook `<file name>`, sheet `"<sheet name>"`: the rates change `<number>` times within the
> billing period `<date>` to `<date>`, on `<dates>`. A billing period can take one change at most

**Where** Cost recovery.

A billing period is priced at the rates in effect on its first day, and at most one change within
it. Two effective dates inside one period are most likely a mistyped date; correct it.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — the rates change within the month

> rates workbook `<file name>`, sheet `"<sheet name>"`: the rates change on `<date>` (row
> `<number>`), within the month `<date>` to `<date>`. A month is reconciled at one set of rates, so
> they can change only on the 1st

**Where** Evolute reimbursement.

Evolute settles a calendar month at one set of rates, so a month is priced at the rates in effect on
the 1st and cannot take a change after it. Check the date on the row named: rates are expected to
change on the 1st of a month.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### Rates workbook — a rate that is not a positive number

> rates workbook `<file name>`, sheet `"<sheet name>"`: cell `<cell>`, the `<column name>` rate
> effective `<date>`, is empty

In place of *is empty*, the message may say *holds `"<the cell's contents>"`, which is not a
number*, *is `<number>`. A rate must be greater than zero*, or *is not a finite number*.

**Where** Cost recovery, Evolute reimbursement.

A rate is a number of dollars per kilowatt-hour, greater than zero. An empty cell is refused rather
than read as zero, which would price that band's energy at nothing and still produce a report.
Only the rows a run uses are checked, so this names a row the period or month actually needs.
Correct the cell named.

`src/rates/excel/error.rs`: `RatesWorkbookError::Display`

### saving a report or a workbook failed

> `<file name>`: `<why not>`

**Where** Cost recovery, Peak power detail, Evolute reimbursement — when saving a report. Convert to
workbook — when writing the workbook.

Everything after the file name is the operating system's wording, such as `Permission denied (os
error 13)`. The report is still on screen and can be saved again somewhere else; nothing has been
lost. Choose a folder you can write to, or free some space.

`src/bin/ev_cost_recovery/surplus.rs`: `export_row`, `detail.rs`: `export_row`,
`reimbursement.rs`: `export_row`,
`src/error.rs`: `ConversionError::Display`

### Session Report — missing required column

**Tell the maintainer**

> Session Report `<file name>`: missing required column `<column name>`

**Where** Cost recovery, Evolute reimbursement, Convert to workbook.

The file was read and does not carry a column the session reader needs. Usually the file is not a
session report — a Charges Report picked at a session-report slot fails exactly this way. Check the
file; if it is the right one, Evolute's report format has changed.

`src/csv.rs`: `CsvReadError::Display`

### Session Report — row … cannot read

**Tell the maintainer**

> Session Report `<file name>`: row `<number>`, column `<column name>`: cannot read
> `"<the cell's contents>"`: `<why not>`

**Where** Cost recovery, Evolute reimbursement, Convert to workbook.

One cell does not hold the kind of value its column is supposed to. The row number counts the header
row, so it is the number the spreadsheet shows.

`src/csv.rs`: `CsvReadError::Display`

### Session Report — the file could not be read

**Tell the maintainer**

> Session Report `<file name>`: `<why not>`

**Where** Cost recovery, Evolute reimbursement, Convert to workbook.

Everything after the file name is the CSV library's own wording. The file is damaged or is not a CSV.

`src/csv.rs`: `CsvReadError::Display`

### the closing date and the calendar disagree

**Tell the maintainer**

> `<date>` cannot end a billing period that closes on day `<number>` of the month; the closing date
> and the calendar disagree

**Where** No route through the app reaches it.

The date taken from the bill is not the day of the month this building's bills close on. As with
[does not name a billing period](#does-not-name-a-billing-period), a bill on another plan or from
another utility would produce it — but every API call that reads the meter export for a period
checks the same condition first, through `billing_period_dates`, and refuses there. So a user meets
[does not name a billing period](#does-not-name-a-billing-period) instead, and this entry stands
only in case a later caller reaches the reader directly.

`src/green_button/read_xml.rs`: `GbReadError::Display`

### the meter data covers only part of the period

**Get the data**

> the meter data covers `<number>` of the `<number>` intervals in the billing period ending
> `<date>`, so its maxima are not the period's

**Where** Cost recovery.

Peak demand is the highest reading in the period, so a period missing hours may be missing the
highest one. The figure is refused rather than estimated from what is there.

Download an export from Toronto Hydro that covers the whole period. If the hours are missing from
Toronto Hydro's own data, the peak cannot be established from it at all, and that is worth passing
on.

`src/api/pure/peak_power.rs`: `PeakPowerError::Display`

### the session reports do not cover the billing period

**Get the data**

> the session reports do not cover the billing period `<date>` to `<date>`:
> ` `
> &nbsp;&nbsp;`<file name>` covers `<date>` to `<date>`

or, on the Reimbursement tab:

> the session reports do not cover the month `<date>` to `<date>`:

One indented line per report given, so the gap can be seen against what was handed in.

**Where** Cost recovery, Reimbursement.

The two spans are different calendars, and the message names which one it means. A billing period
runs from midnight starting the 24th of one month to midnight starting the 24th of the next, so it
always straddles two of Evolute's monthly reports. The reimbursement reconciliation is over a
calendar month instead, taken from the Charges Report's own file name.

Either way, this says the reports given do not span the dates between them — almost always the wrong
months.

The figures are refused rather than worked out from part of the period: a partial answer reads as a
small EV contribution rather than as a missing file.

`src/api/pure/coverage.rs`: `CoverageError::Display`

### there is no maximum to estimate against

**Get the data**

> the billing period ending `<date>` carries no `<kW | kVA>` reading, so it has no `<unit>` maximum
> to estimate against

**Where** Cost recovery.

Delivery charges are worked out against the period's peak demand, and the export carries no reading
of that kind in the period. A fuller export from Toronto Hydro is what settles it.

`src/api/pure/peak_power.rs`: `PeakPowerError::Display`

### a figure the bill states as zero

**Tell the maintainer**

> the bill for the billing period ending `<date>` states `<the figure>` as zero, so the EV share of
> the charge levied on it cannot be worked out

**Where** Cost recovery.

The EV share of a charge is its share of the figure the charge was levied on. A bill stating that
figure as zero leaves no share to take.

`src/hydro_bill/bill.rs`: `ZeroDenominator::Display`

---

# Changes the figures

The result is there, and something in the data moved it or was left out of it. Nothing here needs
you to do anything; it is here so the figures can be read knowing what is behind them.

These appear in three places: the run log beside the file, the *Sessions needing a look* and
*Sessions left out* sections of the report on screen, and the `Anomalies` column of a converted
workbook. The token is the same in all three.

## Session report anomalies

Why each of these rules exists is in [docs/session/README.md](session/README.md).

### `DuplicateId`

> another session in the report carries the same `Charge_Session_ID`; the id is not unique in
> Evolute's reports, so both sessions still count towards every estimate

Two records share an id. Evolute's ids are not unique — the June 2026 report carries `S37487` on two
sessions a week apart — so both are counted as separate sessions, which is what they are until
something says otherwise. Worth a look because the alternative, one session written twice, would
count its energy twice.

Where two records share an id *and* every compared field, one copy is dropped instead and the run log
says so.

`src/session/common.rs`: `Sessions::note_collapsed`

### `InconsistentDuration`

> reported start, end and duration contradict each other by more than a second, which is the
> rounding the source does, or the end is before the start; the session is excluded from every
> estimate

The record's own three fields do not agree, or its end precedes its start.
`Conn_DateTime_Start + Conn_Duration` should equal `Conn_DateTime_End`, and one second of slack is
allowed for the rounding the source does; further out than that, or inverted at all, and neither the
duration nor the span the session would be placed on can be relied on — so it is left out of every
figure.

An inversion is refused however small it is, tolerance or no tolerance.

`src/session/common.rs`: `AnomalyKind::Display`

### `ZeroActiveChargeTime`

> zero `Active_Charge_Time`, so the session delivered its energy in no time at all and has no finite
> average power; its energy still counts towards every estimate, prorated over its connection span
> like any other session's, and the session is worth reviewing individually

The record reports energy delivered over no time, so no average power can be worked out from it and
the `avg_kw` cell shows `#DIV/0!`.

The estimates are unaffected. What a session contributes to an interval is its energy spread over
its *connection* span, which this record states like any other — average power is not an input to
any figure. The flag is there because the record contradicts itself, which is worth a look, not
because a number had to be invented.

`src/session/common.rs`: `AnomalyKind::Display`

## Green Button export anomalies

What each of these means for the meter data is in
[docs/green_button/README.md](green_button/README.md).

These are counted in the run log and, on the Convert tab, listed as `<token> x<count>`. In a
generated workbook they are highlighted against the readings they concern.

### `DuplicateInterval`

> the same interval start appeared more than once within one series

The export gives two readings for one hour in one series.

`src/green_button/common.rs`: `Anomaly::description`

### `ImplausibleGap`

> the hole before this hour was too large to be an outage, so it was left unfilled rather than
> expanded into placeholder rows

A gap in the readings is normally made visible by writing one empty row per missing hour. A single
corrupt timestamp can put a reading thousands of years out, and filling to it would mean millions of
rows. Past a plausible size the gap is recorded instead of filled.

`src/green_button/common.rs`: `Anomaly::description`

### `MisalignedInterval`

> the interval does not start on a whole hour, so it was left out of peak selection and can never be
> a reported maximum

`src/green_button/common.rs`: `Anomaly::description`

### `MissingInterval`

> no series carried this hour, though the hours around it imply it should exist

`src/green_button/common.rs`: `Anomaly::description`

### `MissingKva`

> the hour carried a kWh or kW reading but no kVA

`src/green_button/common.rs`: `Anomaly::description`

### `MissingKw`

> the hour carried a kWh or kVA reading but no kW

`src/green_button/common.rs`: `Anomaly::description`

### `MissingKwh`

> the hour carried a kW or kVA reading but no kWh

`src/green_button/common.rs`: `Anomaly::description`

---

# Worth knowing

The result stands and no figure in it moved. Nothing here needs you to do anything, except where an
entry says otherwise.

The session anomalies in this group are listed after the messages, under
[Session report anomalies that leave the figures standing](#session-report-anomalies-that-leave-the-figures-standing).

### a breaker billed for part of the month

> `<number>` row(s) are billed for `<date>` to `<date>` rather than the whole month: rows
> `<numbers>`. Their kWh and dollars are counted in the totals in full.

**Where** Evolute reimbursement — in the report's *Charges Report* section, and in the Charges
Report's run log.

A breaker billed for part of the month rather than all of it. Under one reading of Evolute's two
date columns this is an ordinary mid-month join or leave; under another it should not happen. It is
reported because the two readings have not been told apart — see
[docs/archive/Questions_for_Evolute.md](archive/Questions_for_Evolute.md).

Rows billed for dates *outside* the month are a different matter and refuse the file; see
[Charges Report — rows billed for dates outside the month](#charges-report--rows-billed-for-dates-outside-the-month).

`src/charges_report.rs`: `ChargesReport::findings`

### periods that do not hold a full billing period's intervals

> `<number>` period(s) do not hold a full billing period's intervals

with, beneath it:

> Highlighted in the sheet. The export's own coverage decides this: the first and last periods it
> reaches are ordinarily partial.

**Where** Convert to workbook.

Nothing is wrong. A Green Button export starts and stops where it starts and stops, so the first and
last billing periods it touches are normally cut short. The workbook marks them in red on
`nbr_of_intervals` so a reader does not take their maxima for a whole period's.

The case that would matter to a figure is caught separately and stops the run; see
[the meter data covers only part of the period](#the-meter-data-covers-only-part-of-the-period).

`src/bin/ev_cost_recovery/convert.rs`: `gb_outcome`, `src/bin/ev_cost_recovery/convert.rs`: `gb_outcome`

### the run's log was not written

> The figures were worked out, but this run's log was not written.
> `<log file name>`: `<why not>`
> Check that the folder can be written to and that the disk is not full.

**Where** Cost recovery, Evolute reimbursement — in red, above the report.

Every run writes a log beside each file it read. The figures below the message are complete and
correct; what is missing is the record of the run on disk. Nothing else in these two functions
writes anything, so there is nothing else to check.

Shown in red because it is easy to walk away from a report believing a log was kept. Fix the folder
and run again if you want the log.

`src/bin/ev_cost_recovery/state.rs`: `SurplusState::report_note`

### the workbook was written, but its run log was not

> The workbook was written, but its run log was not.
> `<log file name>`: `<why not>`
> Check that the folder can be written to and that the disk is not full.

**Where** Convert to workbook — in red, beneath the workbook's path.

The `.xlsx` is complete. Only its log is missing. Whatever the conversion found is still listed on
screen beneath this message; it just has no copy on disk.

`src/bin/ev_cost_recovery/state.rs`: `Conversion::run`, in both the `SessionConversion` and
`GbConversion` implementations

## Session report anomalies that leave the figures standing

Why each of these rules exists is in [docs/session/README.md](session/README.md).

### `ExcessiveAvgKw`

> average kilowatts above the Evolute breaker rating at the top of the normal voltage band, which
> the hardware should not allow; the session still counts towards every estimate

The breaker limits current, so the power a car draws rises and falls with the supply voltage, and a
draw within the normal voltage band is the installation working as it should. Above that band, either
the reported energy or the reported charge time is wrong — and nothing in the record says which, so
the session is counted as it stands.

`src/session/common.rs`: `AnomalyKind::Display`


---

# The run logs

Every run writes a log beside each file it read or wrote:

- **Named** `<the file's name>.<what was read>.log` — for instance
  `Session_Report_June_1_2026-June_30_2026.session.csv.read.log`.
- **Placed** in the same folder as the file it is about.
- **Overwritten** on every run. A log is not a history: run the same thing twice and the first log
  is gone.

A log always says one of two things, so a run that found nothing is never confused with a run that
was never made:

```
Read Session Report: /data/Session_Report_June_1_2026-June_30_2026.csv

Nothing to report. No errors, warnings or anomalies.
```

```
Read Session Report: /data/Session_Report_June_1_2026-June_30_2026.csv

2 item(s) to review, in the order found:

  row 42 (S37487) DuplicateId: another session in the report carries the same ...
  row 91 (S37502) ExcessiveAvgKw: average kilowatts above the Evolute breaker ...
```

## Which logs each tab writes

| Tab | Logs |
| --- | --- |
| Cost recovery | `<name>.session.csv.read.log` beside each session report; `<name>.meter.xml.read.log` beside the Green Button export |
| Peak power detail | none of its own; it reads what the Cost recovery run produced |
| Evolute reimbursement | `<name>.session.csv.read.log` beside the session report; `<name>.charges.csv.read.log` beside the Charges Report |
| Convert to workbook | `<name>.session.convert.log` or `<name>.meter.convert.log`, beside the workbook |

The meter log covers the billing period that was priced, not the whole export.
