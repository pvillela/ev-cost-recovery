pub mod api;

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
