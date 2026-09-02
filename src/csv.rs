//! Reading a CSV whose columns are found by name.
//!
//! Both of Evolute's reports arrive as a CSV with a header row, and both are read the same way:
//! open the file, map each header to the column it occupies, refuse the file if a column the
//! reader needs is not there, then look every cell up by name. Column order and any columns
//! nobody reads are therefore immaterial to both.
//!
//! It is shared so the two cannot drift apart, and the message is as much of the reason as the
//! code is: a refusal names the document it expected before it names the file, and that sentence
//! is now written once. Before this module the two readers each had their own copy of it, kept in
//! step by a comment in each pointing at the other.
//!
//! What is *not* here is anything that depends on which document was opened. The value parsers
//! belong to the format that writes the values — a Charges Report's `01-Jun-26` and a session
//! report's `5:07:53` have nothing to share. Neither do the failures only one of the two can
//! have: a Charges Report with no rows, a session report naming a wall time the calendar cannot
//! place. Each reader keeps an error type of its own, holding those and deferring to
//! [`CsvReadError`] for the rest — see `session::csv::SessionCsvError` and
//! [`ChargesReportError`](crate::charges_report::ChargesReportError).
//!
//! The module is named for what it reads, so the `csv` crate is written `::csv` throughout to keep
//! the two apart.

use std::{
    collections::HashMap,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Which of the two reports a file was offered as.
///
/// Carried by every [`CsvReadError`] and written at the head of its message. The path alone does
/// not say which input slot rejected the file: a Charges Report picked for the session report slot
/// fails on a missing column, and so does a session report picked for the Charges Report slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Document {
    /// Evolute's session report, one row per charging session.
    SessionReport,
    /// Evolute's Charges Report, one row per breaker.
    ChargesReport,
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SessionReport => "Session Report",
            Self::ChargesReport => "Charges Report",
        })
    }
}

/// Why a CSV could not be read, in the terms every column-by-name reader shares.
///
/// Every variant carries the file as a `path` field and the kind of document expected as a
/// [`Document`], and writes both at `Display`. Nothing here is pre-formatted into a string: the
/// row, the column and the offending cell are fields, so a caller wanting to point at the cell
/// reads them rather than parsing the sentence.
#[derive(Debug)]
pub enum CsvReadError {
    /// The file could not be opened, or is not a readable CSV.
    Unreadable {
        document: Document,
        path: PathBuf,
        cause: ::csv::Error,
    },

    /// A column the reader needs is not there.
    MissingColumn {
        document: Document,
        path: PathBuf,
        name: &'static str,
    },

    /// A cell could not be read as the kind of value its column holds.
    BadValue {
        document: Document,
        path: PathBuf,
        /// The CSV row, counting the header, so it is the number a spreadsheet shows.
        row: usize,
        column: &'static str,
        value: String,
        cause: String,
    },
}

impl CsvReadError {
    /// A [`Self::BadValue`] for `value`, read from `column` of `row`.
    ///
    /// The value parsers live with their formats and each raises this from several places, so the
    /// six fields are filled in here rather than at every one of them. `cause` is rendered on the
    /// spot: it is whatever the parse returned, and is wanted as a sentence rather than as a type.
    pub fn bad_value(
        document: Document,
        path: &Path,
        row: usize,
        column: &'static str,
        value: &str,
        cause: impl fmt::Display,
    ) -> Self {
        Self::BadValue {
            document,
            path: path.to_path_buf(),
            row,
            column,
            value: value.to_owned(),
            cause: cause.to_string(),
        }
    }

    /// The kind of document the reader expected.
    pub fn document(&self) -> Document {
        match self {
            Self::Unreadable { document, .. }
            | Self::MissingColumn { document, .. }
            | Self::BadValue { document, .. } => *document,
        }
    }

    /// The file the failure is about. Every variant has one: nothing here is checked before the
    /// file is opened.
    pub fn path(&self) -> &Path {
        match self {
            Self::Unreadable { path, .. }
            | Self::MissingColumn { path, .. }
            | Self::BadValue { path, .. } => path,
        }
    }
}

impl fmt::Display for CsvReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Every arm names the kind of document expected and then the file, from the variant's own
        // fields. The `csv` crate's errors do not carry the path and a per-row failure knows only
        // its row, so without this a caller is told what went wrong and left to guess where.
        write!(f, "{} {}: ", self.document(), self.path().display())?;
        match self {
            Self::Unreadable { cause, .. } => write!(f, "{cause}"),
            Self::MissingColumn { name, .. } => write!(f, "missing required column `{name}`"),
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

impl Error for CsvReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { cause, .. } => Some(cause),
            Self::MissingColumn { .. } | Self::BadValue { .. } => None,
        }
    }
}

/// One CSV file, read whole, with its header row resolved to column positions.
///
/// Whole, because a reader is not done with a record when it has parsed it: the session report's
/// pass-through columns are read again when the workbook is written, long after the parse. Both
/// reports are one building's month, so this is a few hundred rows.
///
/// Records are addressed by index, and the `csv` crate's own types do not leave this module. A
/// caller works in column names and row numbers, which is what its errors are written in.
///
/// The [`Document`] is not kept: it is wanted only while the file is being opened, and a reader
/// that has one of these already knows which document it asked for.
#[derive(Debug)]
pub(crate) struct Table {
    path: PathBuf,
    /// Column name to its position, each name trimmed as the header wrote it.
    headers: HashMap<String, usize>,
    records: Vec<::csv::StringRecord>,
}

