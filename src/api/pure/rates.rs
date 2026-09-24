//! The schedule of EV cost-recovery rates, as the rates workbook states it, and which of its rows
//! price a billing period or a calendar month.
//!
//! The workbook is read by `crate::rates_workbook`, which holds every rule about the file itself:
//! the sheet, the header, and the effective dates. What it hands over is a [`RateSchedule`] whose
//! rate cells are still as found. They are checked here, and only on the rows a calculation
//! actually uses: a bad cell on a row no period reaches is not a reason to refuse a period that
//! never touches it.
//!
//! The rates are ours rather than Toronto Hydro's, and they are in dollars per kilowatt-hour.

use jiff::civil::Date;
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use super::recovery::CostRecoveryRates;

/// Which time-of-use band a rate belongs to, in the order the reports list them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateBand {
    OnPeak,
    MidPeak,
    OffPeak,
}

impl RateBand {
    /// The three bands, on-peak first. [`RateRow::cells`] is indexed in this order.
    pub const ALL: [Self; 3] = [Self::OnPeak, Self::MidPeak, Self::OffPeak];

    /// The band's column name in the rates workbook's header row.
    pub fn column(self) -> &'static str {
        match self {
            Self::OnPeak => "on_peak",
            Self::MidPeak => "mid_peak",
            Self::OffPeak => "off_peak",
        }
    }
}

/// A rate cell as it was found in the workbook, before anything is asked of it.
#[derive(Debug, Clone, PartialEq)]
pub enum RateCell {
    /// A number, of any sign or size.
    Number(f64),
    /// Nothing in the cell.
    Empty,
    /// Anything that is not a number, as the spreadsheet would display it.
    Text(String),
}

/// One row of the rates sheet: the date the rates take effect, and the three rate cells.
#[derive(Debug, Clone, PartialEq)]
pub struct RateRow {
    /// The row's number in the sheet, counting the header as row 1.
    pub row: u32,
    /// The first day these rates are charged on.
    pub effective_date: Date,
    /// The on-peak, mid-peak and off-peak cells, in [`RateBand::ALL`] order.
    pub cells: [RateCell; 3],
}

/// The rates sheet of a workbook, row by row, in effective-date order.
///
/// The effective dates strictly increase, which [`RateSchedule::new`] enforces, so the rows in
/// effect on any date are well defined. The rate cells are unchecked; a calculation checks the rows
/// it uses, and names the cell when one will not do.
#[derive(Debug, Clone, PartialEq)]
pub struct RateSchedule {
    workbook: Option<PathBuf>,
    sheet: String,
    band_columns: [u32; 3],
    rows: Vec<RateRow>,
}

impl RateSchedule {
    /// A schedule of `rows`, from the sheet named `sheet`.
    ///
    /// - `workbook` - the file the rows were read from, or `None` for rows built in memory. Every
    ///   error about the schedule names it.
    /// - `band_columns` - the sheet column, counting from 1, of each band's rates, in
    ///   [`RateBand::ALL`] order. An error about a rate cell gives the cell's address from it.
    ///
    /// # Errors
    ///
    /// [`RateScheduleErrorKind::NoRows`] for no rows at all, and
    /// [`RateScheduleErrorKind::NotIncreasing`] for the first row whose effective date is not
    /// after the one above it.
    pub fn new(
        workbook: Option<PathBuf>,
        sheet: String,
        band_columns: [u32; 3],
        rows: Vec<RateRow>,
    ) -> Result<Self, RateScheduleError> {
        let schedule = Self {
            workbook,
            sheet,
            band_columns,
            rows,
        };
        if schedule.rows.is_empty() {
            return Err(schedule.error(RateScheduleErrorKind::NoRows));
        }
        if let Some([previous, next]) = schedule
            .rows
            .windows(2)
            .find(|pair| pair[1].effective_date <= pair[0].effective_date)
        {
            return Err(schedule.error(RateScheduleErrorKind::NotIncreasing {
                row: next.row,
                date: next.effective_date,
                previous_row: previous.row,
                previous: previous.effective_date,
            }));
        }
        Ok(schedule)
    }

    /// The workbook the schedule was read from, or `None` for one built in memory.
    pub fn workbook(&self) -> Option<&Path> {
        self.workbook.as_deref()
    }

