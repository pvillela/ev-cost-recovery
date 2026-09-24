//! The rates workbook: our EV cost-recovery rates, one row per change, in an Excel workbook.
//!
//! The workbook can have any name. The rates are on the sheet named `rates` or, failing that,
//! `sheet1` -- matched ignoring case and surrounding spaces, so Excel's default `Sheet1` is found.
//! Row 1 names the columns, and four of them are read, in whatever order they stand:
//! `effective_date`, `on_peak`, `mid_peak` and `off_peak`. Any other column, and any other sheet, is
//! ignored.
//!
//! Everything about the effective dates is settled here, on every read, whatever the calculation
//! will use: each is a date entered as a date, the dates run strictly upwards, and the rates end at
//! the first row without one. The rate cells are handed on as found and checked by
//! [`RateSchedule`] on the rows a calculation actually uses.
//!
//! `.xlsx` only: it is what `umya-spreadsheet` reads.

use crate::api::pure::{
    RateBand, RateCell, RateRow, RateSchedule, RateScheduleError, cell_address,
};
use jiff::{ToSpan, civil::Date};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};
use umya_spreadsheet::{Cell, CellRawValue, Worksheet, XlsxError, reader::xlsx};

/// The sheet names looked for, in order, compared ignoring case and surrounding spaces.
const SHEET_NAMES: [&str; 2] = ["rates", "sheet1"];

/// The column holding each row's effective date.
const DATE_COLUMN: &str = "effective_date";

/// Why a rates workbook could not be read.
#[derive(Debug)]
pub(crate) enum RatesWorkbookError {
    /// The file is not named `.xlsx`, in any case.
    NotXlsx { path: PathBuf },
    /// The file could not be opened or is not a workbook.
    Read { path: PathBuf, cause: XlsxError },
    /// No sheet has either of the names looked for.
    NoRatesSheet { path: PathBuf, sheets: Vec<String> },
    /// Row 1 does not name every column read.
    MissingColumns {
        path: PathBuf,
        sheet: String,
        missing: Vec<&'static str>,
    },
    /// Row 1 names a column read more than once.
    RepeatedColumn {
        path: PathBuf,
        sheet: String,
        column: &'static str,
        cells: [String; 2],
    },
    /// An effective date is not a date: text, or a number not formatted as a date.
    NotADate {
        path: PathBuf,
        sheet: String,
        cell: String,
        found: Found,
    },
    /// An effective date carries a time of day.
    NotMidnight {
        path: PathBuf,
        sheet: String,
        cell: String,
        date: Date,
    },
    /// A cell in one of the four columns holds something below the row where the rates end.
    ContentBelowEnd {
        path: PathBuf,
        sheet: String,
        end_row: u32,
        cell: String,
        found: String,
    },
    /// The rows read, but do not make a schedule. Names the workbook itself.
    Schedule(RateScheduleError),
}

/// What stood in an effective-date cell instead of a date.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Found {
    /// Anything that is not a number, as the spreadsheet displays it.
    Text(String),
    /// A number, with a number format that is not a date's.
    Number(f64),
    /// A number too small or too large to be a date.
    OutOfRange(f64),
}

