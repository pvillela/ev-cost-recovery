# Refactor the rates workbook into a domain module, `rates`

## Context

`3941e56` read the cost-recovery rates from a workbook, and put the pieces in the wrong places.
`api::pure::recovery` and `api::pure::reimbursement` took the workbook's `RateSchedule` and
selected rows from it, so their error types carried a `RateScheduleError` full of sheet names and
cell addresses — boxed, to quiet clippy about its size. `RateBand` duplicated `time::Tou`. The
design below was settled in a grilling session on 2026-09-25.

## The layout

`pub mod rates` at the crate root, arranged as `green_button` and `session` are: private
submodules, `pub use` for what callers name, `pub(crate) use` for what only the crate uses.

| File | Holds |
| --- | --- |
| `src/rates/mod.rs` | The module's documentation and its re-exports. |
| `src/rates/pure.rs` | `CostRecoveryRates`, moved from `api::pure::recovery`. Re-exported publicly by `rates`, and again by `api::pure`, whose functions take it. |
| `src/rates/excel/mod.rs` | The module's documentation and its re-exports, like every `mod.rs` in the crate. |
| `src/rates/excel/error.rs` | `RatesWorkbookError`, raised by both halves. |
| `src/rates/excel/read.rs` | The file to rows: the sheet, the header, the effective-date rules (dates entered as dates, at midnight, strictly increasing, nothing below the end). |
| `src/rates/excel/select.rs` | The two functions `api::io` calls, and rows to `CostRecoveryRates`: the row in effect on a date, at most one change in a billing period, none within a month, and the rate-cell checks on the rows selected. |

`src/rates_workbook.rs` and `src/api/pure/rates.rs` are deleted. The rate cells stay unchecked
until a row is selected, as settled when the feature was designed.

## The interface `api::io` uses

Two `pub(crate)` functions, the same shape as `green_button::read_gb_for_billing_period`:

- `read_rates_for_billing_period(path, period_start, period_ending)` returns the rates in effect on
  the period's first day and the rates of the row dated within it, if any:
  `(CostRecoveryRates, Option<CostRecoveryRates>)`.
- `read_rates_for_month(path, month_start)` returns one `CostRecoveryRates`.

Both return the one crate-private `RatesWorkbookError`, covering reading and selecting alike, with
every fact a message needs in a field of the variant and formatted at `Display`. `api::io` wraps it
in `ReadError::RatesWorkbook { path, cause }`, which defers to the cause. That is how Green Button
reports an export that does not reach the period asked for, and `ApiError` gains no variant.

## The `pure` functions

They take rate values, as before the feature: `cost_recovery(ending, sessions, at_start, at_end)`,
`cost_recovery_surplus(bill, values, sessions, at_start, at_end)`, and
`reconcile_evolute_reimbursement(…, cost_recovery_rates)`. Their errors carry nothing about a
workbook, and these checks on the values given return:

- `CostRecoveryError::RatesNotYetInEffect` and `CostRecoveryError::RateChangeOutsidePeriod`.
- `ReimbursementError::RatesNotYetInEffect`.

No route through `api::io` reaches them, since the selection refuses such rates first. They go on
the maintenance manual's list of messages deliberately absent from `docs/ERRORS.md`.

The text form `DATE:ON,MID,OFF` does not return: `FromStr for CostRecoveryRates`,
`CostRecoveryRatesError`, `RATE_SCHEDULE_FORM` and `RateBand` stay deleted. The cell errors name
their band with `time::Tou`, and the column names live in `excel/read.rs`. The sentence in
`Tou::as_str`'s documentation saying the crate reads no workbook is corrected.

## The workbook's name in the reports

The Charges Report pattern: `pure` is handed figures and never sees the file, so `api::io` sets
the name on the struct `pure` returns, with `with_rates_workbook(path)` on `CostRecovery`,
`CostRecoverySurplus` and `ReimbursementReconciliation`. The reports print it when it is set.

## Docs

- A new `docs/rates/README.md` holds what the main `README.md`'s "The rates workbook" section held.
  The main `README.md` links to it from the *Cost recovery* and *Evolute reimbursement* input
  tables, from *Additional documentation*, from the `cost_recovery_cli` entry and from the module
  table, whose `rates_workbook` row becomes `rates`.
- `docs/ERRORS.md`, `docs/app-cheat-sheet.md` and both CLIs' usage text point to the new file.
- The sample workbook is in `data/rates/`: the cheat sheet, the ignored real-data test in
  `src/bin/ev_cost_recovery/state.rs` and the CLI usage examples follow it.
- The rates entries in `docs/ERRORS.md` name the files that raise them.

## Working state

The stash on `aicode` holds an abandoned attempt. `recovery.rs` and `reimbursement.rs` are taken
from it — value-taking signatures, the restored checks, the builders — and the stash is dropped.

## Check before finishing

```sh
cargo check --all-targets
cargo clippy --all-targets
cargo fmt --check
cargo test
cargo test --test docs_errors -- --ignored
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" \
  cargo doc --no-deps --all-features --document-private-items
```

Then re-read every changed file as a reviewer, and `grep` the prose for `rates_workbook`,
`RateSchedule`, `RateBand`, `api::pure::rates` and `data/EV_Cost_Recovery_Rates`.
