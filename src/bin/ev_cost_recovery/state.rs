//! The app's state, and every decision about it, with no egui in sight.
//!
//! The widget code above this is meant to be thin enough to check by eye; everything that could be
//! *wrong* rather than merely ugly — whether a file is the right sort of file, whether an amount is
//! a number, whether the run may go ahead at all, what a saved report is called — is decided here
//! and tested here.

use ev_cost_recovery::{
    api::{
        CostRecoverySurplus, GbWriteReport, OnExistingWorkbook, ReimbursementReconciliation,
        cost_recovery_surplus, gb_xml_to_xlsx, pure::check_reports_cover_period,
        reconcile_evolute_reimbursement, session_csv_to_xlsx,
    },
    hydro_bill::{billing_period_dates, hydro_bill_from_pdf},
    log::SourceLog,
    session::{parse_session_report_name, report_coverage},
};
use jiff::civil;
use std::{
    mem,
    path::{Path, PathBuf},
};

/// Which document is on screen.
///
/// One run produces the first two, so there is no landing screen: the app opens on the tab where
/// the work is asked for. [`Tab::Detail`] holds nothing until that run has succeeded.
///
/// [`Tab::Reimbursement`] answers a different question against a different counterparty over a
/// different calendar, and shares nothing with the other two but the folder the file dialogs open
/// in and the rates workbook. It is a tab rather than a second program because it is the same month's charging seen from
/// the other side, and whoever asks one question asks the other in the same sitting.
///
/// [`Tab::Convert`] answers no question at all. It turns a source file into a workbook to be read
/// by eye, which is what the two command-line converters do, and it is here so that the app is the
/// only thing anyone has to open. It comes last because nothing else needs it: every figure this
/// app produces is taken from the source files directly.
// `Hash` so the tab can salt the central panel's scroll area — see `app.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    #[default]
    Surplus,
    Detail,
    Reimbursement,
    Convert,
}

#[derive(Default)]
pub struct AppState {
    pub tab: Tab,
    pub surplus: SurplusState,
    pub reimbursement: ReimbursementState,
    pub convert: ConvertState,
    pub working_dir: WorkingDir,
    pub rates_workbook: RatesWorkbook,
}

impl AppState {
    /// Whether the detail tab has anything to show. The tab is drawn greyed until it does, rather
    /// than opening on an empty page that says to go back.
    pub fn detail_ready(&self) -> bool {
        self.surplus.outcome.is_some()
    }

    /// Drops the results on both tabs that use the rates workbook, if a different one has been
    /// chosen since this was last called.
    ///
    /// Both, not just the tab the choice was made on: the workbook is shared, and figures on the
    /// other tab were priced from the one it replaced.
    pub fn settle_rates_workbook(&mut self) {
        if mem::take(&mut self.rates_workbook.chosen) {
            self.surplus.clear_results();
            self.reimbursement.clear_results();
        }
    }
}

/// The rates workbook, one for the whole app: choosing it on either tab chooses it on both.
///
/// Read when a run starts, and at no other time, so a workbook edited in the spreadsheet after it
/// was chosen is read as it stands at the run. Like [`WorkingDir`], it lasts as long as the app
/// does and no longer.
#[derive(Default)]
pub struct RatesWorkbook {
    path: Option<PathBuf>,
    /// Set by a choice, and cleared by [`AppState::settle_rates_workbook`] once both tabs have
    /// dropped the results the previous workbook priced.
    chosen: bool,
}

impl RatesWorkbook {
    /// The workbook chosen, or `None` before one has been.
    pub fn get(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Takes the workbook for both tabs.
    pub fn choose(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.chosen = true;
    }

    /// What the file dialog filters on: the description, then the extensions, in both cases for
    /// the reason [`Input::filter`] gives.
    pub fn filter() -> (&'static str, &'static [&'static str]) {
        ("Rates workbook", &["xlsx", "XLSX"])
    }
}

/// The folder the user is working in, shared by every file dialog in the app.
///
/// A month's bill, its meter export and its session reports are ordinarily filed together, so
/// this is one folder for the whole app rather than one per picker. It lasts as long as the app
/// does and no longer: nothing is written to disk, so a fresh launch starts wherever the system
/// would have started anyway.
#[derive(Default)]
pub struct WorkingDir(Option<PathBuf>);

impl WorkingDir {
    /// The folder a dialog should open in, or `None` before the user has picked anything.
    pub fn get(&self) -> Option<&Path> {
        self.0.as_deref()
    }

    /// Remembers where a file was picked from or written to.
    pub fn remember(&mut self, file: &Path) {
        // A bare filename's parent is `""`, which would send the next dialog nowhere in particular.
        if let Some(dir) = file.parent()
            && !dir.as_os_str().is_empty()
        {
            self.0 = Some(dir.to_path_buf());
        }
    }
}

// --------------------------------------------------------------------------------------------
// The inputs

/// Which of the four files a picker is for.
///
/// In the order the run needs them, which is also the order they are drawn: the bill says which
/// period this is, and the other three are read against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Bill,
    Meter,
    Sessions1,
    Sessions2,
}

impl Input {
    pub const ALL: [Self; 4] = [Self::Bill, Self::Meter, Self::Sessions1, Self::Sessions2];

    /// The label beside the picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Bill => "Toronto Hydro bill",
            Self::Meter => "Green Button export",
            Self::Sessions1 => "Session report 1",
            Self::Sessions2 => "Session report 2",
        }
    }

    /// What the file dialog filters on: the description, then the extensions.
    pub fn filter(self) -> (&'static str, &'static [&'static str]) {
        // Each extension in both cases. A Linux dialog turns an extension into the shell glob
        // `*.<ext>` and matches that against the file name, so `*.xml` alone leaves a Green Button
        // export invisible in the chooser: Toronto Hydro names it `.XML`. Windows and macOS
        // disregard case already and neither minds the extra entry.
        match self {
            Self::Bill => ("Hydro bill", &["pdf", "PDF"]),
            Self::Meter => ("Green Button export", &["xml", "XML"]),
            Self::Sessions1 | Self::Sessions2 => ("Session report", &["csv", "CSV"]),
        }
    }

    /// Whether this input is an Evolute session report, whose file name states what it covers.
    fn is_session_report(self) -> bool {
        matches!(self, Self::Sessions1 | Self::Sessions2)
    }

    /// Whether changing this input can change what is wrong with the session-report pickers.
    ///
    /// The two session slots themselves, and the bill: the period the bill names is what decides
    /// whether one report already covers everything, so a new bill can settle that question
    /// differently without either session slot having been touched.
    fn bears_on_session_reports(self) -> bool {
        matches!(self, Self::Bill | Self::Sessions1 | Self::Sessions2)
    }

    /// Whether the run goes ahead without this input. The second session report alone.
    ///
    /// What it decides is whether the picker offers to empty itself. Emptying a required picker
    /// only disables the run, which the user can reach by choosing a different file; emptying the
    /// optional one is a choice with a result, and needs a control of its own to make.
    pub fn is_optional(self) -> bool {
        matches!(self, Self::Sessions2)
    }

    /// The other session-report picker, for a message that has to name it.
    fn other_session_report(self) -> Self {
        match self {
            Self::Sessions1 => Self::Sessions2,
            _ => Self::Sessions1,
        }
    }
}

/// A figure the user typed, refused unless a sum of money could take it.
///
/// `"nan"`, `"inf"` and `"-inf"` all parse as `f64`, so parsing alone lets them through: NaN then
/// spreads into every total it touches and the report still renders, while a negative amount
/// reverses the sign of the variance it enters. The field is named here because a message from
/// deeper in cannot name it.
fn checked_figure(value: f64, what: &str, text: &str) -> Result<f64, String> {
    if !value.is_finite() {
        return Err(format!(
            "cannot read \"{text}\" as the {what}: it is not a finite number"
        ));
    }
    if value < 0.0 {
        return Err(format!("the {what} cannot be negative: \"{text}\""));
    }
    Ok(value)
}

// --------------------------------------------------------------------------------------------
// The run

/// A finished run, with the report and the text of it side by side. The text is what the command
/// line prints, kept verbatim so that a saved report and a piped one are the same file.
pub struct SurplusOutcome {
    pub surplus: CostRecoverySurplus,
    pub text: String,
}