impl fmt::Display for RatesWorkbookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let in_sheet = |f: &mut fmt::Formatter<'_>, path: &Path, sheet: &str| {
            write!(f, "rates workbook {}, sheet \"{sheet}\": ", path.display())
        };
        match self {
            Self::NotXlsx { path } => write!(
                f,
                "rates workbook {}: the name does not end in .xlsx. The rates are read from an \
                 Excel workbook saved as .xlsx",
                path.display()
            ),
            Self::Read { path, cause } => write!(
                f,
                "rates workbook {}: the file could not be read: {cause}",
                path.display()
            ),
            Self::NoRatesSheet { path, sheets } => {
                let sheets: Vec<String> = sheets.iter().map(|s| format!("\"{s}\"")).collect();
                write!(
                    f,
                    "rates workbook {}: there is no sheet named \"rates\" or \"Sheet1\". Its sheets \
                     are {}",
                    path.display(),
                    sheets.join(", ")
                )
            }
            Self::MissingColumns {
                path,
                sheet,
                missing,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "row 1 does not name the column{} {}. Row 1 must name effective_date, \
                     on_peak, mid_peak and off_peak, spelled exactly so",
                    if missing.len() == 1 { "" } else { "s" },
                    missing.join(", ")
                )
            }
            Self::RepeatedColumn {
                path,
                sheet,
                column,
                cells: [first, second],
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "row 1 names the column {column} twice, in cells {first} and {second}"
                )
            }
            Self::NotADate {
                path,
                sheet,
                cell,
                found,
            } => {
                in_sheet(f, path, sheet)?;
                write!(f, "cell {cell} ")?;
                match found {
                    Found::Text(text) => write!(f, "holds the text \"{text}\""),
                    Found::Number(n) => write!(f, "holds the number {n}, not formatted as a date"),
                    Found::OutOfRange(n) => write!(f, "holds {n}, which no date is stored as"),
                }?;
                write!(
                    f,
                    ". An effective_date must be entered as a date, which the spreadsheet \
                     displays in a date format"
                )
            }
            Self::NotMidnight {
                path,
                sheet,
                cell,
                date,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "cell {cell} holds {date} with a time of day. An effective_date is a date \
                     alone, with no time"
                )
            }
            Self::ContentBelowEnd {
                path,
                sheet,
                end_row,
                cell,
                found,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "cell {cell} holds \"{found}\", below row {end_row}, which has no \
                     effective_date. The rates end at the first row without an effective_date, \
                     so nothing may follow it"
                )
            }
            Self::Schedule(e) => e.fmt(f),
        }
    }
}

impl Error for RatesWorkbookError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { cause, .. } => Some(cause),
            Self::Schedule(e) => Some(e),
            _ => None,
        }
    }
}

/// The rates sheet of the workbook at `path`, with every effective date checked.
///
/// # Errors
///
/// [`RatesWorkbookError`], naming the workbook, and the sheet and cell where there is one.
pub(crate) fn read_rates_workbook(path: &Path) -> Result<RateSchedule, RatesWorkbookError> {
    let path_buf = || path.to_path_buf();
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
    {
        return Err(RatesWorkbookError::NotXlsx { path: path_buf() });
    }
    let book = xlsx::read(path).map_err(|cause| RatesWorkbookError::Read {
        path: path_buf(),
        cause,
    })?;
    let sheets = book.sheet_collection();
    let sheet = SHEET_NAMES
        .iter()
        .find_map(|wanted| {
            sheets
                .iter()
                .find(|s| s.name().trim().eq_ignore_ascii_case(wanted))
        })
        .ok_or_else(|| RatesWorkbookError::NoRatesSheet {
            path: path_buf(),
            sheets: sheets.iter().map(|s| s.name().to_owned()).collect(),
        })?;
    let sheet_name = sheet.name().to_owned();
    let in_sheet = || (path_buf(), sheet_name.clone());

    let [date_column, bands @ ..] = header(sheet).map_err(|problem| {
        let (path, sheet) = in_sheet();
        match problem {
            HeaderProblem::Missing(missing) => RatesWorkbookError::MissingColumns {
                path,
                sheet,
                missing,
            },
            HeaderProblem::Repeated(column, cells) => RatesWorkbookError::RepeatedColumn {
                path,
                sheet,
                column,
                cells,
            },
        }
    })?;
    let columns = [date_column, bands[0], bands[1], bands[2]];

    let mut rows = Vec::new();
    let mut row = 2;
    while let Some(cell) = sheet.cell((date_column, row)).filter(|c| !is_blank(c)) {
        let effective_date = effective_date(cell).map_err(|problem| {
            let (path, sheet) = in_sheet();
            let cell = cell_address(date_column, row);
            match problem {
                DateProblem::NotADate(found) => RatesWorkbookError::NotADate {
                    path,
                    sheet,
                    cell,
                    found,
                },
                DateProblem::NotMidnight(date) => RatesWorkbookError::NotMidnight {
                    path,
                    sheet,
                    cell,
                    date,
                },
            }
        })?;
        rows.push(RateRow {
            row,
            effective_date,
            cells: bands.map(|column| rate_cell(sheet.cell((column, row)))),
        });
        row += 1;
    }

    // Formatting alone does not make a row: a sheet whose rows were styled well past the rates is
    // still a sheet whose rates end where the dates do. Only a value counts as content.
    for below in row + 1..=sheet.highest_row() {
        for column in columns {
            if let Some(cell) = sheet.cell((column, below)).filter(|c| !is_blank(c)) {
                let (path, sheet) = in_sheet();
                return Err(RatesWorkbookError::ContentBelowEnd {
                    path,
                    sheet,
                    end_row: row,
                    cell: cell_address(column, below),
                    found: cell.value().into_owned(),
                });
            }
        }
    }

    RateSchedule::new(Some(path_buf()), sheet_name, bands, rows)
        .map_err(RatesWorkbookError::Schedule)
}

