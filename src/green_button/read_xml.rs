//! Reading one billing period's figures straight out of a Green Button export.
//!
//! [`period_values`] takes a parsed [`Readings`] and returns every period the feed covers. A
//! caller checking one invoice starts from a path and a closing date, and needs to be told when
//! the feed does not reach that period rather than handed nothing. This module is that call.

use crate::{
    green_button::{Feed, PeriodValues, Readings, parse_espi_xml, period_values},
    hydro_bill::MAX_BILL_END_DAY,
    time::local_date,
};
use jiff::civil::Date;
use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

/// Why a Green Button export could not be read.
///
/// Every variant that concerns a file carries its `path` as a field and writes it in the message.
/// Nothing here is pre-formatted into a string: a caller wanting the date the feed stops at, or
/// the file to retry, reads the field rather than parsing the sentence.
#[derive(Debug)]
pub enum GbReadError {
    /// The file could not be opened or read.
    Unreadable { path: PathBuf, cause: io::Error },

    /// The bytes were read but are not a well-formed ESPI feed.
    Malformed {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// The feed carries no reading in the billing period asked for.
    ///
    /// An error rather than an empty row: a caller checking an invoice against a period the export
    /// does not reach needs to be told so, not handed zeroes.
    PeriodNotCovered {
        path: PathBuf,
        period_ending: Date,
        /// The first and last day the feed does cover, `None` when it carries no readings at all.
        covers: Option<(Date, Date)>,
    },

    /// `period_ending` is not `bill_end_day` of its month, so the two describe different
    /// calendars. No `path`: settled before any file is opened.
    NotABillingCalendar {
        period_ending: Date,
        bill_end_day: i8,
    },

    /// `bill_end_day` is outside `1..=MAX_BILL_END_DAY`. No `path`, for the same reason.
    BillEndDayOutOfRange { bill_end_day: i8 },
}

impl GbReadError {
    /// The export the failure is about, or `None` for the two argument checks, which are settled
    /// before a file is opened.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Unreadable { path, .. }
            | Self::Malformed { path, .. }
            | Self::PeriodNotCovered { path, .. } => Some(path),
            Self::NotABillingCalendar { .. } | Self::BillEndDayOutOfRange { .. } => None,
        }
    }
}

impl fmt::Display for GbReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, cause } => write!(f, "{}: {cause}", path.display()),
            Self::Malformed { path, cause } => write!(f, "{}: {cause}", path.display()),
            Self::PeriodNotCovered {
                path,
                period_ending,
                covers,
            } => {
                write!(
                    f,
                    "{}: no readings in the billing period ending {period_ending}. ",
                    path.display()
                )?;
                match covers {
                    Some((first, last)) => write!(f, "The feed covers {first} to {last}."),
                    None => write!(f, "The feed carries no readings at all."),
                }
            }
            Self::NotABillingCalendar {
                period_ending,
                bill_end_day,
            } => write!(
                f,
                "{period_ending} cannot end a billing period that closes on day {bill_end_day} of \
                 the month; the closing date and the calendar disagree"
            ),
            Self::BillEndDayOutOfRange { bill_end_day } => write!(
                f,
                "bill_end_day is {bill_end_day}; a billing period must close on day 1 to \
                 {MAX_BILL_END_DAY} of the month, since it starts on the day after and that day \
                 has to exist in February"
            ),
        }
    }
}

impl Error for GbReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unreadable { cause, .. } => Some(cause),
            Self::Malformed { cause, .. } => Some(cause.as_ref()),
            Self::PeriodNotCovered { .. }
            | Self::NotABillingCalendar { .. }
            | Self::BillEndDayOutOfRange { .. } => None,
        }
    }
}

/// Reads the Green Button XML at `xml_path` and returns the values for the billing period ending
/// on `period_ending`.
///
/// `bill_end_day` is the day of the month the bill closes on — [`BILL_END_DAY`](crate::hydro_bill::BILL_END_DAY)
/// for Toronto Hydro — and must be `period_ending`'s day. It is stated rather than inferred
/// because which day a bill closes on belongs to the bill and not to the meter data; see
/// `green_button::billing`.
///
/// A period the feed covers only partly is returned, not refused: the feed's coverage and the
/// bill's period are independent facts, and which discrepancies matter is the caller's judgement.
/// [`PeriodValues::is_complete`] says whether every hour is there, and
/// [`PeriodValues::anomaly_counts`] carries every anomaly found within the period.
///
/// # Errors
///
/// Returns `Err` if the file cannot be read, if the XML cannot be parsed as an ESPI feed, if
/// `period_ending` is not `bill_end_day` of its month, if `bill_end_day` is outside
/// `1..=MAX_BILL_END_DAY`, or if the feed carries no reading in the period asked for. The last is
/// an error rather than an empty row: a caller checking an invoice against a period the export
/// does not reach needs to be told so, not handed zeroes.
pub fn read_gb_for_billing_period(
    xml_path: &Path,
    period_ending: Date,
    bill_end_day: i8,
) -> Result<PeriodValues, GbReadError> {
    check_calendar(period_ending, bill_end_day)?;
    let feed = read_gb_feed(xml_path)?;
    let readings = feed.readings().from_source(xml_path);

    // `period_values` already scopes each period's anomaly counts to that period, so picking the
    // row out of its result is the whole of the work. Nothing is recomputed here, and no second
    // anomaly channel is built beside the counts it carries.
    period_values(&readings, bill_end_day)
        .into_iter()
        .find(|p| p.period.ending == period_ending)
        .ok_or_else(|| GbReadError::PeriodNotCovered {
            path: xml_path.to_path_buf(),
            period_ending,
            covers: coverage(&readings),
        })
}

