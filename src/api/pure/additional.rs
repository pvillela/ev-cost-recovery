//! Whether Evolute paid what our own rates come to.
//!
//! A different question from the surplus the rest of the API answers, and it is worth being clear
//! which is which. The surplus asks whether our rates cover Toronto Hydro's bill, over a billing
//! period that runs from the 24th. This asks whether the money Evolute actually sent matches what
//! those same rates say the month's charging earned, over a calendar month, because a calendar
//! month is what Evolute reports and settles on. Neither figure is derived from the other and
//! neither period contains the other.
//!
//! Two of Evolute's documents meet here. The money, and the kilowatt-hours it was billed on, come
//! off Evolute's Charges Report and are given as values. The kilowatt-hours we price come off the
//! session report, split by time-of-use band. Neither is computed from the other, which is the
//! whole of what makes comparing them worth anything.
//!
//! Nothing here opens anything. The month comes from the session report's file name, which is the
//! only thing that states it — see [`report_coverage`].

use crate::{
    charges_report::ChargesReport,
    markdown::{Left, Right, amounts, field, h1, h2, rounding_note, table, wrap},
    session::{AnomalyKind, SessionNotes, Sessions, TouKwh, file_name::report_coverage, tou_kwh},
    time::{Interval, local_midnight},
};
use jiff::civil::Date;
use std::{error::Error, fmt, path::PathBuf};

// Re-exported for the same reason `recovery` re-exports what it takes: a caller should not have to
// know which module a type comes from in order to spell the call.
pub use crate::api::pure::recovery::CostRecoveryRates;

/// Why a month's reimbursement cannot be reconciled.
///
/// Every variant is settled from the report's file name and the rates given. Nothing here has
/// opened anything, and nothing has been summed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReimbursementError {
    /// The sessions did not come from exactly one session report.
    ///
    /// One report is one calendar month, and a calendar month is what Evolute settles on. Two
    /// reports merged together have no single month to reconcile, and none at all has no month.
    NotOneSessionReport { sources: Vec<PathBuf> },

    /// The report's file name does not state the dates it covers, so the month cannot be read from
    /// it. See [`report_coverage`].
    UndatedSessionReport { path: PathBuf },

    /// The report's file name states a span that is not a whole calendar month.
    ///
    /// A partial month reconciled against a full month's reimbursement is a variance that means
    /// nothing, and looks exactly like Evolute having underpaid.
    NotACalendarMonth { path: PathBuf, from: Date, to: Date },

    /// The Charges Report covers a different span from the month the session report names.
    ///
    /// Two documents for two different months, reconciled against each other, produce a variance
    /// that means nothing and looks exactly like Evolute having underpaid. Both are chosen by
    /// hand, so picking last month's is an ordinary slip.
    ChargesReportIsForAnotherMonth {
        charges_path: PathBuf,
        charges_from: Date,
        charges_to: Date,
        month_start: Date,
        month_end: Date,
    },

    /// The rates given had not taken effect by the first day of the month.
    ///
    /// The same refusal [`cost_recovery`](super::recovery::cost_recovery) makes, for the same
    /// reason: rates that begin mid-month do not price the whole of it, and pricing it with them
    /// anyway states a recovery nobody was charged.
    RatesNotYetInEffect {
        month_start: Date,
        effective_date: Date,
    },
}