/// What was wrong with the header row.
enum HeaderProblem {
    Missing(Vec<&'static str>),
    Repeated(&'static str, [String; 2]),
}

/// The column, counting from 1, of `effective_date` and then of each band in [`RateBand::ALL`]
/// order.
fn header(sheet: &Worksheet) -> Result<[u32; 4], HeaderProblem> {
    let names = [
        DATE_COLUMN,
        RateBand::OnPeak.column(),
        RateBand::MidPeak.column(),
        RateBand::OffPeak.column(),
    ];
    let mut found: [Option<u32>; 4] = [None; 4];
    for column in 1..=sheet.highest_column() {
        let Some(cell) = sheet.cell((column, 1)) else {
            continue;
        };
        let value = cell.value();
        let Some(i) = names.iter().position(|name| *name == value) else {
            continue;
        };
        if let Some(first) = found[i] {
            return Err(HeaderProblem::Repeated(
                names[i],
                [cell_address(first, 1), cell_address(column, 1)],
            ));
        }
        found[i] = Some(column);
    }
    let missing: Vec<&'static str> = names
        .iter()
        .zip(found)
        .filter(|(_, column)| column.is_none())
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() {
        return Err(HeaderProblem::Missing(missing));
    }
    Ok(found.map(|column| column.expect("every column was found")))
}

/// Whether a cell holds nothing a reader would see: no value, or only spaces.
fn is_blank(cell: &Cell) -> bool {
    match cell.raw_value() {
        CellRawValue::Empty => true,
        CellRawValue::String(s) | CellRawValue::Lazy(s) => s.trim().is_empty(),
        CellRawValue::RichText(t) => t.text().trim().is_empty(),
        CellRawValue::Numeric(_) | CellRawValue::Bool(_) | CellRawValue::Error(_) => false,
    }
}

/// What was wrong with an effective-date cell.
enum DateProblem {
    NotADate(Found),
    NotMidnight(Date),
}

/// The date an effective-date cell holds.
///
/// A spreadsheet stores a date as a number of days, and says it is one only through the number
/// format it displays it in. So a number in a date format is a date, and a number in any other
/// format is refused rather than guessed at: `46143` in a `General` cell is far likelier to be a
/// slip than a date.
fn effective_date(cell: &Cell) -> Result<Date, DateProblem> {
    let serial = match cell.raw_value() {
        CellRawValue::Numeric(n) => *n,
        _ => {
            return Err(DateProblem::NotADate(Found::Text(
                cell.value().into_owned(),
            )));
        }
    };
    if !is_date_format(cell) {
        return Err(DateProblem::NotADate(Found::Number(serial)));
    }
    let date =
        serial_date(serial.trunc()).ok_or(DateProblem::NotADate(Found::OutOfRange(serial)))?;
    if serial.fract() != 0.0 {
        return Err(DateProblem::NotMidnight(date));
    }
    Ok(date)
}

/// Whether a cell's number format displays a date.
///
/// The built-in date formats are recognised by number, since a workbook may leave their format
/// code unstated. Any other format is a date's when a day or a year is among the characters it
/// displays as figures -- quoted literals, bracketed sections such as `[$-409]` and escaped
/// characters display as themselves and are skipped.
fn is_date_format(cell: &Cell) -> bool {
    let Some(format) = cell.style().number_format() else {
        return false;
    };
    // 14 to 17 and 22 are the built-in dates; 18 to 21 are times of day alone.
    if matches!(format.number_format_id(), 14..=17 | 22) {
        return true;
    }
    let (mut quoted, mut bracketed, mut escaped) = (false, false, false);
    for c in format.format_code().chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            '[' if !quoted => bracketed = true,
            ']' if bracketed => bracketed = false,
            _ if quoted || bracketed => {}
            'd' | 'D' | 'y' | 'Y' => return true,
            _ => {}
        }
    }
    false
}

