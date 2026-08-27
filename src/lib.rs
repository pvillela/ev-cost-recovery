// `api` is private and its two useful children are re-exported by name, so a caller writes
// `ev_cost_recovery::io::peak_power` rather than `...::api::io::peak_power`. Named rather than
// `pub use api::*`, because the glob would also publish `api::error` at this level, where it would
// collide with the `error` module below. Nothing outside the crate names `api::error`: `ApiError`
// and `ReadError` reach a caller through `io`, which re-exports both.
mod api;
pub use api::{io, pure};

pub mod charges_report;
// `ConversionError` is returned by two public functions -- `session::session_csv_to_xlsx` and
// `green_button::write_gb_workbook` -- so it has to be nameable from outside, and this is where it
// is named. `io` re-exports it as well, being the payload of `ApiError::Conversion`.
pub mod error;
pub mod green_button;
pub mod hydro_bill;
pub mod session;
pub mod time;

mod markdown;

#[cfg(test)]
mod golden;
