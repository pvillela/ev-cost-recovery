//! The rates the workbook prices a billing period or a calendar month at: the two functions
//! `api::io` calls, and which rows of the sheet they choose.
//!
//! The rates in effect on a date are those of the last row taking effect on or before it. A
//! billing period is priced at the rates in effect on its first day and changes at most once
//! within it; a month is priced at the rates in effect on the 1st and does not change within it.
//! Only the rows chosen have their rate cells checked.

use super::{
    error::{RateProblem, RatesWorkbookError},
    read::{BANDS, RateCell, RateRow, RateSheet, rate_sheet},
};
use crate::rates::CostRecoveryRates;
use jiff::civil::Date;
use std::path::Path;

/// The rates the workbook at `path` prices the billing period from `period_start` to
/// `period_ending` at: those in effect on its first day, and those of the row dated within the
/// period, if there is one.
///
/// # Errors
///
/// [`RatesWorkbookError`]: the file breaks one of the workbook's rules, or its rows do not price
/// the period — none is in effect on its first day, more than one is dated within it, or a rate
/// cell of a row selected is not a positive number.
pub(crate) fn read_rates_for_billing_period(
    path: &Path,
    period_start: Date,
    period_ending: Date,
) -> Result<(CostRecoveryRates, Option<CostRecoveryRates>), RatesWorkbookError> {
    rate_sheet(path)?.for_billing_period(period_start, period_ending)
}

/// The rates the workbook at `path` prices the calendar month starting `month_start` at: those in
/// effect on the 1st.
///
/// # Errors
///
/// [`RatesWorkbookError`]: the file breaks one of the workbook's rules, or its rows do not price
/// the month — none is in effect on the 1st, one is dated after the 1st and within the month, or a
/// rate cell of the row in effect is not a positive number.
pub(crate) fn read_rates_for_month(
    path: &Path,
    month_start: Date,
) -> Result<CostRecoveryRates, RatesWorkbookError> {
    rate_sheet(path)?.for_month(month_start)
}

impl RateSheet {
    /// The rates that price the billing period from `period_start` to `period_ending`, inclusive:
    /// those in effect on its first day, and those of the row dated within the period, if there is
    /// one.
    ///
    /// # Errors
    ///
    /// [`RatesWorkbookError::NotYetInEffect`] if no row is in effect on the first day,
    /// [`RatesWorkbookError::TwoChangesInPeriod`] if more than one row is dated within the period,
    /// and [`RatesWorkbookError::Rate`] for the first rate cell of the rows chosen that is not a
    /// positive number.
    fn for_billing_period(
        &self,
        period_start: Date,
        period_ending: Date,
    ) -> Result<(CostRecoveryRates, Option<CostRecoveryRates>), RatesWorkbookError> {
        let opening = self.in_effect_on(period_start)?;
        let changes: Vec<&RateRow> = self.dated_within(period_start, period_ending).collect();
        let change = match changes[..] {
            [] => None,
            [only] => Some(only),
            _ => {
                return Err(RatesWorkbookError::TwoChangesInPeriod {
                    path: self.path.clone(),
                    sheet: self.sheet.clone(),
                    period_start,
                    period_ending,
                    dates: changes.iter().map(|r| r.effective_date).collect(),
                });
            }
        };
        Ok((
            self.rates(opening)?,
            change.map(|row| self.rates(row)).transpose()?,
        ))
    }

    /// The rates that price the calendar month starting `month_start`: those in effect on the 1st.
    ///
    /// # Errors
    ///
    /// [`RatesWorkbookError::NotYetInEffect`] if no row is in effect on the 1st,
    /// [`RatesWorkbookError::ChangeInsideMonth`] if a row is dated after the 1st and within the
    /// month, and [`RatesWorkbookError::Rate`] for the first rate cell of the row in effect that is
    /// not a positive number.
    fn for_month(&self, month_start: Date) -> Result<CostRecoveryRates, RatesWorkbookError> {
        let month_end = month_start.last_of_month();
        let row = self.in_effect_on(month_start)?;
        if let Some(change) = self.dated_within(month_start, month_end).next() {
            return Err(RatesWorkbookError::ChangeInsideMonth {
                path: self.path.clone(),
                sheet: self.sheet.clone(),
                month_start,
                month_end,
                date: change.effective_date,
                row: change.row,
            });
        }
        self.rates(row)
    }

