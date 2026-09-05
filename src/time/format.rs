//! Rendering an instant for a person to read, with the zone it is read in.
//!
//! Every displayed time in this crate is stated on prevailing local time -- the clock a customer
//! reads -- and names its offset, `EST` or `EDT`. Naming it is not decoration here. A session
//! report states its times on a fixed standard-time offset (`time::SESSION_OFFSET`), so a session
//! shown at `17:57 EDT` is the one the portal displays as `16:57`. Without the label the two
//! readings look like a disagreement rather than the same instant on two clocks.
//!
//! A single report can carry both labels. The kW and kVA peaks of one billing period can fall on
//! opposite sides of a transition, and then the two headings differ by an hour of offset as well
//! as by their times.
//!
//! The abbreviation comes from `jiff`'s `%Z`, read off the `Zoned` being printed, so it cannot
//! disagree with the instant beside it. Deriving it from `TZ_OFFSETS` instead would be a second
//! copy of the zone's rules to keep in step.

use super::base::time_zone;
use jiff::{Timestamp, Zoned};

/// The local reading of an instant. Every function here starts from this.
fn local(ts: Timestamp) -> Zoned {
    Zoned::new(ts, time_zone())
}

/// A dated local time to the minute, with its zone: `2026-06-11 19:00 EDT`.
pub fn zoned_minute(ts: Timestamp) -> String {
    local(ts).strftime("%Y-%m-%d %H:%M %Z").to_string()
}

/// A span between two instants, with the zone stated once when both ends share it.
///
/// The far end drops its date when it falls on the near end's day, which is what keeps a table of
/// these inside a readable width: nearly every session begins and ends on one day, so repeating
/// sixteen columns of date would say only what the previous cell said.
///
/// Both ends carry a zone when they differ, which happens on the two nights a year the offset
/// changes mid-span. An interval running `01:30` to `01:30` reads as a window of no duration until
/// the offsets are shown; with them it reads as the hour it is.
pub fn zoned_span(from: Timestamp, to: Timestamp) -> String {
    let (from, to) = (local(from), local(to));
    let (from_zone, to_zone) = (
        from.strftime("%Z").to_string(),
        to.strftime("%Z").to_string(),
    );
    let to_time = match from.date() == to.date() {
        true => to.strftime("%H:%M").to_string(),
        false => to.strftime("%Y-%m-%d %H:%M").to_string(),
    };
    match from_zone == to_zone {
        true => format!(
            "{} - {to_time} {from_zone}",
            from.strftime("%Y-%m-%d %H:%M")
        ),
        false => format!(
            "{} {from_zone} - {to_time} {to_zone}",
            from.strftime("%Y-%m-%d %H:%M")
        ),
    }
}

/// The far end of a span whose near end is already printed, with its zone.
///
/// The counterpart of [`zoned_span`] for a two-column table, where the near end sits in the cell
/// before this one and the same date rule applies across the pair.
pub fn zoned_span_end(from: Timestamp, to: Timestamp) -> String {
    let (from, to) = (local(from), local(to));
    match from.date() == to.date() {
        true => to.strftime("%H:%M %Z").to_string(),
        false => to.strftime("%Y-%m-%d %H:%M %Z").to_string(),
    }
}

// cargo test --lib -- time::format::test --nocapture
#[cfg(test)]
mod test {
    use super::*;

    fn utc(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    /// Summer is EDT and winter is EST, and the label is read off the instant rather than assumed.
    #[test]
    fn an_instant_is_labelled_with_the_offset_in_force() {
        assert_eq!(
            zoned_minute(utc("2026-06-11T23:00:00Z")),
            "2026-06-11 19:00 EDT"
        );
        assert_eq!(
            zoned_minute(utc("2026-12-11T00:00:00Z")),
            "2026-12-10 19:00 EST"
        );
    }

    /// A session report states 16:57 on a standard-time clock; in August that instant reads an hour
    /// later locally. The label is what stops the two looking like a disagreement.
    #[test]
    fn a_summer_session_reads_an_hour_later_than_the_portal_states_it() {
        // 2026-08-30 16:57:00 at SESSION_OFFSET (-05:00).
        assert_eq!(
            zoned_minute(utc("2026-08-30T21:57:00Z")),
            "2026-08-30 17:57 EDT"
        );
    }

    /// One offset for both ends is stated once, at the end.
    #[test]
    fn a_span_inside_one_offset_names_it_once() {
        assert_eq!(
            zoned_span(utc("2026-06-15T20:00:00Z"), utc("2026-06-15T21:00:00Z")),
            "2026-06-15 16:00 - 17:00 EDT"
        );
    }

    /// The fold: the same clock reading an hour apart. Without the two labels this would read as a
    /// span of no duration.
    #[test]
    fn a_span_across_the_fall_transition_names_both_offsets() {
        assert_eq!(
            zoned_span(utc("2026-11-01T05:30:00Z"), utc("2026-11-01T06:30:00Z")),
            "2026-11-01 01:30 EDT - 01:30 EST"
        );
    }

    /// Crossing midnight brings the date back, since the near end's date no longer says it.
    #[test]
    fn a_span_across_midnight_dates_its_far_end() {
        assert_eq!(
            zoned_span(utc("2026-06-16T03:50:00Z"), utc("2026-06-16T04:10:00Z")),
            "2026-06-15 23:50 - 2026-06-16 00:10 EDT"
        );
        assert_eq!(
            zoned_span_end(utc("2026-06-16T03:50:00Z"), utc("2026-06-16T04:10:00Z")),
            "2026-06-16 00:10 EDT"
        );
    }
}
