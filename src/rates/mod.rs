//! Our EV cost-recovery rates: what they are, and reading them from the rates workbook.
//!
//! The rates are ours rather than Toronto Hydro's, set by decision rather than calculated, and in
//! dollars per kilowatt-hour. [`CostRecoveryRates`] is what the calculations in
//! [`api::pure`](crate::api::pure) take. The workbook they are kept in, and which of its rows price
//! a billing period or a month, is described in `docs/rates/README.md`; the functions reading it
//! are for `api::io`, which hands the rates they return to `api::pure` as values.

mod excel;
mod pure;

pub use pure::CostRecoveryRates;

pub(crate) use excel::{RatesWorkbookError, read_rates_for_billing_period, read_rates_for_month};
