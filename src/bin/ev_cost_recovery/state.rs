//! The app's state, and every decision about it, with no egui in sight.
//!
//! The widget code above this is meant to be thin enough to check by eye; everything that could be
//! *wrong* rather than merely ugly — whether a file is the right sort of file, whether a rate is a
//! number, whether the run may go ahead at all, what a saved report is called — is decided here and
//! tested here.

use ev_cost_recovery::{
    io::{
        CostRecoveryRates, CostRecoverySurplus, GbWriteReport, OnExistingWorkbook,
        ReimbursementReconciliation, cost_recovery_surplus, gb_xml_to_xlsx,
        reconcile_evolute_reimbursement, session_csv_to_xlsx,
    },
    session::file_name::report_coverage,
};
use jiff::civil;
use std::path::{Path, PathBuf};

/// Which document is on screen.
///
/// One run produces the first two, so unlike `ev_peak_gui` there is no landing screen: the app
/// opens on the tab where the work is asked for. [`Tab::Detail`] holds nothing until that run has
/// succeeded.
///
/// [`Tab::Reimbursement`] answers a different question against a different counterparty over a
/// different calendar, and shares nothing with the other two but the folder the file dialogs open
/// in. It is a tab rather than a second program because it is the same month's charging seen from
/// the other side, and whoever asks one question asks the other in the same sitting.
///
/// [`Tab::Convert`] answers no question at all. It turns a source file into a workbook to be read
/// by eye, which is what the two command-line converters do, and it is here so that the app is the
/// only thing anyone has to open. It comes last because nothing else needs it: every figure this
/// app produces is taken from the source files directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
}

impl AppState {
    /// Whether the detail tab has anything to show. The tab is drawn greyed until it does, rather
    /// than opening on an empty page that says to go back.
    pub fn detail_ready(&self) -> bool {
        self.surplus.outcome.is_some()
    }
}

/// The folder the user is working in, shared by every file dialog in the app.
///
/// A month's bill, its meter export and the two session reports are ordinarily filed together, so
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
}

/// One cost-recovery schedule as the form holds it: an effective date and three rates still in the
/// text entered manually.
///
/// The rates are text rather than `f64` because a field being edited passes through states that are
/// not numbers — `0.`, `-`, empty — and a numeric widget either rejects or rewrites them under the
/// cursor. They are parsed when the run is asked for, which is also when a bad one can be reported
/// against the field it came from.
pub struct RatesForm {
    pub effective_date: civil::Date,
    pub on_peak: String,
    pub mid_peak: String,
    pub off_peak: String,
}

impl Default for RatesForm {
    fn default() -> Self {
        Self {
            // The first of the current month: a schedule ordinarily takes effect on one, and a
            // date the user must change is better than a date that looks deliberate.
            effective_date: today().first_of_month(),
            on_peak: String::new(),
            mid_peak: String::new(),
            off_peak: String::new(),
        }
    }
}

