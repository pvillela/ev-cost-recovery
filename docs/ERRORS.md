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
- [cost-recovery rates take effect after the period starts](#cost-recovery-rates-take-effect-after-the-period-starts)
- [does not name a billing period](#does-not-name-a-billing-period)
- [Green Button Export — no readings in the billing period](#green-button-export--no-readings-in-the-billing-period)
- [Green Button Export — the file could not be read](#green-button-export--the-file-could-not-be-read)
- [Hydro Bill — the bill has no such figure](#hydro-bill--the-bill-has-no-such-figure)
- [Hydro Bill — a value could not be read as expected](#hydro-bill--a-value-could-not-be-read-as-expected)
- [Hydro Bill — the page could not be read](#hydro-bill--the-page-could-not-be-read)
- [Hydro Bill — unrecognised charge line](#hydro-bill--unrecognised-charge-line)
- [Hydro Bill — the layout is not what was expected](#hydro-bill--the-layout-is-not-what-was-expected)
- [no consumption in a band, so no rate](#no-consumption-in-a-band-so-no-rate)
- [saving a report or a workbook failed](#saving-a-report-or-a-workbook-failed)
- [Session Report — missing required column](#session-report--missing-required-column)
- [Session Report — row … cannot read](#session-report--row--cannot-read)
- [Session Report — the file could not be read](#session-report--the-file-could-not-be-read)
- [the closing date and the calendar disagree](#the-closing-date-and-the-calendar-disagree)
- [the meter data covers only part of the period](#the-meter-data-covers-only-part-of-the-period)
- [the second set of rates falls outside the period](#the-second-set-of-rates-falls-outside-the-period)
- [the session reports do not cover the billing period](#the-session-reports-do-not-cover-the-billing-period)
- [there is no maximum to estimate against](#there-is-no-maximum-to-estimate-against)
- [a figure the bill states as zero](#a-figure-the-bill-states-as-zero)

**Changes the figures**

- Session report: [`DstUnresolvable`](#dstunresolvable), [`DuplicateId`](#duplicateid),
  [`FellInDstGap`](#fellindstgap), [`InconsistentDuration`](#inconsistentduration),
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
- Session report: [`DstAmbiguousDuplicated`](#dstambiguousduplicated),
  [`ExcessiveAvgKw`](#excessiveavgkw), [`OffGridTimes`](#offgridtimes),
  [`WorkbookDiscrepancy`](#workbookdiscrepancy)

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

`src/csv.rs:139`

### Charges Report — row … cannot read

**Tell the maintainer**

> Charges Report `<file name>`: row `<number>`, column `<column name>`: cannot read
> `"<the cell's contents>"`: `<why not>`

**Where** Evolute reimbursement.

One cell does not hold the kind of value its column is supposed to. The row number counts the header
row, so it is the number the spreadsheet shows. The reason at the end is the number or date reader's
own wording.

`src/csv.rs:140-149`

### Charges Report — the file holds no rows

**Tell the maintainer**

> Charges Report `<file name>`: the file holds no rows; a Charges Report carries one row per
> breaker even in a month nothing was billed for

**Where** Evolute reimbursement.

An empty report is not the same as a month with nothing in it — Evolute writes a row per breaker
either way. So this says the file is truncated or is not a Charges Report.

`src/charges_report.rs:305-311`

### Charges Report — the file could not be read

**Tell the maintainer**

> Charges Report `<file name>`: `<why not>`

**Where** Evolute reimbursement.

Everything after the file name is the CSV library's own wording, not this software's — for
instance, `CSV error: record 4 (line: 5, byte: 210): found record with 9 fields, but the previous
record has 11 fields`. The file is damaged or is not a CSV.

`src/csv.rs:138`

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

`src/charges_report.rs:319-336`

### cost-recovery rates take effect after the period starts

> the cost-recovery rates given for the start of the period take effect `<date>`, after it starts
> on `<date>`

**Where** Cost recovery.

The billing period begins before the rates you entered came into force, so part of it would be
priced at rates that did not yet exist. Either the *effective from* date on the form is wrong, or
the schedule in force at the start of the period is a different one.

The Evolute reimbursement tab has its own version of this, worded for a calendar month:

> the rates take effect on `<date>`, after the month begins on `<date>`, so they do not price the
> whole of it

`src/api/pure/recovery.rs:282-286`, `src/api/pure/reimbursement.rs:126-130`

### does not name a billing period

**Tell the maintainer**

> `<date>` does not name a billing period: one is labelled by day 23 of the month it ends in

**Where** Cost recovery.

A billing period is named by the day it closes, which for this building is the 23rd. The date came
from the bill, so a bill closing on another day — a different rate plan, or another utility's bill —
produces this.

`src/hydro_bill/billing_period.rs:181-185`

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

`src/green_button/read_xml.rs:82-92`

### Green Button Export — the file could not be read

**Tell the maintainer**

> Green Button Export `<file name>`: `<why not>`

**Where** Cost recovery, Convert to workbook.

Two different failures share this shape, and everything after the file name comes from elsewhere:

- The file could not be opened — the operating system's wording, such as
  `No such file or directory (os error 2)`. Check the file is still where you picked it from.
- The file is not an ESPI feed this can parse — the XML reader's wording. The file is damaged, or is
  not a Green Button export.

`src/green_button/read_xml.rs:77-78`

### Hydro Bill — the bill has no such figure

**Tell the maintainer**

> Hydro Bill `<file name>`: the bill has no `<the thing that is missing>`

**Where** Cost recovery.

The PDF was read and a figure the calculation needs is not on it. The bill is for a different rate
plan, or Toronto Hydro has changed its layout.

`src/hydro_bill/bill_pdf.rs:195-197`

### Hydro Bill — a value could not be read as expected

**Tell the maintainer**

> Hydro Bill `<file name>`: not `<what was expected>`: `"<the text found instead>"`

**Where** Cost recovery.

A value was where it should be and is not the kind of value expected — a date that is not a date, a
figure that is not a number. The text found is quoted so it can be compared against the bill.

`src/hydro_bill/bill_pdf.rs:201-203`

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

`src/hydro_bill/pdf_text.rs:147-172`

### Hydro Bill — unrecognised charge line

**Tell the maintainer**

> Hydro Bill `<file name>`: unrecognised charge line: `<the line as it appears on the bill>`

**Where** Cost recovery.

A line in the bill's charges is one this software has no rule for. It is refused rather than ignored,
because a charge silently skipped is money missing from a figure that still looks complete. A new
charge on the bill needs a decision about whether EV charging bears any of it.

`src/hydro_bill/bill_pdf.rs:198-200`

### Hydro Bill — the layout is not what was expected

**Tell the maintainer**

> Hydro Bill `<file name>`: `<what is wrong with the layout>`

**Where** Cost recovery.

The bill's text was read and is not arranged the way this software expects. The wording after the
file name describes the mismatch.

`src/hydro_bill/bill_pdf.rs:200`

### no consumption in a band, so no rate

**Tell the maintainer**

> the bill for the billing period ending `<date>` reports no `<on-peak | mid-peak | off-peak>`
> consumption, so it states no `<band>` rate to price the EV share at

**Where** Cost recovery.

The EV share of energy is priced at the bill's own rate for each time-of-use band, and a bill states
a band's rate only where it reports consumption in it. A period with none in a band leaves nothing to
price against.

`src/api/pure/energy.rs:138-145`

### saving a report or a workbook failed

> `<file name>`: `<why not>`

**Where** Cost recovery, Peak power detail, Evolute reimbursement — when saving a report. Convert to
workbook — when writing the workbook.

Everything after the file name is the operating system's wording, such as `Permission denied (os
error 13)`. The report is still on screen and can be saved again somewhere else; nothing has been
lost. Choose a folder you can write to, or free some space.

`src/bin/ev_cost_recovery/surplus.rs:199`, `detail.rs:107`, `reimbursement.rs:272`,
`src/error.rs:80`

### Session Report — missing required column

**Tell the maintainer**

> Session Report `<file name>`: missing required column `<column name>`

**Where** Cost recovery, Evolute reimbursement, Convert to workbook.

The file was read and does not carry a column the session reader needs. Usually the file is not a
session report — a Charges Report picked at a session-report slot fails exactly this way. Check the
file; if it is the right one, Evolute's report format has changed.

`src/csv.rs:139`

### Session Report — row … cannot read

**Tell the maintainer**

> Session Report `<file name>`: row `<number>`, column `<column name>`: cannot read
> `"<the cell's contents>"`: `<why not>`

**Where** Cost recovery, Evolute reimbursement, Convert to workbook.

One cell does not hold the kind of value its column is supposed to. The row number counts the header
row, so it is the number the spreadsheet shows.

`src/csv.rs:140-149`

### Session Report — the file could not be read

**Tell the maintainer**

> Session Report `<file name>`: `<why not>`

**Where** Cost recovery, Evolute reimbursement, Convert to workbook.

Everything after the file name is the CSV library's own wording. The file is damaged or is not a CSV.

`src/csv.rs:138`

### the closing date and the calendar disagree

**Tell the maintainer**

> `<date>` cannot end a billing period that closes on day `<number>` of the month; the closing date
> and the calendar disagree

**Where** Cost recovery.

The date taken from the bill is not the day of the month this building's bills close on. As with
[does not name a billing period](#does-not-name-a-billing-period), a bill on another plan or from
another utility produces it.

`src/green_button/read_xml.rs:93-101`

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

`src/api/pure/peak_power.rs:226-230`

### the second set of rates falls outside the period

> the second set of cost-recovery rates takes effect `<date>`, which is not within the billing
> period `<date>` to `<date>`

**Where** Cost recovery.

*Rates changed during the period* is ticked, and the date given for the change is not inside the
billing period. Either the date is wrong, or the period was priced at one schedule throughout and
the tick should come off.

`src/api/pure/recovery.rs:291-295`

### the session reports do not cover the billing period

**Get the data**

> the session reports do not cover the billing period `<date>` to `<date>`:
> ` `
> &nbsp;&nbsp;`<file name>` covers `<date>` to `<date>`

One indented line per report given, so the gap can be seen against what was handed in.

**Where** Cost recovery.

A billing period runs from the 24th of one month to the 23rd of the next, so it always straddles two
of Evolute's monthly reports. This says the two reports given do not span it between them — almost
always the wrong months.

The figures are refused rather than worked out from part of the period: a partial answer reads as a
small EV contribution rather than as a missing file.

`src/api/pure/coverage.rs:72-80`

### there is no maximum to estimate against

**Get the data**

> the billing period ending `<date>` carries no `<kW | kVA>` reading, so it has no `<unit>` maximum
> to estimate against

**Where** Cost recovery.

Delivery charges are worked out against the period's peak demand, and the export carries no reading
of that kind in the period. A fuller export from Toronto Hydro is what settles it.

`src/api/pure/peak_power.rs:209-212`

### a figure the bill states as zero

**Tell the maintainer**

> the bill for the billing period ending `<date>` states `<the figure>` as zero, so the EV share of
> the charge levied on it cannot be worked out

**Where** Cost recovery.

The EV share of a charge is its share of the figure the charge was levied on. A bill stating that
figure as zero leaves no share to take.

`src/hydro_bill/bill.rs:138-142`

---

# Changes the figures

The result is there, and something in the data moved it or was left out of it. Nothing here needs
you to do anything; it is here so the figures can be read knowing what is behind them.

These appear in three places: the run log beside the file, the *Sessions needing a look* and
*Sessions left out* sections of the report on screen, and the `Anomalies` column of a converted
workbook. The token is the same in all three.

## Session report anomalies

Why each of these rules exists is in [docs/session/README.md](session/README.md).

### `DstUnresolvable`

> DST fold: a reported time falls in the repeated hour and no reading of the record makes its start,
> end and duration agree, so no instant was assigned and the session is excluded from every estimate

When the clocks go back, an hour is lived through twice, and a reported time inside it names two
moments. Which one is normally settled by checking the reported duration. Here neither reading makes
the record's own fields agree, so there is nothing left to choose with, and the session is left out
of every figure rather than placed by a guess.

`src/session/common.rs:997-1001`

### `DuplicateId`

> another session in the report carries the same `Charge_Session_ID`; the id is not unique in
> Evolute's reports, so both sessions still count towards every estimate

Two records share an id. Evolute's ids are not unique — the June 2026 report carries `S37487` on two
sessions a week apart — so both are counted as separate sessions, which is what they are until
something says otherwise. Worth a look because the alternative, one session written twice, would
count its energy twice.

Where two records share an id *and* every compared field, one copy is dropped instead and the run log
says so.

`src/session/common.rs:1006-1009`

### `FellInDstGap`

> reported start or end is a local time that never occurred, in the hour the clocks jump over when
> DST begins; it names no instant, so none was assigned and the session is excluded from every
> estimate

When the clocks go forward, an hour never happens. A reported time inside it names nothing, so the
session cannot be placed on a timeline at all and is left out of every figure. Shifting it to either
side of the gap would be a guess presented as a reading.

`src/session/common.rs:992-996`

### `InconsistentDuration`

> reported start, end and duration contradict each other by more than truncation to the minute can
> explain; the session is excluded from every estimate

The record's own three fields do not agree. Reported times are truncated to the minute, which
accounts for a small disagreement; this is larger than that. Neither the duration nor the span the
session would be placed on can be relied on, so it is left out of every figure.

`src/session/common.rs:987-990`

### `ZeroActiveChargeTime`

> zero `Active_Charge_Time`, so the session delivered its energy in no time at all and has no finite
> average power; the estimating logic substitutes one, and the session is worth reviewing
> individually

The record reports energy delivered over no time. Average power cannot be worked out from that, so
the estimate uses a substituted figure — which is why the session is worth looking at rather than
taken on trust.

`src/session/common.rs:982-986`

## Green Button export anomalies

What each of these means for the meter data is in
[docs/green_button/README.md](green_button/README.md).

These are counted in the run log and, on the Convert tab, listed as `<token> x<count>`. In a
generated workbook they are highlighted against the readings they concern.

### `DuplicateInterval`

> the same interval start appeared more than once within one series

The export gives two readings for one hour in one series.

`src/green_button/common.rs:113-115`

### `ImplausibleGap`

> the hole before this hour was too large to be an outage, so it was left unfilled rather than
> expanded into placeholder rows

A gap in the readings is normally made visible by writing one empty row per missing hour. A single
corrupt timestamp can put a reading thousands of years out, and filling to it would mean millions of
rows. Past a plausible size the gap is recorded instead of filled.

`src/green_button/common.rs:120-123`

### `MisalignedInterval`

> the interval does not start on a whole hour, so it was left out of peak selection and can never be
> a reported maximum

`src/green_button/common.rs:116-119`

### `MissingInterval`

> no series carried this hour, though the hours around it imply it should exist

`src/green_button/common.rs:110-112`

### `MissingKva`

> the hour carried a kWh or kW reading but no kVA

`src/green_button/common.rs:109`

### `MissingKw`

> the hour carried a kWh or kVA reading but no kW

`src/green_button/common.rs:108`

### `MissingKwh`

> the hour carried a kW or kVA reading but no kWh

`src/green_button/common.rs:107`

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
[docs/Questions_for_Evolute.md](Questions_for_Evolute.md).

Rows billed for dates *outside* the month are a different matter and refuse the file; see
[Charges Report — rows billed for dates outside the month](#charges-report--rows-billed-for-dates-outside-the-month).

`src/charges_report.rs:152-168`

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

`src/bin/ev_cost_recovery/convert.rs:236-247`, `src/green_button/excel.rs:305`

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

`src/bin/ev_cost_recovery/state.rs:418-423`

### the workbook was written, but its run log was not

> The workbook was written, but its run log was not.
> `<log file name>`: `<why not>`
> Check that the folder can be written to and that the disk is not full.

**Where** Convert to workbook — in red, beneath the workbook's path.

The `.xlsx` is complete. Only its log is missing. Whatever the conversion found is still listed on
screen beneath this message; it just has no copy on disk.

`src/bin/ev_cost_recovery/state.rs:671-676, 713-718`

## Session report anomalies that leave the figures standing

Why each of these rules exists is in [docs/session/README.md](session/README.md).

### `DstAmbiguousDuplicated`

> ambiguous DST fold; record duplicated as EDT and EST

The session's reported start fell in the hour that is lived through twice when the clocks go back,
and both readings of it reproduce the reported end. Neither can be ruled out, so the record is
counted under both — which is why a converted workbook shows two rows for the one row of the CSV,
told apart by an `-EDT` or `-EST` suffix on the session id.

`src/session/common.rs:991`

### `ExcessiveAvgKw`

> average kilowatts above the Evolute breaker rating at the top of the normal voltage band, which
> the hardware should not allow; the session still counts towards every estimate

The breaker limits current, so the power a car draws rises and falls with the supply voltage, and a
draw within the normal voltage band is the installation working as it should. Above that band, either
the reported energy or the reported charge time is wrong — and nothing in the record says which, so
the session is counted as it stands.

`src/session/common.rs:1002-1005`

### `OffGridTimes`

> `<number>` of `<number>` rows `OffGridTimes`: a reported start or end is not a whole multiple of
> 60s (`<up to three example rows>`). The session report's resolution has become finer than this
> software's time grid. Nothing is wrong with these rows, but the padding and the consistency window
> are now wider than the data needs — see docs/maintenance-manual.md, "Boundaries and the time
> grid".

**Where** the session report's run log only. It is never shown on screen.

Summarised once per file rather than listed per row: a report that has changed resolution has
changed it throughout, so every row would qualify and the list would bury everything else.

Nothing is wrong with the data. It says that an allowance this software makes has become wider than
it needs to be, which is a thing for the maintainer to know about and not something to act on now.

`src/session/csv.rs:278-288`

### `WorkbookDiscrepancy`

> a stored column in the workbook disagrees with what this software recomputes from the row, so the
> sheet is stale or was edited; the recomputed value is the one used

Raised when a workbook is read back rather than when one is written. The recomputed value always
wins, so no figure changes — this only says the stored one no longer matches.

`src/session/common.rs:1017-1020`

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
| Cost recovery | `<name>.session.csv.read.log` beside each of the two session reports; `<name>.meter.xml.read.log` beside the Green Button export |
| Peak power detail | none of its own; it reads what the Cost recovery run produced |
| Evolute reimbursement | `<name>.session.csv.read.log` beside the session report; `<name>.charges.csv.read.log` beside the Charges Report |
| Convert to workbook | `<name>.session.convert.log` or `<name>.meter.convert.log`, beside the workbook |

The meter log covers the billing period that was priced, not the whole export.
