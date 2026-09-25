//! The rates workbook's file to rows.
//!
//! The workbook can have any name. The rates are on the sheet named `rates` or, failing that,
//! `sheet1` -- matched ignoring case and surrounding spaces, so Excel's default `Sheet1` is found.
//! Row 1 names the columns, and four of them are read, in whatever order they stand:
//! `effective_date`, `on_peak`, `mid_peak` and `off_peak`. Any other column, and any other sheet, is
//! ignored.
//!
//! Everything about the effective dates is settled here, whatever the rows will be asked: each is a
//! date entered as a date, the dates run strictly upwards, and the rates end at the first row
//! without one. The rate cells are kept as found, for `select` to judge.
//!
//! `.xlsx` only: it is what `umya-spreadsheet` reads.

use super::error::{Found, RatesWorkbookError};
use crate::time::{Tou, date_of_serial};
use jiff::civil::Date;
use std::path::{Path, PathBuf};
use umya_spreadsheet::{Cell, CellRawValue, Worksheet, reader::xlsx};

/// The sheet names looked for, in order, compared ignoring case and surrounding spaces.
const SHEET_NAMES: [&str; 2] = ["rates", "sheet1"];

/// The column holding each row's effective date.
const DATE_COLUMN: &str = "effective_date";

/// The three bands, in the order [`RateRow::cells`] holds them.
pub(super) const BANDS: [Tou; 3] = [Tou::OnPeak, Tou::MidPeak, Tou::OffPeak];

/// A band's column name in the header row.
pub(super) fn column_name(tou: Tou) -> &'static str {
    match tou {
        Tou::OnPeak => "on_peak",
        Tou::MidPeak => "mid_peak",
        Tou::OffPeak => "off_peak",
    }
}

/// The rates sheet of a workbook, row by row, in effective-date order.
///
/// Built only by [`rate_sheet`], which refuses a sheet with no rows or with effective dates that do
/// not strictly increase, so the row in effect on any date is well defined. The rate cells are as
/// found.
#[derive(Debug)]
pub(super) struct RateSheet {
    /// The workbook the sheet is in.
    pub(super) path: PathBuf,
    /// The sheet's name, as the workbook spells it.
    pub(super) sheet: String,
    /// The sheet column, counting from 1, of each band's rates, in [`BANDS`] order.
    pub(super) rate_columns: [u32; 3],
    /// Earliest effective date first.
    pub(super) rows: Vec<RateRow>,
}

/// One row of the rates sheet: the date the rates take effect, and the three rate cells.
#[derive(Debug)]
pub(super) struct RateRow {
    /// The row's number in the sheet, counting the header as row 1.
    pub(super) row: u32,
    /// The first day these rates are charged on.
    pub(super) effective_date: Date,
    /// The rate cells, in [`BANDS`] order.
    pub(super) cells: [RateCell; 3],
}

/// A rate cell as it was found, before anything is asked of it.
#[derive(Debug)]
pub(super) enum RateCell {
    /// A number, of any sign or size.
    Number(f64),
    /// Nothing in the cell, or only spaces.
    Empty,
    /// Anything that is not a number, as the spreadsheet would display it.
    Text(String),
}

/// The rates sheet of the workbook at `path`, with every effective date checked.
///
/// # Errors
///
/// [`RatesWorkbookError`] for a file that is not an `.xlsx` workbook or cannot be read, and for a
/// sheet, header or effective date that breaks one of the rules in the module documentation.
pub(super) fn rate_sheet(path: &Path) -> Result<RateSheet, RatesWorkbookError> {
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

    let [date_column, rate_columns @ ..] = header(sheet).map_err(|problem| {
        let (path, sheet) = in_sheet();
        match problem {
            HeaderProblem::Missing(missing) => RatesWorkbookError::MissingColumns {
                path,
                sheet,
                missing,
            },
            HeaderProblem::Repeated {
                name,
                first,
                second,
            } => RatesWorkbookError::RepeatedColumn {
                path,
                sheet,
                name,
                first,
                second,
            },
        }
    })?;

    let mut rows: Vec<RateRow> = Vec::new();
    let mut row = 2;
    while let Some(cell) = sheet.cell((date_column, row)).filter(|c| !is_blank(c)) {
        let effective_date = effective_date(cell).map_err(|problem| {
            let (path, sheet) = in_sheet();
            match problem {
                DateProblem::NotADate(found) => RatesWorkbookError::NotADate {
                    path,
                    sheet,
                    column: date_column,
                    row,
                    found,
                },
                DateProblem::NotMidnight(date) => RatesWorkbookError::NotMidnight {
                    path,
                    sheet,
                    column: date_column,
                    row,
                    date,
                },
            }
        })?;
        if let Some(previous) = rows.last()
            && effective_date <= previous.effective_date
        {
            let (path, sheet) = in_sheet();
            return Err(RatesWorkbookError::NotIncreasing {
                path,
                sheet,
                row,
                date: effective_date,
                previous_row: previous.row,
                previous: previous.effective_date,
            });
        }
        rows.push(RateRow {
            row,
            effective_date,
            cells: rate_columns.map(|column| rate_cell(sheet.cell((column, row)))),
        });
        row += 1;
    }

    // Formatting alone does not make a row: a sheet whose rows were styled well past the rates is
    // still a sheet whose rates end where the dates do. Only a value counts as content.
    for below in row + 1..=sheet.highest_row() {
        for column in [
            date_column,
            rate_columns[0],
            rate_columns[1],
            rate_columns[2],
        ] {
            if let Some(cell) = sheet.cell((column, below)).filter(|c| !is_blank(c)) {
                let (path, sheet) = in_sheet();
                return Err(RatesWorkbookError::ContentBelowEnd {
                    path,
                    sheet,
                    end_row: row,
                    column,
                    row: below,
                    found: cell.value().into_owned(),
                });
            }
        }
    }

    if rows.is_empty() {
        let (path, sheet) = in_sheet();
        return Err(RatesWorkbookError::NoRows { path, sheet });
    }
    Ok(RateSheet {
        path: path_buf(),
        sheet: sheet_name,
        rate_columns,
        rows,
    })
}

