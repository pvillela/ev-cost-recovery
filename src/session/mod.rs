mod common;
use common::*;

mod energy;
use energy::*;

// Crate-private. Its one entry point, `csv_sessions`, is called from `api::io` and from `excel`,
// and by nothing outside the crate: the API takes paths and hands back figures, never a `Sessions`.
// Keeping the module private keeps `SessionCsvError` -- the type that call returns -- off the
// public surface too, which is where it belongs while nothing outside matches on it.
mod csv;
use csv::*;

mod file_name;
use file_name::*;

mod excel;
#[cfg(feature = "historic")]
pub use excel::historic::{xlsx_to_interval_estimates, xlsx_to_sessions};
pub use excel::{SessionWriteReport, session_csv_to_xlsx};

mod log;
use log::*;
pub use log::{RunLog, SourceLog};

mod ioi;
use ioi::*;

mod peak;
use peak::*;

mod report;
pub use report::site_load_report;
use report::*;

pub mod site_load;

#[cfg(test)]
mod test_support;

// Two test modules of their own, rather than `#[cfg(test)]` blocks inside a source file: each cuts
// across `csv`, `common` and `peak`, so there is no one file it belongs beside. Both read a CSV
// fixture from `tests/fixtures/` through `golden::fixture` and write nothing.
#[cfg(test)]
mod consistency_band_tests;
#[cfg(test)]
mod report_rendering_tests;
#[cfg(test)]
mod segment_tiling_tests;
