//! Evolute's Charges Report: what Evolute billed a building's chargers for, over one month.
//!
//! A different document from the session report, and the reason reconciling is worth anything.
//! The session report says what each charging session drew; this says what Evolute's own billing
//! made of that, one row per breaker. Neither is computed from the other, so where they disagree
//! there is something to find out.
//!
//! Only two numbers are wanted from it — the month's kilowatt-hours and the month's dollars — and
//! both are column totals. The per-breaker detail is not carried: which breaker drew what is
//! Evolute's business, and nothing this crate computes is broken down that way.
//!
//! In production these files sit in the same folder as the session reports.

use crate::{
    log::{RunLog, SourceLog},
    markdown::{h2, wrap},
};
use jiff::civil::Date;
use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Columns that must be present for the file to mean anything.
///
/// The report carries more than these -- `Building`, `Address`, `Panel`, `Breaker`, `Name` -- and
/// they are not required, because nothing here reads them. A file missing one of these is not a
/// Charges Report.
const REQUIRED_HEADERS: &[&str] = &["Start_Date", "End_Date", "Bill_Status", "kWh", "Cost"];

/// How the report writes a date: `01-Jun-26`.
///
/// A two-digit year, which `%y` reads as 20xx. Nothing older than 2000 is going to appear in a
/// report about EV chargers.
const DATE_FORMAT: &str = "%d-%b-%y";

/// One month's Charges Report, summed.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargesReport {
    /// The file this was read from.
    pub path: PathBuf,
    /// Earliest `Start_Date` on any row.
    ///
    /// The envelope of [`Self::spans`], not a period the file declares. The report states no span
    /// of its own; it states one per row.
    pub from: Date,
    /// Latest `End_Date` on any row, inclusive.
    pub to: Date,
    /// Each distinct `(Start_Date, End_Date)` seen, and the rows carrying it, in row order.
    ///
    /// Kept per row rather than collapsed to one span, because rows are **not** required to agree.
    /// They did in every report seen so far — every row of every one reads the whole month — but
    /// what the two columns mean per row is an open question with Evolute (see
    /// `docs/Questions_for_Evolute.md`), and the reading that fits the data equally well is that
    /// each row states the span its breaker was subscribed for. Under that reading a subscriber
    /// who joins mid-month produces a row that differs from its neighbours, and refusing such a
    /// file would refuse a correct one.
    ///
    /// Whether a span is acceptable is not this reader's question: it depends on the month being
    /// reconciled, which comes from the session report. See
    /// [`charges_notes`](crate::api::pure::charges_notes).
    pub spans: BTreeMap<(Date, Date), Vec<usize>>,
    /// The `kWh` column, totalled over every row.
    pub total_kwh: f64,
    /// The `Cost` column, totalled over every row, in dollars.
    pub total_amount: f64,
    /// How many rows were summed.
    pub rows: usize,
    /// Every distinct `Bill_Status` seen, and how many rows carried it.
    ///
    /// Every row is counted towards the totals whatever its status. The statuses are reported
    /// rather than acted on because only `Issued` has ever been seen, and dropping a row on a
    /// status this code has never met would be a guess that quietly changes a figure. A reader who
    /// sees an unfamiliar status here knows to ask.
    ///
    /// Reaches the reader through [`ChargesNotes`], which the reconciliation prints and logs. It
    /// was collected and read by nothing for months, which made it a warning channel with no exit.
    pub statuses: BTreeMap<String, usize>,
}

