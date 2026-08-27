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
// A fourth tier sits below those: `#[cfg(feature = "historic")] pub use`, for what is reached only
// by targets that themselves carry `required-features = ["historic"]`. Everything left in the
// plain public tier is named by the API, by the desktop app, by `tests/`, or by an ungated binary
// or example.

mod common;
use common::*;

mod energy;

// Crate-private. Its one entry point, `csv_sessions`, is called from `api::io`, from `excel`, and
// from the three `#[cfg(test)]` modules below -- which is why those live in `src/` rather than in
// `tests/`. Nothing outside the crate calls it: the API takes paths and hands back figures, never
// a `Sessions`. Keeping the module private keeps `SessionCsvError` -- the type that call returns --
// off the public surface too, which is where it belongs while nothing outside matches on it.
mod csv;

mod file_name;

mod excel;

// What makes an interval of interest *legal*. Behind the feature with the two front-ends that ask:
// nothing else in the crate calls it, and the API takes the interval it is given. Its ten tests go
// behind the gate with it, which is the cost -- see `docs/historic-feature.md`.
#[cfg(feature = "historic")]
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
pub use log::{RunLog, SourceLog};
pub use peak::IntervalEstimates;
pub use report::site_load_report;

// --- Behind `historic` ---------------------------------------------------------------------------
//
// Here the items are gated, not just the re-exports: `mod ioi` above carries the same `#[cfg]`, and
// `excel::historic` is a gated module. So these names do not exist in a default build.
//
// Named by `ev_peak_cli`, `ev_peak_gui` and `examples/sessions.rs`, all three of which carry
// `required-features` -- and by nothing else: not by the API, not by the desktop app, not by
// `tests/`. The workbook round-trip is one half of that; the rules that decide whether an interval
// of interest is legal are the other, since only a front-end that lets someone *choose* an interval
// has to ask. See `docs/historic-feature.md`.

#[cfg(feature = "historic")]
pub use excel::historic::{xlsx_to_interval_estimates, xlsx_to_sessions};
#[cfg(feature = "historic")]
pub use ioi::{HourEntry, IoiLength, LEGAL_START_MINUTES, checked_interval, hours_of};

// Not named directly by anything outside the crate. `SessionReportCoverage` is public because
// `api::pure` re-exports it; the rest are public because a caller reaches them by reading a field
// of something the API returns -- `SessionNotes` and `TouKwh` off an `Energy`, `AnomalyKind` off a
// `SessionNotes`, `RSession` off a `Sessions`. `api/mod.rs` explains why a field type is not
// re-exported: reading one never requires naming it, but the type still has to be public.
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
