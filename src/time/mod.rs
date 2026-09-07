// Three tiers, and the `pub` on each `use` is what separates them:
//
//   `use x::*;`          module-local -- reachable from here and from files under `time/`
//   `pub(crate) use`     also reachable as `crate::time::X` from elsewhere in the crate
//   `pub use`            also reachable as `ev_cost_recovery::time::X` from outside
//
// The public tier is the shorter list on purpose. What belongs in it is settled by what the
// binaries, examples and integration tests actually name -- plus whatever `api` re-exports, since
// that publishes a type by a second route, and whatever the `deny` in `lib.rs` refuses to leave
// unnameable. Adding to it means a caller outside the crate needs the name.

mod base;
mod excel;
mod format;
mod tou;

// A module rather than a re-export: `gb_peak_values` calls `holidays::holidays(year)`, and
// flattening that to `time::holidays` would collide with the module's own name.
pub mod holidays;

// --- Named outside the crate -------------------------------------------------------------------

pub use base::{local_date, time_zone};
pub use format::zoned_span;
// Not named directly by anything outside the crate. Both type a public field of something the API
// hands back -- `Interval` types `session::Segment::interval` and `session::IntervalEstimates::
// interval`, `Tou` types `green_button::Peak::tou` -- so a caller reading one has to be able to
// write the type. See the `deny` in `lib.rs`.
pub use base::Interval;
pub use tou::Tou;

// --- Named elsewhere inside the crate ----------------------------------------------------------

pub(crate) use base::{
    TZ_OFFSETS, local_datetime, local_hour, local_midnight, standard_date, standard_midnight,
};
pub(crate) use excel::{
    serial_of_civil, serial_of_date, serial_of_duration, serial_of_instant, serial_of_local,
};
pub(crate) use format::{zoned_minute, zoned_span_end};
pub(crate) use tou::{is_off_peak, tou_of, tou_partition};