    /// The rates that price the billing period from `period_start` to `period_ending`, inclusive:
    /// those in effect on its first day, and those it changed to, if it did.
    ///
    /// # Errors
    ///
    /// [`RateScheduleErrorKind::NotYetInEffect`] if no row is in effect on the first day,
    /// [`RateScheduleErrorKind::TwoChangesInPeriod`] if more than one row is dated inside the
    /// period, and [`RateScheduleErrorKind::Rate`] for the first rate cell of those rows that is
    /// not a positive number.
    pub(crate) fn for_billing_period(
        &self,
        period_start: Date,
        period_ending: Date,
    ) -> Result<(CostRecoveryRates, Option<CostRecoveryRates>), RateScheduleError> {
        let opening = self.in_effect_on(period_start)?;
        let changes: Vec<&RateRow> = self.dated_within(period_start, period_ending).collect();
        let change = match changes[..] {
            [] => None,
            [only] => Some(only),
            _ => {
                return Err(self.error(RateScheduleErrorKind::TwoChangesInPeriod {
                    period_start,
                    period_ending,
                    dates: changes.iter().map(|r| r.effective_date).collect(),
                }));
            }
        };
        Ok((
            self.rates(opening)?,
            change.map(|row| self.rates(row)).transpose()?,
        ))
    }

    /// The rates that price the calendar month starting `month_start`.
    ///
    /// # Errors
    ///
    /// [`RateScheduleErrorKind::NotYetInEffect`] if no row is in effect on the 1st,
    /// [`RateScheduleErrorKind::ChangeInsideMonth`] if a row is dated after the 1st and within the
    /// month, and [`RateScheduleErrorKind::Rate`] for the first rate cell of the row in effect that
    /// is not a positive number.
    pub(crate) fn for_month(
        &self,
        month_start: Date,
    ) -> Result<CostRecoveryRates, RateScheduleError> {
        let month_end = month_start.last_of_month();
        let row = self.in_effect_on(month_start)?;
        if let Some(change) = self.dated_within(month_start, month_end).next() {
            return Err(self.error(RateScheduleErrorKind::ChangeInsideMonth {
                month_start,
                month_end,
                date: change.effective_date,
                row: change.row,
            }));
        }
        self.rates(row)
    }

    /// The last row taking effect on or before `date`.
    fn in_effect_on(&self, date: Date) -> Result<&RateRow, RateScheduleError> {
        self.rows
            .iter()
            .rev()
            .find(|r| r.effective_date <= date)
            .ok_or_else(|| {
                self.error(RateScheduleErrorKind::NotYetInEffect {
                    date,
                    first: self.rows[0].effective_date,
                })
            })
    }

    /// The rows taking effect after `first` and on or before `last`.
    fn dated_within(&self, first: Date, last: Date) -> impl Iterator<Item = &RateRow> {
        self.rows
            .iter()
            .filter(move |r| first < r.effective_date && r.effective_date <= last)
    }

    /// One row's rates, each checked to be a positive number.
    fn rates(&self, row: &RateRow) -> Result<CostRecoveryRates, RateScheduleError> {
        let mut rates = [0.0; 3];
        for (i, band) in RateBand::ALL.into_iter().enumerate() {
            let problem = match &row.cells[i] {
                RateCell::Number(value) if value.is_finite() && *value > 0.0 => {
                    rates[i] = *value;
                    continue;
                }
                RateCell::Number(value) if !value.is_finite() => RateProblem::NotFinite,
                RateCell::Number(value) => RateProblem::NotPositive(*value),
                RateCell::Empty => RateProblem::Empty,
                RateCell::Text(text) => RateProblem::Text(text.clone()),
            };
            return Err(self.error(RateScheduleErrorKind::Rate {
                cell: cell_address(self.band_columns[i], row.row),
                band,
                effective_date: row.effective_date,
                problem,
            }));
        }
        let [on_peak, mid_peak, off_peak] = rates;
        Ok(CostRecoveryRates {
            effective_date: row.effective_date,
            on_peak,
            mid_peak,
            off_peak,
        })
    }

    fn error(&self, kind: RateScheduleErrorKind) -> RateScheduleError {
        RateScheduleError {
            workbook: self.workbook.clone(),
            sheet: self.sheet.clone(),
            kind,
        }
    }
}