/// What was wrong with the header row.
enum HeaderProblem {
    Missing(Vec<&'static str>),
    Repeated {
        name: &'static str,
        first: u32,
        second: u32,
    },
}

/// The column, counting from 1, of `effective_date` and then of each band in [`BANDS`] order.
fn header(sheet: &Worksheet) -> Result<[u32; 4], HeaderProblem> {
    let names = [
        DATE_COLUMN,
        column_name(BANDS[0]),
        column_name(BANDS[1]),
        column_name(BANDS[2]),
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
            return Err(HeaderProblem::Repeated {
                name: names[i],
                first,
                second: column,
            });
        }
        found[i] = Some(column);
    }
    match found {
        [Some(date), Some(on), Some(mid), Some(off)] => Ok([date, on, mid, off]),
        _ => Err(HeaderProblem::Missing(
            names
                .iter()
                .zip(found)
                .filter(|(_, column)| column.is_none())
                .map(|(name, _)| *name)
                .collect(),
        )),
    }
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
    let date = date_of_serial(serial.trunc() as i64)
        .ok_or(DateProblem::NotADate(Found::OutOfRange(serial)))?;
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

/// A rate cell as found, for `select` to judge.
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

// cargo test --lib -- rates::excel::read::test
#[cfg(test)]
mod test {
    use super::*;
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

    /// Puts `cells`, by address, into `sheet`.
    fn fill(sheet: &mut Worksheet, cells: &[(&str, Value)]) {
        for (at, value) in cells {
            match value {
                Value::Text(t) => {
                    sheet.cell_mut(*at).set_value_string(*t);
                }
                Value::Number(n) => {
                    sheet.cell_mut(*at).set_value_number(*n);
                }
                Value::Date(n) => {
                    sheet.cell_mut(*at).set_value_number(*n);
                    sheet
                        .style_mut(*at)
                        .number_format_mut()
                        .set_format_code("yyyy-mm-dd");
                }
            }
        }
    }

