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

// --- Re-exported only in `historic` builds -------------------------------------------------------
//
// What the `#[cfg]` gates is the re-export, not the item. Read each line as "nothing reaches this
// by the path `crate::time::X` unless the feature is on" -- which is a fact about this module's
// consumers, and says nothing about whether the item itself is compiled or used.

// Two cases where item and re-export agree, both `#[cfg(any(test, feature = "historic"))]` on the
// item itself: `excel`'s two serial readers, because reading a serial back is done only by the
// workbook reader, and `dst`'s reporting half below. The writing direction, which the API uses, is
// not gated.
#[cfg(feature = "historic")]
pub(crate) use excel::{duration_of_serial, instant_of_serial};

// Both of these are unconditional in `base`, and both are load-bearing in every build: `time_zone`
// resolves `TIME_ZONE_NAME` on every call, and `BILLING_OFFSET` is an entry of `TZ_OFFSETS`. Only
// the paths out of this module are gated, because outside `base` the sole caller of either is
// `session::ioi` -- `TZ_OFFSETS` to label an ambiguous wall time with the zone it was read in,
// `TIME_ZONE_NAME` in its doc links -- plus `ev_peak_cli` and `ev_peak_gui` for `TZ_OFFSETS`.
#[cfg(feature = "historic")]
pub(crate) use base::TIME_ZONE_NAME;
#[cfg(feature = "historic")]
pub use base::TZ_OFFSETS;

// The reporting half of `dst`, which only `session::ioi` calls: it hands a user every reading of an
// ambiguous wall time instead of choosing one. The deciding half above is ungated, because the CSV
// reader is in every build.
#[cfg(feature = "historic")]
pub(crate) use dst::{TzLocalMapping, map_local};