/// What a reconciliation should say about the Charges Report it drew on.
///
/// The third of the three notes types, beside [`SessionNotes`](crate::session::SessionNotes) and
/// [`MeterNotes`](crate::green_button::MeterNotes), and kept apart from them for the same reason
/// they are kept apart from each other: a session anomaly is looked up in a session report by row,
/// a meter anomaly in an export by hour, and one of these in a Charges Report by row and column.
///
/// Nothing here stops a reconciliation. What stops one is an unreadable file, a cell that will not
/// parse, or a row whose span leaves the month — all errors, raised before this is built. These
/// are the findings that leave the figures standing and still want a reader.
#[derive(Debug, Clone, Default)]
pub struct ChargesNotes {
    /// The report the figures came from. `None` only for a default-constructed value.
    pub source: Option<PathBuf>,
    /// Every distinct `Bill_Status` seen, and how many rows carried it. Copied from
    /// [`ChargesReport::statuses`].
    pub statuses: BTreeMap<String, usize>,
    /// Spans that lie inside the month but do not cover the whole of it, and the rows carrying
    /// each.
    ///
    /// A breaker billed for part of the month only. Under the subscription reading of the two date
    /// columns this is what a mid-month join or leave looks like, and it is ordinary; under the
    /// other reading it should never occur. Reported either way, because the two readings are not
    /// yet told apart — see `docs/Questions_for_Evolute.md`.
    pub partial_spans: Vec<((Date, Date), Vec<usize>)>,
}

impl ChargesNotes {
    /// Whether there is nothing worth a reader's attention.
    ///
    /// A tally of nothing but `Issued` is not a finding: it is what every report seen so far says,
    /// and printing it as a warning would train the reader to skip the section that matters.
    pub fn is_clean(&self) -> bool {
        self.partial_spans.is_empty() && self.unfamiliar_statuses().next().is_none()
    }

    /// The statuses that are not `Issued`, with their counts.
    ///
    /// `Issued` is the only value ever seen. Anything else is a value this software has no rule
    /// for and still counted towards the totals, which is exactly what a reader needs to be told.
    pub fn unfamiliar_statuses(&self) -> impl Iterator<Item = (&String, &usize)> {
        self.statuses.iter().filter(|(name, _)| *name != "Issued")
    }

    /// One line per finding, for the run log.
    ///
    /// Shared with [`Self::to_markdown`] so the log and the report cannot say different things
    /// about one file. The full tally goes in whenever anything else does: a reader looking at an
    /// unfamiliar status wants to know how many rows carried the familiar one.
    fn findings(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for ((from, to), rows) in &self.partial_spans {
            out.push(format!(
                "{} row(s) are billed for {from} to {to} rather than the whole month: {}. Their \
                 kWh and dollars are counted in the totals in full.",
                rows.len(),
                row_list(rows)
            ));
        }
        for (status, count) in self.unfamiliar_statuses() {
            out.push(format!(
                "{count} row(s) carry Bill_Status `{status}`, which this software has no rule for. \
                 They are counted towards the totals like any other row, because dropping them \
                 would be a guess that quietly changes a figure."
            ));
        }
        if !out.is_empty() {
            out.push(format!("Bill_Status tally: {}.", self.status_tally()));
        }
        out
    }

