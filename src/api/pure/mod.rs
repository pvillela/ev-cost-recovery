//! The half of the API that computes.
//!
//! Everything here is a function of its arguments alone: no file is opened, no clock is read,
//! nothing is written. The reading half sits directly in [`api`](super), and is deliberately
//! thin — it turns paths into values and hands them here, so that the reasoning a figure rests on
//! can be exercised without a filesystem in the way.
//!
//! Taking a `&Path` is not I/O. [`check_reports_cover_period`] reads a *name*, which is a string
//! that happens to be spelled as a path; it never asks whether the file exists.
//!
//! The submodules are by subject, not by call. The API layer's other axis — reading versus
//! computing — is already spent on the `io`/`pure` division, and spending it twice would leave
//! every subject scattered. `peak_power` is one API operation today; `coverage` is what it and
//! every later operation are built from.
//!
//! What a billing period *is* is not here. It is a fact about the bill, so it lives in
//! [`hydro_bill::billing_period`](crate::hydro_bill) with [`BILL_END_DAY`](crate::hydro_bill::BILL_END_DAY)
//! and the crate-private `hydro_bill::BillingPeriod`, and this module reads it from there.

mod coverage;
mod energy;
mod peak_power;
mod recovery;
mod reimbursement;

#[cfg(test)]
pub(crate) mod test_support;

// The re-exports below are effectively all the public items in the sub-modules, including
// sub-module re-exports.

pub use coverage::{
    CoverageError, SessionReportCoverage, check_reports_cover, check_reports_cover_period,
};
pub use energy::{Energy, EnergyCost, EnergyError, HydroBill, Sessions, energy, energy_cost};
pub use peak_power::{
    DeliveryCost, PeakPowerError, PeriodValues, PowerEstimates, PricedInterval, peak_power,
    peak_power_cost, /* HydroBill, Sessions */
};
pub use recovery::{
    CostRecovery, CostRecoveryError, CostRecoveryRates, CostRecoveryStretch, CostRecoverySurplus,
    CostRecoverySurplusError, cost_recovery,
    cost_recovery_surplus, /* PeriodValues, HydroBill, Sessions */
};
pub use reimbursement::{
    ChargesReport, ReimbursementError, ReimbursementReconciliation,
    reconcile_evolute_reimbursement, /* CostRecoveryRates, Sessions */
};

/// One time-of-use band's row in a cost-recovery table: name, kilowatt-hours, rate, recovery.
///
/// Shared by the two reports that print one — `recovery`'s table per stretch of rates, and
/// `reimbursement`'s single table for the month — along with the header and alignment beside it.
/// The two differ in how many tables they print, not in what a band's row holds, and a change to a
/// cell's precision that landed in one and missed the other would be a difference a reader has no
/// way to explain.
pub(super) const BAND_HEADERS: [&str; 4] = ["TOU", "kWh", "EV rate", "Recovery"];

/// See [`BAND_HEADERS`]: right against the name, so the three figures line up under each other.
pub(super) const BAND_ALIGNMENT: [crate::markdown::Align; 4] = [
    crate::markdown::Left,
    crate::markdown::Right,
    crate::markdown::Right,
    crate::markdown::Right,
];

/// See [`BAND_HEADERS`].
pub(super) fn band_row(name: &str, kwh: f64, rate: f64, recovery: f64) -> Vec<String> {
    vec![
        name.to_owned(),
        format!("{kwh:.3}"),
        format!("{rate:.5}"),
        format!("{recovery:.2}"),
    ]
}

/// An amount rounded to the cent, as the reports state it.
///
/// Shared by the two money reports, each of which prints a column that has to add down to a figure
/// stated below it: `recovery`'s surplus and `reimbursement`'s two variances. A stored figure that
/// disagreed with the printed column it summarizes reads as an arithmetic error in a report whose
/// subject is arithmetic.
///
/// Through the formatter rather than by arithmetic on the value. `(x * 100.0).round() / 100.0`
/// rounds a half away from zero while `{:.2}` rounds it to even, so the two disagree on an amount
/// landing exactly on half a cent. The round trip through a string is what makes the result the
/// printed figure by construction rather than by an argument that the two rules coincide.
fn to_the_cent(amount: f64) -> f64 {
    format!("{amount:.2}")
        .parse()
        .expect("a decimal written by this formatter parses back")
}