#[derive(Default)]
pub struct SurplusState {
    pub bill: Option<PathBuf>,
    pub meter: Option<PathBuf>,
    pub sessions1: Option<PathBuf>,
    pub sessions2: Option<PathBuf>,
    /// The closing date of the period the chosen bill covers, read out of the PDF when it was
    /// chosen.
    ///
    /// The bill is the only document that states which period is being reconciled, and its file
    /// name does not say: the name is Toronto Hydro's, and the statement date it carries is not the
    /// closing date. So this is the one place the app opens a file before the run — everything the
    /// two session-report pickers decide is measured against this date, and neither can be offered
    /// before it is known.
    ///
    /// `None` before a bill is chosen, and when the chosen one could not be read — which is
    /// reported against the bill's own picker, since with no period there is nothing the session
    /// pickers can be asked for.
    pub bill_period_ending: Option<civil::Date>,
    pub outcome: Option<SurplusOutcome>,
    pub error: Option<String>,
    /// Why a report could not be saved, if a save was tried and failed.
    ///
    /// Separate from [`Self::error`] for the same reason [`Self::log_failures`] is: by the time a
    /// save is attempted the figures are worked out, and a failed save reported as the run's error
    /// would say no result was produced when one was. It is also drawn on two tabs -- the Cost
    /// recovery and Peak power detail tabs are two views of one state -- so a save that failed on
    /// one would otherwise appear under the other's "Work out the surplus" button.
    pub save_error: Option<String>,
    /// Why the run's logs could not be written, if they could not.
    ///
    /// Separate from [`Self::error`], and for the same reason [`SessionWorkbook::log_failure`] is
    /// separate from a conversion's: by the time a log is written the figures are worked out, and
    /// reporting a missing log as the run's error would say no result was produced when one was.
    /// One entry per log that failed, since a run writes two.
    pub log_failures: Vec<String>,
    /// What a picked file was refused for, against the picker it was chosen at. Reported where the
    /// choice was made rather than at the foot of the form.
    pub input_notes: Vec<(Input, String)>,
}

impl SurplusState {
    /// Takes a file for one of the four pickers.
    ///
    /// A session report is checked here rather than at run time, because its file name is the only
    /// thing that says which month it holds and a name that says nothing is worth catching while
    /// the file dialog is still fresh in mind.
    ///
    /// The bill is opened here, and it is the only input that is. See [`Self::bill_period_ending`]
    /// for what is taken from it and why the reading cannot wait for the run.
    pub fn select(&mut self, which: Input, path: PathBuf) {
        self.input_notes.retain(|(w, _)| *w != which);
        if which == Input::Bill {
            match hydro_bill_from_pdf(&path) {
                Ok(bill) => self.set_bill_period(Some(bill.period_end_date())),
                // The library's own message, which names the file and says whether the trouble was
                // the PDF or the layout. Reported at the picker, because a bill that does not read
                // leaves the session pickers with nothing to measure against.
                Err(e) => {
                    self.set_bill_period(None);
                    self.input_notes.push((Input::Bill, e.to_string()));
                }
            }
        }
        *self.slot(which) = Some(path);
        if which.bears_on_session_reports() {
            self.recheck_session_reports();
        }
        self.clear_results();
    }

    /// Takes the period a newly chosen bill states, and empties whichever session slots it
    /// invalidates.
    ///
    /// **The second slot always.** Two reports are chosen as a pair, to reach across one period
    /// between them; against a different period the pair means nothing, and the second is the half
    /// that was chosen to fill a gap the first left.
    ///
    /// **The first slot unless its report touches the new period.** A report that overlaps the
    /// period at all is plausibly still wanted — it may be one of the two months the period spans,
    /// or the whole of it — so it is left for the user to keep or replace. One that does not
    /// overlap at all can contribute nothing to the new period and is emptied rather than left to
    /// be refused later.
    ///
    /// An unreadable bill passes `None`, which touches nothing and so empties both.
    fn set_bill_period(&mut self, ending: Option<civil::Date>) {
        self.bill_period_ending = ending;
        self.sessions2 = None;
        if !self.report_touches_period(Input::Sessions1) {
            self.sessions1 = None;
        }
    }

    /// Empties one picker.
    ///
    /// For [`Input::is_optional`], which is the second session report. A period covered by a single
    /// export needs no second file, so a file chosen before that was realised has to be retractable
    /// rather than merely replaceable.
    pub fn clear(&mut self, which: Input) {
        self.input_notes.retain(|(w, _)| *w != which);
        if which == Input::Bill {
            self.set_bill_period(None);
        }
        *self.slot(which) = None;
        if which.bears_on_session_reports() {
            self.recheck_session_reports();
        }
        self.clear_results();
    }

    fn slot(&mut self, which: Input) -> &mut Option<PathBuf> {
        match which {
            Input::Bill => &mut self.bill,
            Input::Meter => &mut self.meter,
            Input::Sessions1 => &mut self.sessions1,
            Input::Sessions2 => &mut self.sessions2,
        }
    }

    /// Rebuilds what is wrong with the two session-report pickers, from what they now hold.
    ///
    /// Both at once and from nothing, because one of the two things it checks is a *relation*
    /// between the slots: replacing the file in one can make the other redundant, or stop it being
    /// so, without that other having been touched.
    fn recheck_session_reports(&mut self) {
        self.input_notes.retain(|(w, _)| !w.is_session_report());
        for which in [Input::Sessions1, Input::Sessions2] {
            if let Some(note) = self.report_note(which) {
                self.input_notes.push((which, note));
            }
        }
        // The relations between the slots, and only where each file stands up on its own. A pair
        // holding a file that belongs to another period says nothing about the period in hand, and
        // a second note underneath the first one would only be that first fact restated.
        if self.input_notes.iter().any(|(w, _)| w.is_session_report()) {
            return;
        }
        // One note per pass, and in this order. All three describe the same pair from different
        // angles, and a row carrying two of them says one thing twice.
        if let Some((which, note)) = self
            .redundant_report()
            .or_else(|| self.unneeded_report())
            .or_else(|| self.uncovered_period())
        {
            self.input_notes.push((which, note));
        }
    }

    /// What is wrong with the report in one slot, judged on its own rather than against the other.
    ///
    /// Two things, in this order, because the second cannot be asked until the first has passed:
    /// whether the name states the dates it covers, and whether those dates reach the billing
    /// period at all. A report from another period contributes nothing to this one, and this note
    /// is about the one file just picked, where the coverage check is about the reports as a set.
    ///
    /// Silent until a bill has been read, since with no period there is nothing to hold it against.
    /// The pickers are shut until then, so nothing reaches this in that state by way of the app.
    fn report_note(&self, which: Input) -> Option<String> {
        let path = self.picked(which)?;
        // The parser's own wording, which states the form expected. Writing it out here again
        // would be a second copy to keep in step with it.
        if let Err(e) = parse_session_report_name(&file_stem(path)) {
            return Some(e.to_string());
        }
        if self.report_touches_period(which) {
            return None;
        }
        let (start, ending) = billing_period_dates(self.bill_period_ending?).ok()?;
        let report = report_coverage(path)?;
        Some(format!(
            "This report covers {} to {}, which is outside the billing period {start} to {ending}. \
             Choose a report that reaches into the period.",
            report.from, report.to,
        ))
    }

    /// Whether the report in one slot reaches into the billing period at all.
    ///
    /// Overlap, not coverage: one day in common is enough. It is the least a report has to do to
    /// belong to this period at all, and two callers ask it — [`Self::report_note`] of a file just
    /// chosen, and [`Self::set_bill_period`] of one already in hand.
    ///
    /// `false` with no bill read, no file in the slot, or a name that does not state its dates —
    /// the three ways the question cannot be answered. Both callers treat that as a reason to hold
    /// the file back, which is the right way to be wrong: what it costs is one pick, and what it
    /// avoids is a report from another period counted into this one's figures.
    fn report_touches_period(&self, which: Input) -> bool {
        let (Some(ending), Some(path)) = (self.bill_period_ending, self.picked(which)) else {
            return false;
        };
        let (Ok((period_start, period_ending)), Some(report)) =
            (billing_period_dates(ending), report_coverage(path))
        else {
            return false;
        };
        report.from <= period_ending && period_start <= report.to
    }

    /// The two reports do not reach across the billing period between them.
    ///
    /// Asked as soon as both slots hold a file: the second report is chosen to close a gap the first
    /// leaves, so whether it closed one is the thing the user wants to know while the dialog is
    /// still fresh in mind.
    ///
    /// Only with both slots filled. One report that does not cover the period is the ordinary state
    /// of a form still being filled in — it is what opens the second picker — and reporting it would
    /// put an error on screen for doing the expected thing.
    ///
    /// The library's own message, which names the period and what each file covers, so the app and
    /// the command line say the same thing about the same pair. Reported against the second slot,
    /// which is the one just chosen and the only one that can be emptied.
    fn uncovered_period(&self) -> Option<(Input, String)> {
        let ending = self.bill_period_ending?;
        let (one, two) = (self.sessions1.as_deref()?, self.sessions2.as_deref()?);
        check_reports_cover_period(ending, &[one, two])
            .err()
            .map(|e| (Input::Sessions2, e.to_string()))
    }

    /// Whether the report in one slot covers the whole billing period by itself, so that the other
    /// slot has nothing left to hold.
    ///
    /// `false` until a bill has been read: which period is being reconciled is the bill's to say,
    /// and without it no report can be shown to cover one. False is the permissive answer — it
    /// leaves the second picker open — which is the right way to be wrong when the question cannot
    /// be asked.
    ///
    /// The library answers it, from the file name alone, and the answer is the same one the run
    /// will get: this asks `check_reports_cover_period` with that one report, which is what the run
    /// asks with both of them.
    fn covers_period_alone(&self, which: Input) -> bool {
        let (Some(ending), Some(path)) = (self.bill_period_ending, self.picked(which)) else {
            return false;
        };
        check_reports_cover_period(ending, &[path]).is_ok()
    }

