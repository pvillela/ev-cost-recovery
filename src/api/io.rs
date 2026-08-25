//! The half of the API that touches the filesystem.
//!
//! Most of it reads: a function turns paths into values and then delegates to [`pure`](super::pure),
//! which is where the arithmetic and the judgement live. Keeping that half thin is the point — what
//! a figure rests on can then be exercised without a filesystem in the way, and the two halves
//! cannot drift apart because there is only one of each calculation.
//!
//! Two functions also write, and they are the exceptions that prove the rule rather than a
//! loosening of it: [`session_csv_to_xlsx`] and [`gb_xml_to_xlsx`] are file-to-file conversions,
//! whose whole product *is* a file. There is no figure in either for a pure function to return, so
//! neither has a counterpart in [`pure`](super::pure). Every other function here leaves the disk as
//! it found it, and the run logs they carry back are written by the caller.

use crate::api::pure;
use crate::green_button::{self, period_values_xml};
use crate::hydro_bill::{BILL_END_DAY, hydro_bill_from_pdf};
use crate::session::{Sessions, csv};
use jiff::civil::Date;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

// Re-exported rather than merely imported: these name what the functions here return, and a caller
// should not have to know which module a call delegates to in order to spell that.
pub use crate::api::error::{
    ApiError, CostRecoveryError, CostRecoverySurplusError, EnergyError, PeakPowerError,
};
pub use crate::api::pure::additional::{ReimbursementError, ReimbursementReconciliation};
pub use crate::api::pure::energy::{Energy, EnergyCost, TouKwh};
pub use crate::api::pure::peak_power::{DeliveryCost, PowerEstimates};
pub use crate::api::pure::recovery::{
    CostRecovery, CostRecoveryRates, CostRecoveryStretch, CostRecoverySurplus,
};
pub use crate::green_button::WriteReport;
pub use crate::session::ConversionReport;

/// A source file could not be read.
///
/// The only error kind this module raises on its own. Everything else it returns comes from a pure
/// function it delegated to.
///
/// `path` is held for a caller that wants to act on which file failed rather than print it. It is
/// deliberately *not* written into the message: both readers name the file they concern, so adding
/// it here produced `data/x.XML: data/x.XML: ...`.
#[derive(Debug)]
pub enum ReadError {
    /// The Green Button export could not be read, could not be parsed, or carries no reading in
    /// the billing period asked for.
    GreenButton {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// A session report could not be read.
    SessionReport {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// A Toronto Hydro bill PDF could not be read, or is not laid out the way one is read.
    ///
    /// [`BillError::is_layout`](crate::hydro_bill::BillError::is_layout) tells those two apart,
    /// and `cause` downcasts to [`BillError`](crate::hydro_bill::BillError) for a caller that
    /// wants to ask.
    Bill {
        path: PathBuf,
        cause: Box<dyn Error>,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GreenButton { cause, .. }
            | Self::SessionReport { cause, .. }
            | Self::Bill { cause, .. } => cause.fmt(f),
        }
    }
}

impl Error for ReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GreenButton { cause, .. }
            | Self::SessionReport { cause, .. }
            | Self::Bill { cause, .. } => Some(cause.as_ref()),
        }
    }
}

/// A workbook could not be produced from the file it was to be converted from.
///
/// Raised only by the two conversions. What the input could not be *read* as is
/// [`ReadError`]; this is about the output.
#[derive(Debug)]
pub enum ConversionError {
    /// The workbook's name would be the input's own, so the conversion would read and write one
    /// file. Reached by handing in something already named `.xlsx`.
    OutputWouldBeInput { path: PathBuf },

    /// A file already stands where the workbook would go, and the caller asked to be refused.
    ///
    /// Refusing is the default rather than a courtesy: the figures in these workbooks get
    /// reconciled against real invoices by hand, and a silent overwrite is how that work is lost.
    /// Move the existing file, delete it, or call again with
    /// [`OnExistingWorkbook::Replace`].
    OutputExists { path: PathBuf },

    /// The workbook could not be built or could not be written.
    Write {
        path: PathBuf,
        cause: Box<dyn Error>,
    },
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputWouldBeInput { path } => write!(
                f,
                "{} is already an .xlsx file, so converting it would read and write the same file",
                path.display()
            ),
            Self::OutputExists { path } => write!(
                f,
                "{} already exists. Move or delete it first, or ask for it to be replaced -- a \
                 conversion never overwrites its output unless told to.",
                path.display()
            ),
            // The path is written in, unlike `ReadError`'s: the writers name no file of their own,
            // so without it a caller is told a workbook could not be written and left to guess
            // which.
            Self::Write { path, cause } => write!(f, "{}: {cause}", path.display()),
        }
    }
}

impl Error for ConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OutputWouldBeInput { .. } | Self::OutputExists { .. } => None,
            Self::Write { cause, .. } => Some(cause.as_ref()),
        }
    }
}