    /// `Issued 44, Void 3`, in the order [`BTreeMap`] holds them.
    fn status_tally(&self) -> String {
        self.statuses
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The run log for the report these figures came from, unwritten.
    ///
    /// `None` when there is no file to sit beside, which only a default-constructed value has.
    ///
    /// Unlike the session and meter logs, this one has no per-row anomalies to carry: the Charges
    /// Report is read all-or-nothing, and everything that can go wrong with a row stops the
    /// reconciliation instead. What it records is the handful of findings that leave the figures
    /// standing.
    pub fn log(&self) -> Option<SourceLog> {
        let source = self.source.clone()?;
        let mut log = RunLog::new();
        for line in self.findings() {
            log.note(line);
        }
        Some(SourceLog {
            source,
            suffix: "charges.csv.read",
            operation: "Read Charges Report",
            log,
        })
    }

    /// Writes the run log beside the report, returning where it went.
    ///
    /// For a binary, as [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) and
    /// [`MeterNotes::write_log`](crate::green_button::MeterNotes::write_log) are.
    ///
    /// # Errors
    ///
    /// Whatever the write failed with, returned rather than swallowed.
    pub fn write_log(&self) -> Result<Option<PathBuf>, Box<dyn Error>> {
        self.log().map(|log| log.write()).transpose()
    }

    /// Renders the Charges Report side as markdown that also reads as plain text.
    ///
    /// Empty when there is nothing to say and no file to name, matching
    /// [`MeterNotes::to_markdown`](crate::green_button::MeterNotes::to_markdown).
    pub fn to_markdown(&self) -> String {
        if self.source.is_none() && self.is_clean() {
            return String::new();
        }
        let mut out: Vec<String> = Vec::new();
        out.push(h2("Charges Report"));
        out.push(String::new());
        match &self.source {
            Some(path) => out.push(format!("- {}", path.display())),
            None => out.push("- (figures not read from a file)".to_owned()),
        }
        out.push(String::new());

        if self.is_clean() {
            out.push(format!("Bill_Status tally: {}.", self.status_tally()));
            out.push(String::new());
            return out.join("\n");
        }
        for line in self.findings() {
            out.push(wrap(&format!("- {line}"), "  "));
        }
        out.push(String::new());
        out.join("\n")
    }
}

/// `rows 2, 3 and 4`, or `rows 2, 3, 4 and 5 more` past the fourth.
///
/// Capped for the reason the Green Button log caps its hours: a subscription change touches one
/// breaker, but a misread date column touches every row, and forty row numbers on one line is a
/// line nobody reads.
///
/// Shared with `ReimbursementError::ChargesReportRowsOutsideMonth`, which lists the same rows in
/// the refusal, so the note and the refusal cannot cap differently.
pub(crate) fn row_list(rows: &[usize]) -> String {
    const SHOWN: usize = 4;
    let shown: Vec<String> = rows.iter().take(SHOWN).map(usize::to_string).collect();
    match rows.len().saturating_sub(shown.len()) {
        0 => format!("rows {}", shown.join(", ")),
        more => format!("rows {}, and {more} more", shown.join(", ")),
    }
}

/// Why a Charges Report could not be read.
#[derive(Debug)]
pub enum ChargesReportError {
    /// The file could not be opened, or is not a readable CSV.
    Unreadable {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// A column this reader needs is not there.
    MissingColumn { path: PathBuf, name: &'static str },

    /// The file parsed but holds no rows. A month Evolute billed nothing for still has a row per
    /// breaker; an empty file is a truncated download, not a quiet month.
    NoRows { path: PathBuf },

    /// A cell could not be read as the kind of value its column holds.
    BadValue {
        path: PathBuf,
        row: usize,
        column: &'static str,
        value: String,
        cause: String,
    },
}

impl ChargesReportError {
    /// The report the failure is about. Every variant has one, since nothing here is checked
    /// before the file is opened.
    pub fn path(&self) -> &Path {
        match self {
            Self::Unreadable { path, .. }
            | Self::MissingColumn { path, .. }
            | Self::NoRows { path }
            | Self::BadValue { path, .. } => path,
        }
    }
}

impl fmt::Display for ChargesReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Every arm names the kind of document expected and then the file, from the variant's own
        // `path` field. The `csv` crate's errors do not carry the path and a per-row failure knows
        // only its row, so without this a caller is told what went wrong and left to guess where.
        // The kind is there because the path alone does not say which input slot rejected the
        // file: a session report picked for this slot fails on a missing column, and so does a
        // Charges Report picked for the session report slot. See `SessionCsvError`'s counterpart.
        write!(f, "Charges Report {}: ", self.path().display())?;
        match self {
            Self::Unreadable { cause, .. } => write!(f, "{cause}"),
            Self::MissingColumn { name, .. } => {
                write!(f, "missing required column `{name}`")
            }
            Self::NoRows { .. } => write!(
                f,
                "the file holds no rows; a Charges Report carries one row per breaker even in a \
                 month nothing was billed for"
            ),
            Self::BadValue {
                row,
                column,
                value,
                cause,
                ..
            } => write!(
                f,
                "row {row}, column `{column}`: cannot read {value:?}: {cause}"
            ),
        }
    }
}

impl Error for ChargesReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { cause, .. } => Some(cause.as_ref()),
            _ => None,
        }
    }
}

