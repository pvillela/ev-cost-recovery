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
//! **The month comes from the file name**, by [`charges_month`], and every row is checked against
//! it before anything is summed. A file holding a row that reaches outside its own month is
//! refused whole. So a [`ChargesReport`] that exists is one whose contents agree with its own
//! name, and nothing downstream re-establishes that — see
//! [`charges_report`], and `api::pure::check_same_month` for the one question this cannot answer
//! alone.
//!
//! In production these files sit in the same folder as the session reports.

use crate::{
    csv::{CsvReadError, Document, Table},
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
/// A two-digit year. jiff reads 00-68 as 20xx and 69-99 as 19xx; every year that appears in a
/// report about EV chargers lands in the first range.
const DATE_FORMAT: &str = "%d-%b-%y";

/// What sits between the building and the timestamp in a Charges Report's file name.
const NAME_MARKER: &str = "_charges_";

/// The first day of the calendar month a Charges Report's file name says it covers.
///
/// Evolute names these `<building>_charges_<start timestamp>.csv`, as in
/// `XX-XX_charges_2026-06-01T00_00_00-04_00.csv`. The timestamp is the *start* of the period, so
/// the month is the one the date part falls in; the time of day and the UTC offset say nothing
/// about which month it is and are not read. The offset moves with the season — `-04_00` in June,
/// `-05_00` in January — which is why only the part before the `T` is parsed: everything after it
/// contains hyphens of its own.
///
/// `None` when the name is not of that form. Nothing else about the file is inspected — in
/// particular, not whether it exists.
///
/// A suffix after the timestamp is ignored, for the reason
/// [`report_coverage`](crate::session::report_coverage) ignores one: a `-bak` or a `-mock` is a
/// note to a person, and a file marked up by hand still says what it covers. Refusing them would
/// make the two documents behave differently for no reason.
///
/// The counterpart of [`report_month`](crate::session::report_month).
pub fn charges_month(path: &Path) -> Option<Date> {
    let stem = path.file_stem()?.to_str()?;
    let (_building, rest) = stem.split_once(NAME_MARKER)?;
    // The date, then whatever the rest of the timestamp and any hand-added suffix hold. Splitting
    // at the `T` is what separates them: the date's own separators are hyphens, and so is the
    // sign of the UTC offset, so no hyphen split can tell the two apart.
    let date = rest.split('T').next()?;
    let [year, month, day] = date.split('-').collect::<Vec<_>>()[..] else {
        return None;
    };
    // `Date::new` rather than a panicking constructor: a file name is input, and a name carrying
    // `2026-06-31` is one to reject rather than to crash on.
    Date::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
        .ok()
        .map(|d| d.first_of_month())
}

/// One month's Charges Report, summed.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargesReport {
    /// The file this was read from.
    pub path: PathBuf,
    /// The first day of the calendar month this report covers, read from [`Self::path`] by
    /// [`charges_month`].
    ///
    /// The month comes from the *name*, never from the rows. The rows are then checked against it,
    /// and a file holding a row outside it is refused whole — see
    /// [`ChargesReportError::RowsOutsideMonth`]. So a `ChargesReport` that exists is one whose rows
    /// agree with its own name, and nothing downstream has to re-establish that.
    pub month: Date,
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
    /// Every span is inside [`Self::month`], because a file holding one that is not was refused
    /// rather than read. What is *not* required is that they agree with each other — see
    /// [`Self::partial_spans`].
    pub spans: BTreeMap<(Date, Date), Vec<usize>>,
    /// Spans that lie inside the month but do not cover the whole of it, and the rows carrying
    /// each.
    ///
    /// A breaker billed for part of the month only. Under the subscription reading of the two date
    /// columns this is what a mid-month join or leave looks like, and it is ordinary; under the
    /// other reading it should never occur. Reported either way, because the two readings are not
    /// yet told apart — see `docs/Questions_for_Evolute.md`.
    ///
    /// A finding rather than a refusal, and the only one this document produces. A span that
    /// *leaves* the month is the refusal.
    pub partial_spans: Vec<((Date, Date), Vec<usize>)>,
    /// The `kWh` column, totalled over every row.
    pub total_kwh: f64,
    /// The `Cost` column, totalled over every row, in dollars.
    pub total_amount: f64,
    /// How many rows were summed.
    ///
    /// Every row counts towards both totals whatever its `Bill_Status`. That column is required —
    /// a file without it is not a Charges Report — and deliberately not read: only `Issued` has
    /// ever been seen, and this software has no rule that would treat any other value differently.
    /// Should one turn up and need one, it is written here then.
    pub rows: usize,
}

