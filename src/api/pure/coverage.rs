//! Whether the session reports named reach across a billing period.
//!
//! The two facts this joins live apart, and stay apart. What a report's file name says is a fact
//! about Evolute's exports, so it is
//! the private `session::file_name`; what a billing period spans is a fact about
//! the bill, so it is [`hydro_bill`](crate::hydro_bill). Neither module knows the other exists.
//! Asking whether one covers the other is a question in the API's terms, which is why it is asked
//! here.
//!
//! Nothing here opens anything. A `&Path` is read as a string.

use crate::{
    hydro_bill::{NotABillingPeriodEnding, billing_period_dates},
    session::{SessionReportNameError, parse_session_report_name, reports_cover},
};
use jiff::civil::Date;
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

// Re-exported because it is what this module's function returns and what `CoverageError` carries,
// so a caller cannot spell either without it. Its own module is where it is documented.
pub use crate::session::SessionReportCoverage;

/// Why the session reports named cannot be checked against a billing period, or do not cover it.
///
/// Every variant is settled from the file *names*. Nothing here has opened anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageError {
    /// See [`NotABillingPeriodEnding`].
    NotABillingPeriodEnding(NotABillingPeriodEnding),

    /// A session report's file name does not state the dates it covers, so it cannot be checked
    /// against the billing period.
    ///
    /// The cause says which of the ways it failed, and carries the expected form. Stated there
    /// rather than here so there is one wording of it in the crate.
    UndatedSessionReport {
        path: PathBuf,
        cause: SessionReportNameError,
    },

    /// No session reports were given at all.
    ///
    /// Distinct from [`Self::PeriodNotCovered`], which lists what each report covers: with no
    /// reports there is nothing to list, and that message ends in a colon with nothing after it.
    NoReports {
        span: CoveredSpan,
        first: Date,
        last: Date,
    },

    /// The session reports given do not cover the whole span between them.
    ///
    /// Almost always the wrong months handed in. The alternative is an estimate that reads as a
    /// small or zero EV contribution, which is a figure someone may go on to argue a bill from.
    PeriodNotCovered {
        /// What the span is, so the message can name it. Two calendars reach this variant.
        span: CoveredSpan,
        period_start: Date,
        period_ending: Date,
        coverage: Vec<SessionReportCoverage>,
    },
}

/// Which calendar a span of dates belongs to.
///
/// The two do not coincide: a billing period runs midnight starting the 24th to midnight starting
/// the 24th of the next month, and a calendar month runs the 1st to the last. Telling a user their
/// reports miss part of "the billing period 1 June to 30 June" names a period that does not exist,
/// and sends them to check the wrong dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoveredSpan {
    /// The bill's own period, closing on [`BILL_END_DAY`](crate::hydro_bill::BILL_END_DAY).
    BillingPeriod,
    /// A calendar month. What the reimbursement reconciliation is over, taken from the Charges
    /// Report's own file name.
    CalendarMonth,
}

impl CoveredSpan {
    /// The span's name as the messages write it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BillingPeriod => "billing period",
            Self::CalendarMonth => "month",
        }
    }
}

impl From<NotABillingPeriodEnding> for CoverageError {
    fn from(e: NotABillingPeriodEnding) -> Self {
        Self::NotABillingPeriodEnding(e)
    }
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABillingPeriodEnding(e) => e.fmt(f),
            // Deferred to entirely. Every `SessionReportNameError` opens with the name it is
            // about, so a `{path}: ` prefix here printed it twice -- `data/June.csv: June is not a
            // session report`. The rule in CLAUDE.md: a wrapper that adds the path must not wrap a
            // cause that already carries one. `path` stays on the variant for a caller that wants
            // to act on which file failed rather than print it.
            Self::UndatedSessionReport { cause, .. } => cause.fmt(f),
            Self::NoReports { span, first, last } => write!(
                f,
                "no session reports were given, so the {} {first} to {last} is not covered at all",
                span.as_str()
            ),
            Self::PeriodNotCovered {
                span,
                period_start,
                period_ending,
                coverage,
            } => {
                write!(
                    f,
                    "the session reports do not cover the {} {period_start} to {period_ending}:",
                    span.as_str()
                )?;
                for c in coverage {
                    write!(f, "\n  {} covers {} to {}", c.path.display(), c.from, c.to)?;
                }
                Ok(())
            }
        }
    }
}

