use super::site_model::{
    Load, NORMAL_VOLTAGE_FLUCTUATION_FACTOR, PANEL_BREAKER_COUNT, PANEL_COUNT, ev_load,
    ev_real_power_kw, single_panel_load,
};
use crate::{
    log::SourceLog,
    time::{Interval, UNPLACEABLE_END, UNPLACEABLE_START, duration, time_zone, truncate_to},
};
use jiff::{Timestamp, Zoned};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Debug},
    iter::Sum,
    ops::{Add, Div, Mul},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

/// Resolution this software works session boundaries to.
///
/// **Ours, not Evolute's.** Evolute currently reports `Conn_DateTime_Start` and `Conn_DateTime_End`
/// truncated to whole minutes, and this is set to match; but the two are different quantities, and
/// the document that derives everything built on this calls them `EV_STEP` and `OUR_STEP` for that
/// reason. See `docs/session/time-reporting-uncertainty.md`.
///
/// The distinction decides what to do if Evolute ever reports seconds. **Do not follow them down
/// to 1 second while reports of both resolutions are still processed together.** This constant is
/// global; `EV_STEP` belongs to a report. A calculation spanning a minute-resolution report and a
/// second-resolution one has no single right value for it, so it has to sit at the coarsest
/// resolution still in scope. Finer reporting is a reason to narrow the allowances below, not to
/// move the grid.
///
/// The constraint lifts once every report in scope reports seconds. 1 second is a legal grid — it
/// divides [`SEGMENT_DURATION`], and `ioi::LEGAL_START_MINUTES` still
/// lands on it — so from then on the change is available, though it moves every figure in the
/// golden files.
///
/// Every allowance the software makes for the reporting's truncation is this one value:
///
/// - Added to the reported session end to give `adj_conn_end`, the session's exclusive end.
/// - The width of the window a sound record's `Conn_start + Conn_Duration` must land in — one step
///   early, one step and a second late. See `duration_is_consistent`.
///
/// `Conn_Duration` and `Active_Charge_Time` are *not* truncated; they carry seconds. That asymmetry
/// is what makes the DST fold inference possible, and it is why the window above has a width at all.
///
/// See docs/session/README.md, "Boundaries and the time grid".
pub const TIME_GRID_STEP: Duration = Duration::from_secs(60);

/// The width of the [`Segment`]s an interval of interest is partitioned into.
///
/// The duration of the interval of interest **must** be a positive multiple of this, and the
/// crate-private `estimates_from_sessions` panics otherwise. Not a convention: rounding the segment
/// count up would tile past the interval's end and count sessions falling outside it, and rounding
/// down would leave part of it unestimated. Neither error would show in any figure the report
/// prints, which is why the check is an assertion rather than an accommodation.
///
/// The two legal interval lengths — 15 minutes and 1 hour — are both multiples, so nothing coming
/// through `ioi::checked_interval` can trip it. A plain code span, not a link: that function is
/// behind the `historic` feature, so a link to it would resolve in one build and not the other.
pub const SEGMENT_DURATION: Duration = Duration::from_mins(15);

/// Continuous use breaker kW rating.
pub const BREAKER_RATING_KW: f64 = ev_real_power_kw();

/// Highest average power a session can draw and still be normal.
///
/// [`BREAKER_RATING_KW`] at the top of the normal voltage band. The breaker limits current, not
/// power, so a vehicle at full pilot current draws more kW when the supply voltage runs high.
pub const BREAKER_MAX_NORMAL_KW: f64 =
    BREAKER_RATING_KW * (1.0 + NORMAL_VOLTAGE_FLUCTUATION_FACTOR);

// ---------------------------------------------------------------------------
// Reported time, adjusted
// ---------------------------------------------------------------------------
//
// The three functions below are the code counterpart of
// `docs/session/time-reporting-uncertainty.md`, which derives all of them. They are free
// functions rather than methods so the write path, which has a CSV record and not yet a
// [`Session`], calls the same code the read path does. Two definitions of `adj_conn_end` is how
// the two drifted apart last time.

/// `adj_start` of the document: the reported start truncated to the time grid, so the true start
/// lies in `[adj_conn_start, adj_conn_start + TIME_GRID_STEP)`.
pub(crate) fn adj_conn_start_of(conn_start: Timestamp) -> Timestamp {
    truncate_to(conn_start, TIME_GRID_STEP)
}

/// `adj_end` of the document: `our_truncate(rep_end + 1s) + OUR_STEP`.
///
/// The `+ 1s` is not padding. The reported end is truncated, *and* it is not known whether the
/// reporting includes or excludes its last second, so the true end may lie a second beyond the
/// minute the report names. Dropping it makes the bound too tight by up to one whole step for any
/// `conn_end` carrying seconds; they agree only while every reported end lands on the minute.
pub(crate) fn adj_conn_end_of(conn_end: Timestamp) -> Timestamp {
    truncate_to(conn_end + Duration::from_secs(1), TIME_GRID_STEP) + TIME_GRID_STEP
}

