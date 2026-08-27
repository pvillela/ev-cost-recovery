//! The EV share of the intervals a billing period peaked in, and what that share cost.
//!
//! [`fn@peak_power`] answers the first: the site's maximum demand happened in some interval, and
//! the chargers account for part of it. [`peak_power_cost`] prices that share against the bill,
//! over the three intervals the three demand-priced delivery lines are each charged on.
//!
//! How long one of those intervals is comes from the meter feed rather than from here: it is a
//! `green_button::METER_INTERVAL`, and a feed stating demand every quarter-hour would shorten it
//! without
//! changing anything below, since every figure is derived through the session module's estimate
//! over whatever interval it is handed.
//!
//! That is a separate question from what a demand charge is levied on, which is the highest
//! 15-minute average within the interval -- one
//! [`Segment`](crate::session::Segment), the length of which does not move with the feed. A
//! `green_button::METER_INTERVAL` holds four of them today; a quarter-hourly feed would make it exactly one.
//!
//! One module rather than two, because the cost is the estimate priced. Both read the same meter
//! figures over the same intervals, both build one [`Sessions`] from the same records by the
//! same rules, and both refuse a period the meter data does not cover. Splitting them would leave
//! two copies of that agreement to keep.

use crate::{
    green_button::{METER_INTERVAL, Peak},
    hydro_bill::{
        NotABillingPeriodEnding, ZeroDenominator, billing_period_dates, billing_period_span,
    },
    markdown::{Left, Right, amounts, field, h1, h2, rounding_note, table},
    session::{
        Bracket, EstimateSet, IntervalEstimates, SessionNotes, Sessions, estimates_from_sessions,
    },
    time::Interval,
};

// Re-exported because `peak_power` and `peak_power_cost` take them. `IntervalEstimates` is
// deliberately not: it is inside `PowerEstimates` and `PricedInterval` rather than named by either
// signature, and a reader who probes that far can go to `session` for it.
pub use crate::{
    green_button::{MeterNotes, PeriodValues},
    hydro_bill::HydroBill,
    session::RSession,
};
use jiff::civil::Date;
use std::{error::Error, fmt};

/// Peak power estimates for a billing period.
pub struct PowerEstimates {
    pub kw_estimates: IntervalEstimates,
    pub kva_estimates: IntervalEstimates,
    /// What the estimates were drawn from, and what was odd about it.
    ///
    /// Every anomaly kind, unlike the consumption side's. An estimate over a single interval turns
    /// on each session's power and on exactly which records touch it, so none of the kinds is
    /// beside the point here. See [`AnomalyKind::bears_on_energy`](crate::session::AnomalyKind::bears_on_energy).
    pub notes: SessionNotes,

    /// The same, for the meter export the two maxima were taken over.
    pub meter: MeterNotes,
}

/// One demand-priced delivery line's interval of interest, and the EV estimate over it.
///
/// The estimate is kept whole rather than reduced to the one figure the charge was computed from.
/// That figure is the mid-point of an energy-based bracket over the
/// [`Segment`](crate::session::Segment) within the interval that maximizes it, and which interval,
/// which segment and which sessions reached it are not recoverable from an `f64`.
///
/// Two maxima, not one, and they are found from different sources: which interval is the one the
/// *building* peaked in, read from the meter export, while which segment within it is the one the
/// *chargers* peaked in, read from the session records.
///
/// How long the interval is comes from the meter feed, not from here: [`Self::estimates`] carries
/// it as [`IntervalEstimates::interval`](crate::session::IntervalEstimates::interval).
#[derive(Debug)]
pub struct PricedInterval {
    /// The demand basis the line is levied on: `"kVA"`, `"kW"` or `"kW 7-7"`. The spelling the
    /// charges table's `Basis` column uses, and the one [`PeakPowerError::NoPeak`] names.
    pub unit: &'static str,

    /// The EV estimate over the interval the building's maximum in that unit fell in.
    pub estimates: IntervalEstimates,
}

/// Breakdown of delivery cost attributable to EV sessions in a billing period.
///
/// Every field is stated rather than left to be recomputed, because the point of the breakdown is
/// to be checked against the bill line by line.
#[derive(Debug)]
pub struct DeliveryCost {
    /// The billing period these figures are for, named by the date it closes on. The bill's own,
    /// since every figure below is a proportion of one of its lines.
    pub billing_period_ending: Date,

    /// `'Distribution Charges' / 'Adj. kVA'` from bill.
    pub blended_distribution_rate: f64,
    /// `'Transmission Connection Charge' / 'Adj. kW'` from bill.
    pub blended_transmission_connection_rate: f64,
    /// `'Transmission Network Charge' / 'Adj. Peak kW 7-7'` from bill.
    pub blended_transmission_network_rate: f64,

    /// Mid-point of energy-based bracket of EV kVA from sessions
    /// for Demand kVA interval of interest.
    pub demand_kva: f64,
    /// Mid-point of energy-based bracket of EV kW from sessions
    /// for Demand kW interval of interest.
    pub demand_kw: f64,
    /// Mid-point of energy-based bracket of EV kW from sessions
    /// for Peak 7-7 kW interval of interest.
    pub peak_7_7_kw: f64,

    /// The three intervals of interest the demand figures above were estimated over, in the order
    /// the charges table lists them: kVA, then kW, then kW 7-7.
    ///
    /// Always three. All three are read before any charge is computed, so a period missing one of
    /// the maxima is refused with [`PeakPowerError::NoPeak`] rather than costed with a gap in it.
    ///
    /// Not rendered by [`fmt::Display`]. This report is a page of money and each of these renders as
    /// a page of sessions; a caller wanting them renders each through
    /// [`IntervalEstimates::to_markdown`](crate::session::IntervalEstimates::to_markdown).
    pub priced_intervals: [PricedInterval; 3],

    /// Days in billing period, as the bill counts them.
    pub days_in_period: u8,
    /// `days_in_period / 30`
    pub days_adj_factor: f64,

    /// Distribution charges attributable to EV sessions.
    pub distribution_charges: f64,
    /// Transmission Connection Charge attributable to EV sessions.
    pub transmission_connection_charge: f64,
    /// Transmission Network Charge attributable to EV sessions.
    pub transmission_network_charge: f64,

    /// HST on delivery charges attributable to EV sessions, before OER.
    pub hst: f64,
    /// Onario Electricity Rebate
    pub ontario_electricity_rebate: f64,

    /// Total delivery cost attributable to EV sessions, net of HST and OER.
    pub delivery_cost: f64,