impl Table {
    /// Opens `path`, reads it, and checks that every name in `required` is among its columns.
    ///
    /// The header is the file's first row; anything above it is read as the header itself and the
    /// file is refused for the columns it then appears to be missing.
    ///
    /// # Errors
    ///
    /// [`CsvReadError::Unreadable`] if the file cannot be opened or does not parse — including a
    /// row whose number of fields differs from the header's — and
    /// [`CsvReadError::MissingColumn`] for the first name in `required` that is not there.
    pub fn read(
        path: &Path,
        document: Document,
        required: &[&'static str],
    ) -> Result<Self, CsvReadError> {
        let unreadable = |cause: ::csv::Error| CsvReadError::Unreadable {
            document,
            path: path.to_path_buf(),
            cause,
        };

        let mut reader = ::csv::Reader::from_path(path).map_err(unreadable)?;
        let headers: HashMap<String, usize> = reader
            .headers()
            .map_err(unreadable)?
            .iter()
            .enumerate()
            .map(|(i, h)| (h.trim().to_owned(), i))
            .collect();

        for name in required {
            if !headers.contains_key(*name) {
                return Err(CsvReadError::MissingColumn {
                    document,
                    path: path.to_path_buf(),
                    name,
                });
            }
        }

        let records = reader
            .records()
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable)?;

        Ok(Self {
            path: path.to_path_buf(),
            headers,
            records,
        })
    }

    /// The file this was read from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many records the file holds, the header excluded.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// The CSV row number of the record at `index`, counting the header, so it is the number a
    /// spreadsheet shows.
    ///
    /// Every error and every log line numbers rows this way, which is the only way a reader can
    /// find the row being complained about.
    pub fn row_number(index: usize) -> usize {
        index + 2
    }

    /// The named column of the record at `index`, trimmed.
    ///
    /// Empty when the cell is blank and when the column is not there at all — a column no reader
    /// requires is a column the file need not carry.
    pub fn cell(&self, index: usize, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|&i| self.records[index].get(i))
            .unwrap_or("")
            .trim()
    }
}

// cargo test --lib -- csv::test
#[cfg(test)]
mod test {
    use super::*;
    use std::{env, fs, process};

    /// The document comes before the path, because the path alone does not say which input slot
    /// rejected the file. The two readers' own messages are checked where they are raised —
    /// `session::csv::test::a_message_opens_with_the_kind_of_document_expected` and its Charges
    /// Report counterpart.
    #[test]
    fn a_message_opens_with_the_kind_of_document_expected() {
        let err = CsvReadError::MissingColumn {
            document: Document::ChargesReport,
            path: PathBuf::from("Session_Report_June.csv"),
            name: "kWh",
        };
        assert_eq!(
            err.to_string(),
            "Charges Report Session_Report_June.csv: missing required column `kWh`"
        );
    }

    #[test]
    fn a_cell_is_found_by_name_wherever_the_column_sits() {
        const CSV: &str = "\
Cost,Extra,kWh
$1.00,ignored,10
";
        let dir = temp_dir("by_name");
        let path = dir.join("report.csv");
        fs::write(&path, CSV).unwrap();

        let table = Table::read(&path, Document::ChargesReport, &["kWh", "Cost"]).unwrap();
        assert_eq!(table.record_count(), 1);
        assert_eq!(table.cell(0, "kWh"), "10");
        assert_eq!(table.cell(0, "Cost"), "$1.00");
        // A column no reader asked for is not a column any reader has to cope with.
        assert_eq!(table.cell(0, "Absent"), "");

        fs::remove_dir_all(&dir).ok();
    }

    /// The first record is row 2, and an error about it says so.
    #[test]
    fn rows_are_numbered_as_a_spreadsheet_numbers_them() {
        const CSV: &str = "\
kWh
n/a
";
        let dir = temp_dir("row_numbers");
        let path = dir.join("report.csv");
        fs::write(&path, CSV).unwrap();

        let table = Table::read(&path, Document::ChargesReport, &["kWh"]).unwrap();
        let err = CsvReadError::bad_value(
            Document::ChargesReport,
            table.path(),
            Table::row_number(0),
            "kWh",
            table.cell(0, "kWh"),
            "expected a number",
        );
        assert_eq!(
            err.to_string(),
            format!(
                "Charges Report {}: row 2, column `kWh`: cannot read \"n/a\": expected a number",
                path.display()
            )
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_required_column_names_the_column() {
        const CSV: &str = "\
kWh
10
";
        let dir = temp_dir("missing_column");
        let path = dir.join("report.csv");
        fs::write(&path, CSV).unwrap();

        let err = Table::read(&path, Document::ChargesReport, &["kWh", "Cost"]).unwrap_err();
        assert!(err.to_string().contains("`Cost`"), "{err}");

        fs::remove_dir_all(&dir).ok();
    }

    /// A scratch directory of its own per test, since these run in parallel within one process.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("ev_csv_{}_{tag}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