/// Whether a record's reported start, end and duration can all be true at once.
///
/// Three checks, and any failure raises [`AnomalyKind::InconsistentDuration`]:
///
/// ```text
/// 1.  rep_start <= rep_end
/// 2.  rep_start + conn_duration  <  rep_end + TIME_GRID_STEP + 1s
/// 3.  rep_end - TIME_GRID_STEP   <  rep_start + conn_duration
/// ```
///
/// Checks 2 and 3 are the document's consistency checks 1 and 2, the second rearranged. Neither is
/// chosen: they are what truncation to `TIME_GRID_STEP` accounts for and nothing more, so widening
/// either lets a real fault through and narrowing either flags a sound record.
///
/// Check 1 is explicit because the document's own check 3, `adj_start <= adj_end`, is too weak to
/// stand in for it: with `rep_start = 10:01:00` and `rep_end = 10:00:00` both sides truncate to
/// `10:01:00`, so a one-minute inversion passes. It only bites beyond roughly two steps. That
/// matters because [`Session::intersects`] panics on an inverted span and documents exclusion by
/// this very test as the reason it cannot happen — an inverted record with a small
/// `conn_duration` satisfies both of the other two checks and would reach it.
pub(crate) fn duration_is_consistent(
    conn_start: Timestamp,
    conn_end: Timestamp,
    conn_duration: Duration,
) -> bool {
    let implied_end = conn_start + conn_duration;
    conn_start <= conn_end
        && implied_end < conn_end + TIME_GRID_STEP + Duration::from_secs(1)
        && conn_end - TIME_GRID_STEP < implied_end
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// A shared [`Session`].
///
/// Public because it appears in public signatures — [`Segment::sessions`],
/// [`Sessions::sessions`], and the API's estimating calls — and naming the type a caller has
/// to write is the point of an alias. The sharing is what lets one session belong to several
/// segments, and to the crate-private `Anomaly`, without being copied.
pub type RSession = Rc<Session>;

#[derive(Debug)]
/// Charging session
pub struct Session {
    /// Path to source data file.
    pub path: Rc<PathBuf>,
    /// Row number in the source data file. The header occupies row 1, so the lowest possible value
    /// is 2.
    pub row: usize,
    /// From `session report`.
    pub id: String,
    /// `conn_start_utc`: connection start date-time from `session report`.
    pub conn_start: Timestamp,
    /// `conn_end_utc`: connection end date-time as reported, truncated to the minute.
    ///
    /// Held for reporting only. Every calculation wants [`Session::adj_conn_end`], which is the
    /// bound that actually contains the session.
    pub conn_end: Timestamp,
    /// `Conn_Duration` from `session report`: the physical elapsed time of the connection, which is
    /// what makes the DST fold inference possible. See docs/time/README.md, "Time zone".
    pub conn_duration: Duration,
    /// Active charge time from `session report`.
    ///
    /// Differs from `adj_conn_end - conn_start` by the padding on `adj_conn_end`, and from
    /// `conn_duration` by about a second. It does **not** measure charging as distinct from
    /// connection. Evolute, 22 Jul 2026:
    ///
    /// > All 3 will show as almost the same, with Active charging being off by maybe 1 second due
    /// > to rounding as it is on a slightly different timer. These fields are here for grant
    /// > reporting, but for our system we do not track them differently.
    ///
    /// The reason previously given here — a car that stays connected without drawing power — was
    /// wrong, and the correction matters: it is what makes a zero `Active_Charge_Time` a reporting
    /// fault rather than an idle connection. See `Questions_for_Evolute.md`, "Answers received".
    pub charge_time: Duration,
    /// From `session report`.
    pub energy_use: f64,
    /// Anomalies associated with this session.
    pub anomalies: Vec<AnomalyKind>,
}

impl Session {
    /// `adj_conn_start_utc`: see `adj_conn_start_of`, which this defers to.
    /// Whether the reported wall times were resolved to instants at all.
    ///
    /// `false` for the two records that name none: a reported time in the DST gap, which never
    /// occurred, and a fold no reading of the record resolves. Both are given the crate-private
    /// sentinels `time::UNPLACEABLE_START` and `time::UNPLACEABLE_END` instead of a guess, and both
    /// are excluded.
    ///
    /// What it is for is the handful of places that legitimately hold an excluded session and would
    /// otherwise read those two fields: the report's listing, and the workbook's writer and reader.
    /// Everywhere else, reaching them is a fault and the inverted span is what says so.
    ///
    /// Tested against the sentinels rather than by asking whether the span is inverted. A record
    /// flagged [`AnomalyKind::InconsistentDuration`] alone may also report an end before its start,
    /// and *its* times are real readings that the listing should still print.
    pub fn is_placeable(&self) -> bool {
        self.conn_start != UNPLACEABLE_START || self.conn_end != UNPLACEABLE_END
    }

    pub fn adj_conn_start(&self) -> Timestamp {
        adj_conn_start_of(self.conn_start)
    }

    /// `adj_conn_end_utc`: see `adj_conn_end_of`, which this defers to.
    ///
    /// This is the end the estimating logic uses throughout, so that
    /// `[adj_conn_start, adj_conn_end)` is the tightest half-open span guaranteed to contain the
    /// real connection. See docs/session/README.md, "Sessions and segments".
    pub fn adj_conn_end(&self) -> Timestamp {
        adj_conn_end_of(self.conn_end)
    }

    /// Whether the session overlaps with an interval.
    ///
    /// # Panics
    ///
    /// If `adj_conn_end` precedes `adj_conn_start`. That is a precondition, not a defensive check: a
    /// session whose span is inverted has fields that contradict each other, is flagged
    /// [`AnomalyKind::InconsistentDuration`] on conversion, and is sorted into
    /// [`Sessions::excluded`] — so it never reaches the estimating logic at all.
    ///
    /// What establishes that is check 1 of [`duration_is_consistent`], `conn_start <= conn_end`; see
    /// its doc for why the other two checks cannot stand in for it.
    ///
    /// Panicking here is therefore the honest behaviour. Reaching it means an excluded session got
    /// somewhere it should not have, and that is worth a crash rather than a plausible answer. The
    /// one caller that legitimately holds excluded sessions — the report, which lists them on
    /// purpose — asks [`Self::lenient_intersects`] instead.
    pub(crate) fn intersects(&self, interval: &Interval) -> bool {
        let sess_itvl = Interval::from_start_end(self.adj_conn_start(), self.adj_conn_end());
        !sess_itvl.intersection(interval).is_empty()
    }

    /// [`Self::intersects`], but answering for a session whose span is inverted rather than
    /// panicking on it.
    ///
    /// For the reporting module alone, and for one question: whether an *excluded* session appears
    /// to fall in the interval of interest. That listing covers the whole workbook by design —
    /// filtering it would apply a judgement to exactly the timestamps that are in doubt — so the
    /// report has to answer for records the estimating logic never touches, including one whose
    /// reported end precedes its start.
    ///
    /// The answer is only ever "appears to", and README says so where the column is described. The
    /// two endpoints are read in whichever order puts them the right way round, which is the most
    /// that can be said for a record whose own fields disagree.
    ///
    /// Identical to [`Self::intersects`] for every session that is not inverted.
    pub(crate) fn lenient_intersects(&self, interval: &Interval) -> bool {
        let (lo, hi) = match self.adj_conn_start() <= self.adj_conn_end() {
            true => (self.adj_conn_start(), self.adj_conn_end()),
            false => (self.adj_conn_end(), self.adj_conn_start()),
        };
        !Interval::from_start_end(lo, hi)
            .intersection(interval)
            .is_empty()
    }

    /// Reported connection start in local time (ET).
    pub fn conn_start_local(&self) -> Zoned {
        Zoned::new(self.conn_start, time_zone())
    }

    /// Reported connection end in local time (ET).
    pub fn conn_end_local(&self) -> Zoned {
        Zoned::new(self.conn_end, time_zone())
    }

    /// Adjusted, inclusive connection start in local time (ET).
    pub fn adj_conn_start_local(&self) -> Zoned {
        Zoned::new(self.adj_conn_start(), time_zone())
    }

    /// Adjusted, exclusive connection end in local time (ET).
    pub fn adj_conn_end_local(&self) -> Zoned {
        Zoned::new(self.adj_conn_end(), time_zone())
    }

    /// Session duration from `adj_conn_start` to `adj_conn_end`
    pub fn adj_duration(&self) -> Duration {
        duration(self.adj_conn_start(), self.adj_conn_end())
    }

    /// Average power draw in kW: [`Self::energy_use`] / ([`Self::charge_time`] in hours).
    ///
    /// Non-finite results (a spike) are substituted: `0.0` for zero energy,
    /// [`BREAKER_RATING_KW`] otherwise.
    pub fn avg_kw(&self) -> f64 {
        let kw = self.energy_use / self.charge_time.as_secs_f64() * 3600.0;
        match kw.is_finite() {
            true => kw,
            false => {
                if self.energy_use == 0.0 {
                    0.0
                } else {
                    BREAKER_RATING_KW
                }
            }
        }
    }

    /// Whether two records sharing an `id` describe *different* sessions.
    ///
    /// The question [`MergedSessions::merge_sessions`] asks of every pair of records with the same
    /// `Charge_Session_ID`. `false` means the two are one session reported in two overlapping
    /// files, and one copy is dropped. `true` means they are not the same session — either Evolute
    /// reused the id, or the two files disagree about one — and both are kept.
    ///
    /// The fields compared are the ones every estimate is built from: where the session sits on the
    /// timeline, how long it charged, and how much energy it took. `row` and `path` are excluded on
    /// purpose — the same session is at different rows in different files, which is precisely the
    /// case this has to see through.
    pub(crate) fn is_inconsistent_duplicate(&self, other: &Session) -> bool {
        self.id == other.id
            && (self.adj_conn_start() != other.adj_conn_start()
                || self.adj_conn_end() != other.adj_conn_end()
                || self.charge_time != other.charge_time
                || self.energy_use != other.energy_use)
    }

    /// The session's overlap with an interval, or `None` when the two do not meet.
    ///
    /// `None` rather than a zero-width [`SessionOverlap`]: there is no pair of brackets that
    /// stands for "no overlap" without also standing for some instant, and a sentinel pair built
    /// from the extremes of the timestamp range only defers the problem to whoever measures its
    /// duration. The absence is in the type instead.
    pub(crate) fn interval_overlap(&self, interval: &Interval) -> Option<SessionOverlap> {
        let sess_itvl = Interval::from_start_end(self.adj_conn_start(), self.adj_conn_end());
        let overlap = sess_itvl.intersection(interval);
        if overlap.is_empty() {
            return None;
        }

        let left = if self.adj_conn_start() == overlap.start {
            Bracket::new(overlap.start, overlap.start + TIME_GRID_STEP)
        } else {
            Bracket::exact(overlap.start)
        };

        let right = if self.adj_conn_end() == overlap.end() {
            Bracket::new(overlap.end() - TIME_GRID_STEP, overlap.end())
        } else {
            Bracket::exact(overlap.end())
        };

        Some(SessionOverlap { left, right })
    }

    /// The duration of the session's overlap with `interval` divided by `interval`'s
    /// duration.
    pub(crate) fn interval_overlap_ratio(&self, interval: &Interval) -> Bracket<f64> {
        match self.interval_overlap(interval) {
            None => Bracket::exact(0.0),
            Some(overlap) => overlap
                .duration()
                .map(|v| v.as_secs_f64() / interval.duration.as_secs_f64()),
        }
    }

    /// Average power (in kW) of this session over `interval`.
    pub(crate) fn interval_avg_kw(&self, interval: &Interval) -> Bracket<f64> {
        let overlap_ratio = self.interval_overlap_ratio(interval);
        overlap_ratio.map(|v| v * self.avg_kw())
    }
}

// `Session` deliberately has no `PartialEq`, `Ord` or `Hash`. It had them so that [`Segment`] could
// hold a `BTreeSet`, and they compared `id` alone — which made segment membership, and every figure
// drawn from it, rest on `Charge_Session_ID` being unique. It is not: Evolute's June 2026 report
// carries `S37487` on two unrelated sessions a week apart. `Segment` holds a `Vec` for that reason,
// and nothing else needs to ask whether two sessions are equal.

/// A [`Session`]'s overlap with an [`Interval`], including quantification of
/// overlap uncertainty due to [`TIME_GRID_STEP`].
pub(crate) struct SessionOverlap {
    left: Bracket<Timestamp>,
    right: Bracket<Timestamp>,
}

impl SessionOverlap {
    pub fn duration(&self) -> Bracket<Duration> {
        let min = if self.left.max < self.right.min {
            duration(self.left.max, self.right.min)
        } else {
            Duration::ZERO
        };
        let max = duration(self.left.min, self.right.max);
        Bracket::new(min, max)
    }
}

/// Several files' sessions as one list, with what is wrong across them.
struct MergedSessions {
    /// Every session, in the order the lists were given, less the records collapsed as identical.
    sessions: Vec<RSession>,
    /// Anomalies that are not properties of any single record and so are not on
    /// [`Session::anomalies`]. Currently [`AnomalyKind::DuplicateId`] only.
    anomalies: Vec<Anomaly>,
    /// Records dropped as identical copies of one already kept, for the log.
    collapsed: Vec<Collapse>,
}

/// One record dropped because another already kept says the same thing.
///
/// Logged rather than flagged. A collapse is not a fault in either record — it is what a session
/// reported in two overlapping files looks like, and the surviving row is the whole of the answer
/// — so there is no session left to hang an [`AnomalyKind`] on. It is still worth a line: a
/// reader counting rows in the source files and rows in the estimate needs to know where the
/// difference went.
struct Collapse {
    dropped: RSession,
    kept: RSession,
}

impl MergedSessions {
    /// Flattens session lists, collapsing records that say the same thing, and flags every
    /// surviving session whose `id` another one shares.
    ///
    /// Two rules, and the distinction between them is the whole of this function:
    ///
    /// - Same `id` and every compared field equal — see [`Session::is_inconsistent_duplicate`] —
    ///   is one session reported twice. One copy is kept, and the drop is noted on the log of the
    ///   file the dropped copy came from. Counting both would inflate every figure derived from
    ///   it, which is why a billing period spanning two monthly reports cannot simply concatenate
    ///   them.
    /// - Same `id` with any field differing is *not* one session. `Charge_Session_ID` is not unique
    ///   in Evolute's reports — the June 2026 report carries `S37487` on two sessions a week apart
    ///   — so such records are kept and estimated from, and flagged
    ///   [`AnomalyKind::DuplicateId`] so a reader can see the id was reused. That flag cannot
    ///   distinguish a reused id from two files genuinely disagreeing about one session; both look
    ///   the same from here, and both are worth seeing.
    ///
    /// Which list a record arrived in makes no difference to either rule. Two identical rows in one
    /// file collapse exactly as two identical rows in two files do, and are logged the same way:
    /// the comparison never looks at where a record came from, so a single-file read and a
    /// two-file one are one code path rather than two.
    fn merge_sessions(session_lists: Vec<Vec<RSession>>) -> Self {
        let mut sessions: Vec<RSession> = Vec::new();
        let mut collapsed = Vec::new();
        for list in session_lists {
            for session in list {
                // Linear against what is already kept. The comparison is on the compared fields,
                // not on the id, so no map keyed by id would serve: an id may legitimately name
                // several distinct sessions, which is the case that produced this function.
                let already_kept = sessions.iter().find(|kept| {
                    kept.id == session.id && !kept.is_inconsistent_duplicate(&session)
                });
                match already_kept {
                    Some(kept) => collapsed.push(Collapse {
                        dropped: session,
                        kept: kept.clone(),
                    }),
                    None => sessions.push(session),
                }
            }
        }

        let anomalies = duplicate_id_anomalies(&sessions);
        Self {
            sessions,
            anomalies,
            collapsed,
        }
    }
}

/// One [`AnomalyKind::DuplicateId`] per session whose `id` another session in the list also
/// carries.
///
/// Symmetric: every member of a colliding group is flagged, not only the later ones. Which record
/// was read first says nothing about which is the odd one out, and a reader looking up either row
/// should find the flag there.
fn duplicate_id_anomalies(sessions: &[RSession]) -> Vec<Anomaly> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for session in sessions {
        *counts.entry(session.id.as_str()).or_default() += 1;
    }

    sessions
        .iter()
        .filter(|s| counts[s.id.as_str()] > 1)
        .map(|s| Anomaly {
            session: s.clone(),
            kind: AnomalyKind::DuplicateId,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Bracket
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
/// Value subject to uncertainty due to `TIME_GRID_STEP`.
pub struct Bracket<T: Clone> {
    /// Minimum value.
    pub min: T,
    /// Maximum value.
    pub max: T,
}

impl<T: Clone> Bracket<T> {
    /// A bracket from `min` and `max`; panics unless `min <= max`.
    pub fn new(min: T, max: T) -> Self
    where
        T: Debug + PartialOrd,
    {
        assert!(min <= max, "min={min:?} must be <= max={max:?}");
        Self { min, max }
    }

    /// Instantiates an exact instance.
    pub fn exact(value: T) -> Self {
        Self {
            min: value.clone(),
            max: value,
        }
    }

    pub fn map<U: Clone>(&self, mut f: impl FnMut(&T) -> U) -> Bracket<U> {
        let min = f(&self.min);
        let max = f(&self.max);
        Bracket { min, max }
    }
}

impl<T: Clone + Default> Default for Bracket<T> {
    fn default() -> Self {
        Self {
            min: Default::default(),
            max: Default::default(),
        }
    }
}

impl<T: Clone + Add<Output = T>> Add for Bracket<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            min: self.min + rhs.min,
            max: self.max + rhs.max,
        }
    }
}