/// Returns peak power estimates for the intervals of interest that maximize kW and kVA in the
/// specified billing period.
///
/// Reads the meter export and the two session reports, and hands them to
/// [`pure::peak_power`](fn@super::pure::peak_power), which states what is done with them and returns
/// the figures. The two intervals are the hours the *building* peaked in, and each estimate says
/// how much of that hour's demand the chargers can account for.
///
/// # Arguments
/// - `billing_period_ending` - the billing period, named by the date it closes on. Must be
///   [`BILL_END_DAY`] of its month.
/// - `gb_xml` - source Green Button XML file covering the billing period.
/// - `session_csv1` - Evolute session report covering the left end of the billing period.
/// - `session_csv2` - Evolute session report covering the right end of the billing period.
///
/// The two reports must cover the billing period completely between them, which is checked from
/// their file names before anything is read. Which is given first makes no difference; the names
/// say what each holds.
///
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
///
/// # Errors
///
/// See [`ApiError`]. Nothing is read until the arguments have been checked against each other, so a
/// caller that has handed in the wrong month is told so rather than after parsing a year of meter
/// readings.
pub fn peak_power(
    billing_period_ending: Date,
    gb_xml: &Path,
    session_csv1: &Path,
    session_csv2: &Path,
) -> Result<PowerEstimates, ApiError> {
    // First, because it is the one check that costs nothing: it reads the two file *names*, and a
    // caller who has handed in the wrong month finds out before a byte is parsed.
    pure::check_reports_cover_period(billing_period_ending, &[session_csv1, session_csv2])?;

    let gb_period_values =
        period_values_xml(gb_xml, billing_period_ending, BILL_END_DAY).map_err(|cause| {
            ReadError::GreenButton {
                path: gb_xml.to_path_buf(),
                cause,
            }
        })?;
    let sessions = read_sessions(&[session_csv1, session_csv2])?;

    pure::peak_power(billing_period_ending, gb_period_values, &sessions).map_err(|cause| {
        ApiError::PeakPower {
            source: gb_source(&cause, None, gb_xml),
            cause,
        }
    })
}

/// Returns the delivery cost attributable to EV charging sessions in a billing period.
///
/// Reads the bill, the meter export and the two session reports, and hands them to
/// [`pure::peak_power_cost`](fn@super::pure::peak_power_cost), which states how the figures are
/// arrived at. Every rate used is the bill's own; nothing here assumes a tariff.
///
/// # Arguments
/// - `bill_pdf` - the Toronto Hydro bill PDF for the period.
/// - `gb_xml` - source Green Button XML file covering the billing period.
/// - `session_csv1` - Evolute session report covering the left end of the billing period.
/// - `session_csv2` - Evolute session report covering the right end of the billing period.
///
/// There is no `billing_period_ending` argument. The bill states which period it covers, and it is
/// read first so that every other file is fetched for that period — the meter export selected by
/// it, and the reports checked against it. A period passed alongside could only agree with the bill
/// or contradict it, and [`pure::peak_power_cost`](fn@super::pure::peak_power_cost) drops it for
/// the same reason.
///
/// The two reports must cover the billing period completely between them, checked from their file
/// names. Which is given first makes no difference; the names say what each holds.
///
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
///
/// # Errors
///
/// See [`ApiError`]. The bill is read before the checks that need a period, so an unreadable bill
/// is reported ahead of anything the reports or the export might also be wrong about.
pub fn peak_power_cost(
    bill_pdf: &Path,
    gb_xml: &Path,
    session_csv1: &Path,
    session_csv2: &Path,
) -> Result<DeliveryCost, ApiError> {
    // First, because it is what says which period this is about. `peak_power` can open with the
    // free name check instead; here that check has nothing to compare against until the bill is in
    // hand.
    let bill = hydro_bill_from_pdf(bill_pdf).map_err(|cause| ReadError::Bill {
        path: bill_pdf.to_path_buf(),
        cause: Box::new(cause),
    })?;
    let billing_period_ending = bill.period_end_date();

    // Still ahead of both parses: it reads the two file *names*, so the wrong month is caught
    // before a year of meter readings is.
    pure::check_reports_cover_period(billing_period_ending, &[session_csv1, session_csv2])?;

    let gb_period_values =
        period_values_xml(gb_xml, billing_period_ending, BILL_END_DAY).map_err(|cause| {
            ReadError::GreenButton {
                path: gb_xml.to_path_buf(),
                cause,
            }
        })?;
    let sessions = read_sessions(&[session_csv1, session_csv2])?;

    pure::peak_power_cost(&bill, gb_period_values, &sessions).map_err(|cause| ApiError::PeakPower {
        source: gb_source(&cause, Some(bill_pdf), gb_xml),
        cause,
    })
}