/// Reads a Charges Report and totals it.
///
/// Every row counts towards both totals, whatever its `Bill_Status`; see
/// [`ChargesReport::statuses`].
///
/// # Errors
///
/// See [`ChargesReportError`]. Nothing is totalled until every row has been read, so a file with a
/// bad cell in the middle of it produces an error rather than a partial sum.
pub fn charges_report(path: &Path) -> Result<ChargesReport, ChargesReportError> {
    let unreadable = |cause: ::csv::Error| ChargesReportError::Unreadable {
        path: path.to_path_buf(),
        cause: cause.into(),
    };

    let mut reader = ::csv::Reader::from_path(path).map_err(unreadable)?;

    let headers: BTreeMap<String, usize> = reader
        .headers()
        .map_err(unreadable)?
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_owned(), i))
        .collect();

    for required in REQUIRED_HEADERS {
        if !headers.contains_key(*required) {
            return Err(ChargesReportError::MissingColumn {
                path: path.to_path_buf(),
                name: required,
            });
        }
    }

    let field = |record: &::csv::StringRecord, name: &str| -> String {
        headers
            .get(name)
            .and_then(|&i| record.get(i))
            .unwrap_or("")
            .trim()
            .to_owned()
    };

    let mut spans: BTreeMap<(Date, Date), Vec<usize>> = BTreeMap::new();
    let mut total_kwh = 0.0;
    let mut total_amount = 0.0;
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = 0;

    for (i, record) in reader.records().enumerate() {
        // Row 1 is the header, so the first record is row 2 -- the number a spreadsheet shows.
        let row = i + 2;
        let record = record.map_err(unreadable)?;

        let from = parse_date(&field(&record, "Start_Date"), path, row, "Start_Date")?;
        let to = parse_date(&field(&record, "End_Date"), path, row, "End_Date")?;
        // Recorded, not judged. Rows differing from each other is not a fault here; a row whose
        // span leaves the month being reconciled is, and only the caller knows which month that
        // is. See the field's own note on `ChargesReport::spans`.
        spans.entry((from, to)).or_default().push(row);

        total_kwh += number(&field(&record, "kWh"), path, row, "kWh")?;
        total_amount += money(&field(&record, "Cost"), path, row, "Cost")?;
        *statuses.entry(field(&record, "Bill_Status")).or_default() += 1;
        rows += 1;
    }

    if spans.is_empty() {
        return Err(ChargesReportError::NoRows {
            path: path.to_path_buf(),
        });
    }
    // The envelope of the rows, not a span the file declares. `spans` is keyed on `(from, to)` and
    // ordered by it, so the earliest start is the first key; the latest end has to be searched
    // for, since a row starting later may end earlier.
    let from = spans.keys().next().expect("a non-empty map").0;
    let to = spans
        .keys()
        .map(|(_, to)| *to)
        .max()
        .expect("a non-empty map");

    Ok(ChargesReport {
        path: path.to_path_buf(),
        from,
        to,
        spans,
        total_kwh,
        total_amount,
        rows,
        statuses,
    })
}

