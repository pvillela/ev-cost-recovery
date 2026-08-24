//! The app's state, and every decision about it, with no egui in sight.
//!
//! The widget code above this is meant to be thin enough to check by eye; everything that could be
//! *wrong* rather than merely ugly — whether a file is the right sort of file, whether a rate is a
//! number, whether the run may go ahead at all, what a saved report is called — is decided here and
//! tested here.

use ev_cost_recovery::io::{CostRecoveryRates, CostRecoverySurplus, cost_recovery_surplus};
use ev_cost_recovery::session::file_name::report_coverage;
use jiff::civil;
use std::path::{Path, PathBuf};

/// Which document is on screen.
///
/// One run produces both, so unlike `ev_peak_gui` there is no landing screen: the app opens on the
/// tab where the work is asked for. [`Tab::Detail`] holds nothing until that run has succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Surplus,
    Detail,
}

#[derive(Default)]
pub struct AppState {
    pub tab: Tab,
    pub surplus: SurplusState,
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
        match self {
            Self::Bill => ("Hydro bill", &["pdf"]),
            Self::Meter => ("Green Button export", &["xml"]),
            Self::Sessions1 | Self::Sessions2 => ("Session report", &["csv"]),
        }
    }

    /// Whether this input is an Evolute session report, whose file name states what it covers.
    fn is_session_report(self) -> bool {
        matches!(self, Self::Sessions1 | Self::Sessions2)
    }
}

/// One cost-recovery schedule as the form holds it: an effective date and three rates still in the
/// text the user typed.
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
// Reading the report back

/// One titled part of the report, as the report itself divides them.
pub struct Section {
    pub title: String,
    pub body: String,
}

/// Splits the report text into its sections, so each can be given its own collapsible heading.
///
/// The report is written to read as plain text, and a section title there is a line underlined by
/// dashes of its own length — which a table's `|:---|` separator is not, so tables inside a section
/// stay inside it. The preamble before the first such title is dropped: what it states is shown
/// above these sections as headings in their own right.
pub fn report_sections(text: &str) -> Vec<Section> {
    let lines: Vec<&str> = text.lines().collect();
    let underlined = |i: usize| {
        i + 1 < lines.len()
            && !lines[i].trim().is_empty()
            && !lines[i + 1].is_empty()
            && lines[i + 1].chars().all(|c| c == '-')
            && lines[i + 1].len() == lines[i].len()
    };

    let starts: Vec<usize> = (0..lines.len()).filter(|&i| underlined(i)).collect();
    starts
        .iter()
        .enumerate()
        .map(|(k, &start)| {
            let end = starts.get(k + 1).copied().unwrap_or(lines.len());
            Section {
                title: lines[start].to_owned(),
                body: lines[start + 2..end]
                    .join("\n")
                    .trim_matches('\n')
                    .to_owned(),
            }
        })
        .collect()
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

    /// A schedule, as the form holds one once it has been typed into.
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
        let typed = form(civil::date(2026, 5, 1), "0.11", "eleven cents", "0.07");
        let e = typed.parse().expect_err("\"eleven cents\" is not a rate");
        assert!(e.contains("mid-peak"), "{e}");

        // Blank is refused too. Read as zero it would price that band's energy at nothing and still
        // produce a report.
        let blank = RatesForm {
            mid_peak: String::new(),
            ..typed
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

    /// The report divides itself by underlined titles, and a table's separator row is not one.
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
        assert_eq!(sections[1].title, "Second");
        assert_eq!(sections[1].body, "body two");
    }
}