impl<T: Clone + Mul<f64, Output = T>> Mul<f64> for Bracket<T> {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            min: self.min * rhs,
            max: self.max * rhs,
        }
    }
}

impl Bracket<f64> {
    pub fn mid(&self) -> f64 {
        (self.min + self.max) / 2.0
    }
}

impl Div<f64> for Bracket<f64> {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self {
            min: self.min / rhs,
            max: self.max / rhs,
        }
    }
}

impl Sum for Bracket<f64> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = Bracket::default();
        for item in iter {
            sum = sum + item;
        }
        sum
    }
}

impl Mul<u32> for Bracket<Duration> {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self {
            min: self.min * rhs,
            max: self.max * rhs,
        }
    }
}

impl Sum for Bracket<Duration> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut sum = Bracket::default();
        for item in iter {
            sum = sum + item;
        }
        sum
    }
}

impl Add for Load {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Load {
            real_kw: self.real_kw + rhs.real_kw,
            reactive_kvar: self.reactive_kvar + rhs.reactive_kvar,
            distortion_kvar: self.distortion_kvar + rhs.distortion_kvar,
        }
    }
}

// ---------------------------------------------------------------------------
// Segment
// ---------------------------------------------------------------------------

/// A shared [`Segment`].
///
/// [`IntervalEstimates`](super::IntervalEstimates) names the same segment three times over — once in the full listing,
/// and again as the maximum of each derivation — so sharing rather than copying is what keeps those
/// references *the same segment* rather than three equal ones. A reader can then ask whether the
/// two derivations peaked together with [`std::rc::Rc::ptr_eq`], instead of comparing clock times
/// and hoping.
///
/// The saved copying is a secondary benefit and a real one: a `Segment` carries a list of its
/// sessions, and cloning that duplicates every entry.
pub type RSegment = Rc<Segment>;

// No `PartialEq`: comparing two segments means comparing their sessions, and `Session` has no
// equality for the reasons given above it.
#[derive(Debug, Clone)]
/// A sub-interval of the interval-of-interest over which power estimates are computed.
pub struct Segment {
    pub interval: Interval,
    /// The sessions intersecting this segment, in the order the report states them.
    pub sessions: Vec<RSession>,
}

impl Segment {
    pub(crate) fn new(start: Timestamp, duration: Duration) -> Self {
        Self {
            interval: Interval::new(start, duration),
            sessions: Default::default(),
        }
    }

    pub fn start(&self) -> Timestamp {
        self.interval.start
    }

    pub fn end(&self) -> Timestamp {
        self.interval.end()
    }

    pub fn agg_count(&self) -> Bracket<f64> {
        self.sessions
            .iter()
            .map(|s| s.interval_overlap_ratio(&self.interval))
            .sum()
    }

    pub fn agg_kw(&self) -> Bracket<f64> {
        self.sessions
            .iter()
            .map(|s| s.interval_avg_kw(&self.interval))
            .sum()
    }

    /// Site load implied by how many vehicles were connected over the segment.
    pub fn count_based_load(&self) -> Bracket<Load> {
        self.agg_count().map(|count| Self::scaled_load(*count))
    }

    /// Site load implied by the energy the segment's sessions drew.
    ///
    /// The aggregate power is converted to an equivalent vehicle count first, so both derivations
    /// go through the same `scaled_load` and can be compared directly.
    pub fn energy_based_load(&self) -> Bracket<Load> {
        let single_ev_real_kw = ev_load().real_kw;
        let scaling = self.agg_kw().map(|v| v / single_ev_real_kw);
        scaling.map(|count| Self::scaled_load(*count))
    }

    /// Site load for a vehicle count, which may be fractional, and may exceed aggregate panel
    /// capacity.
    ///
    /// Every installed panel is present in the figure whether or not it is charging anything.
    /// [`single_panel_load`] carries a standing block — its transformer's core loss and
    /// magnetizing current — and that block is drawn by each of the [`PANEL_COUNT`] panels around
    /// the clock, so an idle panel contributes it and a busy one does not contribute more.
    ///
    /// Without information about which panel a session ran on, the vehicles are packed into the
    /// smallest number of panels that can hold them: panels are filled to [`PANEL_BREAKER_COUNT`]
    /// one at a time, one panel takes whatever is left over, and the rest stand idle. Packing this
    /// way maximises the figure, because a panel's copper loss and leakage reactance rise with the
    /// square of its loading.
    ///
    /// A count above aggregate capacity is one the present installation cannot hold, so it can be
    /// interpreted in three ways, not mutually exclusive: (1) more panels have been installed but
    /// [`PANEL_COUNT`] has not been updated; and/or (2) the session start/end adjustments cause an
    /// artificial overlap of charging sessions; and/or (3) normal power fluctuations cause the
    /// per-EV power draw to exceed [`ev_load()`]. The excess is priced as (1): further panels of
    /// the same kind, so the load is proportional to the count at the rate one full panel sets.
    fn scaled_load(scaling: f64) -> Load {
        Self::load_over_panels(scaling, PANEL_COUNT)
    }

    /// [`scaled_load`](Self::scaled_load) over a stated number of panels rather than over
    /// [`PANEL_COUNT`].
    ///
    /// The count is a parameter for the tests' sake. This site has one panel, so every rule
    /// `scaled_load` follows about several of them — filling one before starting the next, an
    /// idle panel still drawing its standing block, the boundary between one panel and the next —
    /// would otherwise be unreachable, and a test written against it would assert nothing.
    fn load_over_panels(scaling: f64, panels: u8) -> Load {
        let panel_capacity = f64::from(PANEL_BREAKER_COUNT);
        let panel_count = f64::from(panels);
        let aggregate_capacity = panel_count * panel_capacity;

        // Imputed load for a vehicle count (`scaling`) above the aggregate panel capacity.
        // Load is proportional to the count, at the average load of a full panel.
        if scaling > aggregate_capacity {
            return single_panel_load(panel_capacity).scaled(scaling / panel_capacity);
        }

        let full_panels = (scaling / panel_capacity).floor();
        let residual = scaling - full_panels * panel_capacity;
        // A panel holding no vehicles is an idle panel, and `single_panel_load(0.0)` is what it
        // draws; counting it as a partial one would add a second standing block for the same
        // hardware.
        let partial_panels = f64::from(residual > 0.0);
        let idle_panels = panel_count - full_panels - partial_panels;

        single_panel_load(panel_capacity).scaled(full_panels)
            + single_panel_load(residual).scaled(partial_panels)
            + single_panel_load(0.0).scaled(idle_panels)
    }

    pub(crate) fn add_session(&mut self, session: RSession) {
        self.sessions.push(session);
    }
}

