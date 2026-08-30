//! Domain types shared across the `green_button` module.
//!
//! The Excel serial-date arithmetic that used to live here is in [`crate::time::excel`], which both
//! sheet writers now share.
//!
//! The readings carry raw source integers rather than kilowatt figures. Green Button reports each
//! value as an integer with a `powerOfTenMultiplier`, and every sum and maximum here runs on those
//! integers; the division happens once, at cell-write time. That is not premature caution — the
//! spreadsheet is reconciled against a utility invoice to three decimal places, and accumulating
//! 744 floating-point divisions before summing them loses that agreement.

use crate::{
    log::RunLog,
    time::{is_on_grid, time_zone},
};
use jiff::Timestamp;
use std::{collections::BTreeMap, fmt, time::Duration};

/// One hour, the interval every reading in a Toronto Hydro export covers.
///
/// The whole design downstream depends on it: the interval count that drives the red completeness
/// highlight, and the guarantee that an aligned interval cannot straddle a TOU boundary, since
/// Ontario's price-period boundaries all fall on the hour. So it is **checked rather than
/// assumed** — the feed states it per `ReadingType` and again per `IntervalReading`, and
/// `green_button::parse_espi_xml` rejects any other value.
pub const METER_INTERVAL: Duration = Duration::from_secs(3600);

/// One hour of metered data, keyed on the instant the hour starts.
///
/// The three values are independent `Option`s rather than a single "reading is present" flag
/// because the feed can and does carry a timestamp in one series and not another. The Python this
/// replaces substituted zero for a missing companion, which cannot raise a maximum but does write
/// a false `0.000` into the "kVA at interval" columns — a silent wrong number in a cell used to
/// check a bill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub start: Timestamp,
    pub kwh: Option<i64>,
    pub kw: Option<i64>,
    pub kva: Option<i64>,
}

impl Reading {
    /// True when no series carried a value at this timestamp, i.e. the hour is a hole that the
    /// surrounding data implies should exist.
    pub fn is_empty(&self) -> bool {
        self.kwh.is_none() && self.kw.is_none() && self.kva.is_none()
    }

    /// Whether the interval starts on a whole hour.
    ///
    /// Only aligned intervals are eligible to be a reported peak, which is what lets the TOU
    /// column always hold one value: Ontario's price-period boundaries are all on the hour, so an
    /// aligned hour cannot straddle two. Ontario's UTC offsets are whole hours in both seasons, so
    /// a whole hour in UTC is a whole hour locally.
    pub fn is_aligned(&self) -> bool {
        is_on_grid(self.start, METER_INTERVAL)
    }
}

/// A row or period that needs review. Never fatal: the workbook is still written and the figures
/// are still produced.
///
/// The `as_str` tokens are a **stable wire format**. These sheets are meant to be read back by
/// column name, so renaming a variant silently invalidates every workbook already written. Add
/// variants freely; never rename one.
///
/// There is deliberately no DST variant. The feed timestamps every reading as an absolute UTC
/// epoch on a fixed grid, so neither the spring-forward gap nor the fall-back fold can produce an
/// ambiguous or missing record — they are a rendering concern in the local-time column and nothing
/// more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Anomaly {
    /// The hour carried a kW or kVA value but no kWh.
    MissingKwh,
    /// The hour carried a kWh or kVA value but no kW.
    MissingKw,
    /// The hour carried a kWh or kW value but no kVA.
    MissingKva,
    /// No series carried this hour, but the hours around it imply it should exist.
    MissingInterval,
    /// The same interval start appeared more than once within one series.
    DuplicateInterval,
    /// The interval does not start on a whole hour. Excluded from peak selection, so it can never
    /// become a reported maximum.
    MisalignedInterval,
    /// The hole before this interval is too large to be an outage, so it was **not** filled with
    /// placeholder rows. Recorded on the reading that follows the hole.
    ///
    /// A gap is normally made visible by writing one placeholder row per missing hour. That is
    /// only sane while the hole is a plausible one: a single corrupt `<espi:start>` can put a
    /// reading thousands of years out, and filling to it would mean millions of rows — an
    /// out-of-memory kill or an apparent hang, neither of which tells anyone what is wrong.
    /// Appended last, since the variant order is the `Ord` order the goldens are written in.
    ImplausibleGap,
}

