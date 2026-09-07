// A type reachable through a public field, return or variant payload but with no public path is one
// a caller can read and cannot name. Nothing else reports it: the crate compiles clean and `cargo
// doc` raises only a warning. At the crate root these reach every module -- on a `mod.rs` they
// would cover only the types defined beneath it, which is how five of them were missed.
#![deny(private_interfaces, private_bounds, unnameable_types)]

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
