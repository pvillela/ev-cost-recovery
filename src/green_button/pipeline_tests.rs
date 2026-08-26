//! The packaged call and the long-hand pipeline describe the same period.
//!
//! [`read_gb_for_billing_period`](super::read_gb_for_billing_period) is a convenience over
//! [`read_gb_feed`] → `Feed::readings` → [`period_values`], picking one period out of the result.
//! This asserts it really is that, rather than a second implementation that happens to agree
//! today.
//!
//! A unit test rather than an integration test, for the reason [`super::invoice_tests`] gives:
//! `period_values` is `pub(crate)`, so the long-hand half of the comparison is not reachable from
//! `tests/`. The rest of `read_gb_for_billing_period`'s behaviour is checked from outside — see
//! `tests/green_button/read_xml.rs` — because it needs nothing but the public call.
//!
//! Asserted against the pipeline rather than against copied figures, so this cannot drift from
//! what the workbook writer reports for the same period. `invoice_tests` is what pins the figures
//! themselves to something outside the software.

use crate::{
    golden,
    green_button::{period_values, read_gb_feed, read_gb_for_billing_period},
    hydro_bill::BILL_END_DAY,
};
use jiff::civil::date;

#[test]
fn the_packaged_call_matches_the_long_hand_pipeline() {
    let path = golden::fixture("green_button/billed_period.XML");
    let ending = date(2026, 6, 23);

    let readings = read_gb_feed(&path).unwrap().readings();
    let expected = period_values(&readings, BILL_END_DAY)
        .into_iter()
        .find(|p| p.period.ending == ending)
        .expect("the fixture carries the billed period");

    let actual = read_gb_for_billing_period(&path, ending, BILL_END_DAY).unwrap();

    assert_eq!(actual.period, expected.period);
    assert_eq!(actual.interval_count, expected.interval_count);
    assert_eq!(actual.kwh_total, expected.kwh_total);
    assert_eq!(actual.max_kw, expected.max_kw);
    assert_eq!(actual.max_kw_nop, expected.max_kw_nop);
    assert_eq!(actual.max_kva, expected.max_kva);
    assert_eq!(actual.max_kva_nop, expected.max_kva_nop);
    assert_eq!(actual.anomaly_counts, expected.anomaly_counts);

    // The fixture is exactly the billed period, so it is whole.
    assert!(actual.is_complete());
    assert_eq!(actual.interval_count, 744, "31 days, no clock change");

    // The long-hand form above never called `Readings::from_source`, so only the packaged call
    // knows which file the figures came from. That asymmetry is the point of `from_source`.
    assert_eq!(actual.source.as_deref(), Some(path.as_path()));
    assert_eq!(expected.source, None);
}