impl fmt::Display for ReimbursementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOneSessionReport { sources } => {
                write!(
                    f,
                    "a reimbursement is reconciled for one calendar month, so exactly one session \
                     report is expected; {} were given",
                    sources.len()
                )?;
                for path in sources {
                    write!(f, "\n  {}", path.display())?;
                }
                Ok(())
            }
            Self::UndatedSessionReport { path } => write!(
                f,
                "{}: the file name does not say what the report covers; expected a name of the \
                 form Session_Report_June_1_2026-June_30_2026.csv",
                path.display()
            ),
            Self::NotACalendarMonth { path, from, to } => write!(
                f,
                "{}: the file name says the report covers {from} to {to}, which is not a whole \
                 calendar month",
                path.display()
            ),
            Self::ChargesReportIsForAnotherMonth {
                charges_path,
                charges_from,
                charges_to,
                month_start,
                month_end,
            } => write!(
                f,
                "{}: the Charges Report covers {charges_from} to {charges_to}, but the session \
                 report is for {month_start} to {month_end}",
                charges_path.display()
            ),
            Self::RatesNotYetInEffect {
                month_start,
                effective_date,
            } => write!(
                f,
                "the rates take effect on {effective_date}, after the month begins on \
                 {month_start}, so they do not price the whole of it"
            ),
        }
    }
}

impl Error for ReimbursementError {}

/// Reconciliation of Evolute reimbursement for a calendar month.
///
/// Not `PartialEq`. It carries the anomalies its figures were arrived at despite, and those hold
/// `Rc<Session>`s that no equality worth having is defined on. Compare the figures.
#[derive(Debug, Clone)]
pub struct ReimbursementReconciliation {
    /// First day of the calendar month reconciled.
    ///
    /// Read from the session report's file name, not from the sessions. A month nobody charged in
    /// still has a reimbursement to reconcile, and sessions cannot name a month they do not reach.
    pub month_start: Date,
    /// Last day of the calendar month reconciled, inclusive.
    pub month_end: Date,
    /// kWh consumed by EV charging sessions during the calendar month, per Evolute's
    /// Charges Report (NOT the Session Report).
    pub charges_report_kwh: f64,
    /// The dollars Evolute's Charges Report totals for the calendar month.
    ///
    /// What Evolute's own document says it billed, not what arrived. See [`Self::reimbursed`].
    pub charges_report_amount: f64,
    /// What Evolute actually paid for the calendar month, in dollars.
    ///
    /// Independent of [`Self::charges_report_amount`], and given rather than read, because it
    /// comes from wherever the money was seen to land -- a bank statement, a remittance advice --
    /// and not from any document this crate opens. That independence is what makes
    /// [`Self::remittance_variance`] worth computing: if the figure were taken off the Charges
    /// Report it would agree with the report by construction, whatever Evolute had actually sent.
    pub reimbursed: f64,
    /// Energy consumption attributable to EV charging sessions during the calendar month, by TOU,
    /// calculated from session report.
    pub tou_kwh: TouKwh,
    /// `charges_report_kwh - tou_kwh.total_kwh()`
    pub kwh_variance: f64,
    /// Cost recovery rates effective during the calendar month.
    pub cost_recovery_rates: CostRecoveryRates,
    /// Calculated total cost recovery amount for the calendar month.
    pub cost_recovery_amount: f64,
    /// `reimbursed - cost_recovery_amount`.
    ///
    /// The question the tab exists to answer: did the money that arrived match what our own rates
    /// say the month's charging earned. Positive when Evolute sent more than the rates come to,
    /// negative when it sent less.
    pub dollar_variance: f64,
    /// `reimbursed - charges_report_amount`.
    ///
    /// A narrower question than [`Self::dollar_variance`], and one Evolute alone can answer: did
    /// Evolute send what its own Charges Report says it billed. Our rates do not enter into it.
    /// The two variances fail differently -- this one says the remittance does not match the
    /// document, the other says the document does not match our rates -- and a month can be wrong
    /// in either way without being wrong in the other.
    pub remittance_variance: f64,
    /// What these figures were drawn from, and what needed a judgement call along the way.
    pub notes: SessionNotes,
}