    /// What the figures were drawn from, and what was odd about it. Every kind, as
    /// [`PowerEstimates::notes`] carries.
    pub notes: SessionNotes,

    /// The same, for the meter export the demand figures are maxima over.
    pub meter: MeterNotes,
}

/// Why a billing period's figures cannot be turned into peak power estimates, or into the delivery
/// cost drawn from them.
///
/// No variant names a file. Producing this is a computation, and a computation cannot fail to read
/// something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeakPowerError {
    NotABillingPeriodEnding(NotABillingPeriodEnding),

    /// The billing period carries no reading in one of the power series, so it has no maximum to
    /// estimate against. The feed is expected to carry hourly kW *and* kVA.
    NoPeak {
        period_ending: Date,
        unit: &'static str,
    },

    /// The meter figures given describe a different billing period.
    ///
    /// [`PeriodValues`] is one period's row, already picked out of an export that ordinarily spans
    /// many. Which other periods the file holds is nothing to this; handing over the wrong row is.
    ValuesAreForAnotherPeriod {
        period_ending: Date,
        values_period_ending: Date,
    },

    /// The meter figures do not cover the whole billing period.
    ///
    /// Every figure produced here is a maximum over the period, and a maximum over part of one is
    /// not a smaller answer to the same question — it is an answer to a different one. The interval
    /// the site actually peaked in may be among the missing ones, and nothing in the result would
    /// say so.
    ///
    /// `intervals` counts the meter intervals that carried data; placeholder rows standing in for a
    /// gap are not among them. See [`PeriodValues::is_complete`].
    PeriodNotFullyCovered {
        period_ending: Date,
        intervals: i64,
        expected: i64,
    },

    /// A bill figure the cost divides by is zero. See [`ZeroDenominator`].
    ZeroDenominator(ZeroDenominator),
}

impl From<ZeroDenominator> for PeakPowerError {
    fn from(e: ZeroDenominator) -> Self {
        Self::ZeroDenominator(e)
    }
}

impl From<NotABillingPeriodEnding> for PeakPowerError {
    fn from(e: NotABillingPeriodEnding) -> Self {
        Self::NotABillingPeriodEnding(e)
    }
}

impl fmt::Display for PeakPowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABillingPeriodEnding(e) => e.fmt(f),
            Self::NoPeak {
                period_ending,
                unit,
            } => write!(
                f,
                "the billing period ending {period_ending} carries no {unit} reading, so it has no \
                 {unit} maximum to estimate against"
            ),
            Self::ValuesAreForAnotherPeriod {
                period_ending,
                values_period_ending,
            } => write!(
                f,
                "the meter figures given are for the billing period ending \
                 {values_period_ending}, not the one ending {period_ending}"
            ),
            Self::PeriodNotFullyCovered {
                period_ending,
                intervals,
                expected,
            } => write!(
                f,
                "the meter data covers {intervals} of the {expected} intervals in the billing period \
                 ending {period_ending}, so its maxima are not the period's"
            ),
            Self::ZeroDenominator(e) => e.fmt(f),
        }
    }
}

impl Error for PeakPowerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotABillingPeriodEnding(e) => Some(e),
            Self::ZeroDenominator(e) => Some(e),
            _ => None,
        }
    }
}

/// Returns peak power estimates for the intervals of interest that maximize kW and kVA in the
/// specified billing period.
///
/// The reading half of the same call is [`io::peak_power`](crate::io::peak_power), which is where
/// these arguments come from. This is everything that call does once the meter export and the
/// session reports have been read.
///
/// The two intervals are the ones the *building* peaked in, taken from `gb_period_values`, and each
/// estimate says how much of that interval's demand the chargers can account for. They are usually
/// different intervals, and occasionally the same one.
///
/// Each is one whole metering interval, because that is the resolution the feed states demand at.
/// The estimate within it is still a 15-minute figure: an
/// [`IntervalEstimates`](crate::session::IntervalEstimates) reports the highest of the interval's
/// segments, which is the basis the demand charge is billed on. A `green_button::METER_INTERVAL` holds four of
/// them today. See docs/session/README.md, "Interval of interest boundaries".
///
/// The maxima used are the period's unrestricted ones — what an invoice bills as `Demand kW` and
/// `Demand kVA` — not the 07:00-19:00 figures it reports as `Peak kW 7-7`.
///
/// # Arguments
/// - `billing_period_ending` - the billing period, named by the date it closes on. Must be
///   [`BILL_END_DAY`](crate::hydro_bill::BILL_END_DAY) of its month.
/// - `gb_period_values` - that period's figures, read from the meter export.
/// - `sessions` - every session from every report covering the period, as read, in the order the
///   reports were read. Not merged: which records describe one session is decided here, since that
///   is a question about the records rather than about the files they came from.
///
/// # Sessions the reports share
///
/// Monthly reports overlap at the month boundary, so a session near it appears in both. A record
/// the two state identically is one session and is counted once; counting it twice would inflate
/// every figure drawn from it.
///
/// A `Charge_Session_ID` carried by two records that are *not* identical does not collapse. Such
/// records are kept and estimated from, both of them, and each is flagged
/// [`AnomalyKind::DuplicateId`](crate::session::AnomalyKind::DuplicateId) in the returned
/// [`IntervalEstimates::session_anomalies`](crate::session::IntervalEstimates::session_anomalies) — subject
/// to the same scoping as every other anomaly there, so one is listed only if its session reaches
/// that estimate's interval.
///
/// A flag rather than an error, because a shared id is not necessarily a fault.
/// `Charge_Session_ID` is not unique in Evolute's reports — the June 2026 report carries `S37487`
/// on two sessions a week apart, within the one file — so refusing would make that month
/// unestimatable. The flag also cannot tell a reused id from two reports genuinely disagreeing
/// about one session, which is why both records are kept and the judgement is left to a reader who
/// can go back to the source rows.
///
/// # Errors
///
/// [`PeakPowerError::NotABillingPeriodEnding`] if `billing_period_ending` does not label a period,
/// and [`PeakPowerError::NoPeak`] if the period carries no reading in one of the two series.
///
/// [`PeakPowerError::ValuesAreForAnotherPeriod`] if `gb_period_values` describes some other period,
/// and [`PeakPowerError::PeriodNotFullyCovered`] if it is missing intervals of the one asked for.
/// Both
/// estimates rest on a maximum over the whole period, so a partial one is refused rather than
/// estimated from.
///
/// There is no variant naming a file: nothing here reads one.
pub fn peak_power(
    billing_period_ending: Date,
    gb_period_values: PeriodValues,
    sessions: &Sessions,
) -> Result<PowerEstimates, PeakPowerError> {
    // Re-checked rather than assumed. This is an entry point in its own right, and a caller
    // reaching it directly has not been through `io::peak_power`'s validation.
    billing_period_dates(billing_period_ending)?;
    check_period_covered(billing_period_ending, &gb_period_values)?;

    let kw_ioi = peak_interval(gb_period_values.max_kw, "kW", billing_period_ending)?;
    let kva_ioi = peak_interval(gb_period_values.max_kva, "kVA", billing_period_ending)?;

    // Both estimates come off the one report, so the two figures cannot be drawn from different
    // session data.
    let sources = sessions.sources.clone();
    let kw_estimates = estimates_from_sessions(kw_ioi, sources.clone(), sessions);
    let kva_estimates = estimates_from_sessions(kva_ioi, sources, sessions);
    Ok(PowerEstimates {
        notes: notes_for_intervals(sessions, [&kw_estimates, &kva_estimates]),
        meter: gb_period_values.notes(),
        kw_estimates,
        kva_estimates,
    })
}

