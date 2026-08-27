//! Toronto Hydro bills: the charges themselves, as distinct from the metered consumption that
//! `green_button` reads and the charging sessions that `session` reads.
//!
//! The two older modules answer *how much* — kilowatt-hours, kilowatts, kilovolt-amperes, and
//! which quarter-hour the site peaked in. Neither answers *what it cost*, and the project exists
//! to work out how much of a bill EV charging is responsible for. That last step needs the bill:
//! the rate schedule, the delivery and regulatory lines, the loss factor, and the way a demand
//! charge is levied on a monthly peak rather than on consumption. [`HydroBill`] is that bill, and
//! [`hydro_bill_from_pdf`] reads one straight out of the PDF Toronto Hydro issues.
//!
//! Behind those two names the source is a stack, each file knowing less than the one above it:
//! `bill.rs` is the figures alone, `bill_pdf.rs` is everything that knows what a Toronto Hydro bill
//! looks like, and [`pdf_text`] is positioned text out of any PDF at all. Only the last is a module
//! of its own here, because reading a PDF is a job in its own right and its `Line` and `Fragment`
//! read better with it named.
//!
//! Singular because the unit of work is one bill for one billing period. A run reconciles a series
//! of them, but nothing here holds a series: every figure in [`HydroBill`] belongs to the period
//! that bill covers, and `bill.rs` says which one that is.
//!
//! What a billing period *is*, `billing_period.rs` defines: [`BILL_END_DAY`], and the
//! crate-private `BillingPeriod` and `billing_period_dates`. It is here because the period is the
//! bill's, and the rest of the crate divides its data that way only because the bill does.

// Three tiers, and the `pub` on each `use` is what separates them:
//
//   `use x::*;`          module-local -- reachable from here and from files under `hydro_bill/`
//   `pub(crate) use`     also reachable as `crate::hydro_bill::X` from elsewhere in the crate
//   `pub use`            also reachable as `ev_cost_recovery::hydro_bill::X` from outside
//
// What belongs in the public tier is settled by `docs/public-surface-usage.md`, which records what
// the binaries, examples and integration tests actually name.

mod bill_pdf;

mod billing_period;

mod bill;

// A module rather than a set of re-exports, and the only one here. Reading a PDF is a job in its
// own right, and `hydro_bill_dump` and `tests/hydro_bill/all_bills` both write `pdf_text::` --
// `Line` and `Fragment` read better with the module naming them.
pub mod pdf_text;

// --- Named outside the crate -------------------------------------------------------------------

pub use bill_pdf::{BillError, hydro_bill_from_pdf};
pub use billing_period::{BILL_END_DAY, bill_start_day};
// Not named directly by anything outside the crate, but `api::pure` re-exports it: `energy` and
// `peak_power_cost` both take one, so a caller has to be able to write the type.
pub use bill::{HydroBill, ZeroDenominator};
// A variant payload of four public error enums in `api::pure`, so a caller matching past the first
// level has to be able to write it.
pub use billing_period::NotABillingPeriodEnding;

// --- Named elsewhere inside the crate ----------------------------------------------------------

pub(crate) use billing_period::{
    BillingPeriod, MAX_BILL_END_DAY, billing_period_dates, billing_period_span,
};