/// Returns the energy attributable to EV charging sessions in a billing period, split by
/// time-of-use period.
///
/// Reads the two session reports and hands them to [`pure::energy`](fn@super::pure::energy), which
/// states how a session's energy is divided. No meter export and no bill: consumption is billed by
/// the kilowatt-hour, so neither the hour the site peaked in nor any rate on the bill bears on it.
///
/// # Arguments
/// - `billing_period_ending` - the billing period, named by the date it closes on. Must be
///   [`BILL_END_DAY`] of its month.
/// - `session_csv1` - Evolute session report covering the left end of the billing period.
/// - `session_csv2` - Evolute session report covering the right end of the billing period.
///
/// The two reports must cover the billing period completely between them, which is checked from
/// their file names before anything is read. That check matters more here than to a peak estimate:
/// [`pure::energy`](fn@super::pure::energy) sums whatever it is given, so a month's report missing
/// from the call yields a total that is simply too low, with nothing in the figures to say so.
///
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
///
/// # Errors
///
/// See [`ApiError`]. Nothing is read until the file names have been checked against the period.
pub fn energy(
    billing_period_ending: Date,
    session_csv1: &Path,
    session_csv2: &Path,
) -> Result<Energy, ApiError> {
    pure::check_reports_cover_period(billing_period_ending, &[session_csv1, session_csv2])?;
    let sessions = read_sessions(&[session_csv1, session_csv2])?;
    pure::energy(billing_period_ending, &sessions).map_err(|cause| ApiError::Energy {
        source: bill_source(&cause, None),
        cause,
    })
}

/// Returns the energy cost attributable to EV charging sessions in a billing period.
///
/// Reads the bill and the two session reports, and hands them to
/// [`pure::energy_cost`](fn@super::pure::energy_cost), which states how the figures are arrived at.
/// Every rate used is the bill's own; nothing here assumes a tariff.
///
/// No Green Button XML: consumption is billed by the kilowatt-hour, so the hour the site peaked in
/// does not bear on it. The meter export is [`peak_power_cost`]'s.
///
/// # Arguments
/// - `bill_pdf` - the Toronto Hydro bill PDF for the period.
/// - `session_csv1` - Evolute session report covering the left end of the billing period.
/// - `session_csv2` - Evolute session report covering the right end of the billing period.
///
/// There is no `billing_period_ending` argument. The bill states which period it covers, and it is
/// read first so that the reports are checked against that period rather than against one passed
/// alongside it, which could only agree with the bill or contradict it.
///
/// The two reports must cover the billing period completely between them, checked from their file
/// names. Which is given first makes no difference; the names say what each holds.
///
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
///
/// # Errors
///
/// See [`ApiError`]. The bill is read before the check that needs a period, so an unreadable bill is
/// reported ahead of anything the reports might also be wrong about.
pub fn energy_cost(
    bill_pdf: &Path,
    session_csv1: &Path,
    session_csv2: &Path,
) -> Result<EnergyCost, ApiError> {
    // First, because it is what says which period this is about. `energy` can open with the free
    // name check instead; here that check has nothing to compare against until the bill is in hand.
    let bill = hydro_bill_from_pdf(bill_pdf).map_err(|cause| ReadError::Bill {
        path: bill_pdf.to_path_buf(),
        cause: Box::new(cause),
    })?;

    pure::check_reports_cover_period(bill.period_end_date(), &[session_csv1, session_csv2])?;
    let sessions = read_sessions(&[session_csv1, session_csv2])?;

    pure::energy_cost(&bill, &sessions).map_err(|cause| ApiError::Energy {
        source: bill_source(&cause, Some(bill_pdf)),
        cause,
    })
}

/// Returns the cost recovery allocated to a billing period, at the EV cost-recovery rates given.
///
/// Reads the two session reports and hands them to
/// [`pure::cost_recovery`](fn@super::pure::cost_recovery), which states how the figures are arrived
/// at. No bill and no meter export: the rates here are ours rather than Toronto Hydro's, so nothing
/// on the bill bears on the answer.
///
/// # Arguments
/// - `billing_period_ending` - the billing period, named by the date it closes on. Must be
///   [`BILL_END_DAY`] of its month.
/// - `session_csv1` - Evolute session report covering the left end of the billing period.
/// - `session_csv2` - Evolute session report covering the right end of the billing period.
/// - `recovery_rates_at_start` - the rates in effect on the period's first day.
/// - `recovery_rates_at_end` - the rates the period changed to, or `None` if it did not.
///
/// The rates are values rather than a path. Nothing in this crate writes them down, so there is no
/// file for this to read them from and no file a rate failure could be about.
///
/// The two reports must cover the billing period completely between them, which is checked from
/// their file names before anything is read. That check matters as much here as to
/// [`energy`]: the recovery is a sum over whatever it is given, so a month's report missing from
/// the call yields a figure that is simply too low, with nothing in it to say so.
///
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
///
/// # Errors
///
/// See [`ApiError`]. Nothing is read until the file names have been checked against the period, and
/// the rates are checked against it before the reports are opened for the same reason.
pub fn cost_recovery(
    billing_period_ending: Date,
    session_csv1: &Path,
    session_csv2: &Path,
    recovery_rates_at_start: CostRecoveryRates,
    recovery_rates_at_end: Option<CostRecoveryRates>,
) -> Result<CostRecovery, ApiError> {
    pure::check_reports_cover_period(billing_period_ending, &[session_csv1, session_csv2])?;
    let sessions = read_sessions(&[session_csv1, session_csv2])?;
    Ok(pure::cost_recovery(
        billing_period_ending,
        &sessions,
        recovery_rates_at_start,
        recovery_rates_at_end,
    )?)
}

