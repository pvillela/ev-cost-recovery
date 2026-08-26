//! The `InconsistentDuration` band, and its consequence: which sessions reach an estimate and
//! which are put aside.
//!
//! A unit test rather than an integration test, because what it pins is internal. The band is
//! [`duration_is_consistent`](super::duration_is_consistent); the split is
//! `Sessions::from_session_lists`; what draws on the result is
//! [`estimates_from_sessions`](super::estimates_from_sessions). The API reaches all three through
//! `api::peak_power`, which also wants a meter export and a bill, and neither of those bears on
//! the band. So this reads the CSV and calls the two functions the band actually runs through.
//!
//! Unit tests in [`super::csv`] already pin the predicate. This pins the *consequence*. Nothing
//! did that before, which is how commit `1d99e29` moved the band by a whole `TIME_GRID_STEP` —
//! excluding 116 of the 238 sessions in the real June report — with the whole suite green. See
//! `docs/archive/merger-review-findings.md`, finding 1.
//!
//! # The arithmetic every record here is one second away from
//!
//! With a reported start of `16:10` and end of `16:40`, `Δ = 30:00`, the three checks of
//! `duration_is_consistent` reduce to a closed range of sound durations:
//!
//! ```text
//! check 2:  16:10 + d  <  16:40 + 60s + 1s   =>  d <= 0:31:00
//! check 3:  16:40 - 60s  <  16:10 + d        =>  d >= 0:29:01
//! ```
//!
//! So `[0:29:01, 0:31:00]` is sound and everything outside it is not. Each record in the fixture
//! sits on one of those two edges or one second past it. Getting a bound wrong by a single second
//! moves a real record between the two groups, so a second is the right resolution to test at.
//!
//! `INVERT1` tests check 1 on its own, and is the case with no equivalent among the predicate's
//! own unit tests: a one-minute inversion is the smallest the reporting can express, since both
//! reported times are whole minutes, and that forces the duration to zero. It therefore also
//! carries `ZeroActiveChargeTime` — the two travel together by arithmetic, not coincidence.

use crate::{
    golden,
    session::{AnomalyKind, Sessions, csv::csv_sessions, estimates_from_sessions},
    time::Interval,
};
use jiff::Timestamp;
use std::path::PathBuf;

/// 16:00–17:00 local on a date with no DST transition, which contains every record.
const LO: &str = "2026-06-15T20:00:00Z";
const HI: &str = "2026-06-15T21:00:00Z";

/// Ids expected to fail one of the three checks.
const UNSOUND: [&str; 3] = ["EARLYOUT", "INVERT1", "LATEOUT"];
/// Ids expected to pass all three, each one second inside a bound.
const SOUND: [&str; 2] = ["EARLYIN", "LATEIN"];

fn fixture() -> PathBuf {
    golden::fixture("sessions/Session_Report_Band.csv")
}

/// The fixture read straight from `tests/fixtures/`. Nothing is written: `csv_sessions` hands its
/// log back on the result rather than putting it beside the file it read.
fn band() -> Sessions {
    csv_sessions(&fixture()).expect("the band fixture reads")
}

/// The three unsound records are excluded and the two sound ones are not.
///
/// Asserted on the read side, since that is where the split into `excluded` actually happens and
/// what every estimate downstream depends on.
#[test]
fn the_band_decides_which_sessions_reach_an_estimate() {
    let report = band();

    let mut excluded: Vec<&str> = report.excluded.iter().map(|s| s.id.as_str()).collect();
    excluded.sort_unstable();
    assert_eq!(excluded, UNSOUND, "excluded set");

    // The sound ones are present and usable, not merely absent from `excluded`. `EARLYIN` and
    // `LATEIN` both have a non-zero charge time, so neither is a spike.
    let mut kept: Vec<&str> = report.sessions.iter().map(|s| s.id.as_str()).collect();
    kept.sort_unstable();
    assert_eq!(kept, SOUND, "sessions reaching the estimates");
}

/// Each unsound record carries `InconsistentDuration`, and each sound one carries no timing
/// anomaly at all.
///
/// Separate from the test above because a session could in principle be excluded for the wrong
/// reason, and then the split would look right while the diagnosis was wrong.
#[test]
fn the_flag_and_the_exclusion_agree() {
    let report = band();

    for session in &report.excluded {
        assert!(
            session
                .anomalies
                .contains(&AnomalyKind::InconsistentDuration),
            "{} is excluded but not flagged: {:?}",
            session.id,
            session.anomalies
        );
    }
    for session in &report.sessions {
        assert!(
            !session
                .anomalies
                .contains(&AnomalyKind::InconsistentDuration),
            "{} is flagged but not excluded: {:?}",
            session.id,
            session.anomalies
        );
    }

    // The inverted record is the one no other fixture carries. Its zero duration means it picks up
    // `ZeroActiveChargeTime` too, and both are expected.
    let invert = report
        .excluded
        .iter()
        .find(|s| s.id == "INVERT1")
        .expect("the inverted record is excluded");
    assert!(
        invert.conn_end < invert.conn_start,
        "the fixture no longer inverts: {} to {}",
        invert.conn_start,
        invert.conn_end
    );
}

/// An inverted record reaches the estimates without panicking.
///
/// The reason check 1 exists. `Session::intersects` panics on an inverted span and documents
/// exclusion by this test as the reason it cannot happen; before check 1 that was untrue, and a
/// record like `INVERT1` reached it. This is the proof that it no longer does — the excluded
/// listing walks every excluded session, inverted ones included.
#[test]
fn an_inverted_record_is_listed_rather_than_crashing() {
    let report = band();
    let interval = Interval::from_start_end(
        LO.parse::<Timestamp>().unwrap(),
        HI.parse::<Timestamp>().unwrap(),
    );
    let estimates = estimates_from_sessions(interval, report.sources.clone(), &report);

    let listed: Vec<&str> = estimates
        .excluded_sessions
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert!(listed.contains(&"INVERT1"), "not listed: {listed:?}");

    // And the sound pair did reach the figures, so the report is not empty by accident.
    let members: Vec<String> = estimates
        .seg_estimates
        .iter()
        .flat_map(|(seg, _)| seg.sessions.iter().map(|s| s.id.clone()))
        .collect();
    for id in SOUND {
        assert!(
            members.iter().any(|m| m == id),
            "{id} is sound but took part in no segment: {members:?}"
        );
    }
}