impl ChargesReport {
    /// Whether there is nothing worth a reader's attention.
    ///
    /// A report every row of which covers the whole month is what every one seen so far looks
    /// like, and saying so as a warning would train the reader to skip the section that matters.
    pub fn is_clean(&self) -> bool {
        self.partial_spans.is_empty()
    }

    /// One line per finding, for the run log.
    ///
    /// Shared with [`Self::to_markdown`] so the log and the report cannot say different things
    /// about one file.
    fn findings(&self) -> Vec<String> {
        self.partial_spans
            .iter()
            .map(|((from, to), rows)| {
                format!(
                    "{} row(s) are billed for {from} to {to} rather than the whole month: {}. \
                     Their kWh and dollars are counted in the totals in full.",
                    rows.len(),
                    row_list(rows)
                )
            })
            .collect()
    }

    /// The run log for this report, unwritten.
    ///
    /// Unlike the session and meter logs, this one has no per-row anomalies to carry: the Charges
    /// Report is read all-or-nothing, and everything that can go wrong with a row stops the read
    /// instead. What it records is the one finding that leaves the figures standing.
    pub fn log(&self) -> SourceLog {
        let mut log = RunLog::new();
        for line in self.findings() {
            log.note(line);
        }
        SourceLog {
            source: self.path.clone(),
            suffix: "charges.csv.read",
            operation: "Read Charges Report",
            log,
        }
    }

    /// Writes the run log beside the report, returning where it went.
    ///
    /// For a binary, as [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) and
    /// [`MeterNotes::write_log`](crate::green_button::MeterNotes::write_log) are.
    ///
    /// # Errors
    ///
    /// Whatever the write failed with, returned rather than swallowed.
    pub fn write_log(&self) -> Result<PathBuf, Box<dyn Error>> {
        self.log().write()
    }

    /// Renders the Charges Report side as markdown that also reads as plain text.
    pub fn to_markdown(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        out.push(h2("Charges Report"));
        out.push(String::new());
        out.push(format!("- {}", self.path.display()));
        out.push(String::new());

        if !self.is_clean() {
            for line in self.findings() {
                out.push(wrap(&format!("- {line}"), "  "));
            }
            out.push(String::new());
        }
        out.join("\n")
    }
}

/// `rows 2, 3, 4`, or `rows 2, 3, 4, 5, and 12 more` past the fourth.
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
///
/// Everything a column-by-name reader can fail at is [`Self::Csv`], which is where the file, the
/// row and the column live and where the message is written. Only what is particular to *this*
/// document is a variant here.
#[derive(Debug)]
pub enum ChargesReportError {
    /// The file could not be opened, is not a readable CSV, is missing a column this reader needs,
    /// or holds a cell that will not parse.
    Csv(CsvReadError),

    /// The file parsed but holds no rows. A month Evolute billed nothing for still has a row per
    /// breaker; an empty file is a truncated download, not a quiet month.
    NoRows { path: PathBuf },

    /// The file name does not state the month the report covers, so there is nothing to check its
    /// rows against.
    ///
    /// Raised before the file is opened, and the one variant here that is. See [`charges_month`]
    /// for the form expected. A name that says nothing is not distinguishable from a name that
    /// says the wrong thing, and catching the wrong file is what reading the name is for.
    UndatedReport { path: PathBuf },

    /// A row is billed for dates that are not entirely inside the month the file name states.
    ///
    /// **The whole file is refused**, not the offending rows: nothing is summed and no
    /// [`ChargesReport`] is produced. Dropping rows would quietly change both totals, and keeping
    /// them would reconcile a row from outside the month against a month's worth of sessions.
    ///
    /// A span *inside* the month but short of it is not this — it is
    /// [`ChargesReport::partial_spans`], a finding that leaves the figures standing. Whether a row
    /// can reach outside the month at all is the open question in `docs/Questions_for_Evolute.md`;
    /// until it is answered, a figure nobody can account for is worse than a file that will not
    /// read.
    RowsOutsideMonth {
        path: PathBuf,
        /// Each offending span and the rows carrying it, in span order.
        spans: Vec<((Date, Date), Vec<usize>)>,
        month_start: Date,
        month_end: Date,
    },
}

impl From<CsvReadError> for ChargesReportError {
    fn from(cause: CsvReadError) -> Self {
        Self::Csv(cause)
    }
}