/// Returns the EV cost-recovery surplus for a billing period: what the rates recover, less what the
/// chargers' share of the bill cost.
///
/// Reads the bill, the meter export and the two session reports — every source the library has —
/// and hands them to [`pure::cost_recovery_surplus`](fn@super::pure::cost_recovery_surplus), which
/// states how the figures are arrived at. The result carries all three parts whole, so the
/// subtraction can be checked against the reports it came from.
///
/// # Arguments
/// - `bill_pdf` - the Toronto Hydro bill PDF for the period.
/// - `gb_xml` - source Green Button XML file covering the billing period.
/// - `session_csv1` - Evolute session report covering the left end of the billing period.
/// - `session_csv2` - Evolute session report covering the right end of the billing period.
/// - `recovery_rates_at_start` - the rates in effect on the period's first day.
/// - `recovery_rates_at_end` - the rates the period changed to, or `None` if it did not.
///
/// There is no `billing_period_ending` argument. The bill states which period it covers, and it is
/// read first so that every other source is fetched for that period. A date passed alongside could
/// only agree with the bill or contradict it, and contradicting it here would subtract two periods'
/// figures from each other.
///
/// The two reports must cover the billing period completely between them, checked from their file
/// names. Which is given first makes no difference; the names say what each holds.
///
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
///
/// # Errors
///
/// See [`ApiError`]. The bill is read before the checks that need a period, so an unreadable bill is
/// reported ahead of anything the reports or the export might also be wrong about.
pub fn cost_recovery_surplus(
    bill_pdf: &Path,
    gb_xml: &Path,
    session_csv1: &Path,
    session_csv2: &Path,
    recovery_rates_at_start: CostRecoveryRates,
    recovery_rates_at_end: Option<CostRecoveryRates>,
) -> Result<CostRecoverySurplus, ApiError> {
    // First, because it is what says which period this is about, as it is for the two costs.
    let bill = hydro_bill_from_pdf(bill_pdf).map_err(|cause| ReadError::Bill {
        path: bill_pdf.to_path_buf(),
        cause: Box::new(cause),
    })?;
    let billing_period_ending = bill.period_end_date();

    pure::check_reports_cover_period(billing_period_ending, &[session_csv1, session_csv2])?;

    let gb_period_values =
        period_values_xml(gb_xml, billing_period_ending, BILL_END_DAY).map_err(|cause| {
            ReadError::GreenButton {
                path: gb_xml.to_path_buf(),
                cause,
            }
        })?;
    let sessions = read_sessions(&[session_csv1, session_csv2])?;

    pure::cost_recovery_surplus(
        &bill,
        gb_period_values,
        &sessions,
        recovery_rates_at_start,
        recovery_rates_at_end,
    )
    .map_err(|cause| ApiError::CostRecoverySurplus {
        source: surplus_source(&cause, bill_pdf, gb_xml),
        cause,
    })
}

/// Names the file a [`CostRecoverySurplusError`] is about.
///
/// Delegates to the two functions that already answer this for the costing errors, since a surplus
/// fails in exactly their ways plus the recovery's. A recovery failure names no file: the rates are
/// the caller's own values, and the period came from the bill only after it was read successfully.
fn surplus_source(
    cause: &CostRecoverySurplusError,
    bill_pdf: &Path,
    gb_xml: &Path,
) -> Option<PathBuf> {
    match cause {
        CostRecoverySurplusError::Recovery(_) => None,
        CostRecoverySurplusError::PeakPower(e) => gb_source(e, Some(bill_pdf), gb_xml),
        CostRecoverySurplusError::Energy(e) => bill_source(e, Some(bill_pdf)),
    }
}

/// Names the file a [`PeakPowerError`] is about, given the paths this call was made with.
///
/// Which file that is turns on the variant, not on the call: a period only partly covered is the
/// meter export's, while a date that closes no period is the bill's when a bill supplied it. Pure
/// says what went wrong and this says where, because neither knows both.
///
/// `bill_pdf` is `None` for [`peak_power`], where the date is the caller's own argument and no file
/// is at fault.
fn gb_source(cause: &PeakPowerError, bill_pdf: Option<&Path>, gb_xml: &Path) -> Option<PathBuf> {
    match cause {
        // Both are the bill's: it supplied the date, and it states the figure that is zero.
        PeakPowerError::NotABillingPeriodEnding(_) | PeakPowerError::ZeroDenominator(_) => {
            bill_pdf.map(Path::to_path_buf)
        }
        PeakPowerError::NoPeak { .. }
        | PeakPowerError::ValuesAreForAnotherPeriod { .. }
        | PeakPowerError::PeriodNotFullyCovered { .. } => Some(gb_xml.to_path_buf()),
    }
}

/// Names the file an [`EnergyError`] is about.
///
/// Both variants concern the bill: it states the period, and it states the rates. `bill_pdf` is
/// `None` for [`energy`], which takes no bill and gets its date from the caller.
fn bill_source(cause: &EnergyError, bill_pdf: Option<&Path>) -> Option<PathBuf> {
    match cause {
        EnergyError::NotABillingPeriodEnding(_)
        | EnergyError::NoRate { .. }
        | EnergyError::ZeroDenominator(_) => bill_pdf.map(Path::to_path_buf),
    }
}