/// The calendar month a set of sessions is for, read off the one report's file name.
///
/// The sessions themselves are never asked. A quiet month would name a shorter span than it is, or
/// none at all, and everything downstream would be reconciled against the wrong dates without
/// anything saying so.
fn month_of(sessions: &Sessions) -> Result<(Date, Date), ReimbursementError> {
    let [source] = &sessions.sources[..] else {
        return Err(ReimbursementError::NotOneSessionReport {
            sources: sessions.sources.clone(),
        });
    };

    let coverage =
        report_coverage(source).ok_or_else(|| ReimbursementError::UndatedSessionReport {
            path: source.clone(),
        })?;
    let (from, to) = (coverage.from, coverage.to);

    if from != from.first_of_month() || to != from.last_of_month() {
        return Err(ReimbursementError::NotACalendarMonth {
            path: source.clone(),
            from,
            to,
        });
    }
    Ok((from, to))
}

/// Checks that a Charges Report is for the same month as the session report.
///
/// Separate from [`reconcile_evolute_reimbursement`], which is handed the report's figures and
/// never sees the report itself. Both documents are chosen by hand, and picking last month's
/// Charges Report is an ordinary slip that would otherwise show up as a plausible-looking
/// underpayment.
///
/// # Errors
///
/// [`ReimbursementError::ChargesReportIsForAnotherMonth`] when the spans differ, and whatever
/// [`reconcile_evolute_reimbursement`] would raise about the session report's own name.
pub fn check_charges_report_covers_month(
    sessions: &Sessions,
    charges: &ChargesReport,
) -> Result<(), ReimbursementError> {
    let (month_start, month_end) = month_of(sessions)?;
    if charges.covers_month(month_start) {
        return Ok(());
    }
    Err(ReimbursementError::ChargesReportIsForAnotherMonth {
        charges_path: charges.path.clone(),
        charges_from: charges.from,
        charges_to: charges.to,
        month_start,
        month_end,
    })
}

/// What each band recovers: that band's kilowatt-hours at that band's rate, on-peak first.
///
/// One function rather than three expressions, so the total and the table it is shown beside cannot
/// be computed differently.
fn recovery_by_band(kwh: &TouKwh, rates: &CostRecoveryRates) -> [f64; 3] {
    [
        kwh.on_peak * rates.on_peak,
        kwh.mid_peak * rates.mid_peak,
        kwh.off_peak * rates.off_peak,
    ]
}

/// Reconciles Evolute's `reimbursement` for the calendar month corresponding to `sessions`.
///
/// The month is whatever the report's file name says it covers. It is not inferred from the
/// sessions: a quiet month would then name a shorter span than it is, or none at all, and the
/// reimbursement would be reconciled against the wrong dates without anything saying so.
///
/// The recovery is worked out exactly as [`cost_recovery`](super::recovery::cost_recovery) works it
/// out — each band's kilowatt-hours at that band's rate, no loss factor and no tax — over the month
/// rather than over a billing period. Sessions are cut at the month's edges as they are cut at a
/// period's, so energy drawn on the 1st at 00:30 belongs to this month and not the last.
///
/// # Arguments
///
/// - `sessions` - every session from the one report covering the month, as
///   [`energy`](super::energy::energy) takes them, with the same treatment of duplicates and of
///   records that contradict themselves.
/// - `charges_report_kwh` - the kilowatt-hours Evolute's Charges Report totals for the month.
///   Given rather than derived, and it has to be: it comes off the document Evolute billed from,
///   which is not the session report. Summing the session report for it would compare that report
///   with itself, and the two would agree by construction whatever Evolute had actually charged.
/// - `charges_report_amount` - the dollars that same report totals for the month.
/// - `reimbursed` - what Evolute actually paid, from wherever the money was seen to land. A third
///   figure rather than a second reading of the one above, so that a remittance which does not
///   match Evolute's own document shows up instead of being assumed away.
/// - `cost_recovery_rates` - the rates in effect over the month. One schedule only: a rate change
///   inside the month would need two, and our schedules change on the first of a month.
///
/// # Errors
///
/// See [`ReimbursementError`]. Every check is made against the file name and the rates before a
/// single session is summed.
pub fn reconcile_evolute_reimbursement(
    sessions: &Sessions,
    charges_report_kwh: f64,
    charges_report_amount: f64,
    reimbursed: f64,
    cost_recovery_rates: CostRecoveryRates,
) -> Result<ReimbursementReconciliation, ReimbursementError> {
    let (month_start, month_end) = month_of(sessions)?;

    if cost_recovery_rates.effective_date > month_start {
        return Err(ReimbursementError::RatesNotYetInEffect {
            month_start,
            effective_date: cost_recovery_rates.effective_date,
        });
    }

    // Prevailing local midnight at both ends, as a rate change is cut in `recovery`: when a month
    // begins is a fact about the calendar people live in, not about the meter's clock.
    let month = Interval::from_start_end(
        local_midnight(month_start),
        local_midnight(
            month_end
                .tomorrow()
                .expect("the last day of a month has a next day"),
        ),
    );

    let counted = sessions.countable();
    let tou = tou_kwh(month, &counted);
    let cost_recovery_amount = recovery_by_band(&tou, &cost_recovery_rates).iter().sum();

    Ok(ReimbursementReconciliation {
        month_start,
        month_end,
        charges_report_kwh,
        charges_report_amount,
        reimbursed,
        kwh_variance: charges_report_kwh - tou.total_kwh(),
        tou_kwh: tou,
        cost_recovery_rates,
        cost_recovery_amount,
        dollar_variance: reimbursed - cost_recovery_amount,
        remittance_variance: reimbursed - charges_report_amount,
        notes: sessions.notes(AnomalyKind::bears_on_energy),
    })
}