/// A cell's address as a spreadsheet shows it: `C3` for column 3, row 3.
pub(crate) fn cell_address(column: u32, row: u32) -> String {
    let mut letters = Vec::new();
    let mut n = column;
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push(char::from(b'A' + rem as u8));
        n = (n - 1) / 26;
    }
    letters.iter().rev().collect::<String>() + &row.to_string()
}

/// Why a rates schedule does not price what it was asked to.
///
/// Names its own workbook and sheet, so a caller wrapping it adds no path of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct RateScheduleError {
    /// The workbook the schedule was read from, or `None` for one built in memory.
    pub workbook: Option<PathBuf>,
    /// The sheet the rows came from.
    pub sheet: String,
    /// What was wrong.
    pub kind: RateScheduleErrorKind,
}

/// What was wrong with a rates schedule. See [`RateScheduleError`].
#[derive(Debug, Clone, PartialEq)]
pub enum RateScheduleErrorKind {
    /// The sheet holds no rows of rates.
    NoRows,
    /// An effective date is not after the one on the row above it.
    NotIncreasing {
        row: u32,
        date: Date,
        previous_row: u32,
        previous: Date,
    },
    /// No row takes effect on or before `date`, the first day to be priced.
    NotYetInEffect { date: Date, first: Date },
    /// More than one row takes effect inside one billing period.
    TwoChangesInPeriod {
        period_start: Date,
        period_ending: Date,
        dates: Vec<Date>,
    },
    /// A row takes effect after the first day of a calendar month and within it.
    ChangeInsideMonth {
        month_start: Date,
        month_end: Date,
        date: Date,
        row: u32,
    },
    /// A rate cell on a row in use is not a positive number.
    Rate {
        cell: String,
        band: RateBand,
        effective_date: Date,
        problem: RateProblem,
    },
}

/// What is wrong with a rate cell. See [`RateScheduleErrorKind::Rate`].
#[derive(Debug, Clone, PartialEq)]
pub enum RateProblem {
    /// Nothing in the cell.
    Empty,
    /// Something other than a number, as the spreadsheet displays it.
    Text(String),
    /// A number that is zero or negative.
    NotPositive(f64),
    /// A number that is not finite.
    NotFinite,
}