/// Reads and parses the Green Button XML at `xml_path`.
///
/// # Errors
///
/// Returns `Err` if the file cannot be read or the bytes are not a well-formed ESPI feed. Both
/// name `xml_path`, so a caller holding the path should not add it again.
pub fn read_gb_feed(xml_path: &Path) -> Result<Feed, GbReadError> {
    let xml = fs::read_to_string(xml_path).map_err(|cause| GbReadError::Unreadable {
        path: xml_path.to_path_buf(),
        cause,
    })?;
    parse_espi_xml(&xml).map_err(|cause| GbReadError::Malformed {
        path: xml_path.to_path_buf(),
        cause,
    })
}

/// Rejects a `period_ending` and `bill_end_day` that cannot describe the same calendar.
///
/// Runs before the file is opened, so a caller who has mixed up two calendars is told that rather
/// than being sent looking at the export. Neither error it raises carries a path, for that reason.
fn check_calendar(period_ending: Date, bill_end_day: i8) -> Result<(), GbReadError> {
    // Both checks belong here rather than to `BillingPeriod::ending_on`, which asserts the second
    // and would build an invalid `Date` on the first. Either way it panics, and these arguments
    // come from a caller.
    if !(1..=MAX_BILL_END_DAY).contains(&bill_end_day) {
        return Err(GbReadError::BillEndDayOutOfRange { bill_end_day });
    }
    if period_ending.day() != bill_end_day {
        return Err(GbReadError::NotABillingCalendar {
            period_ending,
            bill_end_day,
        });
    }
    Ok(())
}

/// The first and last day the feed covers, for an error that says where to look next.
fn coverage(readings: &Readings) -> Option<(Date, Date)> {
    match (readings.rows.first(), readings.rows.last()) {
        (Some(first), Some(last)) => Some((local_date(first.start), local_date(last.start))),
        _ => None,
    }
}

// cargo test --lib -- green_button::read_xml::test --nocapture
#[cfg(test)]
mod test {
    use super::*;
    use crate::hydro_bill::BILL_END_DAY;
    use jiff::civil::date;
    use std::path::PathBuf;

    /// The arguments are checked before the file is, so a mismatched calendar is reported as such
    /// rather than as a missing file.
    fn err_for(ending: Date, bill_end_day: i8) -> String {
        read_gb_for_billing_period(
            &PathBuf::from("/nonexistent/feed.XML"),
            ending,
            bill_end_day,
        )
        .unwrap_err()
        .to_string()
    }

    #[test]
    fn a_closing_date_off_the_billing_calendar_is_refused() {
        let err = err_for(date(2026, 6, 30), BILL_END_DAY);
        assert!(err.contains("2026-06-30"), "{err}");
        assert!(err.contains("disagree"), "{err}");
    }

    /// The day after `MAX_BILL_END_DAY` would start a period on February 29th, which exists in one
    /// year out of four. `BillingPeriod::ending_on` would build that date and panic.
    #[test]
    fn a_closing_day_that_would_start_a_period_on_a_missing_date_is_refused() {
        let err = err_for(date(2026, 6, 28), 28);
        assert!(err.contains("February"), "{err}");

        // One day earlier is the last legal closing day, so it gets past the calendar check and
        // fails on the file instead.
        let err = err_for(date(2026, 6, 27), 27);
        assert!(err.contains("/nonexistent/feed.XML"), "{err}");
    }

    #[test]
    fn a_missing_file_is_named() {
        let err = err_for(date(2026, 6, 23), BILL_END_DAY);
        assert!(err.contains("/nonexistent/feed.XML"), "{err}");
    }

    /// The figures carry the file they came from. An anomaly counted in a period is a fact about an
    /// export, and this is the only thing in the result that says which one.
    #[test]
    fn the_period_carries_the_export_it_was_read_from() {
        let xml = Path::new("data/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML");
        if !xml.exists() {
            return; // The export is not in every checkout.
        }
        let values = read_gb_for_billing_period(xml, date(2026, 6, 23), BILL_END_DAY)
            .expect("the export covers the June 2026 period");
        assert_eq!(values.source.as_deref(), Some(xml));
    }
}