// ---------------------------------------------------------------------------
// Anomalies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyKind {
    /// `Active_Charge_Time` is zero so its `avg_kw` cell shows `#DIV/0!`.
    ZeroActiveChargeTime,
    /// `Conn_start + Conn_Duration` misses the reported `Conn_DateTime_End` by more than
    /// truncation to `TIME_GRID_STEP` can account for, early or late, so the reported start, end
    /// and duration are mutually inconsistent. The two sides of the window are not the same width;
    /// `duration_is_consistent` states each exactly.
    ///
    /// The test is `duration_is_consistent`, which carries the three checks and their derivation.
    ///
    /// Every direction is a fault, and all of them exclude the session from the estimates: if a
    /// record's own fields disagree by more than the reporting can explain, neither its duration
    /// nor the span the estimating logic would place it on can be relied on.
    ///
    /// See `docs/session/time-reporting-uncertainty.md` and docs/session/README.md, "Anomalies".
    InconsistentDuration,
    /// The start fell in the DST fold and both offsets reproduce the reported end,
    /// so the record was duplicated. See docs/time/README.md, "Time zone".
    DstAmbiguousDuplicated,
    /// The reported start or end fell in the DST gap, i.e. a wall time that never occurred.
    ///
    /// **No instant is assigned.** There is none to assign: the clocks jumped over that wall time,
    /// so it names nothing, and shifting it to either side of the gap would be a guess dressed as a
    /// reading. The session is given the sentinels `time::UNPLACEABLE_START` and
    /// `time::UNPLACEABLE_END` — see [`Self::leaves_no_instant`] — and is excluded from every
    /// estimate.
    ///
    /// Raised once per session whichever end it came from, since the kind does not say which. Every
    /// other test that reads the instants is skipped for such a record; the reported wall times are
    /// still written to the workbook's local columns, and the row and file name where the record
    /// is. See docs/time/README.md, "Time zone".
    FellInDstGap,
    /// A reported wall time fell in the DST fold, and no combination of the readings at either end
    /// satisfies `duration_is_consistent`.
    ///
    /// A wall time in the repeated hour can be read under either offset, so a record with one such
    /// end yields two candidate combinations and one with both yields four. `Conn_Duration` is what
    /// ordinarily says which: the true combination is the one whose elapsed time agrees with the
    /// reported duration. When *no* combination agrees, that evidence has failed — whatever
    /// `Conn_Duration` measures on this row, it is not the elapsed time the inference assumes it to
    /// be — and there is nothing else to choose with.
    ///
    /// **No instant is assigned**: the record names candidates and nothing picks between them, so
    /// picking one would be a guess. The outcome is [`Self::FellInDstGap`]'s and the predicate
    /// naming both is [`Self::leaves_no_instant`], but the reason is not shared — there a wall time
    /// names no instant at all, where here it names several and the evidence that would choose has
    /// failed. [`Self::InconsistentDuration`] accompanies it, since a record whose fields agree
    /// under no reading is inconsistent however it is read.
    ///
    /// Distinct from `InconsistentDuration` alone, which is the same disagreement on a date with
    /// only one reading to test. Keeping them apart is what says *why* the record could not be
    /// placed. See docs/time/README.md, "Time zone".
    DstUnresolvable,
    /// The session's average power exceeds [`BREAKER_MAX_NORMAL_KW`], which the hardware is
    /// supposed to make impossible.
    ///
    /// [`BREAKER_RATING_KW`] alone is not the bound: the breaker limits current, so the power a
    /// vehicle draws rises and falls with the supply voltage, and a draw within the normal voltage
    /// band is what the installation does rather than a fault. Above the band, one of
    /// `Energy_Use` and `Active_Charge_Time` is wrong.
    ///
    /// Informational only: the session still takes part in every estimate, since nothing about the
    /// figure says *which* of the two is wrong, or whether either is. The kinds that exclude a
    /// session are [`AnomalyKind::InconsistentDuration`] and [`AnomalyKind::FellInDstGap`].
    ExcessiveAvgKw,
    /// Another session in the same list carries the same `Charge_Session_ID`.
    ///
    /// `Charge_Session_ID` is not unique in Evolute's reports: the June 2026 report carries
    /// `S37487` on two sessions a week apart. Informational only — every session so flagged takes
    /// part in the estimates exactly as it would otherwise, since two records sharing an id are
    /// two sessions until something says otherwise.
    DuplicateId,

    /// The reported start or end does not land on a whole `TIME_GRID_STEP`.
    ///
    /// Informational only. Every allowance this software makes for the reporting's truncation
    /// assumes the reported times are truncated to that step. If Evolute starts reporting seconds,
    /// they no longer are, and the allowances become too wide rather than wrong — a session gets a
    /// padded end it does not need, and the consistency window admits records it should reject.
    /// Nothing crashes and no figure looks odd, which is exactly why it needs saying.
    ///
    /// A property of the record's own times, so it travels on [`Session::anomalies`] and survives
    /// the round trip through a workbook honestly: the times it describes are the ones written.
    ///
    /// Expect it on every row or on none. A report that has switched resolution has switched it
    /// throughout, so whatever renders these should say how many rather than list them all.
    OffGridTimes,

    /// A workbook column disagrees with what the [`Session`] methods recompute from it, or does not
    /// hold a value of the right kind at all.
    ///
    /// The sheet is stale or was edited. The recomputed value always wins and no figure changes;
    /// this only says the stored one no longer matches.
    ///
    /// A fact about the workbook rather than about the session: the CSV that workbook was written
    /// from disagrees with nothing, and a second workbook of the same sessions need not disagree
    /// either. So it lives on [`Sessions::anomalies`], where the findings that belong to a file
    /// rather than to a record go.
    ///
    /// Says only that a column disagreed. Which one, what it held and what was recomputed are not
    /// carried: an [`AnomalyKind`] is a bare token, and `Anomaly` is a session and a kind.
    ///
    /// Which is why a discrepancy is logged and never changes a figure. Every other kind here
    /// describes the reported data and one of them removes the session from every estimate; if a
    /// stale cell could do the same, editing a workbook would silently change which sessions feed
    /// an estimate, and the estimate would still look clean.
    WorkbookDiscrepancy,
}

impl AnomalyKind {
    /// Whether this kind can move a figure that sums energy over a period.
    ///
    /// Reported by relevance rather than in full, because a list a reader learns to skip is worse
    /// than no list. An energy figure spreads each session's kilowatt-hours over the time it was
    /// connected and cuts the result at the period's boundaries; only three kinds bear on that:
    ///
    /// - [`Self::InconsistentDuration`] — the session is left out of the sum entirely.
    /// - [`Self::FellInDstGap`] and [`Self::DstUnresolvable`] — left out of the sum entirely, for
    ///   the same reason: a wall time naming no instant, or two, leaves the span it bounds
    ///   unknown.
    /// - [`Self::DuplicateId`] — two records may be one session counted twice, or one id on two
    ///   sessions; the energy differs by a whole session either way.
    ///
    /// The rest do not. [`Self::ZeroActiveChargeTime`] and [`Self::ExcessiveAvgKw`] are about
    /// power, which is not what is summed; [`Self::DstAmbiguousDuplicated`] is a fold already
    /// resolved; [`Self::OffGridTimes`] and [`Self::WorkbookDiscrepancy`] are facts about the file
    /// rather than the session.
    ///
    /// The demand side reports every kind instead, since an estimate over a single hour turns on
    /// each session's power and on exactly which records touch that hour.
    pub fn bears_on_energy(&self) -> bool {
        matches!(
            self,
            Self::InconsistentDuration
                | Self::FellInDstGap
                | Self::DuplicateId
                | Self::DstUnresolvable
        )
    }

    /// Whether this kind means no instant could be assigned to the record's reported times.
    ///
    /// The two that hand a session the sentinels `time::UNPLACEABLE_START` and
    /// `time::UNPLACEABLE_END` instead of a reading: [`Self::FellInDstGap`], where a reported wall
    /// time never occurred, and [`Self::DstUnresolvable`], where it occurred twice and nothing in
    /// the record says which.
    ///
    /// This is how the workbook reader recognises such a row. The writer leaves every cell derived
    /// from those two fields empty, so the `anomalies` column is the only thing left saying what the
    /// row is — and it is the column already read first. Recognising it by the token rather than by
    /// the stored instants is also what keeps the round trip exact: a serial carries whole seconds,
    /// and the sentinels are not on a whole second.
    pub fn leaves_no_instant(&self) -> bool {
        matches!(self, Self::FellInDstGap | Self::DstUnresolvable)
    }

    /// Whether this kind removes the session from every estimate.
    ///
    /// Three kinds do, and all for the same reason: the record cannot be placed on a timeline.
    /// [`Self::InconsistentDuration`] means start, end and duration contradict each other. The
    /// other two are [`Self::leaves_no_instant`]'s pair, where there is no timeline position to
    /// argue about — [`Self::FellInDstGap`] because a reported wall time never occurred, and
    /// [`Self::DstUnresolvable`] because it occurred twice and the record does not say which.
    ///
    /// Those two exclude on their own rather than by relying on `InconsistentDuration` travelling
    /// with them. It always does, since a record with no instant is given an inverted span that
    /// fails the consistency test — but the sessions carry the sentinels, and a hand-edited
    /// `anomalies` cell that dropped the companion flag would otherwise put an inverted span in
    /// front of the estimating logic.
    ///
    /// This is what [`Sessions::from_session_lists`] sorts on, so a kind added here excludes
    /// sessions from both readers at once.
    pub fn excludes_session(&self) -> bool {
        matches!(self, Self::InconsistentDuration) || self.leaves_no_instant()
    }