impl Error for CoverageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotABillingPeriodEnding(e) => Some(e),
            Self::UndatedSessionReport { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

/// Checks that the named session reports cover the billing period completely between them, and
/// returns what each one covers.
///
/// Worth calling before anything is opened: a caller that has handed in the wrong month is told so
/// rather than after a year of meter readings has been parsed.
///
/// # Errors
///
/// [`CoverageError::NotABillingPeriodEnding`]; [`CoverageError::UndatedSessionReport`] for a name
/// that does not say what it covers; and [`CoverageError::PeriodNotCovered`] when the names between
/// them leave any day of the period unaccounted for.
pub fn check_reports_cover_period(
    billing_period_ending: Date,
    report_paths: &[&Path],
) -> Result<Vec<SessionReportCoverage>, CoverageError> {
    let (period_start, period_ending) = billing_period_dates(billing_period_ending)?;
    check_reports_cover(
        CoveredSpan::BillingPeriod,
        period_start,
        period_ending,
        report_paths,
    )
}

/// Checks that the named session reports cover `first` to `last` inclusive between them, and
/// returns what each one covers.
///
/// The general form of [`check_reports_cover_period`], for a caller whose span is not a billing
/// period. The reimbursement reconciliation's is a calendar month, taken from the Charges Report's
/// own name.
///
/// `span` says which calendar `first` and `last` came from, and is used for nothing but the
/// message. Without it every refusal calls the span a billing period, including the ones that are
/// a month — see [`CoveredSpan`].
///
/// **How many reports there are is not a rule here.** One covering the whole span is as good as
/// three, and a report reaching outside it neither helps nor blocks. What is refused is a gap.
///
/// # Errors
///
/// [`CoverageError::UndatedSessionReport`] for a name that does not say what it covers, and
/// [`CoverageError::PeriodNotCovered`] when the names between them leave any day unaccounted for.
pub fn check_reports_cover(
    span: CoveredSpan,
    first: Date,
    last: Date,
    report_paths: &[&Path],
) -> Result<Vec<SessionReportCoverage>, CoverageError> {
    // Refused before anything else. An empty slice reaches `reports_cover` and fails there, so the
    // message that comes back is a gap with nothing listed under it -- a sentence ending in a
    // colon. Every entry point accepts the empty slice, so this is where it is caught.
    if report_paths.is_empty() {
        return Err(CoverageError::NoReports { span, first, last });
    }

    let coverage = report_paths
        .iter()
        .map(|path| {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let (from, to) = parse_session_report_name(stem).map_err(|cause| {
                CoverageError::UndatedSessionReport {
                    path: path.to_path_buf(),
                    cause,
                }
            })?;
            Ok::<_, CoverageError>(SessionReportCoverage {
                path: path.to_path_buf(),
                from,
                to,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if !reports_cover(first, last, &coverage) {
        return Err(CoverageError::PeriodNotCovered {
            span,
            period_start: first,
            period_ending: last,
            coverage,
        });
    }
    Ok(coverage)
}

#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;

    /// An empty list of reports is refused, and says so.
    ///
    /// Every entry point accepts `&[]`. Without this the message is the coverage one with nothing
    /// listed under it -- `the session reports do not cover the billing period 2026-05-24 to
    /// 2026-06-23:` and then the end of the output.
    #[test]
    fn no_reports_at_all_is_refused_in_its_own_words() {
        let err = check_reports_cover_period(date(2026, 6, 23), &[])
            .expect_err("no reports cover nothing");
        assert!(matches!(err, CoverageError::NoReports { .. }), "{err:?}");

        let message = err.to_string();
        assert!(
            message.contains("no session reports were given"),
            "{message}"
        );
        assert!(!message.ends_with(':'), "{message}");
    }

    /// A refusal names the calendar its dates came from.
    ///
    /// The reimbursement reconciliation reaches this variant over a calendar month, and a message
    /// calling 1 June to 30 June a billing period names a period that does not exist -- the bill's
    /// runs the 24th to the 23rd.
    #[test]
    fn a_refusal_names_the_calendar_its_dates_came_from() {
        let april = Path::new("Session_Report_April_1_2026-April_30_2026.csv");

        let month = check_reports_cover(
            CoveredSpan::CalendarMonth,
            date(2026, 6, 1),
            date(2026, 6, 30),
            &[april],
        )
        .expect_err("April does not cover June")
        .to_string();
        assert!(
            month.contains("the month 2026-06-01 to 2026-06-30"),
            "{month}"
        );
        assert!(!month.contains("billing period"), "{month}");

        let period = check_reports_cover_period(date(2026, 6, 23), &[april])
            .expect_err("April does not cover the period")
            .to_string();
        assert!(
            period.contains("the billing period 2026-05-24 to 2026-06-23"),
            "{period}"
        );
    }

    /// The names alone settle whether the reports reach the period, so this answers without any of
    /// the files existing.
    #[test]
    fn coverage_is_checked_from_the_names_alone() {
        let may = Path::new("Session_Report_May_1_2026-May_31_2026.csv");
        let june = Path::new("Session_Report_June_1_2026-June_30_2026.csv");
        let april = Path::new("Session_Report_April_1_2026-April_30_2026.csv");

        assert_eq!(
            check_reports_cover_period(date(2026, 6, 23), &[may, june])
                .expect("May and June cover the period")
                .len(),
            2
        );

        let err = check_reports_cover_period(date(2026, 6, 23), &[april, june])
            .expect_err("April and June do not cover a period starting 24 May");
        assert!(
            matches!(err, CoverageError::PeriodNotCovered { .. }),
            "{err}"
        );
        assert!(err.to_string().contains("2026-05-24"), "{err}");

        let err = check_reports_cover_period(date(2026, 6, 23), &[Path::new("June.csv"), june])
            .expect_err("a name that does not state its dates");
        assert!(
            matches!(err, CoverageError::UndatedSessionReport { .. }),
            "{err}"
        );

        // The closing date is checked first, so a date that labels no period is reported as such
        // rather than as a coverage failure.
        let err = check_reports_cover_period(date(2026, 6, 30), &[may, june])
            .expect_err("30 June does not label a billing period");
        assert!(
            matches!(err, CoverageError::NotABillingPeriodEnding(_)),
            "{err}"
        );
    }
}