/// What the remittance variance means, in a sentence.
///
/// A narrower claim than [`verdict`]'s, and worth keeping apart from it: this one says only
/// whether Evolute sent what its own Charges Report came to. Our rates are not in it.
fn remittance_verdict(variance: f64) -> &'static str {
    // Half a cent, for the reason `verdict` uses the same threshold.
    if variance.abs() < 0.005 {
        "Evolute sent exactly what its own Charges Report comes to for this month."
    } else if variance > 0.0 {
        "Evolute sent more than its own Charges Report comes to for this month, by the amount \
         above."
    } else {
        "Evolute sent less than its own Charges Report comes to for this month, by the amount \
         above."
    }
}

/// What the variance means, in a sentence, so the sign does not have to be read off a number.
fn verdict(variance: f64) -> &'static str {
    // Half a cent, which is where the printed figure stops. A variance smaller than that shows as
    // 0.00, and calling that an overpayment or a shortfall contradicts the column above it.
    if variance.abs() < 0.005 {
        "Evolute reimbursed what the cost-recovery rates come to for this month."
    } else if variance > 0.0 {
        "Evolute reimbursed more than the cost-recovery rates come to for this month, by the \
         amount above."
    } else {
        "Evolute reimbursed less than the cost-recovery rates come to for this month, by the \
         amount above."
    }
}