    /// The last row taking effect on or before `date`.
    fn in_effect_on(&self, date: Date) -> Result<&RateRow, RatesWorkbookError> {
        self.rows
            .iter()
            .rev()
            .find(|r| r.effective_date <= date)
            .ok_or_else(|| RatesWorkbookError::NotYetInEffect {
                path: self.path.clone(),
                sheet: self.sheet.clone(),
                date,
                first: self.rows[0].effective_date,
            })
    }

    /// The rows taking effect after `first` and on or before `last`.
    fn dated_within(&self, first: Date, last: Date) -> impl Iterator<Item = &RateRow> {
        self.rows
            .iter()
            .filter(move |r| first < r.effective_date && r.effective_date <= last)
    }

    /// One row's rates, each checked to be a positive number.
    fn rates(&self, row: &RateRow) -> Result<CostRecoveryRates, RatesWorkbookError> {
        let mut rates = [0.0; 3];
        for (i, tou) in BANDS.into_iter().enumerate() {
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
            return Err(RatesWorkbookError::Rate {
                path: self.path.clone(),
                sheet: self.sheet.clone(),
                column: self.rate_columns[i],
                row: row.row,
                tou,
                effective_date: row.effective_date,
                problem,
            });
        }
        let [on_peak, mid_peak, off_peak] = rates;
        Ok(CostRecoveryRates {
            effective_date: row.effective_date,
            on_peak,
            mid_peak,
            off_peak,
        })
    }
}

