pub mod api;

pub mod charges_report;
// `ConversionError` is returned by two public functions -- `session::session_csv_to_xlsx` and
// `green_button::write_gb_workbook` -- so it has to be nameable from outside, and this is the only
// place it is named. `api` does not re-export it: a caller matching past `ApiError::Conversion`
// reaches it here, by the same path the two writers do.
pub mod error;
pub mod green_button;
pub mod hydro_bill;
pub mod session;
pub mod time;

mod markdown;

#[cfg(test)]
mod golden;