    /// The variant name, as written to the workbook's `anomalies` column. Deliberately distinct
    /// from [`fmt::Display`], which is free-form prose for humans and may be reworded at will;
    /// this is a wire format and should preferably stay stable.
    ///
    /// Preferably rather than must: the only thing reading a token back is
    /// `session::excel::historic`, behind the `historic` feature. A rename leaves workbooks already
    /// written spelling the kind one way and the code spelling it another, which costs whoever
    /// reads an old sheet or revives that reader — not anything on the default build.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ZeroActiveChargeTime => "ZeroActiveChargeTime",
            Self::InconsistentDuration => "InconsistentDuration",
            Self::DstAmbiguousDuplicated => "DstAmbiguousDuplicated",
            Self::FellInDstGap => "FellInDstGap",
            Self::DstUnresolvable => "DstUnresolvable",
            Self::ExcessiveAvgKw => "ExcessiveAvgKw",
            Self::DuplicateId => "DuplicateId",
            Self::OffGridTimes => "OffGridTimes",
            Self::WorkbookDiscrepancy => "WorkbookDiscrepancy",
        }
    }

    /// Inverse of [`AnomalyKind::as_str`]. `None` for an unrecognised token.
    pub fn from_token(s: &str) -> Option<Self> {
        Some(match s {
            "ZeroActiveChargeTime" => Self::ZeroActiveChargeTime,
            "InconsistentDuration" => Self::InconsistentDuration,
            "DstAmbiguousDuplicated" => Self::DstAmbiguousDuplicated,
            "FellInDstGap" => Self::FellInDstGap,
            "DstUnresolvable" => Self::DstUnresolvable,
            "ExcessiveAvgKw" => Self::ExcessiveAvgKw,
            "DuplicateId" => Self::DuplicateId,
            "OffGridTimes" => Self::OffGridTimes,
            "WorkbookDiscrepancy" => Self::WorkbookDiscrepancy,
            _ => return None,
        })
    }
}

/// A session that needs review. Never fatal: the conversion still writes the row, and the
/// estimating logic still produces a figure. Used by both sides — see
/// [`crate::session::SessionWriteReport`] and [`crate::session::IntervalEstimates`].
///
/// Holds the session itself rather than a copy of a field or two off it. Copying `id` and `row` out
/// meant every consumer that wanted anything else — the average power beside the flag, the file the
/// row is in — had to find its way back to the session through a key, and no key available is one
/// Evolute guarantees: ids repeat, and `(path, row)` is shared by the two halves of a DST fold.
/// Holding the `Rc` is what removes that question.
#[derive(Debug, Clone)]
pub struct Anomaly {
    pub session: RSession,
    pub kind: AnomalyKind,
}

impl fmt::Display for AnomalyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::ZeroActiveChargeTime => {
                "zero Active_Charge_Time, so the session delivered its energy in no time at all \
                 and has no finite average power; the estimating logic substitutes one, and the \
                 session is worth reviewing individually"
            }
            Self::InconsistentDuration => {
                "reported start, end and duration contradict each other by more than truncation \
                 to the minute can explain; the session is excluded from every estimate"
            }
            Self::DstAmbiguousDuplicated => "ambiguous DST fold; record duplicated as EDT and EST",
            Self::FellInDstGap => {
                "reported start or end is a local time that never occurred, in the hour the \
                 clocks jump over when DST begins; it names no instant, so none was assigned and \
                 the session is excluded from every estimate"
            }
            Self::DstUnresolvable => {
                "DST fold: a reported time falls in the repeated hour and no reading of the \
                 record makes its start, end and duration agree, so no instant was assigned and \
                 the session is excluded from every estimate"
            }
            Self::ExcessiveAvgKw => {
                "average kilowatts above the Evolute breaker rating at the top of the normal \
                 voltage band, which the hardware should not allow; the session still counts \
                 towards every estimate"
            }
            Self::DuplicateId => {
                "another session in the report carries the same Charge_Session_ID; the id is not \
                 unique in Evolute's reports, so both sessions still count towards every estimate"
            }
            Self::OffGridTimes => {
                "the reported start or end does not land on a whole minute, so the report's \
                 resolution has become finer than this software's time grid; nothing is wrong with \
                 the record, but the padding and the consistency window are now wider than the \
                 data needs"
            }
            Self::WorkbookDiscrepancy => {
                "a stored column in the workbook disagrees with what this software recomputes from \
                 the row, so the sheet is stale or was edited; the recomputed value is the one used"
            }
        };
        f.write_str(s)
    }
}

impl fmt::Display for Anomaly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The token as well as the prose, in the order the report's glossary uses. A reader who
        // meets `FellInDstGap` in the workbook's `anomalies` column has no way back to the prose
        // otherwise: the two surfaces this renders -- the run log and the Convert tab's list --
        // are where that connection has to be made.
        write!(
            f,
            "row {} ({}) {}: {}",
            self.session.row,
            self.session.id,
            self.kind.as_str(),
            self.kind
        )
    }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

/// Sessions, grouped by how the peak power contribution logic must treat them, and what else is
/// known about them.
///
/// Named for the objects rather than for the context around them, the way `green_button::Readings`
/// is, so the two sources read the same way.
///
/// This is where a finding goes when it is not a property of any one session: a relation between
/// records, or a fact about the file they were read from. What is true of a record itself goes on
/// [`Session::anomalies`] instead, and travels with it — including out to a workbook and back
/// through the `anomalies` column.
///
/// Returned by both readers — the private `csv::csv_sessions` from the CSV,
/// and `excel::historic::xlsx_to_sessions` from a workbook written from it — because the grouping
/// is a property of the sessions, not of the file they were read out of. The workbook reader is
/// behind the `historic` feature; the CSV one is what the API uses. The writing direction returns
/// a [`crate::session::SessionWriteReport`] instead.
///
/// It was `SessionReport` until this crate had three things called a report: the document a
/// [`Display`](std::fmt::Display) writes, the CSV Evolute exports, and this. Only the CSV is still
/// called one.
#[derive(Debug)]
pub struct Sessions {
    /// Sessions with a finite average power. This is what the peak power contribution logic
    /// consumes unaltered. A session with zero `Energy_Use` belongs here — its `avg_kw` is
    /// legitimately zero, and it still occupies a breaker.
    pub sessions: Vec<RSession>,
    /// Sessions with zero `Active_Charge_Time`, so [`Session::charge_time`] is zero and energy
    /// over charge time is infinite or `NaN`. Kept out of `session` because those values would
    /// swamp or poison any segment they entered.
    ///
    /// Surfaced rather than dropped because such a row is almost certainly a **reporting fault**
    /// and someone should see it. That is a correction: the reason given here used to be that
    /// energy delivered in no time at all is what a demand charge bills on, which read the field
    /// as a real measurement of charging. Evolute has since stated that the three duration fields
    /// track the same thing to within about a second and are not measured separately, so a zero
    /// beside a non-zero `Energy_Use` is a contradiction in the report rather than an event. See
    /// `Questions_for_Evolute.md`, "Answers received". [`Session::avg_kw`] substitutes a finite
    /// figure so the row can still be listed. See docs/session/README.md, "Anomalies".
    pub spikes: Vec<RSession>,
    /// Sessions that cannot be placed on a timeline — every kind [`AnomalyKind::excludes_session`]
    /// names. Either the reported start, end and duration contradict each other, or a reported wall
    /// time names no instant at all and none was assigned. Excluded from the estimates and returned
    /// only for review. See docs/session/README.md, "Anomalies".
    pub excluded: Vec<RSession>,
    /// Anomalies that are not properties of any single record, and so are not reachable through
    /// [`Session::anomalies`]. Currently [`AnomalyKind::DuplicateId`], plus
    /// [`AnomalyKind::WorkbookDiscrepancy`] when the `historic` workbook reader produced this
    /// value.
    ///
    /// Separate from the sessions because such an anomaly is a relation between records rather than
    /// a fault in one: an id is a duplicate only relative to another session, and which of the two
    /// is at fault is not a question the data answers.
    pub anomalies: Vec<Anomaly>,
    /// The files these sessions were read from, in the order they were read.
    ///
    /// Not derived from the sessions. Every [`Session`] carries its own `path`, but a file holding
    /// no session — or none that survived — would then leave no trace, and "this file contributed
    /// nothing" is exactly what a reader checking a period against two monthly exports needs to be
    /// able to see.
    pub sources: Vec<PathBuf>,
    /// The run logs, one per file read, each saying either that nothing was found or what was.
    /// What they hold depends on which reader produced this report — see their docs.
    ///
    /// Held rather than written. A reader returns what it found and leaves writing it to whoever
    /// asked, which for a `Vec<PathBuf>` of already-written files was impossible: by the time the
    /// caller saw the paths the files were there. [`Sessions::write_logs`] is how a binary puts
    /// them where a user can read them.
    ///
    /// A vector because a report can be built from several files at once — see
    /// [`Sessions::merge`] — and each source file has a log of its own.
    pub logs: Vec<SourceLog>,
}