    /// The session report that has nothing to add, because the other one already covers the whole
    /// billing period.
    ///
    /// The picker for the second report is closed while the first covers the period, so the way a
    /// slot comes to hold a file it did not need is the other order: a second report chosen first,
    /// or a bill chosen last. The note is what says so once the order has played out.
    ///
    /// Reported against the slot with nothing to add, as [`Self::redundant_report`] is, and the
    /// remedy differs by slot for the same reason: only the second has a Clear button.
    fn unneeded_report(&self) -> Option<(Input, String)> {
        if self.sessions1.is_none() || self.sessions2.is_none() {
            return None;
        }
        let ending = self.bill_period_ending?;
        let unneeded = if self.covers_period_alone(Input::Sessions1) {
            Input::Sessions2
        } else if self.covers_period_alone(Input::Sessions2) {
            Input::Sessions1
        } else {
            return None;
        };
        let remedy = match unneeded {
            Input::Sessions1 => "Move that file to this slot and clear the second.",
            _ => "Press Clear to empty this slot.",
        };
        Some((
            unneeded,
            format!(
                "{} covers the whole billing period ending {ending} on its own, so a second report \
                 can only bring the same sessions in twice. {remedy}",
                unneeded.other_session_report().label(),
            ),
        ))
    }

    /// Why the picker for `which` is closed, in a few words for the row it sits on, or `None` while
    /// it is open.
    ///
    /// Only the two session reports are ever closed, and each is closed until the thing it is
    /// judged against is in hand: the first until the bill has said which period this is, the
    /// second until the first has shown that period is not covered already. A picker offered before
    /// then can only take a file it will have to refuse, and a refusal after the fact is a worse
    /// thing to hand a user than a button that will not press.
    ///
    /// The bill and the meter export are never closed. They are what everything else is judged
    /// against, so there is nothing to judge them against in turn.
    pub fn picker_closed(&self, which: Input) -> Option<&'static str> {
        match which {
            Input::Sessions1 if self.bill.is_none() => Some("Choose the bill first"),
            // A bill in hand that did not read. Its own row carries the reason; this row says only
            // that it is waiting on that one.
            Input::Sessions1 if self.bill_period_ending.is_none() => {
                Some("The bill above could not be read")
            }
            Input::Sessions2 if self.sessions1.is_none() => Some("Choose Session report 1 first"),
            Input::Sessions2 if self.covers_period_alone(Input::Sessions1) => {
                Some("Session report 1 covers the whole billing period")
            }
            _ => None,
        }
    }

    /// The session report whose dates the other one already covers, if there is one.
    ///
    /// Two reports over the same dates are not two months of a period; they are one month read
    /// twice. Every session in the narrower file is in the wider one, so the merge meets each of
    /// them a second time -- as a copy to drop where the two files agree, and as a
    /// `DuplicateId` to report where they do not. A pair of reports a portal revision apart
    /// disagrees on every row, and the figures then arrive under a list of anomalies as long as the
    /// report.
    ///
    /// Refused at the picker, and by the names alone. Whether the reports reach across the billing
    /// period is `api::pure::check_reports_cover_period`'s question and needs the bill, which is
    /// read only when the run starts; whether one file makes the other pointless needs neither.
    ///
    /// Reported against the redundant slot -- the covered one -- because that is the picker to
    /// change. Two ranges that are equal, which includes the same file chosen twice, are read as
    /// the first covering the second, so the answer is then the optional slot.
    fn redundant_report(&self) -> Option<(Input, String)> {
        let one = report_coverage(self.sessions1.as_deref()?)?;
        let two = report_coverage(self.sessions2.as_deref()?)?;
        let (covered, covering) = if one.from <= two.from && two.to <= one.to {
            (Input::Sessions2, &one)
        } else if two.from <= one.from && one.to <= two.to {
            (Input::Sessions1, &two)
        } else {
            return None;
        };
        let covered_dates = match covered {
            Input::Sessions1 => &one,
            _ => &two,
        };
        // The way out differs by slot, because only the optional one has a Clear button. Told to
        // clear a picker that offers no such control, a user is left looking for it.
        let remedy = match covered {
            Input::Sessions1 => {
                "Move that file to this slot and clear the second, or choose a report reaching \
                 dates it does not."
            }
            _ => "Choose a report reaching dates it does not, or press Clear to empty this slot.",
        };
        Some((
            covered,
            format!(
                "{} covers {} to {}, which already includes this file's {} to {}. {remedy}",
                covered.other_session_report().label(),
                covering.from,
                covering.to,
                covered_dates.from,
                covered_dates.to,
            ),
        ))
    }

    /// The file a picker holds.
    pub fn picked(&self, which: Input) -> Option<&Path> {
        match which {
            Input::Bill => self.bill.as_deref(),
            Input::Meter => self.meter.as_deref(),
            Input::Sessions1 => self.sessions1.as_deref(),
            Input::Sessions2 => self.sessions2.as_deref(),
        }
    }

    /// What was refused at one picker, if anything.
    pub fn note_for(&self, which: Input) -> Option<&str> {
        self.input_notes
            .iter()
            .find(|(w, _)| *w == which)
            .map(|(_, note)| note.as_str())
    }

    /// Whether the run may go ahead: the bill, the meter export, at least one session report, none
    /// of them refused, and a rates workbook.
    ///
    /// **The second session report is optional.** A billing period runs from the 24th to the 23rd,
    /// so it usually takes two monthly exports, but one file covering the whole period is as good
    /// as two. Whether the reports actually reach across the period is
    /// `api::pure::check_reports_cover_period`'s question, asked when the run starts — which also
    /// catches two files that leave a gap, as a count never could.
    pub fn can_run(&self, rates: &RatesWorkbook) -> bool {
        self.bill.is_some()
            && self.meter.is_some()
            && self.sessions1.is_some()
            && self.input_notes.is_empty()
            && rates.get().is_some()
    }

    /// The session reports chosen, in slot order, skipping an empty second slot.
    fn session_paths(&self) -> Vec<&Path> {
        [self.sessions1.as_ref(), self.sessions2.as_ref()]
            .into_iter()
            .flatten()
            .map(PathBuf::as_path)
            .collect()
    }

    /// Works out the surplus at the rates in `rates`, filling in either the outcome or the error.
    pub fn run(&mut self, rates: &RatesWorkbook) {
        self.clear_results();
        let (Some(bill), Some(meter), Some(rates)) =
            (self.bill.clone(), self.meter.clone(), rates.get())
        else {
            return;
        };
        let session_csvs = self.session_paths();
        if session_csvs.is_empty() {
            return;
        }

        match cost_recovery_surplus(&bill, &meter, &session_csvs, rates) {
            Ok(surplus) => {
                // The meter export has notes of its own, kept apart from the session side because
                // the two are checked against different things. Its log covers the billing period
                // priced, not the whole export; `MeterNotes::log` says why.
                //
                // A log that could not be written is reported above the report rather than in place
                // of it. Nothing else here touches the disk, so the figures below are the same
                // figures either way, and withholding them would report a failure that did not
                // happen.
                let meter_log = surplus.meter.log();
                self.log_failures = write_logs(surplus.notes.logs.iter().chain(&meter_log));
                self.outcome = Some(SurplusOutcome {
                    text: surplus.to_string(),
                    surplus,
                });
            }
            // The library's own message, unaltered, so that trouble reported from the app and
            // trouble reported from the command line can be compared word for word. It already
            // names the file it concerns.
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// The name a saved surplus report is offered under.
    pub fn default_save_name(&self) -> String {
        format!("EV_Cost_Recovery_Surplus_{}.report.md", self.period_label())
    }

    /// The name a saved detail report is offered under.
    pub fn default_detail_save_name(&self) -> String {
        format!("EV_Peak_Power_Detail_{}.report.md", self.period_label())
    }

    /// The billing period a saved file is named after, or the bill's own name before there is one.
    fn period_label(&self) -> String {
        match &self.outcome {
            Some(outcome) => outcome.surplus.recovery.billing_period_ending.to_string(),
            None => self.bill.as_deref().map(file_stem).unwrap_or_default(),
        }
    }

    /// Results describe the inputs that produced them, so changing an input drops them rather than
    /// leaving figures on screen that no longer answer what the pickers now say.
    fn clear_results(&mut self) {
        self.outcome = None;
        self.error = None;
        self.save_error = None;
        self.log_failures.clear();
    }
}

/// Writes each of a run's logs, collecting a message for every one that did not reach disk.
///
/// Per log rather than through `SessionNotes::write_logs`, which stops at the first failure and
/// returns an error naming no file. A run writes several logs into whatever folders its inputs came
/// from, and a message that cannot say which one is missing sends the reader to check all of them.
fn write_logs<'a>(logs: impl IntoIterator<Item = &'a SourceLog>) -> Vec<String> {
    logs.into_iter()
        .filter_map(|log| {
            let e = log.write().err()?;
            Some(format!(
                "The figures were worked out, but this run's log was not written.\n{}: {e}\nCheck \
                 that the folder can be written to and that the disk is not full.",
                log.path().display()
            ))
        })
        .collect()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// --------------------------------------------------------------------------------------------
// The reimbursement reconciliation

/// A finished reconciliation, with the report and the text of it side by side, as
/// [`SurplusOutcome`] holds a surplus.
pub struct ReimbursementOutcome {
    pub reconciliation: ReimbursementReconciliation,
    pub text: String,
}

/// The Reimbursement tab's form and what it produced.
///
/// Two of Evolute's documents for the one month, and one figure entered manually. The rates come
/// from the rates workbook, which the app holds for both tabs.
#[derive(Default)]
pub struct ReimbursementState {
    pub sessions: Option<PathBuf>,
    /// Evolute's Charges Report for the same month, which is where both of Evolute's own figures
    /// come from. In production it sits in the same folder as the session report.
    pub charges: Option<PathBuf>,
    /// What Evolute actually paid, in the text entered manually.
    ///
    /// Text rather than `f64` because a field being edited passes through states that are not
    /// numbers — `0.`, `-`, empty — and a numeric widget either rejects or rewrites them under the
    /// cursor. It is parsed when the run is asked for, which is also when a bad one can be reported.
    ///
    /// The one figure entered by hand. It is what was seen to arrive -- from a bank statement or a
    /// remittance advice -- and taking it off the Charges Report instead would make it agree with
    /// that report whatever Evolute had actually sent.
    pub reimbursed: String,
    pub outcome: Option<ReimbursementOutcome>,
    pub error: Option<String>,
    /// Why a report could not be saved, if a save was tried and failed. See
    /// [`SurplusState::save_error`].
    pub save_error: Option<String>,
    /// Why the run's logs could not be written, if they could not. See
    /// [`SurplusState::log_failures`].
    pub log_failures: Vec<String>,
    /// What the picked session report was refused for, shown against its picker rather than at the
    /// foot of the form.
    pub input_note: Option<String>,
}

impl ReimbursementState {
    /// Takes the session report.
    ///
    /// The name is checked here rather than at run time, for the reason
    /// [`SurplusState::select`] checks its own: the file name is the only thing that says what the
    /// report holds, and a name that does not say is worth catching while the dialog is still fresh
    /// in mind.
    ///
    /// **Only that the name reads.** Whether the report covers the month being reconciled is
    /// decided at run time, against the month the Charges Report names — which is not known yet
    /// here. Demanding a whole calendar month at this point would be a requirement the portal may
    /// never let a user satisfy: it exports any range.
    pub fn select(&mut self, path: PathBuf) {
        self.input_note = parse_session_report_name(&file_stem(&path))
            .err()
            .map(|e| e.to_string());
        self.sessions = Some(path);
        self.clear_results();
    }

    /// Takes Evolute's Charges Report.
    ///
    /// Not checked by name, unlike the session report. Its name carries the month too, but as a
    /// timestamp rather than a span, and the file itself states the period it covers -- so the
    /// month is read from inside it and cross-checked there, where a wrong file is caught however
    /// it happens to be named.
    pub fn select_charges(&mut self, path: PathBuf) {
        self.charges = Some(path);
        self.clear_results();
    }

    /// Whether the run may go ahead: both documents chosen, the session report not refused, and a
    /// rates workbook.
    pub fn can_run(&self, rates: &RatesWorkbook) -> bool {
        self.sessions.is_some()
            && self.charges.is_some()
            && self.input_note.is_none()
            && rates.get().is_some()
    }

    /// Marks that the amount was edited. The figures on screen describe what produced them, so they
    /// go rather than sit under inputs that have since moved.
    pub fn edited(&mut self) {
        self.clear_results();
    }

    /// One figure entered manually, named in the message when it cannot be read.
    ///
    /// # Errors
    ///
    /// A blank or unreadable figure, named, and one that is not a figure a sum of money can take.
    /// Blank is refused rather than read as zero: zero is a real answer -- Evolute paid nothing,
    /// nobody charged all month -- and must be entered to be meant.
    fn number(text: &str, what: &str) -> Result<f64, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err(format!("the {what} is blank"));
        }
        let value: f64 = text
            .parse()
            .map_err(|e| format!("cannot read \"{text}\" as the {what}: {e}"))?;
        checked_figure(value, what, text)
    }

    /// What Evolute actually paid; see [`Self::reimbursed`].
    fn amount(&self) -> Result<f64, String> {
        Self::number(&self.reimbursed, "reimbursement amount")
    }

    /// Reconciles the month at the rates in `rates`, filling in either the outcome or the error.
    pub fn run(&mut self, rates: &RatesWorkbook) {
        self.clear_results();
        let (Some(csv), Some(charges), Some(rates)) =
            (self.sessions.clone(), self.charges.clone(), rates.get())
        else {
            return;
        };
        let reimbursed = match self.amount() {
            Ok(amount) => amount,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        match reconcile_evolute_reimbursement(&[&csv], &charges, reimbursed, rates) {
            Ok(reconciliation) => {
                // The Charges Report has a log of its own. It carries no per-row anomalies -- it
                // is read all-or-nothing -- so its log holds only what leaves the figures standing.
                // Always `Some` on this path, which reads the file; the `None` case is a
                // reconciliation built from bare figures.
                //
                // As for a surplus, an unwritten log is reported alongside the reconciliation
                // rather than instead of it.
                let charges_log = reconciliation.charges.as_ref().map(|c| c.log());
                self.log_failures =
                    write_logs(reconciliation.notes.logs.iter().chain(&charges_log));
                self.outcome = Some(ReimbursementOutcome {
                    text: reconciliation.to_string(),
                    reconciliation,
                });
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// The name a saved reconciliation is offered under.
    pub fn default_save_name(&self) -> String {
        let label = match &self.outcome {
            Some(outcome) => outcome
                .reconciliation
                .month_start
                .strftime("%Y-%m")
                .to_string(),
            None => self.sessions.as_deref().map(file_stem).unwrap_or_default(),
        };
        format!("Evolute_Reimbursement_{label}.report.md")
    }

    fn clear_results(&mut self) {
        self.outcome = None;
        self.error = None;
        self.save_error = None;
        self.log_failures.clear();
    }
}

// --------------------------------------------------------------------------------------------
// The workbook conversions

/// One of the two file-to-file conversions, as the tab has to drive it.
///
/// A trait rather than two copies of [`ConversionSlot`], because everything the tab does with a
/// conversion — pick a file, work out what would be overwritten, ask, run, report — is the same
/// for both, and only the three lines here differ.
pub trait Conversion {
    /// What a finished conversion has to show for itself.
    type Outcome;

    /// Where the workbook goes. Asked before the conversion runs, to find out whether anything is
    /// already there.
    fn workbook(input: &Path) -> PathBuf;

    /// Converts, and returns either the outcome or a message to put in front of the user.
    fn run(input: &Path, on_existing: OnExistingWorkbook) -> Result<Self::Outcome, String>;
}

/// The Evolute session report conversion.
pub struct SessionConversion;

/// What one produced.
pub struct SessionWorkbook {
    pub workbook: PathBuf,
    /// The rows that needed a judgement call, as the command line prints them. Empty for a clean
    /// conversion.
    pub anomalies: Vec<String>,
    /// Why the run log could not be written, if it could not.
    ///
    /// Carried on the outcome rather than raised as the conversion's error, because by the time
    /// the log is written the workbook is already on disk. Failing the whole conversion over it
    /// would report that nothing was produced, when in fact the file the user asked for is there
    /// and only its log is missing. Both are said, in that order.
    pub log_failure: Option<String>,
}

impl Conversion for SessionConversion {
    type Outcome = SessionWorkbook;

    fn workbook(input: &Path) -> PathBuf {
        input.with_extension("xlsx")
    }

    fn run(input: &Path, on_existing: OnExistingWorkbook) -> Result<SessionWorkbook, String> {
        // No path prefix. Every way this can fail names the file already: the two refusals carry
        // it in `ConversionError`, a write failure carries the workbook's, and a read failure is a
        // `SessionCsvError` that names the CSV from a field of its own. Adding it here printed it
        // twice.
        let report = session_csv_to_xlsx(input, on_existing).map_err(|e| e.to_string())?;
        // See `Sessions::logs`.
        let log_failure = report.log.write().err().map(|e| {
            format!(
                "The workbook was written, but its run log was not.\n{}: {e}\nCheck that the \
                 folder can be written to and that the disk is not full.",
                report.log.path().display()
            )
        });
        Ok(SessionWorkbook {
            workbook: report.output_path,
            anomalies: report.anomalies.iter().map(|a| a.to_string()).collect(),
            log_failure,
        })
    }
}

/// The Green Button meter export conversion.
pub struct GbConversion;

/// What a Green Button conversion produced, and whether its log reached disk.
///
/// The meter-side counterpart of [`SessionWorkbook`], and it exists for the same reason: the
/// library's own report has nowhere to say that the log failed, because the library does not write
/// the log.
pub struct GbWorkbook {
    pub report: GbWriteReport,
    /// Why the run log could not be written, if it could not. See [`SessionWorkbook::log_failure`]
    /// for why this is carried rather than raised.
    pub log_failure: Option<String>,
}

impl Conversion for GbConversion {
    type Outcome = GbWorkbook;

    fn workbook(input: &Path) -> PathBuf {
        input.with_extension("xlsx")
    }

    fn run(input: &Path, on_existing: OnExistingWorkbook) -> Result<GbWorkbook, String> {
        // No path prefix, for the reason `SessionConversion::run` gives: a read failure here is a
        // `GbReadError`, which names the export itself.
        let report = gb_xml_to_xlsx(input, on_existing).map_err(|e| e.to_string())?;
        // As for a session report.
        let log_failure = report.log.write().err().map(|e| {
            format!(
                "The workbook was written, but its run log was not.\n{}: {e}\nCheck that the \
                 folder can be written to and that the disk is not full.",
                report.log.path().display()
            )
        });
        Ok(GbWorkbook {
            report,
            log_failure,
        })
    }
}

/// One conversion's file, its result and its one question.
pub struct ConversionSlot<C: Conversion> {
    pub input: Option<PathBuf>,
    pub outcome: Option<C::Outcome>,
    pub error: Option<String>,
    /// The workbook a conversion is about to replace, while the user is being asked about it.
    ///
    /// The api refuses an existing workbook unless told otherwise, and this is where being told
    /// otherwise comes from. Asked rather than refused outright: converting a report that has been
    /// corrected is an ordinary thing to want, and asked rather than done quietly because the
    /// workbook may have been reconciled against an invoice by hand.
    pub confirm_replace: Option<PathBuf>,
}

// Derived `Default` would demand `C::Outcome: Default`, which neither outcome is and neither needs
// to be.
impl<C: Conversion> Default for ConversionSlot<C> {
    fn default() -> Self {
        Self {
            input: None,
            outcome: None,
            error: None,
            confirm_replace: None,
        }
    }
}

impl<C: Conversion> ConversionSlot<C> {
    /// Takes the file to convert, and drops whatever the last one produced. A result left standing
    /// under a different file name is the one thing this tab must not show.
    pub fn select(&mut self, input: PathBuf) {
        self.input = Some(input);
        self.outcome = None;
        self.error = None;
        self.confirm_replace = None;
    }

    /// Converts, or asks first if that would replace a workbook already there.
    pub fn start(&mut self) {
        self.error = None;
        self.outcome = None;
        let Some(input) = self.input.as_deref() else {
            return;
        };
        let workbook = C::workbook(input);
        if workbook.exists() {
            self.confirm_replace = Some(workbook);
        } else {
            self.convert(OnExistingWorkbook::Refuse);
        }
    }

    /// Converts, having settled what is to happen to any workbook in the way.
    pub fn convert(&mut self, on_existing: OnExistingWorkbook) {
        self.confirm_replace = None;
        let Some(input) = self.input.clone() else {
            return;
        };
        match C::run(&input, on_existing) {
            Ok(outcome) => self.outcome = Some(outcome),
            Err(message) => self.error = Some(message),
        }
    }
}

/// Which of the two conversions the Convert tab is showing.
///
/// The session report first: it is the one converted every month, while a Green Button export is
/// fetched a few times a year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Which {
    #[default]
    Sessions,
    GreenButton,
}

/// The Convert tab: the two conversions, one shown at a time.
///
/// Both are held whichever is on screen, so switching between them loses neither the file chosen
/// nor the result of a conversion already run. They share nothing else — a Green Button export and
/// a session report have no bearing on each other.
#[derive(Default)]
pub struct ConvertState {
    pub which: Which,
    pub sessions: ConversionSlot<SessionConversion>,
    pub green_button: ConversionSlot<GbConversion>,
}

// --------------------------------------------------------------------------------------------
// Reading the report back

/// One titled part of the report, with whatever the report nests inside it.
pub struct Section {
    pub title: String,
    pub body: String,
    pub subsections: Vec<Section>,
}

/// Splits the report text into its sections, so each can be given its own collapsible heading.
///
/// The report is written to read as plain text, so a title there is a line underlined to its own
/// length: `=` for a section and `-` for one nested inside it. A table's `|:---|` separator row is
/// neither, so tables stay inside the section they belong to. The preamble before the first title
/// is dropped: what it states is shown above these sections as headings in their own right.
pub fn report_sections(text: &str) -> Vec<Section> {
    let lines: Vec<&str> = text.lines().collect();

    // The depth of the title starting at `i`, or `None` where no title starts there.
    //
    // Measured in characters, which is what `markdown::h1` and `h2` repeat the rule to. In bytes, a
    // title carrying any character outside ASCII — an em dash, an accented letter — would never
    // match its own underline, and the section would be swallowed into the one before it with
    // nothing said about it.
    let is_rule = |line: &str| {
        let mut chars = line.chars();
        match chars.next() {
            Some(first @ ('=' | '-')) => chars.all(|c| c == first),
            _ => false,
        }
    };
    let level = |i: usize| -> Option<u8> {
        let (title, rule) = (lines.get(i)?, lines.get(i + 1)?);
        // A rule is not a title, however well it matches the line under it. Without this, the
        // second of two consecutive rules of the same length reads as a title of its own, and its
        // body is `lines[start + 2..end]` with `end` one *below* `start + 2` -- which panics on a
        // backwards slice. The library never emits that shape, but this takes any `&str`.
        if title.trim().is_empty() || is_rule(title) {
            return None;
        }
        if rule.chars().count() != title.chars().count() {
            return None;
        }
        // The first character is taken before `all` is asked, which every character of an empty
        // line vacuously satisfies.
        match rule.chars().next()? {
            '=' if rule.chars().all(|c| c == '=') => Some(1),
            '-' if rule.chars().all(|c| c == '-') => Some(2),
            _ => None,
        }
    };

    let heads: Vec<(usize, u8)> = (0..lines.len())
        .filter_map(|i| level(i).map(|depth| (i, depth)))
        .collect();

    let mut roots: Vec<Section> = Vec::new();
    // The depth each root was found at. The sections do not carry it: it decides where the next
    // title goes and is of no use to a caller rendering them.
    let mut root_depths: Vec<u8> = Vec::new();

    for (k, &(start, depth)) in heads.iter().enumerate() {
        // A section runs to the next title of any depth, so one that nests others keeps only the
        // text above the first of them.
        let end = heads.get(k + 1).map_or(lines.len(), |&(next, _)| next);
        let section = Section {
            title: lines[start].to_owned(),
            body: lines[start + 2..end]
                .join("\n")
                .trim_matches('\n')
                .to_owned(),
            subsections: Vec::new(),
        };

        // A nested title belongs to the section above it, and only where there is one to belong
        // to. A report of nested titles alone yields them all as sections, rather than burying
        // each one in the one before it.
        if depth == 2 && root_depths.last() == Some(&1) {
            roots
                .last_mut()
                .expect("a depth is recorded for every root")
                .subsections
                .push(section);
        } else {
            roots.push(section);
            root_depths.push(depth);
        }
    }

    roots
}

#[cfg(test)]
mod test {
    use super::*;

    /// The five real inputs the tests below run the app against.
    ///
    /// Panics naming the file when one is absent, rather than returning `None` for the caller to
    /// skip on. The harness has no skip outcome, so a test that returns early reports `ok` and its
    /// message is swallowed unless someone passes `--nocapture`: a check nobody is running looks
    /// exactly like one that passed. The tests that call this are `#[ignore]`d for that reason, so
    /// the panic is only ever met by someone who asked for them by name.
    fn real_inputs() -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
        let paths = (
            PathBuf::from("data/hydro_bills/TH_5728140000_2026_06_29.pdf"),
            PathBuf::from("data/green_button/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML"),
            PathBuf::from("data/evolute/Session_Report_May_1_2026-May_31_2026-mock.csv"),
            PathBuf::from("data/evolute/Session_Report_June_1_2026-June_30_2026.csv"),
            PathBuf::from("data/EV_Cost_Recovery_Rates.xlsx"),
        );
        for path in [&paths.0, &paths.1, &paths.2, &paths.3, &paths.4] {
            assert!(
                path.exists(),
                "{} is not in this checkout; these inputs are real customer documents",
                path.display()
            );
        }
        paths
    }

    /// The four inputs `checked_figure` refuses, and the one it accepts.
    ///
    /// Its own doc names them; nothing asserted them. These are the messages a user meets when an
    /// amount will not read, and they are the app's alone -- the library never sees the text of a
    /// form field.
    #[test]
    fn a_figure_that_is_not_a_number_an_amount_could_be_is_refused() {
        for (value, text, expected) in [
            (f64::NAN, "nan", "not a finite number"),
            (f64::INFINITY, "inf", "not a finite number"),
            (f64::NEG_INFINITY, "-inf", "not a finite number"),
            (-0.07, "-0.07", "cannot be negative"),
        ] {
            let err = checked_figure(value, "reimbursement amount", text).expect_err(text);
            assert!(err.contains(expected), "{text}: {err}");
            assert!(err.contains("reimbursement amount"), "{text}: {err}");
        }

        // Zero is a figure: Evolute paid nothing.
        assert_eq!(checked_figure(0.0, "reimbursement amount", "0"), Ok(0.0));
        assert_eq!(
            checked_figure(118.09, "reimbursement amount", "118.09"),
            Ok(118.09)
        );
    }

    /// A rates workbook chosen, as both tabs need one before they can run. Never opened: the tests
    /// that use it stop short of a run.
    fn rates() -> RatesWorkbook {
        let mut rates = RatesWorkbook::default();
        rates.choose(PathBuf::from("/data/EV_Cost_Recovery_Rates.xlsx"));
        rates
    }

    /// Neither tab runs without a rates workbook, whatever else it has.
    #[test]
    fn neither_tab_runs_without_a_rates_workbook() {
        let none = RatesWorkbook::default();

        let mut surplus = with_bill_period();
        surplus.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        assert!(surplus.can_run(&rates()));
        assert!(!surplus.can_run(&none));

        let mut reimbursement = ReimbursementState::default();
        reimbursement.select(PathBuf::from(
            "/data/Session_Report_June_1_2026-June_30_2026.csv",
        ));
        reimbursement.select_charges(PathBuf::from("/data/XX-XX_Charges_June 2026-June 2026.csv"));
        assert!(reimbursement.can_run(&rates()));
        assert!(!reimbursement.can_run(&none));
    }

    /// One choice serves both tabs, and drops the figures on both: each was priced from the
    /// workbook it replaces.
    #[test]
    fn choosing_a_rates_workbook_drops_the_figures_on_both_tabs() {
        let mut app = AppState::default();
        app.surplus.error = Some("stale".to_owned());
        app.reimbursement.error = Some("stale".to_owned());

        // Nothing chosen, nothing dropped.
        app.settle_rates_workbook();
        assert!(app.surplus.error.is_some());

        app.rates_workbook
            .choose(PathBuf::from("/data/EV_Cost_Recovery_Rates.xlsx"));
        app.settle_rates_workbook();
        assert!(app.surplus.error.is_none());
        assert!(app.reimbursement.error.is_none());
        assert_eq!(
            app.rates_workbook.get(),
            Some(Path::new("/data/EV_Cost_Recovery_Rates.xlsx"))
        );

        // Settled once per choice: a later run's figures are not dropped by the same choice.
        app.surplus.error = Some("a later run".to_owned());
        app.settle_rates_workbook();
        assert!(app.surplus.error.is_some());
    }

    /// `WorkingDir::remember` keeps the folder of the file it is given, and ignores a bare name.
    ///
    /// The bare-name case is the whole reason the method is not a one-liner: a bare filename's
    /// parent is `""`, and storing that would send the next dialog nowhere in particular.
    #[test]
    fn the_working_directory_follows_the_last_file_chosen() {
        let mut dir = WorkingDir::default();

        dir.remember(Path::new("/data/evolute/June.csv"));
        assert_eq!(dir.0.as_deref(), Some(Path::new("/data/evolute")));

        // A bare name leaves the last real folder standing.
        dir.remember(Path::new("June.csv"));
        assert_eq!(dir.0.as_deref(), Some(Path::new("/data/evolute")));

        dir.remember(Path::new("/data/hydro_bills/June.pdf"));
        assert_eq!(dir.0.as_deref(), Some(Path::new("/data/hydro_bills")));
    }

    /// Choosing a file drops whatever the last one produced.
    ///
    /// A result left standing under a different file name is the one thing the Convert tab must
    /// not show, and `select` is where that is decided.
    #[test]
    fn choosing_a_file_to_convert_drops_the_last_result() {
        let mut slot: ConversionSlot<SessionConversion> = ConversionSlot::default();
        slot.select(PathBuf::from("/data/evolute/June.csv"));
        slot.error = Some("stale".to_owned());
        slot.confirm_replace = Some(PathBuf::from("/data/evolute/June.xlsx"));

        slot.select(PathBuf::from("/data/evolute/July.csv"));
        assert_eq!(
            slot.input.as_deref(),
            Some(Path::new("/data/evolute/July.csv"))
        );
        assert!(slot.error.is_none(), "{:?}", slot.error);
        assert!(slot.confirm_replace.is_none(), "{:?}", slot.confirm_replace);
    }

    /// A state with the four real files chosen, and the real rates workbook.
    fn ready() -> (SurplusState, RatesWorkbook) {
        let (bill, meter, csv1, csv2, workbook) = real_inputs();
        let mut state = SurplusState::default();
        state.select(Input::Bill, bill);
        state.select(Input::Meter, meter);
        state.select(Input::Sessions1, csv1);
        state.select(Input::Sessions2, csv2);
        let mut rates = RatesWorkbook::default();
        rates.choose(workbook);
        (state, rates)
    }

    /// The contract the whole app rests on: what it shows and saves is the library's own rendering.
    ///
    /// `cost_recovery_surplus_cli` prints `{surplus}` and the save button writes this text
    /// unaltered, so the two files are the same document as long as this holds. What it would catch
    /// is someone assembling the report here instead — a heading added, a figure reformatted — which
    /// is the way the two would come to differ.
    #[test]
    #[ignore = "runs the app against the real inputs under data/"]
    fn the_app_produces_the_same_report_as_the_command_line() {
        let (mut state, rates) = ready();
        state.run(&rates);
        assert!(state.error.is_none(), "{:?}", state.error);
        let outcome = state.outcome.as_ref().expect("the real inputs run");
        assert_eq!(outcome.text, outcome.surplus.to_string());
        assert!(outcome.text.contains("EV Cost Recovery Surplus"));
    }

    /// A run fills the detail tab from the same computation, so the intervals it shows are the ones
    /// the surplus was priced on rather than a second reading of the same files.
    #[test]
    #[ignore = "runs the app against the real inputs under data/"]
    fn a_run_leaves_the_three_priced_intervals_behind() {
        let (mut state, rates) = ready();
        state.run(&rates);
        let outcome = state.outcome.as_ref().expect("the real inputs run");
        let units: Vec<_> = outcome
            .surplus
            .delivery
            .priced_intervals
            .iter()
            .map(|p| p.unit)
            .collect();
        assert_eq!(units, ["kVA", "kW", "kW 7-7"]);
    }

    /// A session report's file name is the only thing that says which month it holds, so a name
    /// that says nothing is caught at the picker rather than after four files have been read.
    #[test]
    fn a_session_report_with_no_dates_in_its_name_is_refused_at_pick_time() {
        let mut state = SurplusState::default();
        state.select(Input::Sessions1, PathBuf::from("/data/sessions.csv"));
        assert!(state.note_for(Input::Sessions1).is_some());
        assert!(
            !state.can_run(&rates()),
            "a refused file must not let the run start"
        );

        // A good name clears it, and the file is still the one now held.
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        assert!(state.note_for(Input::Sessions1).is_none());
    }

    /// The bill and the meter export are not named by convention, so neither is judged by its name.
    ///
    /// The bill is judged all the same, by what is inside it: a file that is not a readable bill is
    /// reported at its own picker, and leaves no period for the session pickers to work against.
    /// The meter export is judged by neither, and is read only when the run starts.
    #[test]
    fn only_session_reports_are_judged_by_their_name() {
        let mut state = SurplusState::default();
        state.select(Input::Meter, PathBuf::from("/data/anything.xml"));
        assert!(state.note_for(Input::Meter).is_none());

        state.select(Input::Bill, PathBuf::from("/data/anything.pdf"));
        assert!(
            state.note_for(Input::Bill).is_some(),
            "a file that is not a bill has to be reported where it was chosen"
        );
        assert_eq!(state.bill_period_ending, None);
        assert_eq!(
            state.picker_closed(Input::Sessions1),
            Some("The bill above could not be read")
        );
        assert!(!state.can_run(&rates()));
    }

    /// Figures describe the inputs that produced them, so changing an input drops them.
    #[test]
    #[ignore = "runs the app against the real inputs under data/"]
    fn changing_an_input_discards_the_figures_it_produced() {
        let (mut state, rates) = ready();
        state.run(&rates);
        assert!(state.outcome.is_some(), "{:?}", state.error);

        state.select(
            Input::Sessions2,
            PathBuf::from("data/Session_Report_July_1_2026-July_31_2026-mock.csv"),
        );
        assert!(state.outcome.is_none(), "stale figures survived a new file");
    }

    /// Nothing runs until the bill, the meter export and one session report are in hand. The
    /// second session report is optional.
    ///
    /// A count is the wrong test: what matters is whether the reports reach across the period, and
    /// one file can do that as readily as two.
    #[test]
    fn the_run_needs_one_session_report_not_two() {
        let mut state = with_bill_period();
        assert!(!state.can_run(&rates()), "no session report yet");

        // May alone leaves 1 to 23 June uncovered, so the run will refuse it -- but the form is
        // filled in, and saying so is the run's job rather than the picker's.
        state.select(
            Input::Sessions1,
            PathBuf::from(sample_name(Input::Sessions1)),
        );
        assert!(
            state.can_run(&rates()),
            "the second session report is optional"
        );

        // And adding the report that closes the gap does not take the offer away.
        state.select(
            Input::Sessions2,
            PathBuf::from(sample_name(Input::Sessions2)),
        );
        assert!(state.note_for(Input::Sessions2).is_none());
        assert!(
            state.can_run(&rates()),
            "May and June cover the period between them"
        );
    }

    /// A heading is recognised whatever its characters are.
    ///
    /// The rule is a repeat of the title, so the two are compared as characters. Compared as bytes,
    /// a title carrying one character outside ASCII never matched its own underline, and the
    /// section was swallowed into the one above it — silently, since a report is read as text
    /// either way.
    #[test]
    fn a_heading_containing_a_non_ascii_character_opens_a_section() {
        // Built rather than typed, so the underline is exactly the title's character count, as
        // `markdown::h2` writes it.
        let title = "Période — notes";
        let report = format!(
            "First\n=====\n\none\n\n{title}\n{}\n\ntwo\n",
            "-".repeat(title.chars().count())
        );

        let sections = report_sections(&report);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "First");
        let nested: Vec<&str> = sections[0]
            .subsections
            .iter()
            .map(|s| s.title.as_str())
            .collect();
        assert_eq!(nested, [title]);
        assert_eq!(sections[0].subsections[0].body, "two");
    }

    fn sample_name(which: Input) -> &'static str {
        match which {
            Input::Bill => "/data/bill.pdf",
            Input::Meter => "/data/usage.xml",
            Input::Sessions1 => "/data/Session_Report_May_1_2026-May_31_2026.csv",
            Input::Sessions2 => "/data/Session_Report_June_1_2026-June_30_2026.csv",
        }
    }

    /// A pair where one file's dates already hold the other's, in the order the pickers allow it to
    /// be reached: a month in the first slot, and the range containing it in the second.
    ///
    /// June leaves 24 to 31 May uncovered, which is what opens the second slot; the May-June file
    /// then holds every session June does. The wider file is the one to keep, so the note lands on
    /// the first slot.
    #[test]
    fn a_second_report_that_already_holds_the_first_is_refused() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        assert!(
            state.can_run(&rates()),
            "one report, and the run may say the rest"
        );

        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        // The whole message, because `docs/app-cheat-sheet.md` quotes it.
        assert_eq!(
            state.note_for(Input::Sessions1),
            Some(
                "Session report 2 covers 2026-05-01 to 2026-06-30, which already includes this \
                 file's 2026-06-01 to 2026-06-30. Move that file to this slot and clear the \
                 second, or choose a report reaching dates it does not."
            ),
            "June is inside May-June"
        );
        assert!(state.note_for(Input::Sessions2).is_none());
        assert!(
            !state.can_run(&rates()),
            "a refused file must not let the run start"
        );

        // The same file in both slots is the same relation, not a special case. Equal ranges are
        // read as the first covering the second, so the note moves to the slot that can be emptied.
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        assert!(state.note_for(Input::Sessions2).is_some());
        assert!(state.note_for(Input::Sessions1).is_none());
    }

    /// Emptying the optional picker is how the refusal above is answered, and it must leave the
    /// form as it was before the file was chosen.
    #[test]
    fn clearing_the_second_report_lifts_what_it_was_refused_for() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );

        state.clear(Input::Sessions2);
        assert!(state.picked(Input::Sessions2).is_none());
        assert!(state.note_for(Input::Sessions2).is_none());
        assert!(state.can_run(&rates()), "the run is offered again");
    }

    /// The refusal is a relation between the two slots, so it has to be re-asked when either of
    /// them moves -- including the one that was not refused.
    #[test]
    fn narrowing_the_first_report_lifts_the_second_ones_refusal() {
        let mut state = SurplusState::default();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        assert!(state.note_for(Input::Sessions2).is_some());

        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-May_31_2026.csv"),
        );
        assert!(
            state.note_for(Input::Sessions2).is_none(),
            "May and June cover different dates"
        );

        // And the other way round: the wider file second makes the first the redundant one, so the
        // refusal is reported where the change has to be made.
        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_April_1_2026-June_30_2026.csv"),
        );
        assert!(state.note_for(Input::Sessions1).is_some());
        assert!(state.note_for(Input::Sessions2).is_none());
    }

    /// A state that has read a bill closing on 23 June 2026, so its period runs from 24 May, with
    /// the meter export chosen as well so that only the session slots decide whether it can run.
    ///
    /// The closing date is set rather than read out of a PDF: what these tests are about is the
    /// decision taken once it is known, and the repository does not carry a bill.
    fn with_bill_period() -> SurplusState {
        SurplusState {
            bill: Some(PathBuf::from(sample_name(Input::Bill))),
            meter: Some(PathBuf::from(sample_name(Input::Meter))),
            bill_period_ending: Some(civil::date(2026, 6, 23)),
            ..Default::default()
        }
    }

    /// The second picker is shut until there is a first report to measure, and shut again as soon
    /// as that report turns out to cover the period on its own.
    ///
    /// Open in either state, it takes a June report alongside a May-June one, and the run then
    /// meets every June session twice and reports an anomaly for each.
    #[test]
    fn the_second_picker_opens_only_when_the_first_report_leaves_something_uncovered() {
        let mut state = with_bill_period();
        assert_eq!(
            state.picker_closed(Input::Sessions2),
            Some("Choose Session report 1 first")
        );

        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        assert_eq!(
            state.picker_closed(Input::Sessions2),
            Some("Session report 1 covers the whole billing period"),
            "24 May to 23 June is inside May-June"
        );

        // A single month leaves the days before it uncovered, which is the case the second slot
        // exists for.
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        assert_eq!(state.picker_closed(Input::Sessions2), None);
    }

    /// Each session picker waits for what its file will be judged against, and says which it is
    /// waiting for. The bill and the meter export wait for nothing.
    #[test]
    fn each_session_picker_waits_for_what_it_is_judged_against() {
        let empty = SurplusState::default();
        assert_eq!(
            empty.picker_closed(Input::Sessions1),
            Some("Choose the bill first")
        );
        assert_eq!(
            empty.picker_closed(Input::Sessions2),
            Some("Choose Session report 1 first")
        );
        assert_eq!(empty.picker_closed(Input::Bill), None);
        assert_eq!(empty.picker_closed(Input::Meter), None);

        // A bill that read opens the first slot.
        assert_eq!(with_bill_period().picker_closed(Input::Sessions1), None);
    }

    /// Widening the first report can leave the second with nothing to add, and the note says so
    /// even where neither file contains the other.
    ///
    /// April-to-5-June closes the gap May alone leaves, so the pair is accepted; the May-June file
    /// then covers the period by itself, and containment cannot be what settles that -- it reaches
    /// neither back to April nor forward past 5 June.
    #[test]
    fn widening_the_first_report_leaves_the_second_with_nothing_to_add() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_April_1_2026-June_5_2026.csv"),
        );
        assert!(state.note_for(Input::Sessions2).is_none());
        assert!(state.can_run(&rates()));

        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        assert_eq!(
            state.note_for(Input::Sessions2),
            Some(
                "Session report 1 covers the whole billing period ending 2026-06-23 on its own, \
                 so a second report can only bring the same sessions in twice. Press Clear to \
                 empty this slot."
            )
        );
        assert!(
            !state.can_run(&rates()),
            "a refused file must not let the run start"
        );

        state.clear(Input::Sessions2);
        assert!(state.note_for(Input::Sessions2).is_none());
        assert!(state.can_run(&rates()), "the run is offered again");
    }

    /// A report from another period is refused where it was chosen, at either slot, and says which
    /// period it was held against.
    ///
    /// An August report against a June bill has not one day in common with the period. Taken
    /// quietly it produces a run whose EV figures are near enough zero, which is a number someone
    /// may go on to argue a bill from.
    #[test]
    fn a_report_that_does_not_reach_the_period_is_refused_at_its_own_picker() {
        let outside = "This report covers 2026-08-01 to 2026-09-04, which is outside the billing \
                       period 2026-05-24 to 2026-06-23. Choose a report that reaches into the \
                       period.";
        let august = "/data/Session_Report_August_1_2026-September_4_2026.csv";

        let mut state = with_bill_period();
        state.select(Input::Sessions1, PathBuf::from(august));
        assert_eq!(state.note_for(Input::Sessions1), Some(outside));
        assert!(
            !state.can_run(&rates()),
            "a refused file must not let the run start"
        );

        // The same of the second slot, and one day of overlap is enough to lift it: June reaches
        // the period by 23 days, May-to-25 by two.
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        state.select(Input::Sessions2, PathBuf::from(august));
        assert_eq!(state.note_for(Input::Sessions2), Some(outside));

        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_May_1_2026-May_25_2026.csv"),
        );
        assert_ne!(state.note_for(Input::Sessions2), Some(outside));
    }

    /// Two reports that both reach the period but leave a day of it between them are refused at the
    /// second picker, not at the run, and in the library's own words.
    #[test]
    fn a_pair_that_leaves_a_gap_is_refused_as_soon_as_the_second_is_chosen() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        assert!(
            state.can_run(&rates()),
            "one report short of the period is a form still being filled in, not an error"
        );

        // May to the 25th reaches two days into the period, so it is not a report from elsewhere;
        // what it leaves out is 26 to 31 May.
        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_May_1_2026-May_25_2026.csv"),
        );
        let note = state
            .note_for(Input::Sessions2)
            .expect("26 to 31 May is in neither file");
        assert!(
            note.starts_with(
                "the session reports do not cover the billing period 2026-05-24 to 2026-06-23:"
            ),
            "{note}"
        );
        assert!(
            note.contains("Session_Report_May_1_2026-May_25_2026.csv"),
            "{note}"
        );
        assert!(!state.can_run(&rates()));

        // The whole of May closes the gap, and the note goes with it.
        state.select(
            Input::Sessions2,
            PathBuf::from(sample_name(Input::Sessions1)),
        );
        assert!(state.note_for(Input::Sessions2).is_none());
        assert!(state.can_run(&rates()));
    }

    /// A new bill is a new period, so the pair chosen for the old one does not carry over: the
    /// second slot is emptied whatever it holds, and the first unless its report reaches into the
    /// new period.
    #[test]
    fn choosing_a_different_bill_empties_the_session_slots_it_invalidates() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        state.select(
            Input::Sessions2,
            PathBuf::from(sample_name(Input::Sessions1)),
        );

        // The period ending 23 July runs from 24 June, which the June report reaches into by a
        // week. It stays; May does not touch it and goes, and so would any second report.
        state.set_bill_period(Some(civil::date(2026, 7, 23)));
        assert_eq!(
            state.picked(Input::Sessions1),
            Some(Path::new(
                "/data/Session_Report_June_1_2026-June_30_2026.csv"
            ))
        );
        assert_eq!(state.picked(Input::Sessions2), None);

        // A period neither report touches empties the first slot too.
        state.set_bill_period(Some(civil::date(2026, 9, 23)));
        assert_eq!(state.picked(Input::Sessions1), None);
    }

    /// A bill that will not read leaves no period, so both session slots are emptied and the first
    /// picker shuts: there is nothing left for a session report to be judged against.
    #[test]
    fn a_bill_that_does_not_read_empties_both_session_slots() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        state.select(
            Input::Sessions2,
            PathBuf::from(sample_name(Input::Sessions1)),
        );

        state.select(Input::Bill, PathBuf::from("/data/not-a-bill.pdf"));
        assert_eq!(state.picked(Input::Sessions1), None);
        assert_eq!(state.picked(Input::Sessions2), None);
        assert!(state.note_for(Input::Bill).is_some());
        assert_eq!(
            state.picker_closed(Input::Sessions1),
            Some("The bill above could not be read")
        );
    }

    /// The wider report in the second slot makes the first the one with nothing to add, and the
    /// note goes to the picker that has to change.
    ///
    /// June-to-July reaches the period without covering it, which is what opens the second slot;
    /// the May-June file then covers the period alone, and neither file contains the other.
    #[test]
    fn the_note_names_whichever_slot_has_nothing_to_add() {
        let mut state = with_bill_period();
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-July_31_2026.csv"),
        );
        state.select(
            Input::Sessions2,
            PathBuf::from("/data/Session_Report_May_1_2026-June_30_2026.csv"),
        );
        assert_eq!(
            state.note_for(Input::Sessions1),
            Some(
                "Session report 2 covers the whole billing period ending 2026-06-23 on its own, \
                 so a second report can only bring the same sessions in twice. Move that file to \
                 this slot and clear the second."
            )
        );
        assert!(state.note_for(Input::Sessions2).is_none());
    }

    /// Only the second session report offers to empty itself. Emptying any of the other three can
    /// do nothing but disable the run.
    #[test]
    fn the_second_session_report_is_the_only_optional_picker() {
        let optional: Vec<Input> = Input::ALL.into_iter().filter(|i| i.is_optional()).collect();
        assert_eq!(optional, [Input::Sessions2]);
    }

    /// A session report is taken when its name reads, whatever range it states.
    ///
    /// The portal exports any range, so demanding a whole calendar month here would be a
    /// requirement a user may never be able to satisfy. Whether the reports cover the month being
    /// reconciled is settled at run time, against the month the Charges Report names.
    #[test]
    fn the_reimbursement_tab_takes_any_range_whose_name_reads() {
        let mut state = ReimbursementState::default();
        // Chosen first so that `can_run` below turns on the session report's name alone.
        state.select_charges(PathBuf::from("/data/XX-XX_Charges_June 2026-June 2026.csv"));

        for name in [
            "/data/Session_Report_June_1_2026-June_30_2026.csv",
            // Half a month, and a range straddling two: both are names the portal writes.
            "/data/Session_Report_June_1_2026-June_15_2026.csv",
            "/data/Session_Report_May_20_2026-July_5_2026.csv",
        ] {
            state.select(PathBuf::from(name));
            assert!(state.input_note.is_none(), "{name}: {:?}", state.input_note);
            assert!(state.can_run(&rates()), "{name}");
        }

        // A name that does not read at all is still refused, in the parser's own words.
        state.select(PathBuf::from("/data/sessions.csv"));
        assert!(
            state
                .input_note
                .as_deref()
                .is_some_and(|n| n.contains("is not a session report")),
            "{:?}",
            state.input_note
        );
        assert!(!state.can_run(&rates()), "a refused report does not run");
    }

    /// A blank figure is refused rather than read as zero. Zero is a real answer -- Evolute paid
    /// nothing, nobody charged all month -- and has to be entered to be meant.
    #[test]
    fn a_blank_figure_is_refused_and_an_entered_zero_is_not() {
        let mut state = ReimbursementState::default();
        assert!(state.amount().is_err(), "blank amount");

        state.reimbursed = "  ".to_owned();
        assert!(state.amount().is_err(), "whitespace only");

        state.reimbursed = "0".to_owned();
        assert_eq!(state.amount(), Ok(0.0));

        state.reimbursed = "118.09".to_owned();
        assert_eq!(state.amount(), Ok(118.09));

        state.reimbursed = "one hundred".to_owned();
        assert!(
            state
                .amount()
                .is_err_and(|e| e.contains("one hundred") && e.contains("reimbursement amount")),
            "the message quotes what was entered and names the field"
        );
    }

    /// The run needs both of Evolute's documents. Either one alone leaves a comparison with
    /// nothing on the other side of it.
    #[test]
    fn both_documents_are_needed_before_a_month_can_be_reconciled() {
        let mut state = ReimbursementState::default();
        assert!(!state.can_run(&rates()), "nothing chosen");

        state.select(PathBuf::from(
            "/data/Session_Report_June_1_2026-June_30_2026.csv",
        ));
        assert!(state.input_note.is_none(), "{:?}", state.input_note);
        assert!(!state.can_run(&rates()), "no Charges Report yet");

        state.select_charges(PathBuf::from("/data/XX-XX_Charges_June 2026-June 2026.csv"));
        assert!(state.can_run(&rates()), "both chosen");
    }

    /// Choosing either document drops whatever the last run produced, as editing a rate does.
    #[test]
    fn choosing_a_charges_report_drops_the_figures_on_screen() {
        let mut state = ReimbursementState {
            error: Some("stale".to_owned()),
            ..Default::default()
        };
        state.select_charges(PathBuf::from("/data/XX-XX_Charges_June 2026-June 2026.csv"));
        assert!(state.error.is_none());
    }

    /// Editing an input drops the figures it produced, as it does on the surplus tab: a
    /// reconciliation on screen describes the amount and rates that produced it.
    #[test]
    fn editing_the_reimbursement_tab_discards_its_figures() {
        let mut state = ReimbursementState {
            error: Some("stale".to_owned()),
            ..Default::default()
        };
        state.edited();
        assert!(state.error.is_none());
    }

    /// The report divides itself by underlined titles, and a table's separator row is not one.
    ///
    /// Nothing here is underlined with `=`, so every title stands on its own. That is what the
    /// whole report looked like to this function before top-level titles were recognised, and it
    /// still has to divide the same way.
    #[test]
    fn the_report_splits_on_its_own_titles_and_not_on_its_tables() {
        let text = "Preamble\n\nFirst\n-----\nbody one\n\n| a | b |\n|:---|:---|\n| 1 | 2 |\n\nSecond\n------\nbody two\n";
        let sections = report_sections(text);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "First");
        assert!(
            sections[0].body.contains("|:---|"),
            "the table was split off"
        );
        assert!(sections[0].subsections.is_empty());
        assert_eq!(sections[1].title, "Second");
        assert_eq!(sections[1].body, "body two");
    }

    /// A rule is never taken as a title, whatever sits under it.
    ///
    /// `pub fn` over any `&str`, and two consecutive rules of the same length make the second one
    /// look like a title whose body runs backwards -- `lines[start + 2..end]` with `end` below
    /// `start + 2`, which panics. The library never emits that shape; this takes whatever it is
    /// given.
    #[test]
    fn two_rules_in_a_row_do_not_make_the_second_one_a_title() {
        let sections = report_sections("abc\n===\n===\nbody\n");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "abc");
        assert_eq!(sections[0].body, "===\nbody");

        // The same for dashes, and for a rule with nothing above it at all.
        assert_eq!(report_sections("---\n---\n").len(), 0);
        assert_eq!(report_sections("===\n===\n===\n").len(), 0);
    }

    /// A `-` title under an `=` title is nested in it, and the `=` title keeps only what sits above
    /// the first one. This is the shape the surplus report has: three top-level sections after the
    /// summary, two of them with a nested one.
    #[test]
    fn a_dashed_title_nests_under_the_equals_title_above_it() {
        let text = "\
Top\n===\nabove the nested part\n\nNested\n------\nnested body\n\nNext Top\n========\nsecond body\n";
        let sections = report_sections(text);
        assert_eq!(sections.len(), 2, "two top-level sections");

        assert_eq!(sections[0].title, "Top");
        assert_eq!(sections[0].body, "above the nested part");
        assert_eq!(sections[0].subsections.len(), 1);
        assert_eq!(sections[0].subsections[0].title, "Nested");
        assert_eq!(sections[0].subsections[0].body, "nested body");

        // The nested section ends where the next top-level title begins, rather than swallowing
        // it. Swallowing it is exactly what buried three sections before this was fixed.
        assert_eq!(sections[1].title, "Next Top");
        assert_eq!(sections[1].body, "second body");
        assert!(sections[1].subsections.is_empty());
    }
}