    /// A workbook of one sheet named `name`, holding `cells` by address.
    fn book(name: &str, cells: &[(&str, Value)]) -> Workbook {
        let mut book = new_file();
        let sheet = book.sheet_mut(0).unwrap();
        sheet.set_name(name);
        fill(sheet, cells);
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
    fn a_workbook_laid_out_as_the_example_is_reads() {
        let dir = temp_dir("reads");
        let path = write(&dir, "Rates.xlsx", &book("rates", &two_rows()));

        let sheet = rate_sheet(&path).expect("a valid workbook");
        assert_eq!(sheet.sheet, "rates");
        assert_eq!(sheet.rate_columns, [2, 3, 4]);
        let dates: Vec<Date> = sheet.rows.iter().map(|r| r.effective_date).collect();
        assert_eq!(dates, [date(2026, 1, 9), date(2026, 5, 1)]);
        assert!(
            matches!(sheet.rows[1].cells, [RateCell::Number(on), RateCell::Number(mid), RateCell::Number(off)]
                if (on, mid, off) == (0.11, 0.09, 0.07)),
            "{:?}",
            sheet.rows[1]
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

        assert_eq!(rate_sheet(&path).unwrap().rate_columns, [3, 5, 1]);

        fs::remove_dir_all(&dir).ok();
    }

    /// `rates` first, then `sheet1`, ignoring case and surrounding spaces; anything else is refused
    /// naming the sheets there are.
    #[test]
    fn the_rates_sheet_is_found_by_name() {
        let dir = temp_dir("sheet");
        for name in ["rates", " Rates ", "RATES", "Sheet1", "sheet1"] {
            let path = write(&dir, "Rates.xlsx", &book(name, &two_rows()));
            assert!(rate_sheet(&path).is_ok(), "{name:?}");
        }

        // `rates` wins over `Sheet1` when both are there.
        let mut both = book("Sheet1", &[("A1", Value::Text("nothing useful"))]);
        fill(both.new_sheet("rates").unwrap(), &two_rows());
        let path = write(&dir, "Both.xlsx", &both);
        assert_eq!(rate_sheet(&path).unwrap().sheet, "rates");

        let path = write(&dir, "Other.xlsx", &book("Tariff", &two_rows()));
        let err = rate_sheet(&path).unwrap_err();
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
        let err = rate_sheet(Path::new("/nonexistent/rates.xls")).unwrap_err();
        assert!(matches!(err, RatesWorkbookError::NotXlsx { .. }), "{err}");
        // Either case of the extension is a workbook's, so this one gets as far as opening.
        let err = rate_sheet(Path::new("/nonexistent/rates.XLSX")).unwrap_err();
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
        let err = rate_sheet(&path).unwrap_err();
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
        let err = rate_sheet(&path).unwrap_err();
        assert!(
            matches!(
                err,
                RatesWorkbookError::RepeatedColumn {
                    name: "mid_peak",
                    first: 3,
                    second: 5,
                    ..
                }
            ),
            "{err}"
        );
        assert!(err.to_string().ends_with("in cells C1 and E1"), "{err}");

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
            let message = rate_sheet(&path).unwrap_err().to_string();
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
            assert_eq!(rate_sheet(&path).is_ok(), is_date, "{code}");
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn effective_dates_that_do_not_increase_are_refused_naming_both_rows() {
        let dir = temp_dir("increase");
        for (second, name) in [(MAY_1, "Descending.xlsx"), (JAN_9, "Repeated.xlsx")] {
            let mut cells = two_rows();
            cells[4] = ("A2", Value::Date(MAY_1));
            cells[8] = ("A3", Value::Date(second));
            let path = write(&dir, name, &book("rates", &cells));

            let err = rate_sheet(&path).unwrap_err();
            assert!(
                matches!(
                    err,
                    RatesWorkbookError::NotIncreasing {
                        row: 3,
                        previous_row: 2,
                        ..
                    }
                ),
                "{err}"
            );
        }

        fs::remove_dir_all(&dir).ok();
    }

    /// The rates end at the first empty effective date. Below it, a value is refused; formatting
    /// alone is not, and nor is a value in a column nothing reads.
    #[test]
    fn nothing_may_follow_the_first_row_without_an_effective_date() {
        let dir = temp_dir("below");
        let mut cells = two_rows();
        cells.push(("C6", Value::Number(0.12)));
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));
        let err = rate_sheet(&path).unwrap_err();
        assert!(
            matches!(
                err,
                RatesWorkbookError::ContentBelowEnd {
                    end_row: 4,
                    column: 3,
                    row: 6,
                    ..
                }
            ),
            "{err}"
        );

        let mut styled = book("rates", &two_rows());
        styled
            .sheet_mut(0)
            .unwrap()
            .style_mut("B9")
            .number_format_mut()
            .set_format_code("#,##0.0000");
        let path = write(&dir, "Styled.xlsx", &styled);
        assert!(rate_sheet(&path).is_ok());

        let mut cells = two_rows();
        cells.push(("F9", Value::Text("a note")));
        let path = write(&dir, "Noted.xlsx", &book("rates", &cells));
        assert!(rate_sheet(&path).is_ok());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_sheet_with_no_rates_is_refused() {
        let dir = temp_dir("empty");
        let path = write(&dir, "Rates.xlsx", &book("rates", &two_rows()[..4]));
        let err = rate_sheet(&path).unwrap_err();
        assert!(matches!(err, RatesWorkbookError::NoRows { .. }), "{err}");

        fs::remove_dir_all(&dir).ok();
    }

    /// Rate cells are kept as found, whatever they hold: judging them is `select`'s.
    #[test]
    fn rate_cells_are_kept_as_found() {
        let dir = temp_dir("cells");
        let mut cells = two_rows();
        cells[5] = ("B2", Value::Text("tbd"));
        cells[6] = ("C2", Value::Text("  "));
        cells[7] = ("D2", Value::Number(-1.0));
        let path = write(&dir, "Rates.xlsx", &book("rates", &cells));

        let sheet = rate_sheet(&path).expect("the dates are sound");
        assert!(
            matches!(&sheet.rows[0].cells, [RateCell::Text(t), RateCell::Empty, RateCell::Number(n)]
                if t == "tbd" && *n == -1.0),
            "{:?}",
            sheet.rows[0]
        );

        fs::remove_dir_all(&dir).ok();
    }
}
