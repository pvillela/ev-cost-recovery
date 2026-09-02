pub mod api;

pub mod charges_report;
// Public because `ChargesReportError::Csv` carries a `CsvReadError`, and a payload with no path is
// a payload a caller cannot match past. `Table` inside it stays `pub(crate)`.
pub mod csv;
// `ConversionError` is public API only through this module; `api::ApiError::Conversion` embeds it.
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
