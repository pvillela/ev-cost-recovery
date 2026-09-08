use jiff::{
    Timestamp,
    civil::{Date, DateTime},
    tz::{Offset, TimeZone},
};
use std::{sync::LazyLock, time::Duration};

// ---------------------------------------------------------------------------
// Date/time
// ---------------------------------------------------------------------------

/// Time zone the session report's timestamps are stated in. See docs/time/README.md, "Time zone".
///
/// Referenced by `session::ioi` and several doc comments; a reader who finds "in local time" in a
/// message needs somewhere to learn which zone that is. Not exported from the crate.
pub const TIME_ZONE_NAME: &str = "America/Toronto";

/// The offsets the crate-private `TIME_ZONE_NAME` uses, under the names a reader of a Toronto
/// Hydro bill will recognise. Naming one resolves a wall time that occurs twice.
///
/// Here rather than in `session` because it is a property of the zone, and the zone is shared.
pub const TZ_OFFSETS: [(&str, i8); 2] = [("EST", -5), ("EDT", -4)];

static TIME_ZONE: LazyLock<TimeZone> = LazyLock::new(|| {
    TimeZone::get(TIME_ZONE_NAME).expect("America/Toronto should be a valid time-zone name")
});

/// Resolved once. Every local-time question in the crate goes through here, so there is one answer
/// to "which zone" rather than one per module.
pub fn time_zone() -> TimeZone {
    TIME_ZONE.clone()
}

pub(crate) fn duration(start: Timestamp, end: Timestamp) -> Duration {
    Duration::try_from(end.duration_since(start))
        .unwrap_or_else(|_| panic!("interval ends at {} before it starts at {}", end, start))
}

/// The local calendar date an instant falls on.
pub fn local_date(ts: Timestamp) -> Date {
    ts.to_zoned(time_zone()).date()
}

/// The local wall-clock reading of an instant, for the workbook's local-time columns.
pub(crate) fn local_datetime(ts: Timestamp) -> DateTime {
    ts.to_zoned(time_zone()).datetime()
}

/// The instant a given local hour begins on a given local date.
///
/// # Panics
///
/// Panics if the local time falls in a daylight-saving gap or fold. Callers pass 0, 7, 11, 17 or
/// 19; Ontario's transitions are at 02:00, so none of them can.
pub(crate) fn local_hour(d: Date, hour: u8) -> Timestamp {
    d.at(hour as i8, 0, 0, 0)
        .to_zoned(time_zone())
        .expect("callers pass hours that never fall in a daylight-saving transition")
        .timestamp()
}

/// The instant a local date begins.
pub(crate) fn local_midnight(d: Date) -> Timestamp {
    local_hour(d, 0)
}

// ---------------------------------------------------------------------------
// Standard time
// ---------------------------------------------------------------------------
//
// Toronto Hydro cuts a billing period on standard time all year round: the meter's own clock does
// not observe daylight saving, and neither does the period boundary read off it. Prevailing local
// time is still what everything else here means by "local" -- Time-of-Use periods, the 07:00-19:00
// demand window and the holiday calendar are all stated in the clock a customer reads, and they
// keep `local_date` and `local_hour` above.
//
// The evidence for the rule, and for that division, is in
// `docs/archive/hydro_bill/dst-energy-anomaly-pre-fix.md`: an EST-fixed window reproduces all 19
// invoices to the milli-kWh, while a prevailing-local one matches 6, and the on-peak and mid-peak
// energy the bills state is reproduced only by TOU periods left on prevailing time.

/// The offset a billing period is cut on, under the name a bill reader will recognise.
///
/// The standard-time entry of [`TZ_OFFSETS`], named rather than indexed so that the reason is
/// visible at the use site. `test::the_billing_offset_is_the_standard_time_one` pins it to the
/// entry it is meant to be, so reordering that array cannot silently move every period boundary.
pub const BILLING_OFFSET: (&str, i8) = TZ_OFFSETS[0];

/// The zone billing periods are cut in: a fixed offset, with no daylight-saving rule at all.
///
/// Built on the spot rather than resolved once, unlike [`time_zone`]. A fixed offset is arithmetic
/// on the offset itself: `jiff` packs it into the value and allocates nothing, so there is no
/// lookup to share and nothing for a `LazyLock` to save.
const fn billing_zone() -> TimeZone {
    TimeZone::fixed(Offset::constant(BILLING_OFFSET.1))
}

/// The standard-time calendar date an instant falls on.
///
/// Differs from [`local_date`] for instants in the hour after midnight during daylight saving:
/// 2025-06-24T00:30 EDT is still the 23rd on a standard-time clock.
pub(crate) fn standard_date(ts: Timestamp) -> Date {
    ts.to_zoned(billing_zone()).date()
}

/// The instant a standard-time date begins.
///
/// Unlike [`local_midnight`] this can never fail: a fixed offset has no gap for a wall time to
/// fall into and no fold for it to be ambiguous in.
pub(crate) fn standard_midnight(d: Date) -> Timestamp {
    d.at(0, 0, 0, 0)
        .to_zoned(billing_zone())
        .expect("a fixed offset has neither gaps nor folds")
        .timestamp()
}