impl fmt::Display for RateScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.workbook {
            Some(path) => write!(f, "rates workbook {}", path.display())?,
            None => write!(f, "rates")?,
        }
        write!(f, ", sheet \"{}\": ", self.sheet)?;
        match &self.kind {
            RateScheduleErrorKind::NoRows => write!(
                f,
                "there are no rates: row 2 has no effective_date. The rates start on row 2, under \
                 the header"
            ),
            RateScheduleErrorKind::NotIncreasing {
                row,
                date,
                previous_row,
                previous,
            } => write!(
                f,
                "the effective_date on row {row}, {date}, is not after the one on row \
                 {previous_row}, {previous}. The effective dates must increase down the sheet, \
                 with no date repeated"
            ),
            RateScheduleErrorKind::NotYetInEffect { date, first } => write!(
                f,
                "no rates are in effect on {date}: the earliest effective_date is {first}"
            ),
            RateScheduleErrorKind::TwoChangesInPeriod {
                period_start,
                period_ending,
                dates,
            } => {
                let dates: Vec<String> = dates.iter().map(Date::to_string).collect();
                write!(
                    f,
                    "the rates change {} times within the billing period {period_start} to \
                     {period_ending}, on {}. A billing period can take one change at most",
                    dates.len(),
                    dates.join(", ")
                )
            }
            RateScheduleErrorKind::ChangeInsideMonth {
                month_start,
                month_end,
                date,
                row,
            } => write!(
                f,
                "the rates change on {date} (row {row}), within the month {month_start} to \
                 {month_end}. A month is reconciled at one set of rates, so they can change only \
                 on the 1st"
            ),
            RateScheduleErrorKind::Rate {
                cell,
                band,
                effective_date,
                problem,
            } => {
                write!(
                    f,
                    "cell {cell}, the {} rate effective {effective_date}, ",
                    band.column()
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

impl Error for RateScheduleError {}

// cargo test --lib -- api::pure::rates::test
#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use jiff::civil::date;

    /// A row of three numeric rates.
    pub(crate) fn row(row: u32, effective: Date, rates: [f64; 3]) -> RateRow {
        RateRow {
            row,
            effective_date: effective,
            cells: rates.map(RateCell::Number),
        }
    }

    /// A schedule built in memory, with the bands in columns B to D as the example workbook has
    /// them.
    pub(crate) fn schedule(rows: Vec<RateRow>) -> RateSchedule {
        RateSchedule::new(None, "rates".to_owned(), [2, 3, 4], rows).expect("a valid schedule")
    }

    /// May's rates and June's, the shape the billing period ending 23 June straddles.
    fn may_and_june() -> RateSchedule {
        schedule(vec![
            row(2, date(2026, 5, 1), [0.11, 0.09, 0.07]),
            row(3, date(2026, 6, 1), [0.12, 0.10, 0.08]),
        ])
    }

    #[test]
    fn the_row_in_effect_is_the_last_one_on_or_before_the_date() {
        let s = may_and_june();

        // A period wholly inside May: May's rates, no change.
        let (opening, change) = s
            .for_billing_period(date(2026, 5, 2), date(2026, 5, 31))
            .expect("May's rates cover it");
        assert_eq!(opening.effective_date, date(2026, 5, 1));
        assert_eq!(opening.on_peak, 0.11);
        assert!(change.is_none());

        // In effect exactly on the first day counts.
        let (opening, change) = s
            .for_billing_period(date(2026, 6, 1), date(2026, 6, 23))
            .expect("June's rates start on the first day");
        assert_eq!(opening.effective_date, date(2026, 6, 1));
        assert!(change.is_none());
    }

    /// The period from 24 May to 23 June opens at May's rates and changes to June's on the 1st.
    #[test]
    fn a_row_dated_inside_the_period_is_its_one_change() {
        let (opening, change) = may_and_june()
            .for_billing_period(date(2026, 5, 24), date(2026, 6, 23))
            .expect("one change inside the period");
        assert_eq!(opening.effective_date, date(2026, 5, 1));
        let change = change.expect("June's row is inside the period");
        assert_eq!(change.effective_date, date(2026, 6, 1));
        assert_eq!(
            (change.on_peak, change.mid_peak, change.off_peak),
            (0.12, 0.10, 0.08)
        );
    }

    #[test]
    fn two_rows_dated_inside_one_period_are_refused() {
        let s = schedule(vec![
            row(2, date(2026, 5, 1), [0.11, 0.09, 0.07]),
            row(3, date(2026, 6, 1), [0.12, 0.10, 0.08]),
            row(4, date(2026, 6, 15), [0.13, 0.11, 0.09]),
        ]);
        let err = s
            .for_billing_period(date(2026, 5, 24), date(2026, 6, 23))
            .expect_err("two changes");
        assert_eq!(
            err.kind,
            RateScheduleErrorKind::TwoChangesInPeriod {
                period_start: date(2026, 5, 24),
                period_ending: date(2026, 6, 23),
                dates: vec![date(2026, 6, 1), date(2026, 6, 15)],
            }
        );
        let message = err.to_string();
        assert!(message.contains("2026-06-01, 2026-06-15"), "{message}");
    }

    #[test]
    fn a_date_before_the_first_row_is_refused_naming_the_first_effective_date() {
        let err = may_and_june()
            .for_month(date(2026, 4, 1))
            .expect_err("April is before May");
        assert_eq!(
            err.kind,
            RateScheduleErrorKind::NotYetInEffect {
                date: date(2026, 4, 1),
                first: date(2026, 5, 1),
            }
        );
        assert_eq!(
            err.to_string(),
            "rates, sheet \"rates\": no rates are in effect on 2026-04-01: the earliest \
             effective_date is 2026-05-01"
        );
    }

    #[test]
    fn a_month_takes_the_row_in_effect_on_the_first() {
        let rates = may_and_june()
            .for_month(date(2026, 7, 1))
            .expect("June's rates are still in effect in July");
        assert_eq!(rates.effective_date, date(2026, 6, 1));
    }

    #[test]
    fn a_row_dated_inside_a_month_after_the_first_is_refused() {
        let s = schedule(vec![
            row(2, date(2026, 6, 1), [0.11, 0.09, 0.07]),
            row(5, date(2026, 6, 15), [0.12, 0.10, 0.08]),
        ]);
        let err = s
            .for_month(date(2026, 6, 1))
            .expect_err("a change mid-June");
        assert_eq!(
            err.kind,
            RateScheduleErrorKind::ChangeInsideMonth {
                month_start: date(2026, 6, 1),
                month_end: date(2026, 6, 30),
                date: date(2026, 6, 15),
                row: 5,
            }
        );
    }

    /// Each way a rate cell can fail names the cell, its column and the rates' date.
    #[test]
    fn a_bad_rate_cell_on_a_row_in_use_is_named() {
        let cases = [
            (RateCell::Empty, RateProblem::Empty, "is empty"),
            (
                RateCell::Text("eleven cents".to_owned()),
                RateProblem::Text("eleven cents".to_owned()),
                "holds \"eleven cents\", which is not a number",
            ),
            (
                RateCell::Number(0.0),
                RateProblem::NotPositive(0.0),
                "is 0. A rate must be greater than zero",
            ),
            (
                RateCell::Number(-0.07),
                RateProblem::NotPositive(-0.07),
                "is -0.07. A rate must be greater than zero",
            ),
            (
                RateCell::Number(f64::NAN),
                RateProblem::NotFinite,
                "is not a finite number",
            ),
        ];
        for (cell, problem, wording) in cases {
            let mut bad = row(3, date(2026, 5, 1), [0.11, 0.09, 0.07]);
            bad.cells[1] = cell;
            let err = schedule(vec![bad])
                .for_month(date(2026, 6, 1))
                .expect_err(wording);
            assert!(
                matches!(
                    &err.kind,
                    RateScheduleErrorKind::Rate {
                        cell,
                        band: RateBand::MidPeak,
                        problem: found,
                        ..
                    } if cell == "C3" && found == &problem
                ),
                "{err:?}"
            );
            let message = err.to_string();
            assert!(
                message.contains("cell C3, the mid_peak rate effective 2026-05-01, "),
                "{message}"
            );
            assert!(message.ends_with(wording), "{message}");
        }
    }

    /// A bad cell on a row nothing uses is never looked at.
    #[test]
    fn a_bad_rate_cell_on_a_row_not_in_use_is_ignored() {
        let mut old = row(2, date(2026, 1, 1), [0.11, 0.09, 0.07]);
        old.cells = [RateCell::Empty, RateCell::Empty, RateCell::Empty];
        let s = schedule(vec![old, row(3, date(2026, 5, 1), [0.11, 0.09, 0.07])]);
        assert!(s.for_month(date(2026, 6, 1)).is_ok());
        assert!(
            s.for_billing_period(date(2026, 5, 24), date(2026, 6, 23))
                .is_ok()
        );

        // The same row, once a period reaches it, is refused.
        let err = s
            .for_billing_period(date(2026, 4, 24), date(2026, 5, 23))
            .expect_err("the January row opens this period");
        assert!(err.to_string().contains("cell B2"), "{err}");
    }

    #[test]
    fn effective_dates_must_strictly_increase() {
        for second in [date(2026, 5, 1), date(2026, 1, 9)] {
            let err = RateSchedule::new(
                Some(PathBuf::from("Rates.xlsx")),
                "rates".to_owned(),
                [2, 3, 4],
                vec![
                    row(2, date(2026, 5, 1), [0.11, 0.09, 0.07]),
                    row(3, second, [0.12, 0.10, 0.08]),
                ],
            )
            .expect_err("not increasing");
            assert_eq!(
                err.kind,
                RateScheduleErrorKind::NotIncreasing {
                    row: 3,
                    date: second,
                    previous_row: 2,
                    previous: date(2026, 5, 1),
                }
            );
            assert!(
                err.to_string()
                    .starts_with("rates workbook Rates.xlsx, sheet \"rates\": "),
                "{err}"
            );
        }
    }

    #[test]
    fn a_schedule_with_no_rows_is_refused() {
        let err = RateSchedule::new(None, "rates".to_owned(), [2, 3, 4], Vec::new())
            .expect_err("no rows");
        assert_eq!(err.kind, RateScheduleErrorKind::NoRows);
    }

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
