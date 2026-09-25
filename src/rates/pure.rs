//! The rates themselves, as the calculations take them.

use jiff::civil::Date;

/// EV cost-recovery TOU rates, in dollars per kilowatt-hour. The rates are effective for at least
/// one month.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostRecoveryRates {
    /// Effective date of the rates. Normally, the first day of a month.
    pub effective_date: Date,
    /// On-peak EV cost-recovery rate.
    pub on_peak: f64,
    /// Mid-peak EV cost-recovery rate.
    pub mid_peak: f64,
    /// Off-peak EV cost-recovery rate.
    pub off_peak: f64,
}
