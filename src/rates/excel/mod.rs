//! The rates workbook: our rates, one row per change, in an Excel workbook.
//!
//! Two halves. `read` turns the file into rows and settles everything about the file itself, on
//! every read: the sheet, the header, and the effective dates. `select` picks the rows a billing
//! period or a month is priced at, and checks their rate cells, and only theirs: a bad cell on a
//! row no period reaches is not a reason to refuse a period that never touches it. `error` is what
//! both halves raise.
//!
//! The format is described in `docs/rates/README.md`.

mod error;
mod read;
mod select;

pub(crate) use error::RatesWorkbookError;
pub(crate) use select::{read_rates_for_billing_period, read_rates_for_month};