impl RatesForm {
    /// The three bands, in the order they are drawn and named.
    fn bands(&self) -> [(&'static str, &String); 3] {
        [
            ("on-peak", &self.on_peak),
            ("mid-peak", &self.mid_peak),
            ("off-peak", &self.off_peak),
        ]
    }

    /// The schedule this form describes.
    ///
    /// # Errors
    ///
    /// The first band that is not a number, named. A rate is refused rather than defaulted: a blank
    /// field read as zero would price that band's energy at nothing and still produce a report.
    pub fn parse(&self) -> Result<CostRecoveryRates, String> {
        let mut rates = [0.0; 3];
        for (i, (band, text)) in self.bands().into_iter().enumerate() {
            let text = text.trim();
            if text.is_empty() {
                return Err(format!("the {band} rate is blank"));
            }
            rates[i] = text
                .parse()
                .map_err(|e| format!("cannot read \"{text}\" as the {band} rate: {e}"))?;
        }
        Ok(CostRecoveryRates {
            effective_date: self.effective_date,
            on_peak: rates[0],
            mid_peak: rates[1],
            off_peak: rates[2],
        })
    }
}

/// Today, in the zone the rest of the app works in.
fn today() -> civil::Date {
    jiff::Zoned::now()
        .with_time_zone(ev_cost_recovery::time::time_zone())
        .date()
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
    pub rates_at_start: RatesForm,
    /// Whether a second schedule took effect during the period. The command line encodes this by
    /// how many arguments were given, which a form should not.
    pub rates_changed: bool,
    pub rates_at_end: RatesForm,
    pub outcome: Option<SurplusOutcome>,
    pub error: Option<String>,
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
    pub fn select(&mut self, which: Input, path: PathBuf) {
        self.input_notes.retain(|(w, _)| *w != which);
        if which.is_session_report() && report_coverage(&path).is_none() {
            self.input_notes.push((
                which,
                format!(
                    "\"{}\" does not say what it covers. Expected a name like \
                     Session_Report_June_1_2026-June_30_2026.csv.",
                    file_name(&path)
                ),
            ));
        }
        let slot = match which {
            Input::Bill => &mut self.bill,
            Input::Meter => &mut self.meter,
            Input::Sessions1 => &mut self.sessions1,
            Input::Sessions2 => &mut self.sessions2,
        };
        *slot = Some(path);
        self.clear_results();
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

    /// Whether the run may go ahead: all four files chosen, and none of them refused.
    pub fn can_run(&self) -> bool {
        Input::ALL.iter().all(|&w| self.picked(w).is_some()) && self.input_notes.is_empty()
    }

    /// Marks that a rate or the effective date was edited. The figures on screen describe the rates
    /// that produced them, so they go rather than sit under rates that have since changed.
    pub fn rates_edited(&mut self) {
        self.clear_results();
    }

    pub fn set_rates_changed(&mut self, changed: bool) {
        self.rates_changed = changed;
        self.clear_results();
    }

    /// The two schedules to run with.
    ///
    /// # Errors
    ///
    /// The first band that is not a number, named, and said to be the second schedule's when it is.
    fn schedules(&self) -> Result<(CostRecoveryRates, Option<CostRecoveryRates>), String> {
        let start = self.rates_at_start.parse()?;
        if !self.rates_changed {
            return Ok((start, None));
        }
        let end = self
            .rates_at_end
            .parse()
            .map_err(|e| format!("second schedule: {e}"))?;
        Ok((start, Some(end)))
    }

    /// Works out the surplus, filling in either the outcome or the error.
    pub fn run(&mut self) {
        self.clear_results();
        let (Some(bill), Some(meter), Some(csv1), Some(csv2)) = (
            self.bill.clone(),
            self.meter.clone(),
            self.sessions1.clone(),
            self.sessions2.clone(),
        ) else {
            return;
        };
        let (start, end) = match self.schedules() {
            Ok(pair) => pair,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        match cost_recovery_surplus(&bill, &meter, &csv1, &csv2, start, end) {
            Ok(surplus) => {
                // The app is the end of the line, so the run logs are written here. The library
                // returns what it found and writes nothing; see `SessionNotes::write_logs`. Written
                // before the report is shown, so a failure to write one is not buried under it.
                if let Err(e) = surplus.notes.write_logs() {
                    self.error = Some(format!("cannot write the run log: {e}"));
                    return;
                }
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
    }
}

/// A path's file name, for a message that has already said which picker it is about.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
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
/// Two of Evolute's documents for the one month, and one figure entered manually. One session
/// report rather
/// than two, because a reimbursement settles a calendar month and one Evolute report is one
/// calendar month. One schedule of rates rather than two, because our schedules change on the
/// first of a month, so a month is priced at one set of rates or it is not a month we can
/// reconcile.
#[derive(Default)]
pub struct ReimbursementState {
    pub sessions: Option<PathBuf>,
    /// Evolute's Charges Report for the same month, which is where both of Evolute's own figures
    /// come from. In production it sits in the same folder as the session report.
    pub charges: Option<PathBuf>,
    /// What Evolute actually paid, still in the text entered manually, for the reason the rates
    /// are text: a field being edited passes through states that are not numbers.
    ///
    /// The one figure still entered by hand. It is what was seen to arrive -- from a bank
    /// statement or a
    /// remittance advice -- and taking it off the Charges Report instead would make it agree with
    /// that report whatever Evolute had actually sent.
    pub reimbursed: String,
    pub rates: RatesForm,
    pub outcome: Option<ReimbursementOutcome>,
    pub error: Option<String>,
    /// What the picked session report was refused for, shown against its picker rather than at the
    /// foot of the form.
    pub input_note: Option<String>,
}

impl ReimbursementState {
    /// Takes the session report.
    ///
    /// The name is checked here rather than at run time, for the reason
    /// [`SurplusState::select`] checks its own: the file name is the only thing that says which
    /// month the report holds, and a name that does not say is worth catching while the dialog is
    /// still fresh in mind. A whole calendar month is wanted, not merely a dated span — half a
    /// month reconciled against a full month's payment is a variance that means nothing.
    pub fn select(&mut self, path: PathBuf) {
        self.input_note = match report_coverage(&path) {
            None => Some(format!(
                "\"{}\" does not say what it covers. Expected a name like \
                 Session_Report_June_1_2026-June_30_2026.csv.",
                file_name(&path)
            )),
            Some(c) if c.from != c.from.first_of_month() || c.to != c.from.last_of_month() => {
                Some(format!(
                    "\"{}\" covers {} to {}, which is not a whole calendar month. A \
                     reimbursement settles one month.",
                    file_name(&path),
                    c.from,
                    c.to
                ))
            }
            Some(_) => None,
        };
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

    /// Whether the run may go ahead: both documents chosen, and the session report not refused.
    pub fn can_run(&self) -> bool {
        self.sessions.is_some() && self.charges.is_some() && self.input_note.is_none()
    }

    /// Marks that a rate, the effective date or the amount was edited. The figures on screen
    /// describe what produced them, so they go rather than sit under inputs that have since moved.
    pub fn edited(&mut self) {
        self.clear_results();
    }

    /// One figure entered manually, named in the message when it cannot be read.
    ///
    /// # Errors
    ///
    /// A blank or unreadable figure, named. Blank is refused rather than read as zero: zero is a
    /// real answer -- Evolute paid nothing, nobody charged all month -- and must be entered to be
    /// meant.
    fn number(text: &str, what: &str) -> Result<f64, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err(format!("the {what} is blank"));
        }
        text.parse()
            .map_err(|e| format!("cannot read \"{text}\" as the {what}: {e}"))
    }

    /// What Evolute actually paid. The one figure still entered manually: it comes off a bank
    /// statement or a remittance advice, neither of which this app opens.
    fn amount(&self) -> Result<f64, String> {
        Self::number(&self.reimbursed, "reimbursement amount")
    }

    /// Reconciles the month, filling in either the outcome or the error.
    pub fn run(&mut self) {
        self.clear_results();
        let (Some(csv), Some(charges)) = (self.sessions.clone(), self.charges.clone()) else {
            return;
        };
        let reimbursed = match self.amount() {
            Ok(amount) => amount,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let rates = match self.rates.parse() {
            Ok(rates) => rates,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        match reconcile_evolute_reimbursement(&csv, &charges, reimbursed, rates) {
            Ok(reconciliation) => {
                // The app is the end of the line, so the run log is written here, as it is for a
                // surplus. See `SessionNotes::write_logs`.
                if let Err(e) = reconciliation.notes.write_logs() {
                    self.error = Some(format!("cannot write the run log: {e}"));
                    return;
                }
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
        let report = session_csv_to_xlsx(input, on_existing)
            .map_err(|e| format!("{}: {e}", input.display()))?;
        // The app is the end of the line, so the run log is written here. The library returns what
        // it found and writes nothing; see `Sessions::logs`.
        let log_failure = report.log.write().err().map(|e| {
            format!(
                "The workbook was written, but its run log was not.\n{}: {e}",
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

impl Conversion for GbConversion {
    type Outcome = GbWriteReport;

    fn workbook(input: &Path) -> PathBuf {
        input.with_extension("xlsx")
    }

    fn run(input: &Path, on_existing: OnExistingWorkbook) -> Result<GbWriteReport, String> {
        gb_xml_to_xlsx(input, on_existing).map_err(|e| format!("{}: {e}", input.display()))
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
    let level = |i: usize| -> Option<u8> {
        let (title, rule) = (lines.get(i)?, lines.get(i + 1)?);
        if title.trim().is_empty() || rule.len() != title.len() {
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
        // to. A report of nested titles alone -- which is how every report looked to this function
        // before top-level titles were recognised -- yields them all as sections, rather than
        // burying each one in the one before it.
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

    /// The real inputs, which the repository carries. `None` when they are not all present, so the
    /// suite still runs on a checkout without the data.
    fn real_inputs() -> Option<(PathBuf, PathBuf, PathBuf, PathBuf)> {
        let paths = (
            PathBuf::from("data/hydro_bills/TH_5728140000_2026_06_29.pdf"),
            PathBuf::from("data/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML"),
            PathBuf::from("data/Session_Report_May_1_2026-May_31_2026-mock.csv"),
            PathBuf::from("data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        let all = [&paths.0, &paths.1, &paths.2, &paths.3]
            .iter()
            .all(|p| p.exists());
        all.then_some(paths)
    }

    /// A schedule, as the form holds one once it has been filled in.
    fn form(effective: civil::Date, on: &str, mid: &str, off: &str) -> RatesForm {
        RatesForm {
            effective_date: effective,
            on_peak: on.to_owned(),
            mid_peak: mid.to_owned(),
            off_peak: off.to_owned(),
        }
    }

    /// A state with the four real files chosen and one schedule filled in.
    fn ready() -> Option<SurplusState> {
        let (bill, meter, csv1, csv2) = real_inputs()?;
        let mut state = SurplusState {
            rates_at_start: form(civil::date(2026, 5, 1), "0.1100", "0.0900", "0.0700"),
            ..Default::default()
        };
        state.select(Input::Bill, bill);
        state.select(Input::Meter, meter);
        state.select(Input::Sessions1, csv1);
        state.select(Input::Sessions2, csv2);
        Some(state)
    }

    /// The contract the whole app rests on: what it shows and saves is the library's own rendering.
    ///
    /// `cost_recovery_surplus_cli` prints `{surplus}` and the save button writes this text
    /// unaltered, so the two files are the same document as long as this holds. What it would catch
    /// is someone assembling the report here instead — a heading added, a figure reformatted — which
    /// is the way the two would come to differ.
    #[test]
    fn the_app_produces_the_same_report_as_the_command_line() {
        let Some(mut state) = ready() else {
            return;
        };
        state.run();
        assert!(state.error.is_none(), "{:?}", state.error);
        let outcome = state.outcome.as_ref().expect("the real inputs run");
        assert_eq!(outcome.text, outcome.surplus.to_string());
        assert!(outcome.text.contains("EV Cost Recovery Surplus"));
    }

    /// A run fills the detail tab from the same computation, so the intervals it shows are the ones
    /// the surplus was priced on rather than a second reading of the same files.
    #[test]
    fn a_run_leaves_the_three_priced_intervals_behind() {
        let Some(mut state) = ready() else {
            return;
        };
        state.run();
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
            !state.can_run(),
            "a refused file must not let the run start"
        );

        // A good name clears it, and the file is still the one now held.
        state.select(
            Input::Sessions1,
            PathBuf::from("/data/Session_Report_June_1_2026-June_30_2026.csv"),
        );
        assert!(state.note_for(Input::Sessions1).is_none());
    }

    /// The bill and the meter export are not named by convention, so nothing is claimed about them.
    #[test]
    fn only_session_reports_are_judged_by_their_name() {
        let mut state = SurplusState::default();
        state.select(Input::Bill, PathBuf::from("/data/anything.pdf"));
        state.select(Input::Meter, PathBuf::from("/data/anything.xml"));
        assert!(state.note_for(Input::Bill).is_none());
        assert!(state.note_for(Input::Meter).is_none());
    }

    /// The second schedule is sent only when it is asked for. The command line says this by how
    /// many arguments were given; here it is a checkbox, and an unticked one must not smuggle a
    /// half-filled form into the run.
    #[test]
    fn the_second_rate_schedule_is_sent_only_when_it_is_asked_for() {
        let mut state = SurplusState {
            rates_at_start: form(civil::date(2026, 5, 1), "0.11", "0.09", "0.07"),
            ..Default::default()
        };

        let (_, end) = state.schedules().expect("one schedule is filled in");
        assert!(end.is_none());

        state.set_rates_changed(true);
        state
            .schedules()
            .expect_err("the second schedule is blank and must be refused, not defaulted");

        state.rates_at_end = form(civil::date(2026, 6, 1), "0.12", "0.10", "0.08");
        let (_, end) = state.schedules().expect("both schedules are filled in");
        assert_eq!(
            end.expect("asked for").effective_date,
            civil::date(2026, 6, 1)
        );
    }

    /// A rate that is not a number names the band it belongs to, so the user knows which of the
    /// three fields to look at.
    #[test]
    fn a_rate_that_is_not_a_number_names_the_band_it_belongs_to() {
        let entered = form(civil::date(2026, 5, 1), "0.11", "eleven cents", "0.07");
        let e = entered.parse().expect_err("\"eleven cents\" is not a rate");
        assert!(e.contains("mid-peak"), "{e}");

        // Blank is refused too. Read as zero it would price that band's energy at nothing and still
        // produce a report.
        let blank = RatesForm {
            mid_peak: String::new(),
            ..entered
        };
        let e = blank.parse().expect_err("a blank rate is not a rate");
        assert!(e.contains("mid-peak"), "{e}");
    }

    /// Figures describe the inputs that produced them, so changing an input drops them.
    #[test]
    fn changing_an_input_discards_the_figures_it_produced() {
        let Some(mut state) = ready() else {
            return;
        };
        state.run();
        assert!(state.outcome.is_some());

        state.select(
            Input::Sessions2,
            PathBuf::from("data/Session_Report_July_1_2026-July_31_2026-mock.csv"),
        );
        assert!(state.outcome.is_none(), "stale figures survived a new file");

        state.run();
        state.rates_edited();
        assert!(state.outcome.is_none(), "stale figures survived a new rate");
    }

    /// Nothing runs until all four files are in hand.
    #[test]
    fn every_input_is_needed_before_the_run_is_offered() {
        let mut state = SurplusState::default();
        for (i, which) in Input::ALL.into_iter().enumerate() {
            assert!(!state.can_run(), "offered with {i} of 4 files");
            state.select(which, PathBuf::from(sample_name(which)));
        }
        assert!(state.can_run(), "all four are chosen");
    }

    fn sample_name(which: Input) -> &'static str {
        match which {
            Input::Bill => "/data/bill.pdf",
            Input::Meter => "/data/usage.xml",
            Input::Sessions1 => "/data/Session_Report_May_1_2026-May_31_2026.csv",
            Input::Sessions2 => "/data/Session_Report_June_1_2026-June_30_2026.csv",
        }
    }

    /// A session report is taken only when its name says it holds a whole calendar month. Both
    /// refusals are made at pick time, while the dialog is still fresh in mind.
    #[test]
    fn the_reimbursement_tab_refuses_a_report_that_is_not_a_whole_month() {
        let mut state = ReimbursementState::default();
        // Chosen first so that `can_run` below turns on the session report's name alone. Whether
        // the Charges Report is for the same month is settled from inside it, at run time.
        state.select_charges(PathBuf::from("/data/XX-XX_charges_2026-06-01.csv"));

        state.select(PathBuf::from(
            "/data/Session_Report_June_1_2026-June_30_2026.csv",
        ));
        assert!(state.input_note.is_none(), "{:?}", state.input_note);
        assert!(state.can_run());

        state.select(PathBuf::from(
            "/data/Session_Report_June_1_2026-June_15_2026.csv",
        ));
        assert!(
            state
                .input_note
                .as_deref()
                .is_some_and(|n| n.contains("not a whole calendar month")),
            "{:?}",
            state.input_note
        );
        assert!(!state.can_run(), "a refused report does not run");

        state.select(PathBuf::from("/data/sessions.csv"));
        assert!(
            state
                .input_note
                .as_deref()
                .is_some_and(|n| n.contains("does not say what it covers")),
            "{:?}",
            state.input_note
        );
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
        assert!(!state.can_run(), "nothing chosen");

        state.select(PathBuf::from(
            "/data/Session_Report_June_1_2026-June_30_2026.csv",
        ));
        assert!(state.input_note.is_none(), "{:?}", state.input_note);
        assert!(!state.can_run(), "no Charges Report yet");

        state.select_charges(PathBuf::from("/data/XX-XX_charges_2026-06-01.csv"));
        assert!(state.can_run(), "both chosen");
    }

    /// Choosing either document drops whatever the last run produced, as editing a rate does.
    #[test]
    fn choosing_a_charges_report_drops_the_figures_on_screen() {
        let mut state = ReimbursementState {
            error: Some("stale".to_owned()),
            ..Default::default()
        };
        state.select_charges(PathBuf::from("/data/XX-XX_charges_2026-06-01.csv"));
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