impl fmt::Display for ReimbursementReconciliation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}\n", h1("Evolute Reimbursement Reconciliation"))?;
        writeln!(
            f,
            "{}",
            field(
                "Month",
                &format!("{} - {}", self.month_start, self.month_end)
            )
        )?;
        writeln!(
            f,
            "{}\n",
            field(
                "EV rates",
                &format!("effective {}", self.cost_recovery_rates.effective_date)
            )
        )?;

        // Both answers first, then the working under headings of its own, as the cost reports do.
        //
        // The remittance leads because it is the narrower question and the one that has to be
        // settled first: whether Evolute sent what its own document says. Only once that holds
        // does comparing the money against our rates mean anything. The Charges Report total
        // appears here and nowhere below -- having been reconciled, it has done its work, and
        // repeating it in a column that does not use it invites reading one subtraction as the
        // other.
        writeln!(
            f,
            "{}",
            amounts(&[
                ("Reimbursement received", self.reimbursed),
                // Negative so the column adds down to the variance, which is a subtraction and
                // cannot be checked against two positive numbers.
                ("Charges Report total", -self.charges_report_amount),
                ("Remittance variance", self.remittance_variance),
            ])
        )?;
        writeln!(
            f,
            "\n{}\n",
            wrap(remittance_verdict(self.remittance_variance), "")
        )?;

        writeln!(
            f,
            "{}",
            amounts(&[
                ("Reimbursement received", self.reimbursed),
                ("Cost recovery earned", -self.cost_recovery_amount),
                ("Dollar variance", self.dollar_variance),
            ])
        )?;
        writeln!(f, "\n{}\n", wrap(verdict(self.dollar_variance), ""))?;
        writeln!(f, "{}\n", rounding_note())?;

        writeln!(f, "{}\n", h2("Cost recovery earned, by time of use"))?;
        let [on_peak, mid_peak, off_peak] =
            recovery_by_band(&self.tou_kwh, &self.cost_recovery_rates);
        let band = |name: &str, kwh: f64, rate: f64, recovery: f64| {
            vec![
                name.to_owned(),
                format!("{kwh:.3}"),
                format!("{rate:.5}"),
                format!("{recovery:.2}"),
            ]
        };
        let rows = vec![
            band(
                "On-peak",
                self.tou_kwh.on_peak,
                self.cost_recovery_rates.on_peak,
                on_peak,
            ),
            band(
                "Mid-peak",
                self.tou_kwh.mid_peak,
                self.cost_recovery_rates.mid_peak,
                mid_peak,
            ),
            band(
                "Off-peak",
                self.tou_kwh.off_peak,
                self.cost_recovery_rates.off_peak,
                off_peak,
            ),
            // No rate on the total line: the three differ, and a weighted mean of them is not a
            // rate anybody was charged.
            vec![
                "Total".to_owned(),
                format!("{:.3}", self.tou_kwh.total_kwh()),
                String::new(),
                format!("{:.2}", self.cost_recovery_amount),
            ],
        ];
        writeln!(
            f,
            "{}\n",
            table(
                &["TOU", "kWh", "EV rate", "Recovery"],
                &rows,
                &[Left, Right, Right, Right],
            )
        )?;

        // After the table it draws on. The kilowatt-hours priced above are one side of this
        // subtraction, so a reader meets them before being asked to check them against Evolute's.
        writeln!(f, "{}\n", h2("Energy variance"))?;
        writeln!(
            f,
            "{}",
            amounts(&[
                ("kWh on Evolute Charges Report", self.charges_report_kwh),
                // Negative so the column adds down to the variance, as the money columns do.
                ("kWh priced above", -self.tou_kwh.total_kwh()),
                ("Energy variance", self.kwh_variance),
            ])
        )?;
        writeln!(
            f,
            "\n{}\n",
            wrap(
                "The two figures come from different documents and are arrived at differently. The \
                 first is from Evolute's billing of users for EV charging during the month. The \
                 second is computed from the Session Report. For a session running across \
                 midnight on the first or the last day of the month, only the portion within the \
                 month counts.",
                "",
            )
        )?;

        write!(f, "{}", self.notes.to_markdown())
    }
}