impl Sessions {
    /// Merges the session lists and sorts what survives into the three buckets.
    ///
    /// One call rather than two because every caller wants both and neither step is useful without
    /// the other. Merging decides which records are the same session — and is also where a shared
    /// `Charge_Session_ID` is noticed — while bucketing decides what each surviving session is fit
    /// for. A caller passing one list still gets the merge; one file can repeat a record as
    /// readily as two files can, which is how a single-file read is the same code path as a
    /// two-file one.
    ///
    /// Bucketing is kept here, and out of both readers, so that a session read from a CSV and the
    /// same session read back from the workbook written from it cannot land in different buckets.
    /// The tests are applied in this order, strongest first:
    ///
    /// 1. Flagged with any kind [`AnomalyKind::excludes_session`] names — [`Sessions::excluded`].
    ///    Such a
    ///    session takes no part in the estimates whatever its charge time, and letting one through
    ///    would put an inverted session in front of the segmenting logic, whose endpoints would
    ///    then arrive out of order.
    /// 2. Zero `Active_Charge_Time` — [`Sessions::spikes`].
    /// 3. Everything else — [`Sessions::sessions`].
    ///
    /// Order within each bucket is the order given, which for both readers is report order.
    ///
    /// Public because the `api::pure` entry points take a [`Sessions`] rather than a list, and a
    /// caller outside this crate with sessions of its own needs a way to make one.
    pub fn from_session_lists(
        session_lists: Vec<Vec<RSession>>,
        sources: Vec<PathBuf>,
        logs: Vec<SourceLog>,
    ) -> Self {
        let MergedSessions {
            sessions,
            anomalies,
            collapsed,
        } = MergedSessions::merge_sessions(session_lists);
        let mut report = Self {
            sessions: Vec::new(),
            spikes: Vec::new(),
            excluded: Vec::new(),
            anomalies,
            sources,
            logs,
        };
        report.note_collapsed(&collapsed);
        for session in sessions {
            if session.anomalies.iter().any(AnomalyKind::excludes_session) {
                report.excluded.push(session);
            } else if session.charge_time.is_zero() {
                report.spikes.push(session);
            } else {
                report.sessions.push(session);
            }
        }
        report
    }

    /// Writes one line per collapsed record onto the log of the file that record came from.
    ///
    /// That file rather than the one holding the surviving copy: a reader who opens a log has the
    /// matching source file in front of them, and the row named in the line has to be a row of it.
    /// A collapse whose file has no log among `logs` is not recorded — the caller that supplied no
    /// log for a file it read has nowhere to put it.
    ///
    /// The wording says the fields are equal, because the other kind of repeated id says the
    /// opposite: records sharing a `Charge_Session_ID` that differ anywhere are all kept, and are
    /// flagged [`AnomalyKind::DuplicateId`] instead of collapsed.
    fn note_collapsed(&mut self, collapsed: &[Collapse]) {
        for Collapse { dropped, kept } in collapsed {
            let Some(log) = self
                .logs
                .iter_mut()
                .find(|log| log.source.as_path() == dropped.path.as_path())
            else {
                continue;
            };
            log.log.note(format!(
                "row {}: session {} repeats {} row {}, with every compared field equal; this copy \
                 was dropped so the session is counted once. Records sharing an id that differ \
                 are kept and flagged {} instead.",
                dropped.row,
                dropped.id,
                kept.path.display(),
                kept.row,
                AnomalyKind::DuplicateId.as_str(),
            ));
        }
    }

    /// One report from several, each read from its own file.
    ///
    /// A billing period straddles two calendar months and an Evolute report covers one, so the
    /// figures for a period are drawn from two files. This is what puts them together, and it is
    /// deliberately not a concatenation: the same session appears in both files when it spans the
    /// month boundary, and counting it twice would inflate every figure derived from it.
    ///
    /// The sessions go back through [`Self::from_session_lists`] as one list per file, which is
    /// the shape the merge needs to tell "one session in two files" from "one id on two sessions".
    /// Each report's own `anomalies` are therefore dropped rather than
    /// concatenated: [`AnomalyKind::DuplicateId`] is re-derived from the combined records, which
    /// finds every duplicate the separate reads found and the cross-file ones besides.
    ///
    /// Re-derivation recovers that kind and no other. [`AnomalyKind::WorkbookDiscrepancy`], which
    /// the `historic` workbook reader also puts on [`Self::anomalies`], does not survive a merge.
    /// No caller merges workbook-sourced reports today; a caller that did would have to carry
    /// those anomalies across itself.
    ///
    /// `sources` and `logs` are concatenated in the order given.
    pub fn merge(reports: Vec<Self>) -> Self {
        let mut session_lists = Vec::with_capacity(reports.len());
        let mut sources = Vec::new();
        let mut logs = Vec::new();
        for mut report in reports {
            sources.append(&mut report.sources);
            logs.append(&mut report.logs);
            let mut all = report.sessions;
            all.extend(report.spikes);
            all.extend(report.excluded);
            session_lists.push(all);
        }
        Self::from_session_lists(session_lists, sources, logs)
    }

    /// Writes each source's log beside it, returning where they went in the same order.
    ///
    /// For a binary, which has nowhere to return what it found. Nothing in the library calls this:
    /// a reader returns its logs and a computation never has any.
    ///
    /// # Errors
    ///
    /// The first write that fails, with none of the later ones attempted. A log the user believes
    /// exists and does not is worse than no log at all, so the failure is reported rather than
    /// passed over.
    pub fn write_logs(&self) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        self.logs.iter().map(SourceLog::write).collect()
    }

    /// The sessions whose energy may be placed on a timeline: [`Self::sessions`] and
    /// [`Self::spikes`].
    ///
    /// [`Self::excluded`] is left out rather than overlooked. Those records' start, end and
    /// duration contradict each other, and an inverted one panics in `adj_duration` before any
    /// figure comes of it.
    ///
    /// A spike is counted because the contradiction in it is between energy and *charge time*: the
    /// energy is still energy, and the connection window it was drawn over is still a window. Only
    /// figures that divide by charge time have to hold one out.
    pub fn countable(&self) -> Vec<RSession> {
        self.sessions.iter().chain(&self.spikes).cloned().collect()
    }

    /// What a figure drawn from these sessions should say about them.
    ///
    /// `relevant` decides which anomaly kinds are worth a reader's attention for the figure being
    /// produced — [`AnomalyKind::bears_on_energy`] for the consumption side, everything for the
    /// demand side. Both the sessions' own anomalies and this report's are put through it, since a
    /// duplicate id bears on a figure whichever list it was found in.
    ///
    /// Excluded sessions and sources are not filtered. A session left out of the figures is left
    /// out whatever the figure is, and every file read is worth naming even when it contributed
    /// nothing — that is the case a reader cannot tell from a wrong file otherwise.
    pub fn notes(&self, relevant: fn(&AnomalyKind) -> bool) -> SessionNotes {
        let own = self
            .sessions
            .iter()
            .chain(&self.spikes)
            .chain(&self.excluded)
            .flat_map(|session| {
                session.anomalies.iter().map(move |kind| Anomaly {
                    session: session.clone(),
                    kind: *kind,
                })
            });
        SessionNotes {
            sources: self.sources.clone(),
            anomalies: own
                .chain(self.anomalies.iter().cloned())
                .filter(|a| relevant(&a.kind))
                .collect(),
            excluded: self.excluded.clone(),
            logs: self.logs.clone(),
        }
    }
}

/// What a figure was drawn from, and what was odd about it.
///
/// Every result the API returns carries one. A figure a reader cannot check is a figure they have
/// to take on trust, and the three things they need in order to check it — which files it came
/// from, which records were left out, and what needed a judgement call — were until now reachable
/// only from a log file written beside the input, or from nowhere at all.
///
/// The anomalies are filtered by what bears on the figure; see [`Sessions::notes`]. The sources and
/// the excluded sessions are not.
#[derive(Debug, Clone, Default)]
pub struct SessionNotes {
    /// Every file read, in the order read, including any that contributed no session.
    pub sources: Vec<PathBuf>,
    /// The anomalies that bear on this figure, both the sessions' own and the relations between
    /// them.
    pub anomalies: Vec<Anomaly>,
    /// Sessions left out of the figures entirely — every kind [`AnomalyKind::excludes_session`]
    /// names.
    ///
    /// Listed rather than counted. Such a record either contradicts itself or names no instant at
    /// all, so nothing short of the row itself lets a reader judge what happened.
    pub excluded: Vec<RSession>,
    /// The run logs of the files read, unwritten. See [`Sessions::logs`].
    pub logs: Vec<SourceLog>,
}

impl SessionNotes {
    /// Whether there is nothing to report: no anomalies and nothing excluded.
    ///
    /// Sources do not count. Every figure has those, and a report that named its files only when
    /// something was wrong would read as an alarm.
    pub fn is_clean(&self) -> bool {
        self.anomalies.is_empty() && self.excluded.is_empty()
    }

    /// Adds anomalies not already held, keeping the order they arrive in.
    ///
    /// For a figure whose parts are scoped differently. The consumption side collects what bears
    /// on energy across the whole period; the demand side collects every kind, but only for the
    /// hours it prices. A figure built from both says both, and a session that is on both lists is
    /// on it once.
    ///
    /// Sameness is the same session and the same kind, and the session is compared by identity
    /// rather than by value: these all come from one [`Sessions`], where a record appears once.
    pub fn add_anomalies(&mut self, anomalies: impl IntoIterator<Item = Anomaly>) {
        for anomaly in anomalies {
            let held = self
                .anomalies
                .iter()
                .any(|a| a.kind == anomaly.kind && Rc::ptr_eq(&a.session, &anomaly.session));
            if !held {
                self.anomalies.push(anomaly);
            }
        }
    }

    /// Writes each source's log beside it, returning where they went.
    ///
    /// For a binary. See [`Sessions::write_logs`], which this is the result-side counterpart of.
    pub fn write_logs(&self) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        self.logs.iter().map(SourceLog::write).collect()
    }
}

