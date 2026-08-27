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

pub use coverage::{CoverageError, SessionReportCoverage, check_reports_cover_period};
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
    check_charges_report_covers_month,
    reconcile_evolute_reimbursement, /* CostRecoveryRates, Sessions */
};
