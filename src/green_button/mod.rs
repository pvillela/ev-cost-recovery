// No re-export: this module adds one inherent method to `hydro_bill::BillingPeriod` and defines
// nothing of its own. Declaring it is what puts the method on the type.
mod billing;

mod common;
pub use common::*;

mod espi;
pub use espi::Feed;
use espi::*;

mod excel;
pub use excel::*;

mod peaks;
pub use peaks::*;

mod read_xml;
pub use read_xml::*;

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