/// The month the delivery lines are priced against: they are levied "per kW per 30 Days", which is
/// where the bill's `Adj.` proration of the demand figures comes from.
const BILLED_DAYS_PER_MONTH: f64 = 30.0;

/// Estimates the net delivery cost attributable to EV charging sessions during a billing period.
///
/// Pure throughout, as [`fn@peak_power`] is: this is everything the call does once the meter
/// export, the session reports and the bill have been read. There is no `io` counterpart yet, so a
/// caller reads the bill with [`hydro_bill_from_pdf`](crate::hydro_bill::hydro_bill_from_pdf) and
/// the rest as [`io::peak_power`](crate::io::peak_power) does.
///
/// # How the figure is arrived at
///
/// Only the three demand-priced delivery lines can be attributed at all. Each is levied on one
/// demand figure, and each demand figure is a maximum over one interval:
///
/// | Bill line | Levied on | Interval of interest |
/// |---|---|---|
/// | Distribution Charges | `Adj. kVA` | the interval the site's kVA peaked in |
/// | Transmission Connection Charge | `Adj. kW` | the interval its kW peaked in |
/// | Transmission Network Charge | `Adj. Peak kW 7-7` | the interval its kW peaked in within 07:00-19:00 |
///
/// For each, the EV share of that same interval is estimated the way [`fn@peak_power`] estimates
/// it, prorated to a 30-day month as the bill prorates its own figure, and priced at the bill's
/// blended rate — the line divided by the adjusted demand it was charged on. "Blended" because a
/// period straddling a rate change carries every delivery line twice, at the old rate and the new,
/// and [`HydroBill`] holds the two added together; the quotient is what was actually charged per
/// kW, whatever the schedule said.
///
/// HST and the Ontario Electricity Rebate are then applied in the bill's own proportions, taken
/// against `total_electricity_charges` rather than from a statutory rate, so a bill that rounds or
/// prorates either of them carries that through to the EV share.
///
/// Everything else on the bill is left out. Consumption is billed by the kilowatt-hour and the
/// customer charge is fixed, so neither turns on which interval the site peaked in and neither
/// belongs in a figure derived from one.
///
/// Note what falls out of pricing at the blended rate: since the rate is a quotient against the
/// *adjusted* demand, and the EV figure is adjusted by the same day count, the proration cancels.
/// Each line comes to the bill's own line times the EV share of the demand it was charged on.
/// [`DeliveryCost::days_adj_factor`] is reported all the same — it is what makes the two adjusted
/// figures comparable, and a reader checking the arithmetic against the bill needs to see it.
///
/// # Arguments
///
/// - `bill` - the Toronto Hydro bill for the period, which supplies every rate and every day count
///   used. Nothing is assumed about the tariff.
/// - `gb_period_values` - that period's figures, read from the meter export.
/// - `sessions` - every session from every report covering the period, as [`fn@peak_power`] takes
///   them.
///
/// The period is not a parameter. `bill` states which one it covers, so passing it alongside would
/// let a caller name two different periods in one call; [`HydroBill::period_end_date`] is the one
/// answer, and every figure here is a proportion of a line on that same bill.
///
/// The session handling is [`fn@peak_power`]'s exactly, duplicates and all: both calls build one
/// [`Sessions`] from the same records by the same rules, so a cost and the estimates it rests
/// on can never disagree about which sessions there were.
///
/// # Errors
///
/// [`PeakPowerError::NotABillingPeriodEnding`] if the bill's meter reading period does not close on
/// [`BILL_END_DAY`](crate::hydro_bill::BILL_END_DAY), and [`PeakPowerError::NoPeak`] if the period
/// carries no reading in one of the three series.
///
/// [`PeakPowerError::ValuesAreForAnotherPeriod`] if `gb_period_values` describes some other period,
/// and [`PeakPowerError::PeriodNotFullyCovered`] if it is missing intervals of the one the bill
/// covers.
/// Every figure here rests on a maximum over the whole period, so a partial one is refused rather
/// than estimated from.
pub fn peak_power_cost(
    bill: &HydroBill,
    gb_period_values: PeriodValues,
    sessions: &Sessions,
) -> Result<DeliveryCost, PeakPowerError> {
    // An off-cycle bill -- one whose meter reading period does not close a billing period -- is
    // refused rather than estimated from. Its demand figures are levied over a window this does not
    // model, so the proration below would be arithmetic on two different bases.
    let billing_period_ending = bill.period_end_date();
    billing_period_dates(billing_period_ending)?;
    check_period_covered(billing_period_ending, &gb_period_values)?;

    let priced_interval = |peak, unit| {
        let ioi = peak_interval(peak, unit, billing_period_ending)?;
        Ok::<_, PeakPowerError>(PricedInterval {
            unit,
            estimates: estimates_from_sessions(ioi, sessions.sources.clone(), sessions),
        })
    };

    // Each maximum is taken over the interval its own bill line is charged on. Reading all three off
    // one interval would price two of the lines against an interval they were never charged for.
    // Taken before the maxima are read off, since those move `gb_period_values` field by field.
    let meter = gb_period_values.notes();
    let kva_ioi = priced_interval(gb_period_values.max_kva, "kVA")?;
    let kw_ioi = priced_interval(gb_period_values.max_kw, "kW")?;
    let kw_nop_ioi = priced_interval(gb_period_values.max_kw_nop, "kW 7-7")?;
    let demand_kva = energy_based(&kva_ioi.estimates, |e| e.energy_based_kva);
    let demand_kw = energy_based(&kw_ioi.estimates, |e| e.energy_based_kw);
    let peak_7_7_kw = energy_based(&kw_nop_ioi.estimates, |e| e.energy_based_kw);

    // Before the three move into the result: a struct literal evaluates its fields in source order,
    // so leaving this call among them would borrow what `priced_intervals` has already taken.
    let notes = notes_for_intervals(
        sessions,
        [&kva_ioi.estimates, &kw_ioi.estimates, &kw_nop_ioi.estimates],
    );

    let days_in_period = bill.number_of_days;
    let days_adj_factor = f64::from(days_in_period) / BILLED_DAYS_PER_MONTH;

    // Each rate is the line as billed over the demand it was billed on, so it carries whatever the
    // bill actually did -- two rate schedules added together, a corrected figure, a rounding.
    let blended_distribution_rate =
        bill.distribution_charges / bill.divisor("Adj. kVA", bill.adj_kva)?;
    let blended_transmission_connection_rate =
        bill.transmission_connection_charge / bill.divisor("Adj. kW", bill.adj_kw)?;
    let blended_transmission_network_rate = bill.transmission_network_charge
        / bill.divisor("Adj. Peak kW 7-7", bill.adj_peak_7_7_kw)?;

    // The EV demand is prorated before pricing because the rate is per adjusted kW or kVA, which is
    // the prorated figure. Pricing the raw figure at that rate would mix the two bases.
    let distribution_charges = blended_distribution_rate * demand_kva * days_adj_factor;
    let transmission_connection_charge =
        blended_transmission_connection_rate * demand_kw * days_adj_factor;
    let transmission_network_charge =
        blended_transmission_network_rate * peak_7_7_kw * days_adj_factor;
    let charges =
        distribution_charges + transmission_connection_charge + transmission_network_charge;

    // Both as fractions of the bill's own charges rather than as rates of their own. The rebate in
    // particular is a policy percentage that has been changed more than once, and reading it off
    // the bill means a change needs no code.
    let total_charges =
        bill.divisor("Total Electricity Charges", bill.total_electricity_charges)?;
    let hst = charges * bill.hst / total_charges;
    let ontario_electricity_rebate = charges * bill.ontario_electricity_rebate / total_charges;

    Ok(DeliveryCost {
        billing_period_ending,
        blended_distribution_rate,
        blended_transmission_connection_rate,
        blended_transmission_network_rate,
        demand_kva,
        demand_kw,
        peak_7_7_kw,
        priced_intervals: [kva_ioi, kw_ioi, kw_nop_ioi],
        days_in_period,
        days_adj_factor,
        distribution_charges,
        transmission_connection_charge,
        transmission_network_charge,
        hst,
        ontario_electricity_rebate,
        // The bill states its own total the same way -- see `HydroBill::bill_total_amount`. The
        // rebate is held as a positive amount and subtracted, though the bill prints it as a
        // credit.
        delivery_cost: charges + hst - ontario_electricity_rebate,
        notes,
        meter,
    })
}

