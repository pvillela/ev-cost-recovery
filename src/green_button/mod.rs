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

// A seam for `tests/`, not API. `espi` is private because nothing outside this module should be
// building a `Feed` by hand; the integration tests do exactly that, and they run out of process so
// `pub(crate)` cannot reach them. `#[doc(hidden)]` is what keeps it out of the rendered docs and
// out of what a caller is invited to use. Nothing in `src/` may go through here.
#[doc(hidden)]
pub mod for_test {
    pub use super::espi::*;
}
