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

mod energy;

// Crate-private. Its two readers -- `csv_sessions` (from `api::io` and the `#[cfg(test)]`
// modules below) and `csv_session_rows` (from `excel`) -- are why those tests live in `src/`
// rather than `tests/`. Nothing outside the crate calls either: the API takes paths and hands
// back figures, never a `Sessions`. Keeping the module private keeps `SessionCsvError` -- the
// type both return -- off the public surface too.
mod csv;

mod file_name;

mod excel;

mod peak;
mod report;

// Only used by sub-modules. Nothing re-exported. It is the electrical engineering site model.
mod site_model;

// --- Named outside the crate -------------------------------------------------------------------

pub use common::{Bracket, Segment, Session, Sessions};
pub use excel::{SessionWriteReport, session_csv_to_xlsx};
pub use file_name::{report_coverage, report_month};
pub use peak::IntervalEstimates;
pub use report::site_load_report;

// --- Reachable outside the crate -------------------------------------------------------------------

// Not named directly by anything outside the crate. `SessionReportCoverage` is public because
// `api::pure` re-exports it; the rest are public because a caller reaches them by reading a field
// of something the API returns -- `SessionNotes` and `TouKwh` off an `Energy`, `AnomalyKind` off a
// `SessionNotes`, `RSession` off a `Sessions`. `api/mod.rs` explains why a field type is not
// re-exported: reading one never requires naming it, but the type still has to be public.
pub use common::{AnomalyKind, BREAKER_MAX_NORMAL_KW, BREAKER_RATING_KW, RSession, SessionNotes};
pub use energy::TouKwh;
pub use file_name::SessionReportCoverage;
pub use peak::EstimateSet;

// --- Named elsewhere inside the crate ----------------------------------------------------------

pub(crate) use csv::csv_sessions;
pub(crate) use energy::tou_kwh;
pub(crate) use file_name::reports_cover;
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