// ---------------------------------------------------------------------------
// Interval
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
/// Time interval, half-open: it includes its start and excludes its end.
pub struct Interval {
    pub start: Timestamp,
    pub duration: Duration,
}

impl Interval {
    pub fn new(start: Timestamp, duration: Duration) -> Interval {
        Self { start, duration }
    }

    /// Construct `Self` from `start` and `end`.
    ///
    /// # Panics
    /// If `end < start`.
    pub fn from_start_end(start: Timestamp, end: Timestamp) -> Interval {
        let duration = duration(start, end);
        Self { start, duration }
    }

    pub fn end(&self) -> Timestamp {
        self.start + self.duration
    }

    /// Whether `at` lies inside, counting the start and excluding the end as everything here does.
    pub fn contains(&self, at: Timestamp) -> bool {
        self.start <= at && at < self.end()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.start >= other.start && self.end() <= other.end()
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    pub fn is_empty(&self) -> bool {
        self.duration == Duration::ZERO
    }

    /// The overlap of the two intervals, `None` when they do not meet.
    ///
    /// An empty interval stands for the instant at its start, and meets an interval holding that
    /// instant. The result is then empty too, so an empty `Some` is a meeting and not a miss:
    /// callers must distinguish it from `None` rather than asking [`Self::is_empty`].
    ///
    /// Two abutting intervals do not meet, half-open ends being what they are, and two empty ones
    /// never meet at all.
    pub fn intersection(&self, other: &Interval) -> Option<Self> {
        let start = self.start.max(other.start);
        let end = self.end().min(other.end());
        if start < end {
            return Some(Self::from_start_end(start, end));
        }
        // At this point, `start >= end`.
        // Two non-empty intervals either abut or don't touch each other ==> `meets == false`.
        // Two empty intervals either are identical or don't touch each other ==> `meets == false`.
        // So, `meets == true` iff one interval is empty and the other one is non-empty and the
        // non-empty one contains the empty one.
        let meets = other.contains(self.start) || self.contains(other.start);
        meets.then(|| Self::new(start, Duration::ZERO))
    }

    /// The fraction of `self` that overlaps `other`.
    ///
    /// If `self` and `other` don't overlap then it is `0.0`,
    /// else if `self` is contained in `other` it is 1.0,
    /// else it is the ratio of the overlap duration to `self` duration.
    ///
    /// The result is always finite, even when `self` has zero duration.
    pub fn overlap_ratio(&self, other: &Self) -> f64 {
        match self.intersection(other) {
            None => 0.0,
            Some(overlap) => {
                if overlap == *self {
                    1.0
                } else {
                    overlap.duration.as_secs_f64() / self.duration.as_secs_f64()
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// cargo test --lib -- time::base::test --nocapture
#[cfg(test)]
mod test {
    use super::*;

    use jiff::civil::date;

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    /// [`BILLING_OFFSET`] is taken from [`TZ_OFFSETS`] by index, so this pins down which entry it
    /// is meant to be. Reordering that array would otherwise move every billing period boundary by
    /// an hour with nothing to say so.
    #[test]
    fn the_billing_offset_is_the_standard_time_one() {
        assert_eq!(BILLING_OFFSET, ("EST", -5));
        assert!(TZ_OFFSETS.contains(&BILLING_OFFSET));
    }

    /// Standard-time midnight is a fixed UTC-5 all year, where local midnight follows the clocks.
    /// In winter the two agree; in summer standard-time midnight is an hour later in UTC.
    #[test]
    fn standard_midnight_does_not_follow_daylight_saving() {
        // January: the prevailing offset is already -5, so the two coincide.
        assert_eq!(
            standard_midnight(date(2026, 1, 24)),
            local_midnight(date(2026, 1, 24))
        );
        assert_eq!(
            standard_midnight(date(2026, 1, 24)),
            ts("2026-01-24T05:00:00Z")
        );
        // June: prevailing local midnight is 04:00Z, standard-time midnight stays at 05:00Z.
        assert_eq!(
            local_midnight(date(2026, 6, 24)),
            ts("2026-06-24T04:00:00Z")
        );
        assert_eq!(
            standard_midnight(date(2026, 6, 24)),
            ts("2026-06-24T05:00:00Z")
        );
    }

    /// The hour between the two midnights belongs to the previous day on a standard-time clock.
    /// This is the hour every billing-period difference in `docs/hydro_bill/` came down to.
    #[test]
    fn the_midnight_hour_belongs_to_the_previous_standard_day() {
        let t = ts("2026-06-24T04:30:00Z"); // 00:30 EDT on the 24th
        assert_eq!(local_date(t), date(2026, 6, 24));
        assert_eq!(standard_date(t), date(2026, 6, 23));
    }

    /// Every standard-time day is exactly 24 hours, including the two the clocks change on. That
    /// is what makes a billing period always a whole number of days.
    #[test]
    fn standard_time_days_are_always_24_hours() {
        for (y, m, d) in [(2026, 3, 8), (2025, 11, 2), (2026, 6, 15)] {
            let start = standard_midnight(date(y, m, d));
            let next = standard_midnight(date(y, m, d) + jiff::Span::new().days(1));
            assert_eq!(
                next.as_second() - start.as_second(),
                24 * 3600,
                "{y}-{m}-{d}"
            );
        }
    }
}