/// The notes for a figure priced on particular intervals: every anomaly kind, but only for the
/// sessions that reach one of them.
///
/// Scoped that way because the figures are. A demand charge is levied on one interval's maximum,
/// and what a session did elsewhere in the month cannot move it. Reporting the period's whole
/// anomaly list beside an interval's figure buries the handful of rows that bear on it -- one real
/// period turned up 139 sessions above the breaker rating, none of them in any of the three priced
/// intervals.
///
/// The kinds are not filtered, unlike the consumption side's. Within an interval that is priced,
/// every kind bears on the estimate: it turns on each session's power and on exactly which records
/// touch the interval. See
/// [`AnomalyKind::bears_on_energy`](crate::session::AnomalyKind::bears_on_energy).
fn notes_for_intervals<'a>(
    sessions: &Sessions,
    intervals: impl IntoIterator<Item = &'a IntervalEstimates>,
) -> SessionNotes {
    // `false` for the kinds, so this starts with the sources, the excluded sessions and the logs
    // and none of the period-wide anomalies.
    let mut notes = sessions.notes(|_| false);
    for ioi in intervals {
        notes.add_anomalies(ioi.session_anomalies.iter().cloned());
    }
    notes
}

impl fmt::Display for DeliveryCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Each line carries the demand it is levied on and the prorated figure it was actually
        // priced at, so that rate times adjusted demand comes to the charge on the same row. With
        // the raw demand alone a reader checking the arithmetic would be out by the day factor.
        let line = |name: &str, unit: &str, demand: f64, rate: f64, charge: f64| {
            vec![
                name.to_owned(),
                unit.to_owned(),
                format!("{demand:.3}"),
                format!("{:.3}", demand * self.days_adj_factor),
                format!("{rate:.4}"),
                format!("{charge:.2}"),
            ]
        };
        let charges = self.distribution_charges
            + self.transmission_connection_charge
            + self.transmission_network_charge;
        let rows = vec![
            line(
                "Distribution Charges",
                "kVA",
                self.demand_kva,
                self.blended_distribution_rate,
                self.distribution_charges,
            ),
            line(
                "Transmission Connection Charge",
                "kW",
                self.demand_kw,
                self.blended_transmission_connection_rate,
                self.transmission_connection_charge,
            ),
            line(
                "Transmission Network Charge",
                "kW 7-7",
                self.peak_7_7_kw,
                self.blended_transmission_network_rate,
                self.transmission_network_charge,
            ),
        ];

        writeln!(f, "{}\n", h1("EV Delivery Cost"))?;
        writeln!(
            f,
            "{}",
            field("Period", &billing_period_span(self.billing_period_ending))
        )?;
        writeln!(
            f,
            "{}\n",
            field(
                "Days adj.",
                &format!(
                    "{}/{BILLED_DAYS_PER_MONTH:.0} = {:.4}",
                    self.days_in_period, self.days_adj_factor
                )
            )
        )?;
        // The figure the surplus is built from comes first, and the line-by-line working that
        // produced it after, for the reason given in `EnergyCost`'s rendering: the answer is what
        // a reader came for, and the working is what they turn to only when they want to check it.
        writeln!(
            f,
            "{}",
            amounts(&[
                ("Delivery charges", charges),
                ("HST", self.hst),
                // Negative because it is a credit. The bill prints it as one, and a column that is
                // alternately added and subtracted cannot be checked by eye.
                (
                    "Ontario Electricity Rebate",
                    -self.ontario_electricity_rebate
                ),
                ("Delivery cost", self.delivery_cost),
            ])
        )?;
        writeln!(f, "\n{}\n", rounding_note())?;

        writeln!(f, "{}\n", h2("Delivery charges by component"))?;
        writeln!(
            f,
            "{}\n",
            table(
                &[
                    "Delivery charges component",
                    "Basis",
                    "EV demand",
                    "Adj. demand",
                    "TH blended rate",
                    "Charge",
                ],
                &rows,
                &[Left, Left, Right, Right, Right, Right],
            )
        )?;
        writeln!(f, "{}", self.notes.to_markdown())?;
        write!(f, "{}", self.meter.to_markdown())
    }
}