/// Reconciles what Evolute reimbursed for a calendar month against what the cost-recovery rates
/// come to over the same month.
///
/// Reads the one session report and hands it to
/// [`pure::reconcile_evolute_reimbursement`](fn@super::pure::reconcile_evolute_reimbursement),
/// which states how the figures are arrived at.
///
/// Independent of the surplus the rest of this module computes, and not a part of it. That asks
/// whether our rates cover Toronto Hydro's bill over a billing period running from the 24th; this
/// asks whether Evolute paid what those rates earned over a calendar month. No bill and no meter
/// export are read, because neither counterparty's figures depend on them.
///
/// # Arguments
///
/// - `session_csv` - the Evolute session report for the month. One report, and its file name is
///   what says which month this is.
/// - `charges_report_kwh` - the kilowatt-hours Evolute's Charges Report states for the month. A
///   value rather than a path: that document is not one this crate reads.
/// - `reimbursement` - what Evolute actually paid for the month, in dollars.
/// - `cost_recovery_rates` - the rates in effect over the month, as values rather than a path, for
///   the reason [`cost_recovery`] takes them that way: nothing in this crate writes them down.
///
/// Nothing here writes. The report's `csv.read` log comes back unwritten on the result's `notes` --
/// see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls to
/// put it beside its input.
///
/// # Errors
///
/// See [`ApiError`]. The file is opened before the month is read off its name, unlike
/// [`cost_recovery`]: there is only one report here, so a name that says nothing and a file that
/// cannot be read are the same trip to the disk either way.
pub fn reconcile_evolute_reimbursement(
    session_csv: &Path,
    charges_report_kwh: f64,
    reimbursement: f64,
    cost_recovery_rates: CostRecoveryRates,
) -> Result<ReimbursementReconciliation, ApiError> {
    let sessions = read_sessions(&[session_csv])?;
    Ok(pure::reconcile_evolute_reimbursement(
        &sessions,
        charges_report_kwh,
        reimbursement,
        cost_recovery_rates,
    )?)
}

// --------------------------------------------------------------------------------------------
// The conversions
//
// The two functions that write. Both take one file and put a workbook beside it, and both settle
// before reading anything whether that workbook may be written where it would have to go.

/// What a conversion is to do about a workbook that is already there.
///
/// An argument rather than a policy, because the two callers want opposite things and both are
/// right. A batch job wants to be stopped; someone who has just edited a session report and is
/// converting it again wants the new workbook. What is not wanted is the middle case — an
/// overwrite nobody asked for — so replacing has to be said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnExistingWorkbook {
    /// Leave the existing file alone and fail with [`ConversionError::OutputExists`].
    Refuse,

    /// Overwrite it. The old workbook is gone, along with anything written into it by hand.
    Replace,
}

/// Where a conversion's workbook goes, once it is settled that it may be written.
///
/// Both refusals are made before the input is opened. Parsing a year of meter readings and then
/// discovering there is nowhere to put the result wastes the time and, worse, reports the second
/// problem as though it were the first.
///
/// [`OnExistingWorkbook::Replace`] waives only the second refusal. An input that is its own output
/// is refused either way: there is no reading a file that the writer has already truncated.
fn workbook_path(
    input: &Path,
    output: PathBuf,
    on_existing: OnExistingWorkbook,
) -> Result<PathBuf, ConversionError> {
    if output == input {
        return Err(ConversionError::OutputWouldBeInput {
            path: input.to_path_buf(),
        });
    }
    if on_existing == OnExistingWorkbook::Refuse && output.exists() {
        return Err(ConversionError::OutputExists { path: output });
    }
    Ok(output)
}

/// Converts an Evolute session report into a workbook beside it, and says what needed a judgement
/// call on the way.
///
/// Delegates to [`session::excel::session_csv_to_xlsx`], which states what the workbook holds: the
/// report's own columns in the order it states them, then the columns this software derives, with
/// `adj_conn_duration` and `avg_kw` as live formulas and the anomalies of each row in the last
/// column. Every session is written, anomalous ones included — which of them takes part in an
/// estimate is decided on the reading side.
///
/// # Arguments
///
/// - `session_csv` - the Evolute session report to convert.
/// - `on_existing` - what to do about a workbook already standing where this one goes.
///
/// The workbook's name is not an argument. It is the input's, with the extension replaced, which is
/// what [`session::excel::workbook_path`] settles and what every reader of these files expects to
/// find beside the report.
///
/// Nothing else here writes. The conversion's run log comes back unwritten on the result's `log` --
/// see [`ConversionReport`] -- and [`SourceLog::write`](crate::session::SourceLog::write) is what a
/// binary calls to put it beside the workbook.
///
/// # Errors
///
/// See [`ApiError`]. Whether an existing workbook is refused or replaced is `on_existing`'s to say,
/// and either way it is settled before the report is opened.
pub fn session_csv_to_xlsx(
    session_csv: &Path,
    on_existing: OnExistingWorkbook,
) -> Result<ConversionReport, ApiError> {
    workbook_path(
        session_csv,
        crate::session::excel::workbook_path(session_csv),
        on_existing,
    )?;
    // The path is not passed on: `session::excel::session_csv_to_xlsx` derives the same one from
    // the same function, and taking it as an argument there would let a caller send the workbook
    // somewhere the check above never looked at.
    crate::session::excel::session_csv_to_xlsx(session_csv).map_err(|cause| {
        ApiError::Conversion(ConversionError::Write {
            path: crate::session::excel::workbook_path(session_csv),
            cause,
        })
    })
}

