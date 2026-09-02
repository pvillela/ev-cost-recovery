//! Daylight saving: what a local wall time names, and what a session gets when it names nothing.
//!
//! One probe underlies everything here. Read a wall time *as if* at each offset in
//! [`TZ_OFFSETS`](super::TZ_OFFSETS), and keep the readings where the zone really is at that offset
//! on the instant you land on. How many survive says which situation this is, so the gap and the
//! fold need no special-casing: none is the gap, one is an ordinary wall time, two is the fold.
//!
//! Two callers ask different questions of it and are right to answer differently — see
//! docs/time/README.md, "Two resolvers, deliberately". `session::ioi` reports the ambiguity to a
//! user choosing an interval, because a wall time is all it has. `session::csv` settles it against
//! `Conn_Duration`, which is evidence the other lacks.

use jiff::{
    Timestamp, civil,
    tz::{Offset, TimeZone},
};

use super::base::{TZ_OFFSETS, time_zone};

/// The `conn_start` of a session whose reported times cannot be placed on a timeline.
///
/// Paired with [`UNPLACEABLE_END`], which is *earlier*: the span is inverted, and deliberately so.
/// There is no instant to record — a wall time in the DST gap never occurred, and a fold no reading
/// resolves could be either of two — so the pair records that absence in the only field a
/// [`Session`](crate::session::Session) has for it. Anything that tries to place such a session on
/// a timeline gets a panic rather than a plausible answer, which is what
/// `Session::adj_duration`, `SessionOverlap::duration` and `Session::intersects` are for.
///
/// Not a value to compute with. The sessions carrying it are flagged and sorted into
/// `Sessions::excluded`, and the report and the workbook writer skip the cells derived from it.
pub const UNPLACEABLE_START: Timestamp = Timestamp::MAX;

/// The `conn_end` of a session whose reported times cannot be placed on a timeline. See
/// [`UNPLACEABLE_START`].
pub const UNPLACEABLE_END: Timestamp = Timestamp::MIN;

/// Every reading of a local wall time that the zone agrees with, earliest first.
///
/// The list length is the classification: empty is the DST gap, one is every other wall time in the
/// year, two is the repeated hour when DST ends. Each reading is paired with the name that picks it
/// out — `EDT` before `EST`, since the summer offset is the earlier instant.
///
/// Ordering by instant rather than by [`TZ_OFFSETS`](super::TZ_OFFSETS) is what lets a caller take
/// "the earliest reading" without knowing which offset that is.
pub(crate) fn local_readings(dt: civil::DateTime) -> Vec<(&'static str, Timestamp)> {
    let tz = time_zone();
    let mut readings: Vec<(&'static str, Timestamp)> = TZ_OFFSETS
        .iter()
        .filter_map(|(name, hours)| {
            let offset = Offset::constant(*hours);
            // A fixed offset has neither gap nor fold, so this can only fail on a civil date-time
            // UTC cannot represent -- which no parsed report field is.
            let ts = dt.to_zoned(TimeZone::fixed(offset)).ok()?.timestamp();
            (tz.to_offset(ts) == offset).then_some((*name, ts))
        })
        .collect();
    readings.sort_unstable_by_key(|&(_, ts)| ts);
    readings
}

/// Whether a wall time fell in the hour the clocks jump over when DST begins.
///
/// It names no instant at all, which is why nothing downstream may guess one.
pub(crate) fn falls_in_gap(dt: civil::DateTime) -> bool {
    local_readings(dt).is_empty()
}

/// What instants a local wall time names in the zone.
///
/// Reports the ambiguity rather than resolving it, for a caller that has nothing to resolve it
/// *with*: a user has named a wall time and that is all.
///
/// Gated because the only caller is `session::ioi`, which is itself `historic` — see
/// [`local_readings`], the ungated half that the session reader uses in every build.
#[cfg(any(test, feature = "historic"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TzLocalMapping {
    /// Exactly one instant. Every wall time except during the two transitions each year.
    Unique(Timestamp),
    /// Two instants an hour apart, on the night DST ends, each paired with the name that picks it
    /// out.
    Twice(Vec<(&'static str, Timestamp)>),
    /// No instant: the clocks jump forward over this wall time when DST begins.
    Never,
}

/// Maps a local wall time to the instant or instants it names. `historic`, for the reason
/// [`TzLocalMapping`] gives.
#[cfg(any(test, feature = "historic"))]
pub(crate) fn map_local(dt: civil::DateTime) -> TzLocalMapping {
    let readings = local_readings(dt);
    match readings.len() {
        0 => TzLocalMapping::Never,
        1 => TzLocalMapping::Unique(readings[0].1),
        _ => TzLocalMapping::Twice(readings),
    }
}

// cargo test --lib -- time::dst::test --nocapture
#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;

    fn utc(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    /// The three cases, on the three kinds of date they arise on.
    #[test]
    fn a_wall_time_names_none_one_or_two_instants() {
        assert!(local_readings(date(2026, 3, 8).at(2, 30, 0, 0)).is_empty());
        assert_eq!(local_readings(date(2026, 6, 15).at(12, 0, 0, 0)).len(), 1);
        assert_eq!(local_readings(date(2026, 11, 1).at(1, 30, 0, 0)).len(), 2);
    }

    /// The fold's two readings come back earliest first, and the earlier one is EDT. Callers rely
    /// on the order to take "the earliest reading" without naming an offset.
    #[test]
    fn fold_readings_are_ordered_by_instant() {
        let readings = local_readings(date(2026, 11, 1).at(1, 30, 0, 0));
        assert_eq!(readings[0], ("EDT", utc("2026-11-01T05:30:00Z")));
        assert_eq!(readings[1], ("EST", utc("2026-11-01T06:30:00Z")));
        assert!(readings[0].1 < readings[1].1);
    }

    /// The gap is the empty case of the same probe, not a test of its own.
    #[test]
    fn the_gap_is_the_absence_of_any_reading() {
        assert!(falls_in_gap(date(2026, 3, 8).at(2, 0, 0, 0)));
        assert!(falls_in_gap(date(2026, 3, 8).at(2, 59, 0, 0)));
        // The hours either side of it exist.
        assert!(!falls_in_gap(date(2026, 3, 8).at(1, 59, 0, 0)));
        assert!(!falls_in_gap(date(2026, 3, 8).at(3, 0, 0, 0)));
    }

    /// [`map_local`] is the same probe read as a classification.
    #[test]
    fn map_local_agrees_with_the_probe() {
        for dt in [
            date(2026, 3, 8).at(2, 30, 0, 0),
            date(2026, 6, 15).at(12, 0, 0, 0),
            date(2026, 11, 1).at(1, 30, 0, 0),
        ] {
            let expected = match local_readings(dt).len() {
                0 => TzLocalMapping::Never,
                1 => TzLocalMapping::Unique(local_readings(dt)[0].1),
                _ => TzLocalMapping::Twice(local_readings(dt)),
            };
            assert_eq!(map_local(dt), expected, "{dt}");
        }
    }

    /// The two sentinels are inverted, which is the property everything downstream leans on.
    #[test]
    fn the_sentinels_are_inverted() {
        assert!(UNPLACEABLE_END < UNPLACEABLE_START);
    }
}