/// One figure off the segment that maximises an interval's energy-based estimate.
///
/// The mid-point of the bracket, because the reported session times are stated only to the minute
/// and the overlap they imply is a range. A bill needs one number, and the mid-point is the only
/// choice that does not systematically over- or under-state the share.
///
/// Both units are read off the one segment [`IntervalEstimates`] already chose, which is the
/// selector's reason for existing: the choice is made on energy-based kW, and the load model is
/// monotone in it — a segment drawing more real power draws more apparent power — so the segment
/// maximising kVA is the same one. Re-choosing per unit could only introduce a disagreement.
fn energy_based(
    estimates: &IntervalEstimates,
    which: impl Fn(&EstimateSet) -> Bracket<f64>,
) -> f64 {
    which(&estimates.energy_based_seg_estimate.1).mid()
}

/// Whether the meter figures cover the whole of the billing period.
///
/// Coverage is the only question. An export ordinarily holds many periods — the one this project
/// reads spans nineteen months — and [`PeriodValues`] is one period's row picked out of it, so the
/// file carrying other periods is expected and means nothing here. What matters is that the row is
/// this period's and that no interval of it is missing.
///
/// [`read_gb_for_billing_period`](crate::green_button::read_gb_for_billing_period) returns a period
/// the feed covers
/// only partly rather than refusing it, on the grounds that which discrepancies matter is the
/// caller's judgement. This is that judgement, for both entry points: nothing here can be estimated
/// from a partial period.
///
/// Every figure either function produces is a maximum over the billing period — the interval the
/// site peaked in, and the EV share of that interval. A gap in the feed does not make the maximum
/// smaller but still true; it makes it a maximum over some other set of intervals, and the real
/// peak may be in
/// the gap. Nothing downstream could detect that, because the estimate is drawn from the sessions
/// and the sessions are complete.
fn check_period_covered(
    billing_period_ending: Date,
    gb_period_values: &PeriodValues,
) -> Result<(), PeakPowerError> {
    let values_period_ending = gb_period_values.period.ending;
    if values_period_ending != billing_period_ending {
        return Err(PeakPowerError::ValuesAreForAnotherPeriod {
            period_ending: billing_period_ending,
            values_period_ending,
        });
    }
    if !gb_period_values.is_complete() {
        return Err(PeakPowerError::PeriodNotFullyCovered {
            period_ending: billing_period_ending,
            intervals: gb_period_values.interval_count,
            expected: gb_period_values.period.expected_intervals(),
        });
    }
    Ok(())
}

/// The metering interval a peak occurred in, as an interval of interest.
///
/// # Errors
///
/// [`PeakPowerError::NoPeak`] when the period carries no reading in that series at all.
fn peak_interval(
    peak: Option<Peak>,
    unit: &'static str,
    period_ending: Date,
) -> Result<Interval, PeakPowerError> {
    let peak = peak.ok_or(PeakPowerError::NoPeak {
        period_ending,
        unit,
    })?;
    // Only an interval that starts on the hour can be a peak — see `green_button::peaks` — so this
    // is always a legal interval of interest and needs no further checking.
    Ok(Interval::new(peak.at, METER_INTERVAL))
}