// cargo test --lib -- api::pure::additional::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        api::pure::test_support::{as_report, close},
        session::test_support::session,
    };
    use jiff::civil::date;

    const JUNE: &str = "data/Session_Report_June_1_2026-June_30_2026.csv";

    fn rates(effective: Date, on: f64, mid: f64, off: f64) -> CostRecoveryRates {
        CostRecoveryRates {
            effective_date: effective,
            on_peak: on,
            mid_peak: mid,
            off_peak: off,
        }
    }

    /// One session wholly inside the month, in a single time-of-use band, so the recovery is one
    /// multiplication and both variances are subtractions that can be done by hand.
    #[test]
    fn the_recovery_is_the_month_s_energy_at_the_month_s_rates() {
        // 02:00 EDT on a June weekday is off-peak, and an hour long stays there.
        let s = session(JUNE, 2, "S1", "2026-06-10T06:00:00Z", 60, 10.0);
        let r = reconcile_evolute_reimbursement(
            &as_report(vec![s]),
            10.0,
            5.00,
            5.00,
            rates(date(2026, 6, 1), 0.11, 0.09, 0.07),
        )
        .expect("a June report and rates effective on the 1st");

        assert_eq!(
            (r.month_start, r.month_end),
            (date(2026, 6, 1), date(2026, 6, 30))
        );
        assert!(close(r.tou_kwh.off_peak, 10.0), "{:?}", r.tou_kwh);
        assert!(close(r.tou_kwh.total_kwh(), 10.0), "{:?}", r.tou_kwh);
        assert!(
            close(r.cost_recovery_amount, 0.70),
            "{}",
            r.cost_recovery_amount
        );
        assert!(close(r.dollar_variance, 4.30), "{}", r.dollar_variance);
        assert!(close(r.kwh_variance, 0.0), "{}", r.kwh_variance);
    }

    /// The Charges Report figure is carried through untouched and is never reconciled against
    /// itself. Whatever Evolute states, the reconciliation reports that figure and measures our own
    /// against it.
    #[test]
    fn the_charges_report_figure_is_taken_as_given() {
        let s = session(JUNE, 2, "S1", "2026-06-10T06:00:00Z", 60, 10.0);
        let r = reconcile_evolute_reimbursement(
            &as_report(vec![s]),
            12.5,
            0.0,
            0.0,
            rates(date(2026, 6, 1), 0.11, 0.09, 0.07),
        )
        .expect("a June report and rates effective on the 1st");

        assert!(
            close(r.charges_report_kwh, 12.5),
            "{}",
            r.charges_report_kwh
        );
        assert!(close(r.tou_kwh.total_kwh(), 10.0), "{:?}", r.tou_kwh);
        assert!(close(r.kwh_variance, 2.5), "{}", r.kwh_variance);
    }

    /// We price only what falls inside the month. A session running past midnight on the last day
    /// counts here for its June part alone, and the rest of it is one of the things the energy
    /// variance can be made of.
    #[test]
    fn a_session_crossing_the_month_end_is_priced_for_its_part_inside_the_month() {
        // Starts 23:00 EDT on 30 June and runs two hours, so half of it falls in July.
        let s = session(JUNE, 2, "S1", "2026-07-01T03:00:00Z", 120, 8.0);
        let r = reconcile_evolute_reimbursement(
            &as_report(vec![s]),
            8.0,
            0.0,
            0.0,
            rates(date(2026, 6, 1), 0.11, 0.09, 0.07),
        )
        .expect("a June report and rates effective on the 1st");

        // About half, not exactly half. A session's energy is spread over its *adjusted* span,
        // which pads the reported end out to the time grid, so the two halves of a two-hour
        // session either side of midnight are not quite equal.
        assert!(
            (r.tou_kwh.total_kwh() - 4.0).abs() < 0.05,
            "{:?}",
            r.tou_kwh
        );
        // Evolute billed the whole session, so the part we did not price is the variance.
        assert!((r.kwh_variance - 4.0).abs() < 0.05, "{}", r.kwh_variance);
    }

    /// A session report can carry sessions dated outside the month it is named for -- the
    /// anonymised sample in `data/`, which came from another building, has some in its June file.
    /// Whether our own reports will is not yet known, so this fixes what happens when they do
    /// rather than what to expect: none of that energy is priced, because none of it was drawn in
    /// the month being settled.
    #[test]
    fn a_session_dated_outside_the_month_is_not_priced() {
        let inside = session(JUNE, 2, "S1", "2026-06-10T06:00:00Z", 60, 10.0);
        let july = session(JUNE, 3, "S2", "2026-07-01T18:00:00Z", 60, 7.0);
        let r = reconcile_evolute_reimbursement(
            &as_report(vec![inside, july]),
            10.0,
            0.0,
            0.0,
            rates(date(2026, 6, 1), 0.11, 0.09, 0.07),
        )
        .expect("a June report and rates effective on the 1st");

        assert!(close(r.tou_kwh.total_kwh(), 10.0), "{:?}", r.tou_kwh);
        assert!(
            close(r.cost_recovery_amount, 0.70),
            "{}",
            r.cost_recovery_amount
        );
    }

    /// The month is read from the file name, not from the sessions. A month nobody charged in has
    /// a reimbursement to reconcile and no session to name itself by.
    #[test]
    fn a_month_with_no_sessions_still_names_its_month() {
        let r = reconcile_evolute_reimbursement(
            &Sessions::from_session_lists(vec![Vec::new()], vec![PathBuf::from(JUNE)], Vec::new()),
            0.0,
            0.0,
            0.0,
            rates(date(2026, 6, 1), 0.11, 0.09, 0.07),
        )
        .expect("a June report and rates effective on the 1st");

        assert_eq!(r.month_start, date(2026, 6, 1));
        assert!(close(r.cost_recovery_amount, 0.0));
        assert!(close(r.tou_kwh.total_kwh(), 0.0));
    }

    /// Each refusal is made before anything is summed, and names what is wrong with the call.
    #[test]
    fn what_cannot_be_reconciled_is_refused() {
        let good = rates(date(2026, 6, 1), 0.11, 0.09, 0.07);
        let one = || vec![session(JUNE, 2, "S1", "2026-06-10T06:00:00Z", 60, 10.0)];

        // Two reports are two months, and neither is the one to reconcile.
        let two_files = Sessions::from_session_lists(
            vec![one()],
            vec![
                PathBuf::from(JUNE),
                PathBuf::from("data/Session_Report_May_1_2026-May_31_2026.csv"),
            ],
            Vec::new(),
        );
        assert!(matches!(
            reconcile_evolute_reimbursement(&two_files, 0.0, 0.0, 0.0, good),
            Err(ReimbursementError::NotOneSessionReport { .. })
        ));

        // A name that says nothing about what it holds.
        let undated = as_report(vec![session(
            "data/sessions.csv",
            2,
            "S1",
            "2026-06-10T06:00:00Z",
            60,
            10.0,
        )]);
        assert!(matches!(
            reconcile_evolute_reimbursement(&undated, 0.0, 0.0, 0.0, good),
            Err(ReimbursementError::UndatedSessionReport { .. })
        ));

        // A name that says it holds part of a month.
        let partial = as_report(vec![session(
            "data/Session_Report_June_1_2026-June_15_2026.csv",
            2,
            "S1",
            "2026-06-10T06:00:00Z",
            60,
            10.0,
        )]);
        assert!(matches!(
            reconcile_evolute_reimbursement(&partial, 0.0, 0.0, 0.0, good),
            Err(ReimbursementError::NotACalendarMonth { .. })
        ));

        // Rates that begin after the month does price only part of it.
        assert!(matches!(
            reconcile_evolute_reimbursement(
                &as_report(one()),
                0.0,
                0.0,
                0.0,
                rates(date(2026, 6, 15), 0.11, 0.09, 0.07)
            ),
            Err(ReimbursementError::RatesNotYetInEffect { .. })
        ));
    }

    /// A variance too small to print is neither an overpayment nor a shortfall, whatever its sign.
    #[test]
    fn a_variance_below_a_printed_cent_is_called_neither_way() {
        assert!(verdict(0.0).contains("reimbursed what"));
        assert!(verdict(-0.001).contains("reimbursed what"));
        assert!(verdict(0.01).contains("more than"));
        assert!(verdict(-0.01).contains("less than"));
    }
}
