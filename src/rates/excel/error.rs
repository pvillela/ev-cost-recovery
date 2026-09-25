//! Why the rates workbook does not yield the rates asked for.
//!
//! One error type for both halves of the module, reading and selecting alike: `api::io` reports
//! either as the workbook failing to give the rates asked for, as it reports a Green Button export
//! that does not reach the period asked for.

use super::read;
use crate::time::Tou;
use jiff::civil::Date;
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};
use umya_spreadsheet::XlsxError;

/// Why the rates workbook does not yield the rates asked for.
///
/// Every variant names the workbook, and the sheet once one was found. A cell's position is held
/// as its column and row, counting from 1, and given as an address such as `C3` in the message.
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
    /// Row 1 names a column read in two places.
    RepeatedColumn {
        path: PathBuf,
        sheet: String,
        name: &'static str,
        first: u32,
        second: u32,
    },
    /// An effective date is not a date: text, or a number not formatted as a date.
    NotADate {
        path: PathBuf,
        sheet: String,
        column: u32,
        row: u32,
        found: Found,
    },
    /// An effective date carries a time of day.
    NotMidnight {
        path: PathBuf,
        sheet: String,
        column: u32,
        row: u32,
        date: Date,
    },
    /// A cell in one of the four columns holds something below `end_row`, where the rates end.
    ContentBelowEnd {
        path: PathBuf,
        sheet: String,
        end_row: u32,
        column: u32,
        row: u32,
        found: String,
    },
    /// The sheet holds no rows of rates.
    NoRows { path: PathBuf, sheet: String },
    /// An effective date is not after the one on the row above it.
    NotIncreasing {
        path: PathBuf,
        sheet: String,
        row: u32,
        date: Date,
        previous_row: u32,
        previous: Date,
    },
    /// No row takes effect on or before `date`, the first day to be priced.
    NotYetInEffect {
        path: PathBuf,
        sheet: String,
        date: Date,
        first: Date,
    },
    /// More than one row takes effect within one billing period.
    TwoChangesInPeriod {
        path: PathBuf,
        sheet: String,
        period_start: Date,
        period_ending: Date,
        dates: Vec<Date>,
    },
    /// A row takes effect after the first day of a calendar month and within it.
    ChangeInsideMonth {
        path: PathBuf,
        sheet: String,
        month_start: Date,
        month_end: Date,
        date: Date,
        row: u32,
    },
    /// A rate cell on a selected row is not a positive number.
    Rate {
        path: PathBuf,
        sheet: String,
        column: u32,
        row: u32,
        tou: Tou,
        effective_date: Date,
        problem: RateProblem,
    },
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

/// What is wrong with a rate cell. See [`RatesWorkbookError::Rate`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RateProblem {
    /// Nothing in the cell.
    Empty,
    /// Something other than a number, as the spreadsheet displays it.
    Text(String),
    /// A number that is zero or negative.
    NotPositive(f64),
    /// A number that is not finite.
    NotFinite,
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
                name,
                first,
                second,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "row 1 names the column {name} twice, in cells {} and {}",
                    cell_address(*first, 1),
                    cell_address(*second, 1)
                )
            }
            Self::NotADate {
                path,
                sheet,
                column,
                row,
                found,
            } => {
                in_sheet(f, path, sheet)?;
                write!(f, "cell {} ", cell_address(*column, *row))?;
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
                column,
                row,
                date,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "cell {} holds {date} with a time of day. An effective_date is a date alone, \
                     with no time",
                    cell_address(*column, *row)
                )
            }
            Self::ContentBelowEnd {
                path,
                sheet,
                end_row,
                column,
                row,
                found,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "cell {} holds \"{found}\", below row {end_row}, which has no effective_date. \
                     The rates end at the first row without an effective_date, so nothing may \
                     follow it",
                    cell_address(*column, *row)
                )
            }
            Self::NoRows { path, sheet } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "there are no rates: row 2 has no effective_date. The rates start on row 2, \
                     under the header"
                )
            }
            Self::NotIncreasing {
                path,
                sheet,
                row,
                date,
                previous_row,
                previous,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "the effective_date on row {row}, {date}, is not after the one on row \
                     {previous_row}, {previous}. The effective dates must increase down the \
                     sheet, with no date repeated"
                )
            }
            Self::NotYetInEffect {
                path,
                sheet,
                date,
                first,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "no rates are in effect on {date}: the earliest effective_date is {first}"
                )
            }
            Self::TwoChangesInPeriod {
                path,
                sheet,
                period_start,
                period_ending,
                dates,
            } => {
                in_sheet(f, path, sheet)?;
                let dates: Vec<String> = dates.iter().map(Date::to_string).collect();
                write!(
                    f,
                    "the rates change {} times within the billing period {period_start} to \
                     {period_ending}, on {}. A billing period can take one change at most",
                    dates.len(),
                    dates.join(", ")
                )
            }
            Self::ChangeInsideMonth {
                path,
                sheet,
                month_start,
                month_end,
                date,
                row,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "the rates change on {date} (row {row}), within the month {month_start} to \
                     {month_end}. A month is reconciled at one set of rates, so they can change \
                     only on the 1st"
                )
            }
            Self::Rate {
                path,
                sheet,
                column,
                row,
                tou,
                effective_date,
                problem,
            } => {
                in_sheet(f, path, sheet)?;
                write!(
                    f,
                    "cell {}, the {} rate effective {effective_date}, ",
                    cell_address(*column, *row),
                    read::column_name(*tou)
                )?;
                match problem {
                    RateProblem::Empty => write!(f, "is empty"),
                    RateProblem::Text(text) => {
                        write!(f, "holds \"{text}\", which is not a number")
                    }
                    RateProblem::NotPositive(value) => {
                        write!(f, "is {value}. A rate must be greater than zero")
                    }
                    RateProblem::NotFinite => write!(f, "is not a finite number"),
                }
            }
        }
    }
}

impl Error for RatesWorkbookError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

/// A cell's address as a spreadsheet shows it: `C3` for column 3, row 3.
fn cell_address(column: u32, row: u32) -> String {
    let mut letters = Vec::new();
    let mut n = column;
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push(char::from(b'A' + rem as u8));
        n = (n - 1) / 26;
    }
    letters.iter().rev().collect::<String>() + &row.to_string()
}

// cargo test --lib -- rates::excel::error::test
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cell_addresses_are_spelled_as_a_spreadsheet_shows_them() {
        assert_eq!(cell_address(1, 1), "A1");
        assert_eq!(cell_address(3, 12), "C12");
        assert_eq!(cell_address(26, 2), "Z2");
        assert_eq!(cell_address(27, 2), "AA2");
        assert_eq!(cell_address(52, 2), "AZ2");
        assert_eq!(cell_address(703, 2), "AAA2");
    }
}
