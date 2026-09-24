# Read the cost-recovery rates from a workbook

## Context

The prompt is `Prompt_Reading of EV_Cost_Recovery_Rates.xlsx.md`, beside this file. The design
below was settled in a grilling session on 2026-09-24; every rule here is one the user chose.

Today the rates are typed in. The Cost recovery tab takes one schedule (an effective date and
three rates) and, behind a "rates changed" checkbox, a second. The Evolute reimbursement tab takes
exactly one. The two CLIs take one or two `DATE:ON,MID,OFF` arguments. All of that goes, and a
rates workbook replaces it everywhere.

## The workbook

- **Any file name.** `.xlsx` only: `umya-spreadsheet`, already a dependency, reads nothing else.
  The dialog filters on `xlsx` and `XLSX`, as the other pickers list both cases.
- **The sheet** is the one named `rates`; failing that, `sheet1`. The sheet name is compared
  ignoring case and surrounding spaces, so Excel's default `Sheet1` is found.
- **Row 1 is the header.** It must hold `effective_date`, `on_peak`, `mid_peak` and `off_peak`,
  spelled exactly, in any order. A missing one is an error naming every column missing. Other
  columns, and other sheets, are ignored.
- **Rates are in $/kWh**, the unit the tabs use today.

### Dates — checked on every read, whatever the run uses

- The data runs from row 2 down to the first row whose `effective_date` cell is empty.
- Any content below that row, in any of the four columns, is an error. Trailing rows that carry
  only formatting are not content.
- Every `effective_date` down to that point must be an Excel date: a number with a date number
  format. A formula whose cached result is such a number counts. Text is refused.
- The time part must be midnight. An effective *date* with a time is taken as a typo.
- The dates must be **strictly increasing** down the sheet. Equal or descending dates are an error
  naming both rows.
- No data rows at all is an error.

### Rates — checked only on the rows a run uses

A rate cell must be a number greater than zero. Empty, text, zero, negative and non-finite cells
are each refused, naming the cell (sheet, column name, cell address). A row that is used must have
all three rates; nothing is carried forward from the row above. A bad cell on a row no run uses is
never looked at.

## Which rows a run uses

The row in effect on a date is the last row whose `effective_date` is on or before it. A date
before the first row is an error naming the first effective date.

- **Cost recovery** (a billing period): the row in effect on the period's first day, plus the row
  dated inside the period if there is one. **Two or more rows dated inside one period is an
  error** naming the period and the dates; the calculation keeps its two-stretch shape.
- **Evolute reimbursement** (a calendar month): the row in effect on the 1st. **A row dated inside
  the month, after the 1st, is an error** naming the month and the date.

These rules replace today's `CostRecoveryError::RatesNotYetInEffect`,
`CostRecoveryError::RateChangeOutsidePeriod` and `ReimbursementError::RatesNotYetInEffect`, which
existed to police hand-entered schedules the selection now cannot produce.

## Code

### The reader — `src/rates_workbook.rs`

A new crate-level module, beside `charges_report`. It opens the workbook, finds the sheet and the
header, and applies every date rule above. It returns a `RateSchedule` (below) whose rate cells are
not yet checked.

Its error, `RatesWorkbookError`, follows `BillError`: every fact a message needs is a field of the
variant (path, sheet, cell address, the value found) and is formatted at `Display`. It is
`pub(crate)`, because the reading function is: only `api::io` calls it, and it reaches `ApiError`
boxed inside a new `ReadError::RatesWorkbook { path, cause }`, which defers to the cause as the
other readers' variants do.

### The schedule and the selection — `src/api/pure/rates.rs`

A new `pure` submodule, because selecting rows is judgement and `pure` is where judgement is
tested without files.

- `RateSchedule` holds the workbook it came from (`Option<PathBuf>`, `None` when built in memory),
  the sheet name, and the rows. Its constructor enforces strictly increasing dates, so no
  `RateSchedule` can exist with rows out of order, however it was built.