/// The date a whole number of days stands for, in the 1900 date system every current spreadsheet
/// writes by default, or `None` for a number no date is stored as.
///
/// The system counts 1900 as a leap year, as Lotus 1-2-3 did: day 60 is a 29 February that never
/// was, and every day from 61 on is one later than a plain count from 1 January 1900 would make it.
fn serial_date(days: f64) -> Option<Date> {
    const LAST: f64 = 2_958_465.0; // 9999-12-31
    if !(1.0..=LAST).contains(&days) || days == 60.0 {
        return None;
    }
    let origin = if days < 60.0 {
        Date::constant(1899, 12, 31)
    } else {
        Date::constant(1899, 12, 30)
    };
    origin.checked_add((days as i64).days()).ok()
}

/// A rate cell as found, for [`RateSchedule`] to judge.
fn rate_cell(cell: Option<&Cell>) -> RateCell {
    match cell {
        None => RateCell::Empty,
        Some(cell) if is_blank(cell) => RateCell::Empty,
        Some(cell) => match cell.raw_value() {
            CellRawValue::Numeric(n) => RateCell::Number(*n),
            _ => RateCell::Text(cell.value().into_owned()),
        },
    }
}

// cargo test --lib -- rates_workbook::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::api::pure::{RateScheduleErrorKind, cell_address as address};
    use jiff::civil::date;
    use std::{env, fs, process};
    use umya_spreadsheet::{Workbook, new_file, writer};

    /// A folder of its own for one test, so tests running at once write no file in common.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("ev_rates_{}_{tag}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// One cell's contents in a test workbook.
    enum Value<'a> {
        Text(&'a str),
        Number(f64),
        /// A number shown in a date format, as a spreadsheet stores a date.
        Date(f64),
    }

    /// A workbook of one sheet named `sheet`, holding `cells` by address.
    fn book(sheet: &str, cells: &[(&str, Value)]) -> Workbook {
        let mut book = new_file();
        let ws = book.sheet_mut(0).unwrap();
        ws.set_name(sheet);
        for (at, value) in cells {
            let cell = ws.cell_mut(*at);
            match value {
                Value::Text(t) => {
                    cell.set_value_string(*t);
                }
                Value::Number(n) => {
                    cell.set_value_number(*n);
                }
                Value::Date(n) => {
                    cell.set_value_number(*n);
                    ws.style_mut(*at)
                        .number_format_mut()
                        .set_format_code("yyyy-mm-dd");
                }
            }
        }
        book
    }

    fn write(dir: &Path, name: &str, book: &Workbook) -> PathBuf {
        let path = dir.join(name);
        writer::xlsx::write(book, &path).unwrap();
        path
    }

    /// 2026-01-09 and 2026-05-01, as a spreadsheet stores them.
    const JAN_9: f64 = 46031.0;
    const MAY_1: f64 = 46143.0;

    /// The header in the example workbook's order, and two rows of rates.
    fn two_rows() -> Vec<(&'static str, Value<'static>)> {
        vec![
            ("A1", Value::Text("effective_date")),
            ("B1", Value::Text("on_peak")),
            ("C1", Value::Text("mid_peak")),
            ("D1", Value::Text("off_peak")),
            ("A2", Value::Date(JAN_9)),
            ("B2", Value::Number(0.5152)),
            ("C2", Value::Number(0.474)),
            ("D2", Value::Number(0.4218)),
            ("A3", Value::Date(MAY_1)),
            ("B3", Value::Number(0.11)),
            ("C3", Value::Number(0.09)),
            ("D3", Value::Number(0.07)),
            // A note beside the rates, in a column nothing reads.
            ("F3", Value::Text("This row is for testing purposes only.")),
        ]
    }

    #[test]
    fn a_workbook_as_the_example_lays_it_out_reads() {
        let dir = temp_dir("reads");
        let path = write(&dir, "Rates.xlsx", &book("rates", &two_rows()));

        let schedule = read_rates_workbook(&path).expect("a valid workbook");
        assert_eq!(schedule.workbook(), Some(path.as_path()));
        let may = schedule
            .for_month(date(2026, 6, 1))
            .expect("May's rates are in effect in June");
        assert_eq!(may.effective_date, date(2026, 5, 1));
        assert_eq!(
            (may.on_peak, may.mid_peak, may.off_peak),
            (0.11, 0.09, 0.07)
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// The columns are found by name, so their order is free.
    #[test]
    fn the_columns_may_stand_in_any_order() {
        let dir = temp_dir("order");
        let cells = [
            ("A1", Value::Text("off_peak")),
            ("B1", Value::Text("notes")),
            ("C1", Value::Text("on_peak")),
            ("D1", Value::Text("effective_date")),
            ("E1", Value::Text("mid_peak")),
            ("A2", Value::Number(0.07)),
            ("C2", Value::Number(0.11)),
            ("D2", Value::Date(MAY_1)),
            ("E2", Value::Number(0.09)),
        ];
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));

        let rates = read_rates_workbook(&path)
            .unwrap()
            .for_month(date(2026, 5, 1))
            .unwrap();
        assert_eq!(
            (rates.on_peak, rates.mid_peak, rates.off_peak),
            (0.11, 0.09, 0.07)
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// `rates` first, then `sheet1`, ignoring case and surrounding spaces; anything else is refused
    /// naming the sheets there are.
    #[test]
    fn the_rates_sheet_is_found_by_name() {
        let dir = temp_dir("sheet");
        for name in ["rates", " Rates ", "RATES", "Sheet1", "sheet1"] {
            let path = write(&dir, "Rates.xlsx", &book(name, &two_rows()));
            assert!(read_rates_workbook(&path).is_ok(), "{name:?}");
        }

        // `rates` wins over `Sheet1` when both are there.
        let mut both = book("Sheet1", &[("A1", Value::Text("nothing useful"))]);
        let rates = both.new_sheet("rates").unwrap();
        for (at, value) in two_rows() {
            match value {
                Value::Text(t) => {
                    rates.cell_mut(at).set_value_string(t);
                }
                Value::Number(n) => {
                    rates.cell_mut(at).set_value_number(n);
                }
                Value::Date(n) => {
                    rates.cell_mut(at).set_value_number(n);
                    rates
                        .style_mut(at)
                        .number_format_mut()
                        .set_format_code("yyyy-mm-dd");
                }
            }
        }
        let path = write(&dir, "Both.xlsx", &both);
        assert!(read_rates_workbook(&path).is_ok());

        let path = write(&dir, "Other.xlsx", &book("Tariff", &two_rows()));
        let err = read_rates_workbook(&path).unwrap_err();
        assert!(
            matches!(&err, RatesWorkbookError::NoRatesSheet { sheets, .. } if sheets == &["Tariff"]),
            "{err}"
        );
        assert!(
            err.to_string().ends_with("Its sheets are \"Tariff\""),
            "{err}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_file_not_named_xlsx_is_refused_unopened() {
        let err = read_rates_workbook(Path::new("/nonexistent/rates.xls")).unwrap_err();
        assert!(matches!(err, RatesWorkbookError::NotXlsx { .. }), "{err}");
        // Either case of the extension is a workbook's, so this one gets as far as opening.
        let err = read_rates_workbook(Path::new("/nonexistent/rates.XLSX")).unwrap_err();
        assert!(matches!(err, RatesWorkbookError::Read { .. }), "{err}");
        assert_eq!(err.to_string().matches("rates.XLSX").count(), 1, "{err}");
    }

    /// The header names are exact: a capital or a space is a different name.
    #[test]
    fn the_header_must_name_every_column_exactly_once() {
        let dir = temp_dir("header");
        let mut cells = two_rows();
        cells[1] = ("B1", Value::Text("On_Peak"));
        cells[3] = ("D1", Value::Text("off_peak "));
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));
        let err = read_rates_workbook(&path).unwrap_err();
        assert!(
            matches!(&err, RatesWorkbookError::MissingColumns { missing, .. }
                if missing == &["on_peak", "off_peak"]),
            "{err}"
        );
        assert!(
            err.to_string()
                .contains("row 1 does not name the columns on_peak, off_peak."),
            "{err}"
        );

        let mut cells = two_rows();
        cells.push(("E1", Value::Text("mid_peak")));
        let path = write(&dir, "Repeated.xlsx", &book("rates", &cells));
        let err = read_rates_workbook(&path).unwrap_err();
        assert!(
            matches!(&err, RatesWorkbookError::RepeatedColumn { column: "mid_peak", cells, .. }
                if cells == &["C1".to_owned(), "E1".to_owned()]),
            "{err}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// An effective date is a date: not text that reads like one, not a bare number, not a date
    /// with a time of day.
    #[test]
    fn an_effective_date_must_be_entered_as_a_date() {
        let dir = temp_dir("dates");
        let cases = [
            (Value::Text("2026-05-01"), "holds the text \"2026-05-01\""),
            (
                Value::Number(MAY_1),
                "holds the number 46143, not formatted as a date",
            ),
            (
                Value::Date(MAY_1 + 0.5),
                "holds 2026-05-01 with a time of day",
            ),
            (Value::Date(-3.0), "holds -3, which no date is stored as"),
        ];
        for (value, wording) in cases {
            let mut cells = two_rows();
            cells[8] = ("A3", value);
            let path = write(&dir, "Rates.xlsx", &book("rates", &cells));
            let message = read_rates_workbook(&path).unwrap_err().to_string();
            assert!(
                message.starts_with(&format!(
                    "rates workbook {}, sheet \"rates\": cell A3 {wording}",
                    path.display()
                )),
                "{message}"
            );
        }

        fs::remove_dir_all(&dir).ok();
    }

    /// A date format of any spelling counts, and a number format that only looks busy does not.
    #[test]
    fn a_date_format_is_recognised_by_what_it_displays() {
        let dir = temp_dir("formats");
        for (code, is_date) in [
            ("yyyy/mm/dd;@", true),
            ("d-mmm-yy", true),
            ("[$-409]mmmm d, yyyy", true),
            ("#,##0.0000", false),
            ("General", false),
            ("\"day \"0", false),
            ("h:mm", false),
        ] {
            let mut formatted = book("rates", &two_rows());
            formatted
                .sheet_mut(0)
                .unwrap()
                .style_mut("A3")
                .number_format_mut()
                .set_format_code(code);
            let path = write(&dir, "Rates.xlsx", &formatted);
            assert_eq!(read_rates_workbook(&path).is_ok(), is_date, "{code}");
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn effective_dates_that_do_not_increase_are_refused_naming_both_rows() {
        let dir = temp_dir("increase");
        // May above January: the dates run downwards.
        let mut cells = two_rows();
        cells[4] = ("A2", Value::Date(MAY_1));
        cells[8] = ("A3", Value::Date(JAN_9));
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));

        let err = read_rates_workbook(&path).unwrap_err();
        assert!(
            matches!(&err, RatesWorkbookError::Schedule(e)
                if matches!(e.kind, RateScheduleErrorKind::NotIncreasing { row: 3, previous_row: 2, .. })),
            "{err}"
        );
        // Named once, by the schedule's own message.
        assert_eq!(
            err.to_string().matches(&path.display().to_string()).count(),
            1,
            "{err}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// The rates end at the first empty effective date. Below it, a value is refused; formatting
    /// alone is not.
    #[test]
    fn nothing_may_follow_the_first_row_without_an_effective_date() {
        let dir = temp_dir("below");
        let mut cells = two_rows();
        cells.push(("C6", Value::Number(0.12)));
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));
        let err = read_rates_workbook(&path).unwrap_err();
        assert!(
            matches!(&err, RatesWorkbookError::ContentBelowEnd { end_row: 4, cell, .. } if cell == "C6"),
            "{err}"
        );

        // Styled but empty cells below the rates are not content.
        let mut styled = book("rates", &two_rows());
        styled
            .sheet_mut(0)
            .unwrap()
            .style_mut("B9")
            .number_format_mut()
            .set_format_code("#,##0.0000");
        let path = write(&dir, "Styled.xlsx", &styled);
        assert!(read_rates_workbook(&path).is_ok());

        // Nor is a value in a column nothing reads.
        let mut cells = two_rows();
        cells.push(("F9", Value::Text("a note")));
        let path = write(&dir, "Noted.xlsx", &book("rates", &cells));
        assert!(read_rates_workbook(&path).is_ok());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_sheet_with_no_rates_is_refused() {
        let dir = temp_dir("empty");
        let path = write(&dir, "Rates.xlsx", &book("rates", &two_rows()[..4]));
        let err = read_rates_workbook(&path).unwrap_err();
        assert!(
            matches!(&err, RatesWorkbookError::Schedule(e) if e.kind == RateScheduleErrorKind::NoRows),
            "{err}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// Rate cells are handed on as found: a bad one is reported only when a period uses its row,
    /// and then by its address.
    #[test]
    fn rate_cells_are_judged_only_on_the_rows_used() {
        let dir = temp_dir("lazy");
        let mut cells = two_rows();
        cells[5] = ("B2", Value::Text("tbd"));
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));

        let schedule = read_rates_workbook(&path).expect("the dates are sound");
        assert!(schedule.for_month(date(2026, 6, 1)).is_ok());
        let err = schedule
            .for_month(date(2026, 2, 1))
            .expect_err("January's row prices February");
        assert!(
            err.to_string().contains(&format!(
                "cell {}, the on_peak rate effective 2026-01-09, holds \"tbd\"",
                address(2, 2)
            )),
            "{err}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn serial_dates_follow_the_1900_date_system() {
        assert_eq!(serial_date(1.0), Some(date(1900, 1, 1)));
        assert_eq!(serial_date(59.0), Some(date(1900, 2, 28)));
        assert_eq!(serial_date(60.0), None);
        assert_eq!(serial_date(61.0), Some(date(1900, 3, 1)));
        assert_eq!(serial_date(MAY_1), Some(date(2026, 5, 1)));
        assert_eq!(serial_date(0.0), None);
    }
}
