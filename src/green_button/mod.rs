// Three tiers, and the `pub` on each `use` is what separates them:
//
//   `use x::*;`          module-local -- reachable from here and from files under `green_button/`
//   `pub(crate) use`     also reachable as `crate::green_button::X` from elsewhere in the crate
//   `pub use`            also reachable as `ev_cost_recovery::green_button::X` from outside
//
// What belongs in the public tier is settled by `docs/public-surface-usage.md`, which records what
// the binaries, examples and integration tests actually name.

// No re-export at all: this module adds one inherent method to `hydro_bill::BillingPeriod` and
// defines nothing of its own. Declaring it is what puts the method on the type.
mod billing;

mod common;
use common::*;

mod espi;
use espi::*;

mod excel;

mod peaks;
use peaks::*;

mod read_xml;

// --- Named outside the crate -------------------------------------------------------------------

pub use common::Anomaly;
pub use espi::Feed;
pub use excel::{GbWriteReport, write_gb_workbook};
pub use read_xml::{read_gb_feed, read_gb_for_billing_period};
// Not named directly by anything outside the crate, but `api` re-exports both, which makes them
// public by that route: `PeriodValues` is a parameter of `pure::peak_power` and `MeterNotes` is a
// field of what it returns.
pub use peaks::{MeterNotes, PeriodValues};

// --- Named elsewhere inside the crate ----------------------------------------------------------

pub(crate) use common::METER_INTERVAL;
pub(crate) use peaks::Peak;
pub(crate) use read_xml::GbReadError;

// `parse_espi_xml`, `Readings`, `Series` and `Reading` stay module-local. They are the parsed feed
// in its working form, and everything outside this module reaches it through `Feed` and the two
// readers above.

// Two test modules of their own, rather than `#[cfg(test)]` blocks inside a source file: both need
// `period_values`, which is `pub(crate)`, and neither belongs beside any one of the modules it
// draws on. They read fixtures from `tests/fixtures/green_button/` through `golden::fixture`.
//
// There is no `for_test` escape hatch. There was one -- a public re-export of all of `espi`, so
// that integration tests could call `parse_espi_xml` -- and it turned out those tests were each
// doing `fs::read_to_string` followed by a parse, which is `read_gb_feed`'s whole body. They call
// that instead now, and the tests that also needed `period_values` moved in here.
#[cfg(test)]
mod invoice_tests;
#[cfg(test)]
mod pipeline_tests;
