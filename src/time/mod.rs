// Three tiers, and the `pub` on each `use` is what separates them:
//
//   `use x::*;`          module-local -- reachable from here and from files under `time/`
//   `pub(crate) use`     also reachable as `crate::time::X` from elsewhere in the crate
//   `pub use`            also reachable as `ev_cost_recovery::time::X` from outside
//
// The public tier is the shorter list on purpose. What belongs in it is settled by
// `docs/public-surface-usage.md`, which records what the binaries, examples and integration tests
// actually name; adding to it means a caller outside the crate needs the name.

mod base;

mod dst;

mod excel;

mod tou;

// A module rather than a re-export: `gb_peak_values` calls `holidays::holidays(year)`, and
// flattening that to `time::holidays` would collide with the module's own name.
pub mod holidays;

// --- Named outside the crate -------------------------------------------------------------------

pub use base::{Interval, local_date, time_zone};
// Not named directly by anything outside the crate, but `api::pure::energy` re-exports it, which
// makes it public by that route: `TouKwh` is keyed on it, so a caller reading one has to be able
// to write the key.
pub use tou::Tou;

// --- Named elsewhere inside the crate ----------------------------------------------------------

pub(crate) use base::{
    duration, is_on_grid, local_datetime, local_hour, local_midnight, standard_date,
    standard_midnight, truncate_to,
};
// What the session reader needs to place a reported wall time, and the sentinels for when it
// cannot. Nothing outside the crate names any of them, so `dst` publishes nothing.
pub(crate) use dst::{UNPLACEABLE_END, UNPLACEABLE_START, falls_in_gap, local_readings};
pub(crate) use excel::{
    serial_of_civil, serial_of_date, serial_of_duration, serial_of_instant, serial_of_local,
};
pub(crate) use tou::{is_off_peak, tou_of, tou_partition};
