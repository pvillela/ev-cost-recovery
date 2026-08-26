//! Golden-file test for the rendered [`IntervalEstimates`](super::IntervalEstimates).
//!
//! Each case pairs an input CSV in `tests/fixtures/sessions/` with the report it must produce,
//! checked byte for byte. Layout is the thing under test, and layout is only judged by looking at
//! it — so the expectation is a file you can read rather than a list of assertions about
//! substrings. A change in wrapping, padding or column order shows up as a diff in the golden
//! file, which is exactly where it should be visible during review.
//!
//! A unit test, for the reason [`super::segment_tiling_tests`] gives: the renderer's input comes
//! from [`estimates_from_sessions`](super::estimates_from_sessions), and reaching that from
//! outside the crate means going through `api::peak_power`, which chooses the interval off a
//! meter export rather than taking the one these cases pin.
//!
//! The cases between them cover every shape the renderer has: a report whose four segments differ
//! from each other, so the Estimates section has a maximal quarter to name and the Segments table
//! has something to show; and one carrying session anomalies and an excluded-sessions section.
//!
//! What the golden files *are*, as opposed to what produces them, is checked from `tests/` — see
//! `tests/session/report_rendering.rs`, which reads them as text and needs no renderer.
//!
//! These files are the one deliberate exception to the rule that no test may depend on the value
//! of a freely-declared constant. They pin *rendering* — column widths, decimal places, wrapping —
//! and no relational reformulation preserves any of that. Changing an electrical constant is
//! therefore expected to fail exactly these tests and no others; see `docs/maintenance-manual.md`,
//! "Which constants are free, and which are derived".
//!
//! To regenerate after an intended change, having read the diff:
//!
//! ```sh
//! UPDATE_REPORT_GOLDEN=1 cargo test --lib -- session::report_rendering_tests
//! ```

use crate::{
    golden,
    session::{csv::csv_sessions, estimates_from_sessions},
    time::Interval,
};
use jiff::Timestamp;

/// `(fixture stem, interval start UTC, interval end UTC)`.
///
/// Both sit on 2026-06-15, a date with no DST transition, and run 16:00–17:00 local — a legal
/// interval of interest per README.
const CASES: [(&str, &str, &str); 2] = [
    (
        "Session_Report_Diagram",
        "2026-06-15T20:00:00Z",
        "2026-06-15T21:00:00Z",
    ),
    (
        "Session_Report_Anomalies",
        "2026-06-15T20:00:00Z",
        "2026-06-15T21:00:00Z",
    ),
];

/// Reads the fixture and renders its report.
///
/// The fixture is read where it sits. Nothing is written and no scratch directory is needed, so
/// the `Source` line the report prints is the fixture's own name rather than a temporary path.
fn render(stem: &str, lo: &str, hi: &str) -> String {
    let path = golden::fixture(&format!("sessions/{stem}.csv"));
    let sessions = csv_sessions(&path).unwrap_or_else(|e| panic!("{stem} reads: {e}"));

    let (lo, hi): (Timestamp, Timestamp) = (lo.parse().unwrap(), hi.parse().unwrap());
    let interval = Interval::from_start_end(lo, hi);
    let report = estimates_from_sessions(interval, sessions.sources.clone(), &sessions);

    let rendered = report.to_markdown();
    // Display must agree, or there would be two renderings to keep in step.
    assert_eq!(format!("{report}"), rendered, "{stem}: Display disagrees");
    rendered
}

#[test]
fn rendered_reports_match_their_golden_files() {
    for (stem, lo, hi) in CASES {
        golden::check(&format!("sessions/{stem}.report.md"), &render(stem, lo, hi));
    }
}
