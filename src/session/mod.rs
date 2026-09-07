// Three tiers, and the `pub` on each `use` is what separates them:
//
//   `use x::*;`          module-local -- reachable from here and from files under `session/`
//   `pub(crate) use`     also reachable as `crate::session::X` from elsewhere in the crate
//   `pub use`            also reachable as `ev_cost_recovery::session::X` from outside
//
// What belongs in the public tier is settled by what the binaries and integration tests actually
// name -- plus whatever `api` re-exports, since that publishes a type by a second route.

mod common;
use common::*;

// Crate-private. Its two readers -- `csv_sessions` (from `api::io` and the `#[cfg(test)]`
// modules below) and `csv_session_rows` (from `excel`) -- are why those tests live in `src/`
// rather than `tests/`. Nothing outside the crate calls either: the API takes paths and hands
// back figures, never a `Sessions`. Keeping the module private keeps `SessionCsvError` -- the
// type both return -- off the public surface too.
mod csv;

mod energy;
mod excel;
mod file_name;
mod peak;
mod report;

// Only used by sub-modules. Nothing re-exported. It is the electrical engineering site model.
mod site_model;

// --- Named outside the crate -------------------------------------------------------------------

pub use common::{Segment, Session, Sessions};
pub use excel::{SessionWriteReport, session_csv_to_xlsx};
pub use file_name::{SessionReportNameError, parse_session_report_name, report_coverage};
pub use report::site_load_report;

// --- Reachable outside the crate -------------------------------------------------------------------

pub use common::{AnomalyKind, BREAKER_RATING_KW};
pub use energy::TouKwh;
pub use file_name::SessionReportCoverage;
pub use peak::EstimateSet;

// --- Named elsewhere inside the crate ----------------------------------------------------------

pub(crate) use common::{BREAKER_MAX_NORMAL_KW, RSession, SessionNotes};
pub(crate) use csv::csv_sessions;
pub(crate) use energy::tou_kwh;
pub(crate) use file_name::reports_cover;
pub(crate) use peak::IntervalEstimates;
pub(crate) use peak::estimates_from_sessions;

// --- Tests -------------------------------------------------------------------------------------

// `pub(crate)`: `api::pure`'s tests build sessions with it.
#[cfg(test)]
pub(crate) mod test_support;

// Test modules of their own, rather than `#[cfg(test)]` blocks inside a source file: each cuts
// across `csv`, `common` and `peak`, so there is no one file it belongs beside. All three read a
// CSV fixture from `tests/fixtures/` through `golden::fixture` and write nothing.
#[cfg(test)]
mod consistency_band_tests;
#[cfg(test)]
mod report_rendering_tests;
#[cfg(test)]
mod segment_tiling_tests;