/// What [`gb_xml_to_xlsx`] produced.
///
/// A pair rather than a single type, because [`WriteReport`] is the workbook writer's own account
/// of what it put in the sheets and knows nothing about where the file went.
#[derive(Debug, Clone)]
pub struct GbConversionReport {
    /// Where the workbook was written.
    pub output_path: PathBuf,
    /// What went into it.
    pub written: WriteReport,
}

/// Converts a Toronto Hydro Green Button export into the peak-values workbook beside it.
///
/// Two sheets, as [`green_button::write_workbook`] builds them: `Peak_values` carries one row per
/// billing period — the energy used, the highest kW and kVA over the period and within the 7-7
/// demand window, when each fell and in which Time-of-Use period — and `Interval_values` carries
/// every hour of the export. A period holding fewer intervals than a whole one should, and any cell
/// reporting an anomaly, are highlighted in the sheet.
///
/// # Arguments
///
/// - `gb_xml` - the Green Button (ESPI) XML export to convert.
/// - `on_existing` - what to do about a workbook already standing where this one goes.
///
/// There is no `bill_end_day` argument, for the reason the reading functions here take none: the
/// billing period is [`BILL_END_DAY`], which is a fact about Toronto Hydro rather than a choice a
/// caller makes. The workbook's name is likewise the input's with the extension replaced.
///
/// The feed must carry hourly readings for all three of kWh, kW and kVA. Anything else is an error
/// naming what was missing, rather than a workbook with a hole in it.
///
/// # Errors
///
/// See [`ApiError`]. Whether an existing workbook is refused or replaced is `on_existing`'s to say,
/// and either way it is settled before the export is opened — which matters more here than for a
/// session report, since parsing a multi-year export is not quick.
pub fn gb_xml_to_xlsx(
    gb_xml: &Path,
    on_existing: OnExistingWorkbook,
) -> Result<GbConversionReport, ApiError> {
    let output_path = workbook_path(gb_xml, gb_xml.with_extension("xlsx"), on_existing)?;

    let xml = std::fs::read_to_string(gb_xml).map_err(|cause| ReadError::GreenButton {
        path: gb_xml.to_path_buf(),
        cause: Box::new(cause),
    })?;
    let feed = green_button::parse(&xml).map_err(|cause| ReadError::GreenButton {
        path: gb_xml.to_path_buf(),
        cause,
    })?;

    let written =
        green_button::write_workbook(&output_path, &feed, BILL_END_DAY).map_err(|cause| {
            ApiError::Conversion(ConversionError::Write {
                path: output_path.clone(),
                cause,
            })
        })?;

    Ok(GbConversionReport {
        output_path,
        written,
    })
}