impl ChargesReportError {
    /// The report the failure is about. Every variant has one, since nothing here is checked
    /// before the file is opened.
    pub fn path(&self) -> &Path {
        match self {
            Self::Csv(cause) => cause.path(),
            Self::NoRows { path }
            | Self::UndatedReport { path }
            | Self::RowsOutsideMonth { path, .. } => path,
        }
    }
}

impl fmt::Display for ChargesReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Already written in full, document and path included. Adding either here would print
            // it twice.
            Self::Csv(cause) => cause.fmt(f),
            // The same opening the shared errors use, from the same `Document`, so a reader cannot
            // tell which half of this type refused the file.
            Self::NoRows { path } => write!(
                f,
                "{} {}: the file holds no rows; a Charges Report carries one row per breaker even \
                 in a month nothing was billed for",
                Document::ChargesReport,
                path.display()
            ),
            Self::UndatedReport { path } => write!(
                f,
                "{} {}: the file name does not say what month the report covers; expected a name \
                 of the form XX-XX_charges_2026-06-01T00_00_00-04_00.csv",
                Document::ChargesReport,
                path.display()
            ),
            Self::RowsOutsideMonth {
                path,
                spans,
                month_start,
                month_end,
            } => {
                write!(
                    f,
                    "{} {}: the file name says the report covers {month_start} to {month_end}, \
                     but these rows are billed for dates outside it",
                    Document::ChargesReport,
                    path.display()
                )?;
                for ((from, to), rows) in spans {
                    write!(f, "\n  {from} to {to}: {}", row_list(rows))?;
                }
                Ok(())
            }
        }
    }
}

impl Error for ChargesReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Csv(cause) => Some(cause),
            Self::NoRows { .. } | Self::UndatedReport { .. } | Self::RowsOutsideMonth { .. } => {
                None
            }
        }
    }
}

/// Reads a Charges Report and totals it.
///
/// The month comes from the file name, by [`charges_month`], and every row is checked against it.
/// A row reaching outside that month refuses the whole file. So a `ChargesReport` handed back is
/// one whose contents agree with its own name, and no caller has to establish that again.
///
/// What this does **not** check is whether that month is the one anybody wanted. Comparing it to
/// the session report's month needs both documents, and is done where both are in hand — see
/// `api::io::reconcile_evolute_reimbursement`.
///
/// Every row counts towards both totals, whatever its `Bill_Status`; see [`ChargesReport::rows`].
///
/// # Errors
///
/// See [`ChargesReportError`]. Nothing is totalled until every row has been read, so a file with a
/// bad cell in the middle of it produces an error rather than a partial sum.
pub fn charges_report(path: &Path) -> Result<ChargesReport, ChargesReportError> {
    // Before the file is opened: a name that does not say what month it is cannot be checked
    // against anything, and reading the rows first would only delay the same refusal.
    let month = charges_month(path).ok_or_else(|| ChargesReportError::UndatedReport {
        path: path.to_path_buf(),
    })?;
    let month_end = month.last_of_month();

    let table = Table::read(path, Document::ChargesReport, REQUIRED_HEADERS)?;

    let mut spans: BTreeMap<(Date, Date), Vec<usize>> = BTreeMap::new();
    let mut total_kwh = 0.0;
    let mut total_amount = 0.0;
    let mut rows = 0;

    for i in 0..table.record_count() {
        let row = Table::row_number(i);

        let from = parse_date(table.cell(i, "Start_Date"), path, row, "Start_Date")?;
        let to = parse_date(table.cell(i, "End_Date"), path, row, "End_Date")?;
        // Collected, then judged in one pass below, so the refusal can name every offending span
        // rather than stopping at the first. Rows differing from *each other* is not a fault; a
        // row leaving the month is.
        spans.entry((from, to)).or_default().push(row);

        total_kwh += number(table.cell(i, "kWh"), path, row, "kWh")?;
        total_amount += money(table.cell(i, "Cost"), path, row, "Cost")?;
        rows += 1;
    }

    if spans.is_empty() {
        return Err(ChargesReportError::NoRows {
            path: path.to_path_buf(),
        });
    }

    let outside: Vec<((Date, Date), Vec<usize>)> = spans
        .iter()
        .filter(|((from, to), _)| *from < month || *to > month_end)
        .map(|(span, rows)| (*span, rows.clone()))
        .collect();
    if !outside.is_empty() {
        return Err(ChargesReportError::RowsOutsideMonth {
            path: path.to_path_buf(),
            spans: outside,
            month_start: month,
            month_end,
        });
    }

    // Everything left lies within the month. A span shorter than the whole of it is the one thing
    // worth saying, and it stops nothing.
    let partial_spans: Vec<((Date, Date), Vec<usize>)> = spans
        .iter()
        .filter(|((from, to), _)| *from != month || *to != month_end)
        .map(|(span, rows)| (*span, rows.clone()))
        .collect();

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
        month,
        from,
        to,
        spans,
        partial_spans,
        total_kwh,
        total_amount,
        rows,
    })
}