fn parse_date(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<Date, ChargesReportError> {
    Date::strptime(DATE_FORMAT, s).map_err(|e| ChargesReportError::BadValue {
        path: path.to_path_buf(),
        row,
        column,
        value: s.to_owned(),
        cause: e.to_string(),
    })
}

fn number(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<f64, ChargesReportError> {
    s.parse().map_err(
        |e: std::num::ParseFloatError| ChargesReportError::BadValue {
            path: path.to_path_buf(),
            row,
            column,
            value: s.to_owned(),
            cause: e.to_string(),
        },
    )
}

/// A dollar amount as the report writes it: `$70.62`, or `-$1.00` for a credit.
///
/// The sign is outside the `$` in every negative figure seen, which is how spreadsheets export
/// currency. Thousands separators are stripped too, since a busy month could reach four figures.
fn money(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<f64, ChargesReportError> {
    let cleaned: String = s.chars().filter(|c| *c != '$' && *c != ',').collect();
    number(&cleaned, path, row, column)
}

// cargo test --lib -- charges_report::test
#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;
    use std::{env, fs, process};

    #[test]
    fn a_dollar_amount_is_read_whatever_dressing_it_arrives_in() {
        for (text, expected) in [
            ("$70.62", 70.62),
            ("$0.00", 0.0),
            ("-$1.25", -1.25),
            ("$1,234.50", 1234.5),
            ("246.26", 246.26),
        ] {
            assert_eq!(
                money(text, &fake_path(), 2, "Cost").unwrap(),
                expected,
                "{text}"
            );
        }
    }

    /// Stands in for the report a cell came from. These three helpers never open it; they carry it
    /// so the error they raise can name it.
    fn fake_path() -> PathBuf {
        PathBuf::from("Charges_Report_June.csv")
    }

    /// The message opens with the kind of document expected, ahead of the path. Picking a session
    /// report for this slot fails on a missing column, and so does picking a Charges Report for
    /// the session report slot; the path alone leaves the two indistinguishable. The paired test
    /// is `session::csv::test::a_message_opens_with_the_kind_of_document_expected`.
    #[test]
    fn a_message_opens_with_the_kind_of_document_expected() {
        let err = ChargesReportError::MissingColumn {
            path: PathBuf::from("Session_Report_June.csv"),
            name: "kWh",
        };
        assert_eq!(
            err.to_string(),
            "Charges Report Session_Report_June.csv: missing required column `kWh`"
        );
    }

    #[test]
    fn a_cell_that_is_not_a_number_names_its_file_its_row_and_its_column() {
        let err = money("n/a", &fake_path(), 7, "Cost").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Charges_Report_June.csv"), "{message}");
        assert!(message.contains("row 7"), "{message}");
        assert!(message.contains("Cost"), "{message}");
        assert!(message.contains("n/a"), "{message}");
    }

    /// The report's own date format, which is neither ISO nor anything jiff parses by default.
    #[test]
    fn the_report_s_dates_are_read_in_the_form_it_writes_them() {
        let p = fake_path();
        assert_eq!(
            parse_date("01-Jun-26", &p, 2, "Start_Date").unwrap(),
            june(1)
        );
        assert_eq!(
            parse_date("30-Jun-26", &p, 2, "End_Date").unwrap(),
            june(30)
        );
    }

    fn june(day: i8) -> Date {
        date(2026, 6, day)
    }

    /// Rows carrying different spans are read, not refused. Which spans are acceptable is settled
    /// against the month being reconciled, which this reader does not know — see
    /// `api::pure::charges_notes`.
    #[test]
    fn rows_that_disagree_about_their_span_are_all_read() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,kWh,Cost
01-Jun-26,30-Jun-26,Issued,10,$1.00
01-Jun-26,30-Jun-26,Issued,20,$2.00
15-Jun-26,30-Jun-26,Issued,5,$0.50
";
        let dir = temp_dir("mixed_spans");
        let path = dir.join("charges.csv");
        fs::write(&path, CSV).unwrap();

        let report = charges_report(&path).unwrap();
        assert_eq!(report.rows, 3);
        assert_eq!(report.total_kwh, 35.0);

        // Two distinct spans, each naming the rows that carried it. Rows are numbered as a
        // spreadsheet numbers them, so the first record is row 2.
        assert_eq!(report.spans.len(), 2);
        assert_eq!(report.spans[&(june(1), june(30))], vec![2, 3]);
        assert_eq!(report.spans[&(june(15), june(30))], vec![4]);

        // The envelope of the rows, not a span any single row states.
        assert_eq!(report.from, june(1));
        assert_eq!(report.to, june(30));

        fs::remove_dir_all(&dir).ok();
    }

    /// The envelope takes the latest end, which need not belong to the row with the latest start.
    #[test]
    fn the_envelope_takes_the_widest_pair_not_the_last_row() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,kWh,Cost
10-Jun-26,30-Jun-26,Issued,1,$0.10
01-Jun-26,20-Jun-26,Issued,1,$0.10
";
        let dir = temp_dir("envelope");
        let path = dir.join("charges.csv");
        fs::write(&path, CSV).unwrap();

        let report = charges_report(&path).unwrap();
        assert_eq!(
            report.from,
            june(1),
            "the earliest start, from the later row"
        );
        assert_eq!(report.to, june(30), "the latest end, from the earlier row");

        fs::remove_dir_all(&dir).ok();
    }

    /// A scratch directory of its own per test, since these run in parallel within one process.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("ev_charges_{}_{tag}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