/// The named reports as one [`Sessions`].
///
/// Merged rather than flattened, and merged as reports rather than as lists of sessions. What each
/// file contributed — the anomalies that are relations between its records, the path it was read
/// from, the log written beside it — is context that belongs to the read, and flattening the
/// buckets into a bare `Vec` threw all of it away at the one point in the program that had it.
///
/// [`Sessions::merge`] is where the merging is defined; this only fetches the files and hands them
/// over in the order given.
fn read_sessions(paths: &[&Path]) -> Result<Sessions, ReadError> {
    let mut reports = Vec::with_capacity(paths.len());
    for path in paths {
        reports.push(
            csv::session_list(path).map_err(|cause| ReadError::SessionReport {
                path: path.to_path_buf(),
                cause,
            })?,
        );
    }
    Ok(Sessions::merge(reports))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::api::error::CoverageError;
    use crate::hydro_bill::billing_period_dates;
    use jiff::civil::date;

    /// The two refusals a conversion makes before opening anything, and the case that passes.
    ///
    /// Both are settled from the paths alone, so this needs no fixture and writes nothing. The
    /// repository's own tracked files stand in for a workbook that is already there.
    #[test]
    fn a_conversion_refuses_an_output_it_would_destroy() {
        // Handing in something already named `.xlsx` would read and write one file.
        let xlsx = Path::new("book.xlsx");
        assert!(matches!(
            workbook_path(
                xlsx,
                xlsx.with_extension("xlsx"),
                OnExistingWorkbook::Refuse
            ),
            Err(ConversionError::OutputWouldBeInput { .. })
        ));

        // A file already standing where the workbook would go is not overwritten unasked.
        assert!(matches!(
            workbook_path(
                Path::new("Cargo.lock"),
                PathBuf::from("Cargo.toml"),
                OnExistingWorkbook::Refuse
            ),
            Err(ConversionError::OutputExists { .. })
        ));

        // Nothing in the way, so the conversion may proceed and is told where to put it.
        assert_eq!(
            workbook_path(
                Path::new("data/Session_Report_June_1_2026-June_30_2026.csv"),
                PathBuf::from("data/no_such_workbook_here.xlsx"),
                OnExistingWorkbook::Refuse
            )
            .unwrap(),
            PathBuf::from("data/no_such_workbook_here.xlsx")
        );
    }

    /// Asking for a replacement waives the second refusal and only the second.
    ///
    /// Nothing is written: `workbook_path` decides and returns, and the caller does the writing.
    #[test]
    fn asking_to_replace_waives_only_the_existing_file() {
        // The file is there and would be overwritten, which is what was asked for.
        assert_eq!(
            workbook_path(
                Path::new("Cargo.lock"),
                PathBuf::from("Cargo.toml"),
                OnExistingWorkbook::Replace
            )
            .unwrap(),
            PathBuf::from("Cargo.toml")
        );

        // An input that is its own output stays refused. There would be nothing left to read.
        let xlsx = Path::new("book.xlsx");
        assert!(matches!(
            workbook_path(
                xlsx,
                xlsx.with_extension("xlsx"),
                OnExistingWorkbook::Replace
            ),
            Err(ConversionError::OutputWouldBeInput { .. })
        ));
    }

    /// The session conversion asks `session::excel` where the workbook goes rather than deriving
    /// the name itself, so the file it checks and the file it writes cannot come apart.
    #[test]
    fn the_session_workbook_is_named_in_one_place() {
        let csv = Path::new("data/Session_Report_June_1_2026-June_30_2026.csv");
        assert_eq!(
            crate::session::excel::workbook_path(csv),
            csv.with_extension("xlsx")
        );
    }

    /// A date that is not a closing date is the caller's mistake, and is reported as such rather
    /// than reaching the panic in `BillingPeriod::ending_on`.
    #[test]
    fn a_date_that_does_not_close_a_billing_period_is_refused() {
        let err = peak_power(
            date(2026, 6, 30),
            Path::new("nothing.XML"),
            Path::new("Session_Report_May_1_2026-May_31_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
        )
        .err()
        .expect("30 June does not label a billing period");
        assert!(
            matches!(
                err,
                ApiError::Coverage(CoverageError::NotABillingPeriodEnding(_))
            ),
            "{err}"
        );
    }

    /// The arguments are checked against each other before any file is opened, so the wrong months
    /// are named as such even though none of the paths exist.
    #[test]
    fn the_reports_are_checked_before_anything_is_read() {
        let err = peak_power(
            date(2026, 6, 23),
            Path::new("nothing.XML"),
            Path::new("Session_Report_April_1_2026-April_30_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
        )
        .err()
        .expect("April and June do not cover a period starting 24 May");
        assert!(
            matches!(
                err,
                ApiError::Coverage(CoverageError::PeriodNotCovered { .. })
            ),
            "{err}"
        );
        assert!(err.to_string().contains("2026-05-24"), "{err}");
    }

    /// The cost takes no period, so the bill is what supplies one and is read before anything that
    /// needs it. With every path bad, the bill is still the failure reported.
    #[test]
    fn the_bill_is_read_first_because_it_is_what_names_the_period() {
        let err = peak_power_cost(
            Path::new("nothing.pdf"),
            Path::new("nothing.XML"),
            // Months that do not cover a period between them, so the report check would fire
            // first if it could run at all. It cannot: it has no period to check against yet.
            Path::new("Session_Report_April_1_2026-April_30_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
        )
        .expect_err("there is no such bill");
        assert!(
            matches!(err, ApiError::Read(ReadError::Bill { .. })),
            "{err}"
        );
        // Named once, by the reader, as every other file in this module is.
        assert!(err.to_string().contains("nothing.pdf"), "{err}");
    }

    /// The energy total is a sum over whatever it is given, so a month missing from the call comes
    /// back as a figure that is simply too low. The names are what catch it, before either file is
    /// opened.
    #[test]
    fn energy_refuses_reports_that_do_not_cover_the_period() {
        let err = energy(
            date(2026, 6, 23),
            Path::new("Session_Report_April_1_2026-April_30_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
        )
        .expect_err("April and June do not cover a period starting 24 May");
        assert!(
            matches!(
                err,
                ApiError::Coverage(CoverageError::PeriodNotCovered { .. })
            ),
            "{err}"
        );
        assert!(err.to_string().contains("2026-05-24"), "{err}");
    }

    /// A validation failure names the file it is about, which pure could not: it is handed figures,
    /// not paths, so it can say a period is only partly covered but not which export left the hole.
    #[test]
    fn a_validation_failure_names_the_file_it_is_about() {
        let bill = Path::new("June.pdf");
        let xml = Path::new("meter.XML");

        // The meter export's fault, whether or not a bill was given.
        let gap = PeakPowerError::PeriodNotFullyCovered {
            period_ending: date(2026, 6, 23),
            intervals: 743,
            expected: 744,
        };
        assert_eq!(gb_source(&gap, Some(bill), xml).as_deref(), Some(xml));
        assert_eq!(gb_source(&gap, None, xml).as_deref(), Some(xml));

        // The bill's fault, when a bill is what supplied the date. Same variant, different file:
        // this is why the choice cannot be made from the variant alone.
        let off_cycle = PeakPowerError::NotABillingPeriodEnding(
            billing_period_dates(date(2026, 6, 30)).expect_err("30 June closes no period"),
        );
        assert_eq!(
            gb_source(&off_cycle, Some(bill), xml).as_deref(),
            Some(bill)
        );
        // `peak_power` takes the date from its caller, so no file is at fault.
        assert_eq!(gb_source(&off_cycle, None, xml), None);

        // The rate a bill does not state is the bill's.
        let no_rate = EnergyError::NoRate {
            period_ending: date(2026, 6, 23),
            tou: crate::time::Tou::OnPeak,
        };
        assert_eq!(bill_source(&no_rate, Some(bill)).as_deref(), Some(bill));
        assert_eq!(bill_source(&no_rate, None), None);
    }

    /// The path is written into the message, unlike `ReadError::path`. These errors name no file of
    /// their own, so without it a caller holding four paths is told a period is uncovered and left
    /// to guess by which.
    #[test]
    fn the_named_file_reaches_the_message() {
        let cause = PeakPowerError::PeriodNotFullyCovered {
            period_ending: date(2026, 6, 23),
            intervals: 743,
            expected: 744,
        };
        let named = ApiError::PeakPower {
            source: Some(PathBuf::from("meter.XML")),
            cause: cause.clone(),
        };
        assert!(named.to_string().starts_with("meter.XML: "), "{named}");

        // Without one the message is the pure error's, unchanged - no stray separator.
        let bare = ApiError::PeakPower {
            source: None,
            cause: cause.clone(),
        };
        assert_eq!(bare.to_string(), cause.to_string());
    }

    /// The recovery is a sum over whatever it is given, as the energy total is, so the same name
    /// check has to catch a month missing from the call before either file is opened.
    #[test]
    fn cost_recovery_refuses_reports_that_do_not_cover_the_period() {
        let rates = CostRecoveryRates {
            effective_date: date(2026, 5, 1),
            on_peak: 0.11,
            mid_peak: 0.09,
            off_peak: 0.07,
        };
        let err = cost_recovery(
            date(2026, 6, 23),
            Path::new("Session_Report_April_1_2026-April_30_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
            rates,
            None,
        )
        .expect_err("April and June do not cover a period starting 24 May");
        assert!(
            matches!(
                err,
                ApiError::Coverage(CoverageError::PeriodNotCovered { .. })
            ),
            "{err}"
        );
    }

    /// The surplus reads every source the library has, and the bill is what names the period for
    /// all of them, so it is read before anything else. With every path bad, it is still the
    /// failure reported.
    #[test]
    fn the_surplus_reads_the_bill_first_too() {
        let rates = CostRecoveryRates {
            effective_date: date(2026, 5, 1),
            on_peak: 0.11,
            mid_peak: 0.09,
            off_peak: 0.07,
        };
        let err = cost_recovery_surplus(
            Path::new("nothing.pdf"),
            Path::new("nothing.XML"),
            // Months that do not cover a period between them, so the report check would fire first
            // if it could run at all. It cannot: it has no period to check against yet.
            Path::new("Session_Report_April_1_2026-April_30_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
            rates,
            None,
        )
        .expect_err("there is no such bill");
        assert!(
            matches!(err, ApiError::Read(ReadError::Bill { .. })),
            "{err}"
        );
        assert!(err.to_string().contains("nothing.pdf"), "{err}");
    }

    /// A surplus fails in the two costing operations' ways plus the recovery's, and names a file
    /// only where they would. The rates are the caller's own values, so a recovery failure names
    /// none.
    #[test]
    fn a_surplus_failure_names_a_file_only_where_a_cost_would() {
        let bill = Path::new("June.pdf");
        let xml = Path::new("meter.XML");

        let gap = CostRecoverySurplusError::PeakPower(PeakPowerError::PeriodNotFullyCovered {
            period_ending: date(2026, 6, 23),
            intervals: 743,
            expected: 744,
        });
        assert_eq!(surplus_source(&gap, bill, xml).as_deref(), Some(xml));

        let no_rate = CostRecoverySurplusError::Energy(EnergyError::NoRate {
            period_ending: date(2026, 6, 23),
            tou: crate::time::Tou::OnPeak,
        });
        assert_eq!(surplus_source(&no_rate, bill, xml).as_deref(), Some(bill));

        // The rates came from the caller and the period from a bill already read, so no file is at
        // fault.
        let rates = CostRecoverySurplusError::Recovery(CostRecoveryError::RatesNotYetInEffect {
            period_start: date(2026, 5, 24),
            effective_date: date(2026, 6, 1),
        });
        assert_eq!(surplus_source(&rates, bill, xml), None);
    }

    /// The energy cost takes no period either, so the bill is read before the report check, as it
    /// is for the delivery cost. With every path bad, the bill is still the failure reported.
    #[test]
    fn the_energy_cost_reads_the_bill_first_too() {
        let err = energy_cost(
            Path::new("nothing.pdf"),
            // Months that do not cover a period between them, so the report check would fire first
            // if it could run at all. It cannot: it has no period to check against yet.
            Path::new("Session_Report_April_1_2026-April_30_2026.csv"),
            Path::new("Session_Report_June_1_2026-June_30_2026.csv"),
        )
        .expect_err("there is no such bill");
        assert!(
            matches!(err, ApiError::Read(ReadError::Bill { .. })),
            "{err}"
        );
        assert!(err.to_string().contains("nothing.pdf"), "{err}");
    }
}
