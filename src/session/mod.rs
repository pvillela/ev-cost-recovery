// Three tiers, and the `pub` on each `use` is what separates them:
//
//   `use x::*;`          module-local -- reachable from here and from files under `session/`
//   `pub(crate) use`     also reachable as `crate::session::X` from elsewhere in the crate
//   `pub use`            also reachable as `ev_cost_recovery::session::X` from outside
//
// What belongs in the public tier is settled by `docs/public-surface-usage.md`, which records what
// the binaries, examples and integration tests actually name -- plus whatever `api` re-exports,
// since that publishes a type by a second route.
//
// Most of the public tier here serves `ev_peak_cli` and `ev_peak_gui` rather than the API. See the
// same document: of the paths named from outside this module, thirteen have no consumer but those
// two, and only the two `historic` ones are gated. Narrowing that is a separate decision.

mod common;
use common::*;

mod energy;

// Crate-private. Its one entry point, `csv_sessions`, is called from `api::io` and from `excel`,
// and by nothing outside the crate: the API takes paths and hands back figures, never a `Sessions`.
// Keeping the module private keeps `SessionCsvError` -- the type that call returns -- off the
// public surface too, which is where it belongs while nothing outside matches on it.
mod csv;

mod file_name;

mod excel;

mod ioi;

mod log;

mod peak;

mod report;

// A module rather than a set of re-exports: it is the site model, a page of named constants and
// the functions over them, and `site_load::XFMR_RATING_KVA` says where a figure comes from in a
// way a bare re-export would not. Nothing outside the crate names the path today.
pub mod site_load;

// --- Named outside the crate -------------------------------------------------------------------

pub use common::{Bracket, Segment, Session, Sessions};
pub use excel::{SessionWriteReport, session_csv_to_xlsx};
pub use file_name::report_coverage;
pub use ioi::{HourEntry, IoiLength, LEGAL_START_MINUTES, checked_interval, hours_of};
pub use log::{RunLog, SourceLog};
pub use peak::IntervalEstimates;
pub use report::site_load_report;

// The workbook round-trip. Behind the feature because only `ev_peak_cli` and `ev_peak_gui` use it;
// see `docs/historic-feature.md`.
#[cfg(feature = "historic")]
pub use excel::historic::{xlsx_to_interval_estimates, xlsx_to_sessions};

// Not named directly by anything outside the crate, but `api` re-exports them, which makes them
// public by that route: `Sessions` is a parameter of every `pure` operation, `SessionNotes` and
// `TouKwh` are fields of what they return, and `SessionReportCoverage` is `pure::coverage`'s.
pub use common::{AnomalyKind, BREAKER_RATING_KW, RSession, SessionNotes};
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