// cargo test --lib -- rates::excel::select::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::time::Tou;
    use jiff::civil::date;
    use std::path::PathBuf;

    /// A row of three numeric rates.
    fn row(row: u32, effective: Date, rates: [f64; 3]) -> RateRow {
        RateRow {
            row,
            effective_date: effective,
            cells: rates.map(RateCell::Number),
        }
    }

    /// A sheet of `rows`, with the rates in columns B to D as the example workbook has them.
    fn sheet(rows: Vec<RateRow>) -> RateSheet {
        RateSheet {
            path: PathBuf::from("Rates.xlsx"),
            sheet: "rates".to_owned(),
            rate_columns: [2, 3, 4],
            rows,
        }
    }

    /// May's rates and June's, the shape the billing period ending 23 June straddles.
    fn may_and_june() -> RateSheet {
        sheet(vec![
            row(2, date(2026, 5, 1), [0.11, 0.09, 0.07]),
            row(3, date(2026, 6, 1), [0.12, 0.10, 0.08]),
        ])
    }

    #[test]
    fn the_row_in_effect_is_the_last_one_on_or_before_the_date() {
        let s = may_and_june();

        let (opening, change) = s
            .for_billing_period(date(2026, 5, 2), date(2026, 5, 31))
            .expect("May's rates cover it");
        assert_eq!(opening.effective_date, date(2026, 5, 1));
        assert_eq!(opening.on_peak, 0.11);
        // June's row is dated the day after the period ends, so it belongs to the next one.
        assert!(change.is_none());

        // In effect exactly on the first day counts, and is not a change.
        let (opening, change) = s
            .for_billing_period(date(2026, 6, 1), date(2026, 6, 23))
            .expect("June's rates start on the first day");
        assert_eq!(opening.effective_date, date(2026, 6, 1));
        assert!(change.is_none());
    }

    /// The period from 24 May to 23 June opens at May's rates and changes to June's on the 1st.
    #[test]
    fn a_row_dated_within_the_period_is_its_one_change() {
        let (opening, change) = may_and_june()
            .for_billing_period(date(2026, 5, 24), date(2026, 6, 23))
            .expect("one change within the period");
        assert_eq!(opening.effective_date, date(2026, 5, 1));
        let change = change.expect("June's row is within the period");
        assert_eq!(change.effective_date, date(2026, 6, 1));
        assert_eq!(
            (change.on_peak, change.mid_peak, change.off_peak),
            (0.12, 0.10, 0.08)
        );
    }

    #[test]
    fn two_rows_dated_within_one_period_are_refused() {
        let s = sheet(vec![
            row(2, date(2026, 5, 1), [0.11, 0.09, 0.07]),
            row(3, date(2026, 6, 1), [0.12, 0.10, 0.08]),
            row(4, date(2026, 6, 15), [0.13, 0.11, 0.09]),
        ]);
        let err = s
            .for_billing_period(date(2026, 5, 24), date(2026, 6, 23))
            .expect_err("two changes");
        assert!(
            matches!(&err, RatesWorkbookError::TwoChangesInPeriod { dates, .. }
                if dates == &[date(2026, 6, 1), date(2026, 6, 15)]),
            "{err:?}"
        );
        assert!(
            err.to_string().contains("on 2026-06-01, 2026-06-15."),
            "{err}"
        );
    }

    #[test]
    fn a_date_before_the_first_row_is_refused_naming_the_first_effective_date() {
        let err = may_and_june()
            .for_month(date(2026, 4, 1))
            .expect_err("April is before May");
        assert_eq!(
            err.to_string(),
            "rates workbook Rates.xlsx, sheet \"rates\": no rates are in effect on 2026-04-01: the \
             earliest effective_date is 2026-05-01"
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
    fn a_row_dated_within_a_month_after_the_first_is_refused() {
        let s = sheet(vec![
            row(2, date(2026, 6, 1), [0.11, 0.09, 0.07]),
            row(5, date(2026, 6, 15), [0.12, 0.10, 0.08]),
        ]);
        let err = s
            .for_month(date(2026, 6, 1))
            .expect_err("a change mid-June");
        assert!(
            matches!(
                err,
                RatesWorkbookError::ChangeInsideMonth { month_end, row: 5, .. }
                    if month_end == date(2026, 6, 30)
            ),
            "{err:?}"
        );
    }

    /// Each way a rate cell can fail names the cell, its column and the rates' date.
    #[test]
    fn a_bad_rate_cell_on_a_chosen_row_is_named() {
        let cases = [
            (RateCell::Empty, "is empty"),
            (
                RateCell::Text("eleven cents".to_owned()),
                "holds \"eleven cents\", which is not a number",
            ),
            (
                RateCell::Number(0.0),
                "is 0. A rate must be greater than zero",
            ),
            (
                RateCell::Number(-0.07),
                "is -0.07. A rate must be greater than zero",
            ),
            (RateCell::Number(f64::NAN), "is not a finite number"),
        ];
        for (cell, wording) in cases {
            let mut bad = row(3, date(2026, 5, 1), [0.11, 0.09, 0.07]);
            bad.cells[1] = cell;
            let err = sheet(vec![bad])
                .for_month(date(2026, 6, 1))
                .expect_err(wording);
            assert!(
                matches!(
                    err,
                    RatesWorkbookError::Rate {
                        column: 3,
                        row: 3,
                        tou: Tou::MidPeak,
                        ..
                    }
                ),
                "{err:?}"
            );
            assert_eq!(
                err.to_string(),
                format!(
                    "rates workbook Rates.xlsx, sheet \"rates\": cell C3, the mid_peak rate \
                     effective 2026-05-01, {wording}"
                )
            );
        }
    }

    /// A bad cell on a row nothing chooses is never looked at.
    #[test]
    fn a_bad_rate_cell_on_a_row_not_chosen_is_ignored() {
        let mut old = row(2, date(2026, 1, 1), [0.11, 0.09, 0.07]);
        old.cells = [RateCell::Empty, RateCell::Empty, RateCell::Empty];
        let s = sheet(vec![old, row(3, date(2026, 5, 1), [0.11, 0.09, 0.07])]);
        assert!(s.for_month(date(2026, 6, 1)).is_ok());
        assert!(
            s.for_billing_period(date(2026, 5, 24), date(2026, 6, 23))
                .is_ok()
        );

        // The same row, once a period reaches it, is refused.
        let err = s
            .for_billing_period(date(2026, 4, 24), date(2026, 5, 23))
            .expect_err("the January row opens this period");
        assert!(
            matches!(
                err,
                RatesWorkbookError::Rate {
                    column: 2,
                    row: 2,
                    ..
                }
            ),
            "{err:?}"
        );
    }
}
