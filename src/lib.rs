pub mod api;

pub mod charges_report;
// `ConversionError` is returned by two public functions -- `session::session_csv_to_xlsx` and
// `green_button::write_gb_workbook` -- so it has to be nameable from outside, and this module is
// its only public path. `api` does not re-export it: a caller matching past `ApiError::Conversion`
// reaches it here, by the same path the two writers do.
pub mod error;
pub mod green_button;
pub mod hydro_bill;
// Not `session::log`, though the session readers were its only users when it was written. A run
// log is a fact about reading *a* file, not about reading a session report, and `green_button`
// now writes them too -- reaching into `crate::session` for the type would have `green_button`
// depend on a module it shares nothing else with.
pub mod log;
pub mod session;
pub mod time;

mod markdown;

#[cfg(test)]
mod golden;