- Each row holds its sheet row number, its effective date, and one cell per band: the cell address
  and the value as found (a number, empty, or text).
- `RateBand` moves here from `recovery.rs`. It names the column a bad cell is in.
- Two selection methods: one for a billing period, returning the opening `CostRecoveryRates` and
  the optional change; one for a calendar month, returning one `CostRecoveryRates`. Each checks the
  rate cells of the rows it returns, and only those.
- `RateScheduleError` carries the workbook and the sheet and writes them into its message, so it
  names its own file. No wrapper above it adds the path again.

### The calculations

`pure::cost_recovery`, `pure::cost_recovery_surplus` and `pure::reconcile_evolute_reimbursement`
take a `&RateSchedule` in place of rate values, select from it, and raise the selection's errors as
a variant of their own error type. The three `api::io` wrappers take the workbook's path, read it,
and hand the schedule on. Only these signatures change in `src/api/io.rs`; the file stays one file.

`CostRecovery` and `ReimbursementReconciliation` keep the workbook's path, and their reports print
it beside the rates and effective dates they already print. A schedule built in memory has no path,
and the line is left out.

### Deleted

`FromStr for CostRecoveryRates`, `CostRecoveryRatesError`, `RATE_SCHEDULE_FORM`, the app's
`RatesForm`, `checked_figure`'s rate use and `widgets::schedule`, with their tests.

### The CLIs

`cost_recovery_cli <YYYY-MM-DD> <RATES.xlsx> <SESSIONS.csv>...` and
`cost_recovery_surplus_cli <BILL.pdf> <GREEN_BUTTON.XML> <RATES.xlsx> <SESSIONS.csv>...`. The
workbook takes the position the rate arguments had. With one fixed argument before the reports,
the argument-shape logic that found where the reports begin is no longer needed.

### The app

- Each tab has a **Rates workbook** picker. The pick is one shared value in `AppState`, like
  `WorkingDir`: picking on either tab sets it for both, and the latest pick wins.
- A new pick clears the results on both tabs, since results describe the inputs that produced them.
- The file is read when a run starts, and at no other time.
- The date picker, the three rate fields and the "rates changed" checkbox are removed.

## Tests

Test workbooks are built in code with `umya-spreadsheet` and written to a temporary folder, so each
test shows the exact cells it tests and no binary fixture is committed. The example workbook in
`data/` is gitignored and is not used.

- The reader: sheet lookup and fallback, header rules, every date rule, content below the end,
  formula dates.
- The selection, with schedules built in memory: in effect on the first day, one change inside a
  period, two changes, a change inside a month, a date before the first row, and each kind of bad
  rate cell, used and unused.
- The golden surplus report is checked against its file, and regenerated only after reading the
  diff.

## Docs

- `README.md` gets a **Rates workbook** section: the sheet, the columns, the date and rate rules.
  The input tables and the CLI list point to it.
- `docs/app-cheat-sheet.md`: the steps pick a workbook instead of typing rates, and describe the
  shared pick. The rate experiments become workbooks to build.
- `docs/ERRORS.md`: every new message gets a full entry (quote, where, what it means, what to
  do). The entries for the removed errors go.
- `docs/maintenance-manual.md`: the list of deliberately absent messages loses the rate-field
  validation, and the note on `ReimbursementError` is re-derived.

## Known limit

`umya-spreadsheet` does not expose a workbook's 1904 date system flag. A workbook saved in that
system (old Excel for Mac) would read every date four years and a day early. Nothing current
writes it by default; the README says so.

## Check before finishing

```sh
cargo check --all-targets
cargo test
cargo test --test docs_errors -- --ignored
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" \
  cargo doc --no-deps --all-features --document-private-items
```

And `grep` the prose — `docs/`, `README.md`, `.github/` — for `DATE:ON`, `rates changed`,
`Effective from`, `RatesForm`, `CostRecoveryRatesError` and the removed variants.