/// A cell of this report that will not parse.
///
/// The document is pinned here, once, rather than at each of the parsers below.
fn bad_value(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
    cause: impl fmt::Display,
) -> CsvReadError {
    CsvReadError::bad_value(Document::ChargesReport, path, row, column, s, cause)
}

fn parse_date(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<Date, CsvReadError> {
    Date::strptime(DATE_FORMAT, s).map_err(|e| bad_value(s, path, row, column, e))
}

fn number(s: &str, path: &Path, row: usize, column: &'static str) -> Result<f64, CsvReadError> {
    parse_number(s, s, path, row, column)
}

/// `text` is what is parsed; `cell` is what the report wrote, and is what any error quotes.
///
/// The two differ for [`money`], which takes the `$` off before parsing. An error quoting `1,2`
/// for a cell that reads `$1,2` sends a reader looking for something the file does not contain,
/// and the quoted value is there to be looked for.
fn parse_number(
    text: &str,
    cell: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<f64, CsvReadError> {
    // Thousands separators are stripped, since a busy month's kWh easily reaches four figures --
    // but only where they group digits in threes. Stripping every comma reads `1,2` as 12, which
    // turns a malformed cell into a plausible figure in a reader whose posture is an error rather
    // than a partial sum.
    if !commas_group_thousands(text.trim()) {
        return Err(bad_value(
            cell,
            path,
            row,
            column,
            "a comma here does not separate thousands",
        ));
    }
    let cleaned: String = text.chars().filter(|c| *c != ',').collect();
    cleaned
        .parse()
        .map_err(|e: std::num::ParseFloatError| bad_value(cell, path, row, column, e))
}

/// Whether every comma in `text` separates a group of three digits.
///
/// `1,234.5` and `12,345,678` pass; `1,2`, `,123` and `1,2345` do not, and neither does a comma
/// after the decimal point. A number with no comma in it passes untouched.
///
/// The same rule is written out in `hydro_bill::bill_pdf::commas_group_thousands`, over the same
/// question about a different document. Change one and change the other.
fn commas_group_thousands(text: &str) -> bool {
    if !text.contains(',') {
        return true;
    }
    let (integer, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.contains(',') {
        return false;
    }
    let digits = integer
        .strip_prefix('-')
        .or_else(|| integer.strip_prefix('+'))
        .unwrap_or(integer);
    let mut groups = digits.split(',');
    let leading = groups.next().unwrap_or("");
    (1..=3).contains(&leading.len()) && groups.all(|g| g.len() == 3)
}

/// A dollar amount as the report writes it: `$70.62`, or `-$1.00` for a credit.
///
/// The sign is outside the `$` in every negative figure seen, which is how spreadsheets export
/// currency. Thousands separators are handled by `number`.
fn money(s: &str, path: &Path, row: usize, column: &'static str) -> Result<f64, CsvReadError> {
    let cleaned: String = s.chars().filter(|c| *c != '$').collect();
    parse_number(&cleaned, s, path, row, column)
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
    ///
    /// Through `charges_report`, not over a hand-built error: what is at stake is the
    /// [`Document`] this reader hands the shared reader, and only the real call passes it.
    #[test]
    fn a_message_opens_with_the_kind_of_document_expected() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,Cost
01-Jun-26,30-Jun-26,Issued,$1.00
";
        let dir = temp_dir("document_kind");
        let path = june_path(&dir);
        fs::write(&path, CSV).unwrap();

        let err = charges_report(&path).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "Charges Report {}: missing required column `kWh`",
                path.display()
            )
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// A file named as a session report is refused before it is opened, and in the same voice.
    ///
    /// The opening comes from the slot the file was offered to, not from what the file is called —
    /// which is the point the missing-column test above used to carry on its own, before the name
    /// became something this reader reads. Handing a session report to this slot now fails here
    /// rather than on a column, and has to say `Charges Report` just the same.
    #[test]
    fn a_file_not_named_as_a_charges_report_is_refused() {
        let dir = temp_dir("wrong_name");
        let path = dir.join("Session_Report_June_1_2026-June_30_2026.csv");
        // Contents that would read perfectly well, so the name is the only thing refusing it.
        fs::write(
            &path,
            "Start_Date,End_Date,Bill_Status,kWh,Cost\n01-Jun-26,30-Jun-26,Issued,10,$1.00\n",
        )
        .unwrap();

        let err = charges_report(&path).unwrap_err();
        assert!(matches!(err, ChargesReportError::UndatedReport { .. }));
        let message = err.to_string();
        assert!(
            message.starts_with(&format!("Charges Report {}: ", path.display())),
            "{message}"
        );
        assert!(
            message.contains("what month the report covers"),
            "{message}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// A row reaching outside the month the name states refuses the **whole file**: no totals, no
    /// `ChargesReport`, and the message names every offending span rather than stopping at the
    /// first.
    #[test]
    fn a_row_outside_the_named_month_refuses_the_whole_file() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,kWh,Cost
01-Jun-26,30-Jun-26,Issued,10,$1.00
20-May-26,30-Jun-26,Issued,20,$2.00
01-Jun-26,05-Jul-26,Issued,30,$3.00
";
        let dir = temp_dir("outside_month");
        let path = june_path(&dir);
        fs::write(&path, CSV).unwrap();

        let err = charges_report(&path).unwrap_err();
        assert!(matches!(err, ChargesReportError::RowsOutsideMonth { .. }));
        let message = err.to_string();
        // Both offending spans, not just the first found.
        assert!(message.contains("2026-05-20"), "{message}");
        assert!(message.contains("2026-07-05"), "{message}");
        assert!(message.contains("rows 3"), "{message}");
        assert!(message.contains("rows 4"), "{message}");

        fs::remove_dir_all(&dir).ok();
    }

    /// A span inside the month but short of it is a finding, not a refusal: under the subscription
    /// reading of the two date columns it is what a mid-month join looks like.
    #[test]
    fn a_span_short_of_the_month_is_a_finding_not_a_refusal() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,kWh,Cost
01-Jun-26,30-Jun-26,Issued,10,$1.00
15-Jun-26,30-Jun-26,Issued,5,$0.50
";
        let dir = temp_dir("partial_span");
        let path = june_path(&dir);
        fs::write(&path, CSV).unwrap();

        let report = charges_report(&path).unwrap();
        assert_eq!(report.month, june(1));
        assert!(!report.is_clean());
        assert_eq!(report.partial_spans, vec![((june(15), june(30)), vec![3])]);
        // Counted in full regardless.
        assert_eq!(report.total_kwh, 15.0);

        fs::remove_dir_all(&dir).ok();
    }

    /// The refusal this reader raises itself opens the same way as the shared ones, so a reader
    /// cannot tell which half of `ChargesReportError` refused the file.
    #[test]
    fn an_empty_report_is_refused_in_the_same_voice() {
        const CSV: &str = "Start_Date,End_Date,Bill_Status,kWh,Cost\n";
        let dir = temp_dir("no_rows");
        let path = june_path(&dir);
        fs::write(&path, CSV).unwrap();

        let err = charges_report(&path).unwrap_err();
        let message = err.to_string();
        assert!(
            message.starts_with(&format!("Charges Report {}: ", path.display())),
            "{message}"
        );
        assert!(message.contains("one row per breaker"), "{message}");

        fs::remove_dir_all(&dir).ok();
    }

    /// A comma that does not group three digits is a malformed cell, not a separator to ignore.
    /// Stripping every comma would read `1,2` as 12 — a plausible figure standing in for a cell
    /// nobody read correctly, in a reader whose whole posture is to refuse rather than guess.
    #[test]
    fn a_comma_that_does_not_separate_thousands_is_refused() {
        for text in ["1,2", ",123", "1,2345", "1.234,5"] {
            let err = number(text, &fake_path(), 2, "kWh").unwrap_err();
            let message = err.to_string();
            assert!(
                message.contains("does not separate thousands"),
                "{text}: {message}"
            );
            assert!(message.contains(&format!("{text:?}")), "{text}: {message}");
        }
    }

    /// The grouped forms the reports do carry, and a figure with no comma at all.
    #[test]
    fn a_comma_that_does_separate_thousands_is_read() {
        for (text, expected) in [
            ("1,234.5", 1234.5),
            ("12,345,678", 12_345_678.0),
            ("35", 35.0),
        ] {
            assert_eq!(
                number(text, &fake_path(), 2, "kWh").unwrap(),
                expected,
                "{text}"
            );
        }
    }

    /// The message quotes the cell as the report wrote it, `$` and all. `money` parses a copy with
    /// the `$` taken off, and an error quoting that copy would name a value the file does not hold
    /// — the quoted value is there to be searched for.
    #[test]
    fn a_bad_cost_cell_is_quoted_as_the_report_wrote_it() {
        let err = money("$1,2", &fake_path(), 4, "Cost").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("\"$1,2\""), "{message}");
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

    /// Rows carrying different spans are read, not refused, as long as each lies inside the month
    /// the file name states. Agreeing with *each other* is not required: under the subscription
    /// reading of the two date columns a mid-month join produces a row differing from its
    /// neighbours in a perfectly correct file.
    #[test]
    fn rows_that_disagree_about_their_span_are_all_read() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,kWh,Cost
01-Jun-26,30-Jun-26,Issued,10,$1.00
01-Jun-26,30-Jun-26,Issued,20,$2.00
15-Jun-26,30-Jun-26,Issued,5,$0.50
";
        let dir = temp_dir("mixed_spans");
        let path = june_path(&dir);
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
        let path = june_path(&dir);
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

    /// A path in `dir` named the way Evolute names a June 2026 Charges Report.
    ///
    /// Every fixture below carries June dates, and the reader now reads the month off the name, so
    /// the name is part of the fixture rather than an arbitrary label.
    fn june_path(dir: &Path) -> PathBuf {
        dir.join("XX-XX_charges_2026-06-01T00_00_00-04_00.csv")
    }

    /// The name Evolute gives its exports, which is the only form this reads. The building prefix
    /// varies, and the timestamp's time of day and offset say nothing about the month.
    #[test]
    fn a_charges_report_name_states_its_month() {
        assert_eq!(
            charges_month(Path::new(
                "data/evolute/XX-XX_charges_2026-06-01T00_00_00-04_00.csv"
            )),
            Some(june(1))
        );
        // A different building, and the winter offset.
        assert_eq!(
            charges_month(Path::new("Bldg7_charges_2026-01-01T00_00_00-05_00.csv")),
            Some(date(2026, 1, 1))
        );
    }

    /// The month is the one the date falls in, whatever the day. Evolute stamps the start of the
    /// period, so for a calendar month that is the first — but the reader normalises rather than
    /// insisting, because the day is not what the check is about.
    #[test]
    fn a_day_other_than_the_first_still_names_its_month() {
        assert_eq!(
            charges_month(Path::new("XX-XX_charges_2026-06-15T09_30_00-04_00.csv")),
            Some(june(1))
        );
    }

    /// A suffix is a note to a person, so it is ignored rather than allowed to hide what the file
    /// covers — the same tolerance `session::report_coverage` extends to session reports, which
    /// `data/evolute/` relies on for its `-mock` and `-bak` copies.
    #[test]
    fn a_marked_up_name_still_states_its_month() {
        for name in [
            "XX-XX_charges_2026-06-01T00_00_00-04_00-bak.csv",
            "XX-XX_charges_2026-06-01T00_00_00-04_00-mock.csv",
            "XX-XX_charges_2026-06-01T00_00_00-04_00 (1).csv",
            // The date alone, with no time after it. A narrower name than Evolute writes, and it
            // states its month unambiguously; insisting on the `T` would be a rule about a form
            // seen in exactly one file.
            "XX-XX_charges_2026-06-01.csv",
        ] {
            assert_eq!(charges_month(Path::new(name)), Some(june(1)), "{name}");
        }
    }

    /// Anything else is refused rather than guessed at, because a guess would then be compared
    /// against the session report's month and could pass.
    #[test]
    fn a_name_that_does_not_state_its_month_is_refused() {
        for name in [
            "charges.csv",
            "Charges_Report_June.csv",
            "Session_Report_June_1_2026-June_30_2026.csv",
            "XX-XX_charges_June_2026.csv",
            // June has 30 days, so this is a name to reject rather than a date to build.
            "XX-XX_charges_2026-06-31T00_00_00-04_00.csv",
        ] {
            assert_eq!(charges_month(Path::new(name)), None, "{name}");
        }
    }
}