// cargo test --lib -- session::common::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::log::RunLog;

    fn session(path: &str, row: usize, id: &str, start: &str, energy_use: f64) -> RSession {
        let conn_start: Timestamp = start.parse().expect("an RFC 3339 timestamp");
        let conn_duration = Duration::from_secs(3600);
        Rc::new(Session {
            path: Rc::new(PathBuf::from(path)),
            row,
            id: id.to_owned(),
            conn_start,
            conn_end: conn_start + conn_duration,
            conn_duration,
            charge_time: conn_duration,
            energy_use,
            anomalies: Vec::new(),
        })
    }

    /// An empty log for one source file, as a reader would hand one back.
    fn source_log(path: &str) -> SourceLog {
        SourceLog {
            source: PathBuf::from(path),
            suffix: "session.csv.read",
            operation: "Read Session Report",
            log: RunLog::new(),
        }
    }

    fn ids(sessions: &[RSession]) -> Vec<&str> {
        sessions.iter().map(|s| s.id.as_str()).collect()
    }

    fn flagged(anomalies: &[Anomaly]) -> Vec<(&str, usize)> {
        anomalies
            .iter()
            .map(|a| {
                assert_eq!(a.kind, AnomalyKind::DuplicateId);
                (a.session.id.as_str(), a.session.row)
            })
            .collect()
    }

    /// The case merging exists for: two monthly reports overlap, and the session in both is one
    /// session. Counting it twice would inflate every figure drawn from it.
    #[test]
    fn a_record_present_identically_in_two_files_is_kept_once() {
        let may = vec![
            session("May.csv", 2, "S1", "2026-05-30T12:00:00Z", 4.0),
            session("May.csv", 3, "S2", "2026-05-31T12:00:00Z", 5.0),
        ];
        // The same two sessions as the next report states them, at its own row numbers.
        let june = vec![
            session("June.csv", 2, "S1", "2026-05-30T12:00:00Z", 4.0),
            session("June.csv", 3, "S2", "2026-05-31T12:00:00Z", 5.0),
            session("June.csv", 4, "S3", "2026-06-01T12:00:00Z", 6.0),
        ];

        let merged = MergedSessions::merge_sessions(vec![may, june]);
        assert_eq!(ids(&merged.sessions), ["S1", "S2", "S3"]);
        // The first file's copy is the one kept, so the rows are its.
        assert_eq!(merged.sessions[0].row, 2);
        assert_eq!(*merged.sessions[0].path, PathBuf::from("May.csv"));
        assert!(
            merged.anomalies.is_empty(),
            "one session reported twice is not a duplicate id"
        );
    }

    /// `Charge_Session_ID` is not unique in Evolute's reports — the June 2026 report carries
    /// `S37487` on two sessions a week apart. Both count, and both are flagged.
    #[test]
    fn a_reused_id_within_one_file_keeps_both_sessions_and_flags_them() {
        let june = vec![
            session("June.csv", 2, "S37487", "2026-06-18T15:29:00Z", 0.0),
            session("June.csv", 3, "S1", "2026-06-20T12:00:00Z", 4.0),
            session("June.csv", 4, "S37487", "2026-06-25T10:08:00Z", 1.0),
        ];

        let merged = MergedSessions::merge_sessions(vec![june]);
        assert_eq!(
            ids(&merged.sessions),
            ["S37487", "S1", "S37487"],
            "two sessions sharing an id are two sessions"
        );
        // Symmetric: both members of the colliding pair, not only the later one.
        assert_eq!(flagged(&merged.anomalies), [("S37487", 2), ("S37487", 4)]);
    }

    /// Two files disagreeing about one session looks exactly like a reused id from here, and is
    /// treated the same way: nothing is dropped, and both records are flagged for a reader to
    /// judge.
    #[test]
    fn files_that_disagree_about_a_session_keep_both_records() {
        let may = vec![session("May.csv", 2, "S1", "2026-05-30T12:00:00Z", 4.0)];
        let june = vec![session("June.csv", 9, "S1", "2026-05-30T12:00:00Z", 4.5)];

        let merged = MergedSessions::merge_sessions(vec![may, june]);
        assert_eq!(merged.sessions.len(), 2);
        assert_eq!(flagged(&merged.anomalies), [("S1", 2), ("S1", 9)]);
    }

    /// The same rule inside one file: a report that states a record twice, identically, states one
    /// session. Nothing here looks at which list a record arrived in, and this pins that.
    #[test]
    fn a_record_repeated_identically_within_one_file_is_kept_once() {
        let june = vec![
            session("June.csv", 2, "S1", "2026-06-01T12:00:00Z", 4.0),
            session("June.csv", 3, "S1", "2026-06-01T12:00:00Z", 4.0),
            session("June.csv", 4, "S2", "2026-06-02T12:00:00Z", 5.0),
        ];

        let merged = MergedSessions::merge_sessions(vec![june]);
        assert_eq!(ids(&merged.sessions), ["S1", "S2"]);
        // The first copy is the one kept, exactly as across two files.
        assert_eq!(merged.sessions[0].row, 2);
        assert!(
            merged.anomalies.is_empty(),
            "one session stated twice is not a duplicate id"
        );
        assert_eq!(merged.collapsed.len(), 1);
        assert_eq!(merged.collapsed[0].dropped.row, 3);
        assert_eq!(merged.collapsed[0].kept.row, 2);
    }

    /// A collapse is not silent. The line goes on the log of the file the dropped copy came from,
    /// and says the fields were equal — which is what tells it from the reused-id flag.
    #[test]
    fn a_collapsed_record_is_logged_against_its_own_file() {
        let may = vec![session("May.csv", 2, "S1", "2026-05-30T12:00:00Z", 4.0)];
        let june = vec![session("June.csv", 7, "S1", "2026-05-30T12:00:00Z", 4.0)];
        let logs = vec![source_log("May.csv"), source_log("June.csv")];

        let report = Sessions::from_session_lists(
            vec![may, june],
            vec![PathBuf::from("May.csv"), PathBuf::from("June.csv")],
            logs,
        );

        assert!(
            report.logs[0].log.is_empty(),
            "the kept copy's file is quiet"
        );
        let text = report.logs[1]
            .log
            .render(report.logs[1].operation, &report.logs[1].source);
        assert!(text.contains("row 7"), "{text}");
        assert!(text.contains("May.csv row 2"), "{text}");
        assert!(text.contains("every compared field equal"), "{text}");
        assert!(text.contains("DuplicateId"), "{text}");
    }

    /// A single list is merged like any other -- see
    /// `a_record_repeated_identically_within_one_file_is_kept_once`. With nothing repeated in it,
    /// nothing is dropped and nothing is flagged.
    #[test]
    fn merging_one_list_leaves_it_alone() {
        let sessions = vec![
            session("June.csv", 2, "S1", "2026-06-01T12:00:00Z", 4.0),
            session("June.csv", 3, "S2", "2026-06-02T12:00:00Z", 5.0),
        ];

        let merged = MergedSessions::merge_sessions(vec![sessions]);
        assert_eq!(ids(&merged.sessions), ["S1", "S2"]);
        assert!(merged.anomalies.is_empty());
    }

    /// Two reports become one, and the file that contributed nothing is still named.
    ///
    /// That last part is the point of keeping `sources` rather than deriving them from the
    /// sessions: a month nobody charged in and a wrong file picked by mistake produce the same
    /// figures, and only the list of files read tells them apart.
    #[test]
    fn merging_reports_keeps_every_file_that_was_read() {
        let june = Sessions::from_session_lists(
            vec![vec![session(
                "June.csv",
                2,
                "S1",
                "2026-06-01T12:00:00Z",
                4.0,
            )]],
            vec![PathBuf::from("June.csv")],
            Vec::new(),
        );
        let quiet = Sessions::from_session_lists(
            vec![Vec::new()],
            vec![PathBuf::from("May.csv")],
            Vec::new(),
        );

        let merged = Sessions::merge(vec![quiet, june]);
        assert_eq!(ids(&merged.sessions), ["S1"]);
        assert_eq!(
            merged.sources,
            [PathBuf::from("May.csv"), PathBuf::from("June.csv")]
        );
    }

    /// A duplicate id spanning two files is found by merging the reports, not by concatenating
    /// what each of them found on its own.
    #[test]
    fn merging_reports_finds_the_duplicates_neither_file_could() {
        let one = |file: &str, row, id: &str, start: &str| {
            Sessions::from_session_lists(
                vec![vec![session(file, row, id, start, 4.0)]],
                vec![PathBuf::from(file)],
                Vec::new(),
            )
        };
        // The same id on two different sessions, a day apart, one in each file. Neither report can
        // see it alone.
        let may = one("May.csv", 2, "S1", "2026-05-30T12:00:00Z");
        let june = one("June.csv", 2, "S1", "2026-06-01T12:00:00Z");
        assert!(may.anomalies.is_empty() && june.anomalies.is_empty());

        let merged = Sessions::merge(vec![may, june]);
        assert_eq!(flagged(&merged.anomalies), [("S1", 2), ("S1", 2)]);
    }

    /// The consumption side is told only what can move a sum of kilowatt-hours; the demand side
    /// takes everything. A list a reader learns to skip is worse than no list.
    #[test]
    fn notes_report_by_relevance() {
        let mut over = session("June.csv", 2, "HOT", "2026-06-01T12:00:00Z", 4.0);
        Rc::get_mut(&mut over)
            .expect("sole owner")
            .anomalies
            .push(AnomalyKind::ExcessiveAvgKw);
        let report = Sessions::from_session_lists(
            vec![vec![over]],
            vec![PathBuf::from("June.csv")],
            Vec::new(),
        );

        assert!(
            report
                .notes(AnomalyKind::bears_on_energy)
                .anomalies
                .is_empty(),
            "power above the breaker rating cannot move a sum of kilowatt-hours"
        );
        assert_eq!(report.notes(|_| true).anomalies.len(), 1);
    }

    /// An excluded session is listed whatever the figure, since its absence moves every figure and
    /// appears in none of them.
    #[test]
    fn notes_list_what_was_left_out_whatever_the_figure() {
        let mut broken = session("June.csv", 2, "BAD", "2026-06-01T12:00:00Z", 4.0);
        Rc::get_mut(&mut broken)
            .expect("sole owner")
            .anomalies
            .push(AnomalyKind::InconsistentDuration);
        let report = Sessions::from_session_lists(
            vec![vec![broken]],
            vec![PathBuf::from("June.csv")],
            Vec::new(),
        );

        for notes in [
            report.notes(AnomalyKind::bears_on_energy),
            report.notes(|_| true),
        ] {
            assert_eq!(notes.excluded.len(), 1, "{notes:?}");
            assert!(!notes.is_clean());
        }
    }

    // -----------------------------------------------------------------------
    // Scaling past one panel
    // -----------------------------------------------------------------------

    const LOAD_TOLERANCE: f64 = 1e-9;

    /// The panel counts every rule below is checked at.
    ///
    /// `PANEL_COUNT` first, because that is the site the crate actually reports on, then two and
    /// three, which are the only way to reach the multi-panel arithmetic while this site has one
    /// panel. Three rather than just two: at two panels "all but the first" and "the last" name
    /// the same panel, and a rule that confused them would still pass.
    const PANEL_COUNTS: [u8; 3] = [PANEL_COUNT, 2, 3];

    fn panel_capacity() -> f64 {
        f64::from(PANEL_BREAKER_COUNT)
    }

    /// What `panels` panels can hold between them.
    fn aggregate_capacity(panels: u8) -> f64 {
        panel_capacity() * f64::from(panels)
    }

    /// The standing block of every panel but the one the vehicles are packed into.
    fn idle_remainder(panels: u8) -> Load {
        single_panel_load(0.0).scaled(f64::from(panels) - 1.0)
    }

    fn assert_load_close(actual: Load, expected: Load, context: &str) {
        for (a, e, component) in [
            (actual.real_kw, expected.real_kw, "real_kw"),
            (
                actual.reactive_kvar,
                expected.reactive_kvar,
                "reactive_kvar",
            ),
            (
                actual.distortion_kvar,
                expected.distortion_kvar,
                "distortion_kvar",
            ),
        ] {
            assert!(
                (a - e).abs() < LOAD_TOLERANCE,
                "{context}: {component} was {a}, expected {e}"
            );
        }
    }

    /// An installed panel draws its standing block whether or not anything is plugged into it.
    ///
    /// Core loss and magnetizing current are present the moment a transformer is energised, so a
    /// site with nothing charging still draws one standing block per panel. The count that comes
    /// nearest to losing it is a hair above zero, not zero itself, so both are checked: an
    /// arrangement that gave the first branch its own answer for an empty panel would pass the
    /// first assertion and fail the second.
    #[test]
    fn every_installed_panel_draws_its_standing_block() {
        let standing = single_panel_load(0.0);

        for panels in PANEL_COUNTS {
            assert_load_close(
                Segment::load_over_panels(0.0, panels),
                standing.scaled(f64::from(panels)),
                &format!("{panels} panels, nothing charging"),
            );
            assert_load_close(
                Segment::load_over_panels(1e-9, panels),
                single_panel_load(1e-9) + idle_remainder(panels),
                &format!("{panels} panels, almost nothing charging"),
            );
        }
    }

    /// Building another panel adds one standing block and nothing else, so long as the vehicles
    /// already fitted without it.
    ///
    /// The idle-panel rule stated as a difference rather than as a formula, so it does not simply
    /// repeat the implementation: what a panel adds while it holds nothing is its transformer's
    /// standing block, whatever else the site is doing. Counts are held at or below what the
    /// smaller site can take, since above that the two sites are in different branches and the
    /// difference between them is no longer one panel.
    #[test]
    fn an_extra_panel_adds_one_standing_block() {
        let capacity = panel_capacity();
        let standing = single_panel_load(0.0);

        for panels in 1..3u8 {
            let fits = capacity * f64::from(panels);
            for count in [0.0, 0.5, 3.0, capacity, capacity + 0.5, fits] {
                if count > fits {
                    continue;
                }
                assert_load_close(
                    Segment::load_over_panels(count, panels + 1),
                    Segment::load_over_panels(count, panels) + standing,
                    &format!("{count} vehicles, {panels} panels against {}", panels + 1),
                );
            }
        }
    }

    /// The vehicles are packed into as few panels as will hold them, not spread over all of them.
    ///
    /// One panel's worth of vehicles on a two-panel site is one full panel beside one idle one.
    /// Spreading them would give two half-full panels, which is a different and smaller figure —
    /// the same standing blocks, but the copper loss of two panels at half load rather than one
    /// at full. The test names both and asserts which one the model takes.
    #[test]
    fn vehicles_are_packed_into_as_few_panels_as_possible() {
        let capacity = panel_capacity();
        let packed = single_panel_load(capacity) + single_panel_load(0.0);
        let spread = single_panel_load(capacity / 2.0).scaled(2.0);

        let actual = Segment::load_over_panels(capacity, 2);
        assert_load_close(actual, packed, "one panel's worth over two panels");
        assert!(
            actual.real_kw > spread.real_kw,
            "packing gave {}, spreading would have given {}",
            actual.real_kw,
            spread.real_kw
        );
    }

    /// A site filled to aggregate capacity is that many full panels.
    #[test]
    fn a_full_site_is_its_panels_all_full() {
        let full = single_panel_load(panel_capacity());

        for panels in PANEL_COUNTS {
            assert_load_close(
                Segment::load_over_panels(aggregate_capacity(panels), panels),
                full.scaled(f64::from(panels)),
                &format!("{panels} panels, all full"),
            );
        }
    }

    /// Vehicles that fit in one panel load that panel, and every other panel stands idle.
    ///
    /// This is the packing rule at its first step: `Segment::scaled_load` fills panels one at a
    /// time, so a count within one panel's capacity is `single_panel_load` of that count, plus the
    /// standing block of each panel left empty.
    ///
    /// Fractional counts are checked as well as whole ones, because `Segment::agg_count` sums the
    /// share of the segment each session covers, so its count is almost never an integer.
    #[test]
    fn vehicles_within_one_panel_load_that_panel_and_leave_the_rest_idle() {
        for panels in PANEL_COUNTS {
            for tenths in 1..=(PANEL_BREAKER_COUNT * 10) {
                let count = f64::from(tenths) / 10.0;
                assert_load_close(
                    Segment::load_over_panels(count, panels),
                    single_panel_load(count) + idle_remainder(panels),
                    &format!("{panels} panels, at {count} vehicles"),
                );
            }
        }
    }

    /// The load does not jump as a count crosses a panel boundary, nor as it crosses aggregate
    /// capacity into the proportional branch.
    ///
    /// If it did, two segments with almost the same vehicle count would get noticeably different
    /// estimates purely because one fell either side of a boundary. Each boundary is approached
    /// from both sides, by a step small enough that the load itself cannot move by more than the
    /// tolerance even where the function is behaving.
    #[test]
    fn the_load_is_continuous_at_every_boundary() {
        let capacity = panel_capacity();
        let epsilon = 1e-12;

        for panels in PANEL_COUNTS {
            for panel in 1..=u32::from(panels) {
                let boundary = capacity * f64::from(panel);
                let at = Segment::load_over_panels(boundary, panels);
                assert_load_close(
                    Segment::load_over_panels(boundary - epsilon, panels),
                    at,
                    &format!("{panels} panels, just below {panel} full"),
                );
                assert_load_close(
                    Segment::load_over_panels(boundary + epsilon, panels),
                    at,
                    &format!("{panels} panels, just above {panel} full"),
                );
            }
        }
    }

    /// Above aggregate capacity, load rises in proportion to the count: twice the site's worth of
    /// vehicles draws twice the site's load, and so on.
    ///
    /// The assertion is a ratio against one full panel rather than a kW figure, so it pins the
    /// proportionality rule and not the load a full panel happens to draw.
    #[test]
    fn beyond_aggregate_capacity_load_is_proportional_to_the_count() {
        let capacity = panel_capacity();
        let full_panel = single_panel_load(capacity);

        for panels in PANEL_COUNTS {
            for multiple in [1.5, 2.0, 3.7, 10.0] {
                let count = aggregate_capacity(panels) * multiple;
                assert_load_close(
                    Segment::load_over_panels(count, panels),
                    full_panel.scaled(count / capacity),
                    &format!("{panels} panels, at {multiple} times capacity"),
                );
            }
        }
    }

    /// Adding a vehicle never lowers the site load — within a panel, across a panel boundary, or
    /// past aggregate capacity.
    ///
    /// The sweep runs to three times aggregate capacity, so it crosses every boundary rather than
    /// stopping at one. Only the ordering of successive loads is checked, never how far apart they
    /// are.
    #[test]
    fn load_never_falls_as_vehicles_are_added() {
        for panels in PANEL_COUNTS {
            let mut previous = Segment::load_over_panels(0.0, panels).apparent_kva();

            #[expect(clippy::cast_possible_truncation, reason = "a loop bound in tenths")]
            let last_tenth = (aggregate_capacity(panels) * 30.0) as u32;
            for tenths in 1..=last_tenth {
                let count = f64::from(tenths) / 10.0;
                let current = Segment::load_over_panels(count, panels).apparent_kva();
                assert!(
                    current >= previous,
                    "{panels} panels: load fell from {previous} to {current} at {count} vehicles"
                );
                previous = current;
            }
        }
    }

    /// Above aggregate capacity, `Segment::scaled_load` reports less than `single_panel_load`
    /// would for the same count.
    ///
    /// `single_panel_load` on its own models one transformer driven past its nameplate. Its
    /// copper-loss and reactance terms rise with the square of loading, so past capacity it grows
    /// faster than in proportion to the count, while further panels on further transformers grow
    /// in proportion.
    #[test]
    fn the_extension_charges_less_than_overloading_one_transformer() {
        for panels in PANEL_COUNTS {
            for extra in [0.5, 1.0, 5.0, 20.0] {
                let count = aggregate_capacity(panels) + extra;
                let extended = Segment::load_over_panels(count, panels).apparent_kva();
                let overloaded = single_panel_load(count).apparent_kva();
                assert!(
                    extended < overloaded,
                    "{panels} panels: at {count} vehicles the extension gave {extended}, \
                     not below the {overloaded} one overloaded transformer would draw"
                );
            }
        }
    }
}
