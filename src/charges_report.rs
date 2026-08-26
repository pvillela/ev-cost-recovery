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
    /// First day the report covers, from `Start_Date`.
    pub from: Date,
    /// Last day the report covers, inclusive, from `End_Date`.
    pub to: Date,
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
    pub statuses: BTreeMap<String, usize>,
}

impl ChargesReport {
    /// Whether the report covers exactly the calendar month beginning on `month_start`.
    pub fn covers_month(&self, month_start: Date) -> bool {
        self.from == month_start && self.to == month_start.last_of_month()
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

    /// The rows do not all cover the same span.
    ///
    /// One report is one period. Rows disagreeing about which means the file is two reports
    /// concatenated, and no single month can be read off it.
    MixedPeriods {
        path: PathBuf,
        first: (Date, Date),
        row: usize,
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
            | Self::BadValue { path, .. }
            | Self::MixedPeriods { path, .. } => path,
        }
    }
}

impl fmt::Display for ChargesReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Every arm names the file, from the variant's own `path` field. The `csv` crate's errors
        // do not carry it and a per-row failure knows only its row, so without this a caller is
        // told what went wrong and left to guess where.
        write!(f, "{}: ", self.path().display())?;
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
            Self::MixedPeriods {
                first: (from, to),
                row,
                ..
            } => write!(
                f,
                "row {row} covers a different period from the first row ({from} to {to}); one \
                 Charges Report covers one period"
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

    let mut span: Option<(Date, Date)> = None;
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
        match span {
            None => span = Some((from, to)),
            Some(first) if first != (from, to) => {
                return Err(ChargesReportError::MixedPeriods {
                    path: path.to_path_buf(),
                    first,
                    row,
                });
            }
            Some(_) => {}
        }

        total_kwh += number(&field(&record, "kWh"), path, row, "kWh")?;
        total_amount += money(&field(&record, "Cost"), path, row, "Cost")?;
        *statuses.entry(field(&record, "Bill_Status")).or_default() += 1;
        rows += 1;
    }

    let Some((from, to)) = span else {
        return Err(ChargesReportError::NoRows {
            path: path.to_path_buf(),
        });
    };

    Ok(ChargesReport {
        path: path.to_path_buf(),
        from,
        to,
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

    #[test]
    fn a_report_covers_a_month_only_when_it_spans_the_whole_of_it() {
        let report = ChargesReport {
            path: PathBuf::from("charges.csv"),
            from: june(1),
            to: june(30),
            total_kwh: 0.0,
            total_amount: 0.0,
            rows: 1,
            statuses: BTreeMap::new(),
        };
        assert!(report.covers_month(june(1)));
        assert!(!report.covers_month(date(2026, 5, 1)));

        let short = ChargesReport {
            to: june(29),
            ..report
        };
        assert!(!short.covers_month(june(1)));
    }
}
