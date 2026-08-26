//! `read_gb_for_billing_period` from outside the crate, against a real export.
//!
//! What is checkable from here is what a caller can actually reach: the packaged call, a fixture,
//! and the error it raises. The unit tests beside the function cover the argument checks, which
//! need no file; `green_button::pipeline_tests` covers the one comparison that needs the
//! crate-private `period_values`, which is why it is not here.

use ev_cost_recovery::{green_button::read_gb_for_billing_period, hydro_bill::BILL_END_DAY};
use jiff::civil::date;

use super::fixture;

/// A period the export does not reach is an error, and the error says what the export does cover.
///
/// Handing back an empty row instead would let a caller check an invoice against a period no
/// reading supports and see zeroes rather than a problem.
#[test]
fn a_period_outside_the_export_is_an_error_naming_what_is_there() {
    let err = read_gb_for_billing_period(
        &fixture("billed_period.XML"),
        date(2020, 1, 23),
        BILL_END_DAY,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("2020-01-23"), "{err}");
    assert!(err.contains("The feed covers"), "{err}");
    assert!(err.contains("2026"), "the covered range is named: {err}");
}