// cargo test --lib -- api::pure::peak_power::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        api::pure::test_support::{
            KVA_PEAK_HOUR, KW_PEAK_HOUR, NOP_PEAK_HOUR, as_report, bill, close, period_ending_date,
            period_values_with_nop, ts, two_report_sessions, two_reports,
        },
        hydro_bill::{BILL_END_DAY, BillingPeriod},
        session::{AnomalyKind, IntervalEstimates, test_support::session},
    };
    use jiff::civil::date;
    use std::{path::PathBuf, rc::Rc};

    /// A bill figure of zero is refused rather than divided by. Every one of the four is checked,
    /// because they are four separate divisions and three of them went unguarded until this test.
    #[test]
    fn a_bill_figure_of_zero_is_refused() {
        let peaks =
            || period_values_with_nop(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR), Some(NOP_PEAK_HOUR));
        let zeroed = |set: fn(&mut HydroBill)| {
            let mut b = bill();
            set(&mut b);
            peak_power_cost(&b, peaks(), &two_reports()).expect_err("a zero divisor")
        };

        for (figure, set) in [
            (
                "Adj. kVA",
                (|b: &mut HydroBill| b.adj_kva = 0.0) as fn(&mut HydroBill),
            ),
            ("Adj. kW", |b: &mut HydroBill| b.adj_kw = 0.0),
            ("Adj. Peak kW 7-7", |b: &mut HydroBill| {
                b.adj_peak_7_7_kw = 0.0
            }),
            ("Total Electricity Charges", |b: &mut HydroBill| {
                b.total_electricity_charges = 0.0
            }),
        ] {
            let err = zeroed(set);
            let PeakPowerError::ZeroDenominator(z) = &err else {
                panic!("{figure} should give a ZeroDenominator, got {err}");
            };
            assert_eq!(z.figure, figure, "{err}");
            // The message names the figure, so a reader knows which line of the bill to look at.
            assert!(err.to_string().contains(figure), "{err}");
        }
    }

    /// The fixture period's maxima, with no 7-7 hour: the estimate does not read one.
    fn period_values(kw_at: Option<&str>, kva_at: Option<&str>) -> PeriodValues {
        period_values_with_nop(kw_at, kva_at, None)
    }

    /// The cost for the fixture bill, sessions and meter figures.
    fn cost() -> DeliveryCost {
        peak_power_cost(
            &bill(),
            period_values_with_nop(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR), Some(NOP_PEAK_HOUR)),
            &two_reports(),
        )
        .expect("the bill closes a billing period and it has all three maxima")
    }

    /// The same cost, with one session in the kW interval carrying an anomaly.
    ///
    /// `ExcessiveAvgKw` because the consumption side does not collect that kind, so a test using it
    /// is asking about the delivery side alone. It sits in the kW interval and in neither of the
    /// other two.
    fn hot_cost() -> DeliveryCost {
        let mut hot = session("June.csv", 7, "HOT", KW_PEAK_HOUR, 60, 6.0);
        Rc::get_mut(&mut hot)
            .expect("sole owner")
            .anomalies
            .push(AnomalyKind::ExcessiveAvgKw);
        let mut sessions = two_report_sessions();
        sessions.push(hot);

        peak_power_cost(
            &bill(),
            period_values_with_nop(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR), Some(NOP_PEAK_HOUR)),
            &as_report(sessions),
        )
        .expect("the bill closes a billing period and it has all three maxima")
    }

    /// The ids a test expects in one segment.
    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    /// The ids in each segment of an estimate, in segment order.
    fn membership(estimates: &IntervalEstimates) -> Vec<Vec<String>> {
        estimates
            .seg_estimates
            .iter()
            .map(|(seg, _)| seg.sessions.iter().map(|s| s.id.clone()).collect())
            .collect()
    }

    /// Each estimate covers the metering hour its own maximum fell in, and reports the highest of
    /// that hour's four segments.
    #[test]
    fn each_estimate_covers_the_hour_its_own_maximum_fell_in() {
        let estimates = peak_power(
            period_ending_date(),
            period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR)),
            &two_reports(),
        )
        .expect("the period has both maxima");

        assert_eq!(estimates.kw_estimates.interval.start, ts(KW_PEAK_HOUR));
        assert_eq!(estimates.kw_estimates.interval.duration, METER_INTERVAL);
        assert_eq!(estimates.kva_estimates.interval.start, ts(KVA_PEAK_HOUR));

        // The hour is tiled by four 15-minute segments, and only sessions reaching the hour appear.
        assert_eq!(
            membership(&estimates.kw_estimates),
            vec![
                ids(&["WHOLE"]),
                ids(&["WHOLE", "MID_A", "MID_B"]),
                ids(&["WHOLE"]),
                ids(&["WHOLE"]),
            ]
        );
        // The busiest segment is the maximum on both derivations.
        let (energy_seg, _) = &estimates.kw_estimates.energy_based_seg_estimate;
        let (count_seg, _) = &estimates.kw_estimates.count_based_seg_estimate;
        assert_eq!(energy_seg.start(), ts("2026-06-10T20:15:00Z"));
        assert_eq!(count_seg.start(), ts("2026-06-10T20:15:00Z"));

        // The other hour is a different hour, holding a different session.
        assert_eq!(
            membership(&estimates.kva_estimates),
            vec![ids(&[]), ids(&[]), ids(&[]), ids(&["EVENING"])]
        );
    }

    /// The files the sessions came from, named once each, in the order they were read.
    #[test]
    fn the_report_names_the_files_its_sessions_came_from() {
        let estimates = peak_power(
            period_ending_date(),
            period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR)),
            &two_reports(),
        )
        .unwrap();
        assert_eq!(
            estimates.kw_estimates.sources,
            [PathBuf::from("May.csv"), PathBuf::from("June.csv")]
        );
        // Both estimates come off the same sessions, so both say the same thing about their source.
        assert_eq!(
            estimates.kw_estimates.sources,
            estimates.kva_estimates.sources
        );
    }

    /// The overlap case: a session at the end of May appears in both months' reports, stated
    /// identically. It is one session, and counting it twice would inflate every figure it enters.
    #[test]
    fn a_session_both_reports_state_identically_is_counted_once() {
        let one_copy = two_report_sessions();
        let mut two_copies = one_copy.clone();
        // The same session as June's `WHOLE`, as May's report states it: its own row in its own
        // file, and every figure the same.
        two_copies.insert(1, session("May.csv", 9, "WHOLE", KW_PEAK_HOUR, 60, 6.0));

        let estimate = |sessions: Vec<RSession>| {
            peak_power(
                period_ending_date(),
                period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR)),
                &as_report(sessions),
            )
            .unwrap()
        };

        assert_eq!(
            membership(&estimate(two_copies.clone()).kw_estimates),
            membership(&estimate(one_copy).kw_estimates),
            "the duplicate should leave the segments as they were"
        );
        assert!(
            estimate(two_copies)
                .kw_estimates
                .session_anomalies
                .is_empty(),
            "one session reported twice is not an anomaly"
        );
    }

    /// The other case: two records share an id but describe different sessions. Evolute reuses ids,
    /// so both count, and both are flagged for a reader rather than one being discarded.
    #[test]
    fn a_reused_id_keeps_both_sessions_and_flags_them() {
        let mut sessions = two_report_sessions();
        // Same id as `MID_A`, a different session entirely — a week earlier, in another file.
        sessions.push(session(
            "May.csv",
            7,
            "MID_A",
            "2026-06-10T20:15:00Z",
            30,
            2.0,
        ));

        let estimates = peak_power(
            period_ending_date(),
            period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR)),
            &as_report(sessions),
        )
        .unwrap();

        // Both are in the 20:15 segment, so the id appears there twice.
        let ids = &membership(&estimates.kw_estimates)[1];
        assert_eq!(ids.iter().filter(|id| *id == "MID_A").count(), 2);

        let flagged: Vec<_> = estimates
            .kw_estimates
            .session_anomalies
            .iter()
            .filter(|a| a.kind == AnomalyKind::DuplicateId)
            .map(|a| (a.session.id.as_str(), a.session.row))
            .collect();
        // Symmetric: the earlier record is flagged as well as the later one.
        assert_eq!(flagged, [("MID_A", 3), ("MID_A", 7)]);
    }

    /// A period with no reading in one of the two series has no maximum to estimate against, and
    /// says which series is missing rather than returning a figure of zero.
    #[test]
    fn a_period_missing_a_series_has_no_estimate_for_it() {
        let err = peak_power(
            period_ending_date(),
            period_values(Some(KW_PEAK_HOUR), None),
            &two_reports(),
        )
        .err()
        .expect("the period carries no kVA reading");
        assert!(
            matches!(err, PeakPowerError::NoPeak { unit: "kVA", .. }),
            "{err}"
        );

        let err = peak_power(
            period_ending_date(),
            period_values(None, Some(KVA_PEAK_HOUR)),
            &two_reports(),
        )
        .err()
        .expect("the period carries no kW reading");
        assert!(
            matches!(err, PeakPowerError::NoPeak { unit: "kW", .. }),
            "{err}"
        );
    }

    /// Each demand figure is read off the hour its own bill line was charged on, not off a single
    /// hour used for all three.
    ///
    /// The three fixture hours hold different sessions, so a figure taken from the wrong one shows
    /// as a wrong number rather than as a coincidence.
    #[test]
    fn each_bill_line_is_priced_on_the_hour_it_was_charged_for() {
        let cost = cost();
        let estimates = peak_power(
            period_ending_date(),
            period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR)),
            &two_reports(),
        )
        .unwrap();

        // The two unrestricted figures are the ones `peak_power` reports for the same period, so
        // the cost and the estimates behind it cannot state different demands.
        assert_eq!(
            cost.demand_kw,
            estimates
                .kw_estimates
                .energy_based_seg_estimate
                .1
                .energy_based_kw
                .mid()
        );
        assert_eq!(
            cost.demand_kva,
            estimates
                .kva_estimates
                .energy_based_seg_estimate
                .1
                .energy_based_kva
                .mid()
        );

        // The 7-7 hour is a third hour holding only `ELSEWHERE`, which is a smaller load than the
        // three sessions in the kW peak hour. All three figures are distinct and none is zero.
        assert!(cost.peak_7_7_kw > 0.0);
        assert!(
            cost.peak_7_7_kw < cost.demand_kw,
            "7-7 {} should be under the unrestricted {}",
            cost.peak_7_7_kw,
            cost.demand_kw
        );
        assert_ne!(cost.demand_kw, cost.demand_kva);
    }

    /// The rates are the bill's lines over the demand each was charged on, and the day proration is
    /// the bill's own.
    #[test]
    fn the_rates_are_the_bills_own_lines_over_the_demand_they_were_charged_on() {
        let cost = cost();
        assert!(close(cost.blended_distribution_rate, 10.0));
        assert!(close(cost.blended_transmission_connection_rate, 3.0));
        assert!(close(cost.blended_transmission_network_rate, 5.0));
        assert_eq!(cost.days_in_period, 31);
        assert!(close(cost.days_adj_factor, 31.0 / 30.0));
    }

    /// Each delivery line comes to the bill's line times the EV share of the demand it was charged
    /// on — the day proration cancels between the rate and the EV figure.
    ///
    /// Derived independently of the code under test: it never divides by a bill line, so this would
    /// fail if the proration were applied on one side only, or omitted from the rate, or applied
    /// twice.
    #[test]
    fn each_delivery_line_is_the_bills_line_times_the_ev_share_of_that_demand() {
        let (cost, bill) = (cost(), bill());
        assert!(close(
            cost.distribution_charges,
            bill.distribution_charges * cost.demand_kva / bill.demand_kva
        ));
        assert!(close(
            cost.transmission_connection_charge,
            bill.transmission_connection_charge * cost.demand_kw / bill.demand_kw
        ));
        assert!(close(
            cost.transmission_network_charge,
            bill.transmission_network_charge * cost.peak_7_7_kw / bill.peak_7_7_kw
        ));
    }

    /// HST and the rebate are the bill's own proportions of the EV charges, and the total is the
    /// charges plus the one less the other — the shape `HydroBill::bill_total_amount` uses.
    #[test]
    fn tax_and_rebate_follow_the_bills_own_proportions() {
        let cost = cost();
        let charges = cost.distribution_charges
            + cost.transmission_connection_charge
            + cost.transmission_network_charge;
        // 1300 / 10000 and 1000 / 10000 on the fixture bill.
        assert!(close(cost.hst, charges * 0.13));
        assert!(close(cost.ontario_electricity_rebate, charges * 0.10));
        assert!(close(
            cost.delivery_cost,
            charges + cost.hst - cost.ontario_electricity_rebate
        ));
        // The rebate is smaller than the tax on these proportions, so the total exceeds the charges.
        assert!(cost.delivery_cost > charges);
    }

    /// Each row carries the adjusted demand it was actually priced at, so rate times adjusted
    /// demand comes to the charge on the same line. With the raw demand alone, a reader checking
    /// the arithmetic would be out by the day factor.
    #[test]
    fn the_report_rows_can_be_checked_across() {
        let cost = cost();
        let text = cost.to_string();
        assert!(text.starts_with("EV Delivery Cost\n"), "{text}");
        assert!(
            text.contains("Period       2026-05-24 - 2026-06-23  (31 days)"),
            "{text}"
        );
        assert!(text.contains("Days adj.    31/30 = 1.0333"), "{text}");
        // Rates are labelled as Toronto Hydro's, not left as a bare "rate" that could be read as
        // what the EV owners are charged.
        assert!(text.contains("TH blended rate"), "{text}");

        // The distribution row: 10.0000 per kVA on the fixture bill, against the adjusted kVA.
        let row = text
            .lines()
            .find(|l| l.contains("Distribution Charges"))
            .expect("the table lists the distribution line");
        let adj = cost.demand_kva * cost.days_adj_factor;
        assert!(row.contains(&format!("{adj:.3}")), "{row}");
        assert!(
            row.contains(&format!("{:.2}", adj * cost.blended_distribution_rate)),
            "{row}"
        );
    }

    /// Every figure is a proportion of a bill line, so a bill from another month would give a
    /// plausible number resting on nothing.
    #[test]
    fn the_period_comes_from_the_bill() {
        // May's bill against June's figures. The period is not a parameter, so the only thing that
        // can disagree with the figures is the bill -- and it is the bill that decides, which is
        // what the refusal naming June as the *figures'* period shows. Were the period taken from
        // the figures instead, there would be nothing here to catch.
        let mut may = bill();
        may.meter_reading_period_from = date(2026, 4, 23);
        may.meter_reading_period_to = date(2026, 5, 23);
        may.number_of_days = 30;

        let err = peak_power_cost(
            &may,
            period_values_with_nop(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR), Some(NOP_PEAK_HOUR)),
            &two_reports(),
        )
        .expect_err("the figures are June's and the bill is May's");
        assert!(
            matches!(
                err,
                PeakPowerError::ValuesAreForAnotherPeriod {
                    period_ending,
                    values_period_ending,
                } if period_ending == date(2026, 5, 23) && values_period_ending == period_ending_date()
            ),
            "{err}"
        );
    }

    /// A maximum over part of a period is not the period's maximum, and the hour the site actually
    /// peaked in may be among the missing ones. Neither entry point can tell, because the estimate
    /// comes from the sessions and the sessions are complete.
    #[test]
    fn meter_data_that_does_not_cover_the_whole_period_is_refused() {
        let mut gappy =
            period_values_with_nop(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR), Some(NOP_PEAK_HOUR));
        // One hour short of the 744 the period holds -- the smallest gap there is.
        gappy.interval_count -= 1;

        let err = peak_power(period_ending_date(), gappy.clone(), &two_reports())
            .err()
            .expect("an hour of the period is missing");
        assert!(
            matches!(
                err,
                PeakPowerError::PeriodNotFullyCovered {
                    intervals: 743,
                    expected: 744,
                    ..
                }
            ),
            "{err}"
        );

        // Both entry points make the same judgement; neither leaves it to the other.
        let err = peak_power_cost(&bill(), gappy, &two_reports())
            .expect_err("an hour of the period is missing");
        assert!(
            matches!(err, PeakPowerError::PeriodNotFullyCovered { .. }),
            "{err}"
        );
    }

    /// The previous period's row carries maxima of its own, complete and sound, and they are not
    /// this period's. Coverage of the period asked for is nil, so the count of hours it does carry
    /// would say nothing.
    #[test]
    fn meter_figures_for_a_different_period_are_refused() {
        let mut may = period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR));
        may.period = BillingPeriod::ending_on(date(2026, 5, 23), BILL_END_DAY);
        may.interval_count = may.period.expected_intervals();

        let err = peak_power(period_ending_date(), may, &two_reports())
            .err()
            .expect("the figures are May's");
        assert!(
            matches!(
                err,
                PeakPowerError::ValuesAreForAnotherPeriod {
                    period_ending,
                    values_period_ending,
                } if period_ending == period_ending_date() && values_period_ending == date(2026, 5, 23)
            ),
            "{err}"
        );
    }

    /// Reached directly rather than through `io::peak_power`, this still checks its own arguments.
    #[test]
    fn a_date_that_does_not_close_a_billing_period_is_refused() {
        let err = peak_power(
            date(2026, 6, 30),
            period_values(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR)),
            &two_reports(),
        )
        .err()
        .expect("30 June does not label a billing period");
        assert!(
            matches!(err, PeakPowerError::NotABillingPeriodEnding(_)),
            "{err}"
        );

        // The cost takes no date, so the same refusal reaches it through the bill: an off-cycle
        // bill, whose meter reading period does not close on the 23rd, is not something to prorate
        // to a 30-day month.
        let mut off_cycle = bill();
        off_cycle.meter_reading_period_to = date(2026, 6, 30);
        let err = peak_power_cost(
            &off_cycle,
            period_values_with_nop(Some(KW_PEAK_HOUR), Some(KVA_PEAK_HOUR), Some(NOP_PEAK_HOUR)),
            &two_reports(),
        )
        .expect_err("30 June does not label a billing period");
        assert!(
            matches!(err, PeakPowerError::NotABillingPeriodEnding(_)),
            "{err}"
        );
    }

    /// Each kept estimate is the one its own scalar was read from, and the three are in the order
    /// the charges table lists them.
    ///
    /// The three fixture intervals hold different sessions, so a pair swapped between them shows as
    /// a wrong number rather than as a coincidence.
    #[test]
    fn each_priced_interval_holds_the_estimate_its_own_figure_was_taken_from() {
        let cost = cost();
        let expected = [
            (KVA_PEAK_HOUR, cost.demand_kva),
            (KW_PEAK_HOUR, cost.demand_kw),
            (NOP_PEAK_HOUR, cost.peak_7_7_kw),
        ];
        let read = [
            |e: &EstimateSet| e.energy_based_kva,
            |e: &EstimateSet| e.energy_based_kw,
            |e: &EstimateSet| e.energy_based_kw,
        ];

        for (i, ((at, figure), read)) in expected.into_iter().zip(read).enumerate() {
            let kept = &cost.priced_intervals[i];
            assert_eq!(kept.estimates.interval.start, ts(at), "{}", kept.unit);
            assert_eq!(
                kept.estimates.interval.duration, METER_INTERVAL,
                "{}",
                kept.unit
            );
            assert_eq!(
                read(&kept.estimates.energy_based_seg_estimate.1).mid(),
                figure,
                "{} was priced off some other interval",
                kept.unit
            );
        }
    }

    /// The kept intervals name their bill line's demand basis the way the charges table prints it.
    ///
    /// Two copies of these three strings exist -- here and in `Display` -- so that retaining an
    /// estimate cannot change a rendered report. This is what keeps them the same.
    #[test]
    fn the_priced_intervals_name_the_basis_the_charges_table_prints() {
        let cost = cost();
        assert_eq!(
            cost.priced_intervals.each_ref().map(|p| p.unit),
            ["kVA", "kW", "kW 7-7"]
        );

        let report = cost.to_string();
        for unit in cost.priced_intervals.each_ref().map(|p| p.unit) {
            assert!(
                report.contains(&format!("| {unit} ")),
                "the charges table has no {unit} row:\n{report}"
            );
        }
    }

    /// Retaining the estimates is not rendering them. The delivery report is a page of money, and
    /// each estimate would add a page of sessions to it.
    #[test]
    fn the_delivery_report_says_nothing_about_the_intervals_it_kept() {
        let report = cost().to_string();
        for heading in [
            "EV Peak Power Contribution",
            "Sessions by segment",
            "Segments",
        ] {
            assert!(
                !report.contains(heading),
                "the delivery report has grown a {heading} section:\n{report}"
            );
        }
    }

    /// An anomaly is listed against the interval it reaches and against no other.
    ///
    /// A demand charge is levied on one interval's maximum, so what a session did elsewhere in the
    /// month cannot move it. Listing it anyway is what buries the rows that do bear on the figure.
    #[test]
    fn each_priced_interval_lists_only_the_anomalies_that_reached_it() {
        let cost = hot_cost();
        let reached = |i: usize| {
            cost.priced_intervals[i]
                .estimates
                .session_anomalies
                .iter()
                .any(|a| a.session.id == "HOT")
        };
        // Index 1 is the kW interval, which is the one `HOT` was placed in.
        assert!(
            reached(1),
            "the kW interval did not pick up its own session"
        );
        assert!(!reached(0), "the kVA interval picked up another's session");
        assert!(!reached(2), "the 7-7 interval picked up another's session");
    }

    /// Every anomaly on a kept interval is in the cost's own notes, so a caller rendering the
    /// intervals can never show a row the summary left out.
    #[test]
    fn the_notes_are_the_anomalies_of_the_intervals_the_cost_keeps() {
        let cost = hot_cost();
        let mut checked = 0;
        for kept in &cost.priced_intervals {
            for anomaly in &kept.estimates.session_anomalies {
                assert!(
                    cost.notes.anomalies.iter().any(|listed| {
                        Rc::ptr_eq(&listed.session, &anomaly.session) && listed.kind == anomaly.kind
                    }),
                    "{} holds an anomaly the notes do not: {anomaly:?}",
                    kept.unit
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "the fixture raised no anomaly to compare");
    }
}