impl Anomaly {
    /// What the token means, in one clause, for a report's glossary.
    ///
    /// Separate from [`fmt::Display`], which writes the bare token: the token is what a workbook
    /// cell holds and what [`Self::from_token`] reads back, and a cell full of prose would not
    /// survive the round trip. This is for a reader who has met the token and wants to know what
    /// it says.
    pub fn description(&self) -> &'static str {
        match self {
            Self::MissingKwh => "the hour carried a kW or kVA reading but no kWh",
            Self::MissingKw => "the hour carried a kWh or kVA reading but no kW",
            Self::MissingKva => "the hour carried a kWh or kW reading but no kVA",
            Self::MissingInterval => {
                "no series carried this hour, though the hours around it imply it should exist"
            }
            Self::DuplicateInterval => {
                "the same interval start appeared more than once within one series"
            }
            Self::MisalignedInterval => {
                "the interval does not start on a whole hour, so it was left out of peak \
                 selection and can never be a reported maximum"
            }
            Self::ImplausibleGap => {
                "the hole before this hour was too large to be an outage, so it was left unfilled \
                 rather than expanded into placeholder rows"
            }
        }
    }

    /// The stable token written to the `anomalies` column. See the type-level note.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingKwh => "MissingKwh",
            Self::MissingKw => "MissingKw",
            Self::MissingKva => "MissingKva",
            Self::MissingInterval => "MissingInterval",
            Self::DuplicateInterval => "DuplicateInterval",
            Self::MisalignedInterval => "MisalignedInterval",
            Self::ImplausibleGap => "ImplausibleGap",
        }
    }

    /// Inverse of [`Anomaly::as_str`], for reading a workbook back.
    pub fn from_token(s: &str) -> Option<Self> {
        Some(match s {
            "MissingKwh" => Self::MissingKwh,
            "MissingKw" => Self::MissingKw,
            "MissingKva" => Self::MissingKva,
            "MissingInterval" => Self::MissingInterval,
            "DuplicateInterval" => Self::DuplicateInterval,
            "MisalignedInterval" => Self::MisalignedInterval,
            "ImplausibleGap" => Self::ImplausibleGap,
            _ => return None,
        })
    }
}

impl fmt::Display for Anomaly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How many offending hours a log line names before it stops and gives a count instead.
///
/// The same three [`csv::note_off_grid_rows`](crate::session) settled on, and for the same reason:
/// an export that lost a month names 744 hours under `MissingInterval`, and a log that long is a
/// log nobody reads. Three is enough to go and look at one.
const EXAMPLE_HOURS: usize = 3;

/// Summarises anomalies into a run log: one line per kind, however many hours carried it.
///
/// Shared by the two things that write a Green Button log — the workbook conversion, which sees
/// the whole feed, and the billing-period read, which sees one period. Both group by kind rather
/// than listing hours, because the failure that matters is nearly always systematic: a meter that
/// stopped reporting kVA does not produce one anomalous hour, it produces every hour.
///
/// Each line ends in the kind's own [`Anomaly::description`], so a log read on its own says what
/// the token means without the reader going to find a glossary.
///
/// Hours are written in local time, as the workbook's own columns and the report's anomaly table
/// are. The feed timestamps in UTC, but nobody reconciling a bill thinks in it.
pub fn note_anomalies(anomalies: impl IntoIterator<Item = (Timestamp, Anomaly)>, log: &mut RunLog) {
    let mut by_kind: BTreeMap<Anomaly, Vec<Timestamp>> = BTreeMap::new();
    for (at, kind) in anomalies {
        by_kind.entry(kind).or_default().push(at);
    }

    for (kind, hours) in by_kind {
        let examples: Vec<String> = hours
            .iter()
            .take(EXAMPLE_HOURS)
            .map(|at| {
                at.to_zoned(time_zone())
                    .strftime("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .collect();
        let more = hours.len().saturating_sub(examples.len());
        let and_more = if more == 0 {
            String::new()
        } else {
            format!(", and {more} more")
        };
        log.note(format!(
            "{} hour(s) carry {kind}: {}{and_more}. {}.",
            hours.len(),
            examples.join(", "),
            kind.description()
        ));
    }
}

// cargo test --lib -- green_button::common::test --nocapture
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_anomaly_token_round_trips() {
        for a in [
            Anomaly::MissingKwh,
            Anomaly::MissingKw,
            Anomaly::MissingKva,
            Anomaly::MissingInterval,
            Anomaly::DuplicateInterval,
            Anomaly::MisalignedInterval,
        ] {
            assert_eq!(Anomaly::from_token(a.as_str()), Some(a));
        }
    }
}
