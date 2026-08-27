//! Per-period aggregates: the totals and the four maxima each billing period reports.
//!
//! Everything here runs on the raw source integers. The division that turns them into kWh, kW and
//! kVA happens once, in the sheet writer. The June 2026 invoice agrees with these figures to the
//! digit, and it would not survive accumulating 744 floating-point divisions before summing them.

use super::{Anomaly, METER_INTERVAL, Reading, Readings};
use crate::{
    hydro_bill::BillingPeriod,
    markdown::{Left, h2, table, wrap},
    time::{Interval, Tou, is_off_peak, time_zone, tou_of},
};
use jiff::Timestamp;
use std::{collections::BTreeMap, path::PathBuf};

/// A reported maximum, and the state of the interval it was found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peak {
    /// Raw source integer, not yet divided.
    pub value: i64,
    pub at: Timestamp,
    /// The other power figure at the same interval: kVA alongside a kW peak, kW alongside a kVA
    /// peak. `None` when that series had no reading for the hour, rather than zero.
    pub companion: Option<i64>,
    pub tou: Tou,
}

/// One row of the `Peak_values` sheet.
#[derive(Debug, Clone)]
pub struct PeriodValues {
    pub period: BillingPeriod,
    /// Hours that actually carried data. Placeholder rows standing in for a gap are excluded, so
    /// a feed with a hole reports fewer intervals than the period should contain and gets flagged.
    pub interval_count: i64,
    pub kwh_total: i64,
    /// Highest kW over every interval in the period.
    pub max_kw: Option<Peak>,
    /// Highest kW within Toronto Hydro's 7-7 demand window. `None` when the period contains no
    /// such interval at all, which happens only for a period truncated to a weekend.
    pub max_kw_nop: Option<Peak>,
    pub max_kva: Option<Peak>,
    pub max_kva_nop: Option<Peak>,
    pub anomaly_counts: BTreeMap<Anomaly, usize>,

    /// The same anomalies as `anomaly_counts`, kept hour by hour rather than tallied.
    ///
    /// Ascending by hour, one entry per hour and kind. A count answers "how much of this feed is
    /// suspect"; only the hours answer "does any of it bear on the figure in front of me", which
    /// is what a reader checking a demand charge against the hour it was levied on is asking.
    /// `gb_peak_values` writes the counts into a workbook cell and wants nothing finer; the API
    /// reports the hours.
    pub anomalies: Vec<(Timestamp, Anomaly)>,

    /// The export these figures were read from, carried through from the private `Readings`.
    ///
    /// `None` when the readings came from a string rather than a file. Every route a caller can
    /// take reaches this through
    /// [`read_gb_for_billing_period`](crate::green_button::read_gb_for_billing_period), which
    /// always sets it. An anomaly counted above is a fact about a file, and this is the only thing
    /// here that says which one.
    pub source: Option<PathBuf>,
}

impl PeriodValues {
    /// Whether the period holds every hour it should. Drives the red fill on `nbr_of_intervals`.
    pub fn is_complete(&self) -> bool {
        self.interval_count == self.period.expected_intervals()
    }

    /// What a figure drawn from these meter readings should say about them.
    ///
    /// Taken rather than borrowed: [`PeriodValues`] is consumed by the functions that price a
    /// period, and this is what survives them.
    pub fn notes(&self) -> MeterNotes {
        MeterNotes {
            source: self.source.clone(),
            anomalies: self.anomalies.clone(),
        }
    }
}

/// What a figure drawn from the meter export should say about it: which file, and which hours in
/// it needed a judgement call.
///
/// The meter-side counterpart of [`SessionNotes`](crate::session::SessionNotes), and carried
/// alongside one by every figure that reads both. Kept apart from it rather than merged, because
/// the two are checked against different things: a session anomaly is looked up in a session
/// report by row, a meter anomaly in an export by hour.
///
/// Not filtered by relevance, unlike the session side's. The demand figures rest on maxima over
/// the whole period, so an hour anywhere in it that carried no kW is an hour that could have held
/// the maximum and did not offer one.
#[derive(Debug, Clone, Default)]
pub struct MeterNotes {
    /// The export the readings came from. `None` when they were parsed from a string.
    pub source: Option<PathBuf>,
    /// Ascending by hour, one entry per hour and kind.
    pub anomalies: Vec<(Timestamp, Anomaly)>,
}

impl MeterNotes {
    /// Whether there is nothing to report.
    pub fn is_clean(&self) -> bool {
        self.anomalies.is_empty()
    }

    /// Renders the meter side as markdown that also reads as plain text.
    ///
    /// Empty when there is nothing to say and no file to name — which is what a figure that gives
    /// its notes up to a hoisted section leaves behind.
    ///
    /// Hours are stated in local time, as every other hour in these reports is. The export
    /// timestamps in UTC, but a reader comparing an hour against a bill is reading a local clock.
    pub fn to_markdown(&self) -> String {
        if self.source.is_none() && self.is_clean() {
            return String::new();
        }
        let mut out: Vec<String> = Vec::new();
        out.push(h2("Meter data"));
        out.push(String::new());
        match &self.source {
            Some(path) => out.push(format!("- {}", path.display())),
            None => out.push("- (readings not read from a file)".to_owned()),
        }
        out.push(String::new());

        if self.is_clean() {
            return out.join("\n");
        }
        out.push(wrap(
            "These hours of the export needed a judgement call. Every figure on the demand side is \
             a maximum over the whole period, so an hour anywhere in it that carried no reading is \
             an hour that could have held the maximum and offered nothing.",
            "",
        ));
        out.push(String::new());
        let rows: Vec<Vec<String>> = self
            .anomalies
            .iter()
            .map(|(at, kind)| {
                vec![
                    at.to_zoned(time_zone())
                        .strftime("%Y-%m-%d %H:%M")
                        .to_string(),
                    kind.as_str().to_owned(),
                ]
            })
            .collect();
        out.push(table(&["Hour", "Anomaly"], &rows, &[Left, Left]));
        out.push(String::new());

        let mut seen: Vec<Anomaly> = Vec::new();
        for (_, kind) in &self.anomalies {
            if !seen.contains(kind) {
                seen.push(*kind);
                out.push(wrap(
                    &format!("- {} - {}.", kind.as_str(), kind.description()),
                    "  ",
                ));
            }
        }
        out.push(String::new());
        out.join("\n")
    }
}

/// Groups hourly readings into billing periods and computes each period's row, ascending by
/// period.
///
/// `bill_end_day` is the day of the month the bill closes on, which is what decides where one
/// period ends and the next begins. See [`BillingPeriod`].
///
/// `pub(crate)` rather than public. It takes a [`Readings`], which is the parsed feed's internal
/// form and is not part of the API, and nothing outside this crate asks for every period as
/// values: [`read_gb_for_billing_period`](super::read_gb_for_billing_period) gives a caller the
/// one period an invoice concerns, and [`write_gb_workbook`](super::write_gb_workbook) puts all of
/// them in a sheet. Exporting it so that a test could reach it would have been the wrong way
/// round; the tests that need it are `super::invoice_tests` and `super::pipeline_tests`.
pub(crate) fn period_values(readings: &Readings, bill_end_day: i8) -> Vec<PeriodValues> {
    let mut grouped: BTreeMap<BillingPeriod, Vec<&Reading>> = BTreeMap::new();
    for reading in &readings.rows {
        grouped
            .entry(BillingPeriod::containing(reading.start, bill_end_day))
            .or_default()
            .push(reading);
    }

    grouped
        .into_iter()
        .map(|(period, rows)| {
            let mut anomaly_counts: BTreeMap<Anomaly, usize> = BTreeMap::new();
            let mut anomalies: Vec<(Timestamp, Anomaly)> = Vec::new();
            for (at, kinds) in &readings.anomalies {
                if period.contains(*at) {
                    for kind in kinds {
                        *anomaly_counts.entry(*kind).or_default() += 1;
                        anomalies.push((*at, *kind));
                    }
                }
            }

            PeriodValues {
                interval_count: rows.iter().filter(|r| !r.is_empty()).count() as i64,
                kwh_total: rows.iter().filter_map(|r| r.kwh).sum(),
                max_kw: peak(&rows, |r| r.kw, |r| r.kva, false),
                max_kw_nop: peak(&rows, |r| r.kw, |r| r.kva, true),
                max_kva: peak(&rows, |r| r.kva, |r| r.kw, false),
                max_kva_nop: peak(&rows, |r| r.kva, |r| r.kw, true),
                anomaly_counts,
                anomalies,
                source: readings.source.clone(),
                period,
            }
        })
        .collect()
}

/// The first interval maximising `value`, optionally restricted to the demand window.
///
/// Strictly greater, so an earlier interval keeps the title against a later equal one -- the
/// convention the reference workbook was built with.
///
/// Misaligned intervals are skipped entirely. That is what makes [`Peak::tou`] a plain `Tou`
/// rather than an `Option`: only an interval that starts on the hour can be a peak, and such an
/// interval cannot straddle a price-period boundary.
fn peak(
    rows: &[&Reading],
    value: impl Fn(&Reading) -> Option<i64>,
    companion: impl Fn(&Reading) -> Option<i64>,
    demand_window_only: bool,
) -> Option<Peak> {
    let mut best: Option<Peak> = None;
    for reading in rows {
        if !reading.is_aligned() {
            continue;
        }
        let Some(v) = value(reading) else { continue };
        let interval = Interval::new(reading.start, METER_INTERVAL);
        if demand_window_only && is_off_peak(interval) {
            continue;
        }
        if best.is_some_and(|b| v <= b.value) {
            continue;
        }
        let tou = tou_of(interval).expect("an aligned hourly interval lies in one price period");
        debug_assert!(
            !demand_window_only || tou != Tou::OffPeak,
            "the demand window is exactly the complement of off-peak"
        );
        best = Some(Peak {
            value: v,
            at: reading.start,
            companion: companion(reading),
            tou,
        });
    }
    best
}

// cargo test --lib -- green_button::peaks::test --nocapture
#[cfg(test)]
mod test {
    use super::*;
    use crate::{hydro_bill::BILL_END_DAY, time::local_hour};
    use jiff::civil::date;
    use std::collections::BTreeSet;

    /// [`period_values`] on Toronto Hydro's own billing calendar, which is the only one these
    /// tests care about; they are about the peaks, not about where a period ends.
    fn period_values_at(readings: &Readings) -> Vec<PeriodValues> {
        period_values(readings, BILL_END_DAY)
    }

    /// Readings for consecutive hours starting at a local hour, with `(kwh, kw, kva)` each.
    fn readings_from(start: Timestamp, values: &[(i64, i64, i64)]) -> Readings {
        let rows = values
            .iter()
            .enumerate()
            .map(|(i, &(kwh, kw, kva))| Reading {
                start: Timestamp::from_second(
                    start.as_second() + i as i64 * METER_INTERVAL.as_secs() as i64,
                )
                .unwrap(),
                kwh: Some(kwh),
                kw: Some(kw),
                kva: Some(kva),
            })
            .collect();
        Readings {
            source: None,
            rows,
            anomalies: BTreeMap::new(),
        }
    }

    /// A tie goes to the earlier interval.
    #[test]
    fn the_first_maximising_interval_wins() {
        // Three summer-weekday hours from 12:00, all in the demand window.
        let start = local_hour(date(2026, 6, 15), 12);
        let values = period_values_at(&readings_from(
            start,
            &[(10, 50, 60), (10, 50, 60), (10, 40, 45)],
        ));
        let peak = values[0].max_kw.unwrap();
        assert_eq!(peak.value, 50);
        assert_eq!(peak.at, start, "the earlier of two equal maxima");
        assert_eq!(peak.companion, Some(60));
        assert_eq!(peak.tou, Tou::OnPeak);
    }

    /// The demand-window maximum ignores a larger overnight peak.
    #[test]
    fn the_demand_window_peak_excludes_off_peak_hours() {
        // 05:00 and 06:00 are off-peak; 07:00 is the first hour of the demand window.
        let start = local_hour(date(2026, 6, 15), 5);
        let values = period_values_at(&readings_from(
            start,
            &[(10, 99, 99), (10, 10, 10), (10, 20, 20)],
        ));
        let row = &values[0];
        assert_eq!(
            row.max_kw.unwrap().value,
            99,
            "unrestricted peak sees the overnight hour"
        );
        assert_eq!(row.max_kw.unwrap().tou, Tou::OffPeak);
        let restricted = row.max_kw_nop.unwrap();
        assert_eq!(
            restricted.value, 20,
            "the 07:00 hour, not the larger 05:00 one"
        );
        assert_eq!(restricted.at, local_hour(date(2026, 6, 15), 7));
        assert_ne!(
            restricted.tou,
            Tou::OffPeak,
            "a demand-window peak is never off-peak"
        );
    }

    /// A period made only of off-peak hours has no demand-window peak at all, and the columns are
    /// left blank rather than filled with the unrestricted figure.
    #[test]
    fn a_weekend_only_period_has_no_demand_window_peak() {
        let start = local_hour(date(2026, 6, 13), 0); // Saturday
        let values = period_values_at(&readings_from(start, &[(10, 50, 60); 24]));
        assert!(values[0].max_kw.is_some());
        assert!(values[0].max_kw_nop.is_none());
        assert!(values[0].max_kva_nop.is_none());
    }

    /// Totals and counts ignore holes, so an incomplete period reads as incomplete.
    #[test]
    fn a_placeholder_row_counts_towards_neither_the_total_nor_the_interval_count() {
        let start = local_hour(date(2026, 6, 15), 12);
        let mut readings = readings_from(start, &[(10, 50, 60), (10, 50, 60)]);
        readings.rows.push(Reading {
            start: Timestamp::from_second(start.as_second() + 7200).unwrap(),
            kwh: None,
            kw: None,
            kva: None,
        });
        let row = &period_values_at(&readings)[0];
        assert_eq!(row.interval_count, 2);
        assert_eq!(row.kwh_total, 20);
        assert!(!row.is_complete());
    }

    /// A misaligned interval cannot become a peak, however large.
    #[test]
    fn a_misaligned_interval_is_never_the_peak() {
        let start = local_hour(date(2026, 6, 15), 12);
        let mut readings = readings_from(start, &[(10, 50, 60)]);
        readings.rows.push(Reading {
            start: Timestamp::from_second(start.as_second() + 1800).unwrap(),
            kwh: Some(99),
            kw: Some(999),
            kva: Some(999),
        });
        let row = &period_values_at(&readings)[0];
        assert_eq!(row.max_kw.unwrap().value, 50);
        assert_eq!(
            row.kwh_total, 109,
            "but its energy still counts towards the total"
        );
    }

    /// A companion series with no reading for the hour stays blank rather than becoming zero.
    #[test]
    fn a_missing_companion_is_none_not_zero() {
        let start = local_hour(date(2026, 6, 15), 12);
        let mut readings = readings_from(start, &[(10, 50, 60)]);
        readings.rows[0].kva = None;
        let peak = period_values_at(&readings)[0].max_kw.unwrap();
        assert_eq!(peak.companion, None);
    }

    /// Readings either side of standard-time midnight on the 24th land in different periods.
    ///
    /// June, so the clocks are forward and that boundary reads 01:00 on a wall clock. The four
    /// readings run 22:00, 23:00, 00:00 and 01:00 EDT, and the split falls before the last of
    /// them, not before the third: the midnight hour still belongs to the period that is closing.
    #[test]
    fn readings_are_grouped_by_billing_period() {
        let start = local_hour(date(2026, 6, 23), 22);
        let values = period_values_at(&readings_from(start, &[(1, 1, 1); 4]));
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].period.ending, date(2026, 6, 23));
        assert_eq!(values[0].interval_count, 3);
        assert_eq!(values[1].period.ending, date(2026, 7, 23));
        assert_eq!(values[1].interval_count, 1);
    }

    /// Each period keeps its own anomalies hour by hour, not merely tallied, and takes none from
    /// the period next door.
    ///
    /// The count answers "how much of this feed is suspect". Only the hours answer "does any of it
    /// bear on the figure in front of me", which is what a reader checking a demand charge against
    /// the hour it was levied on is asking.
    #[test]
    fn each_period_keeps_the_hours_its_anomalies_fell_in() {
        let start = local_hour(date(2026, 6, 23), 22);
        let next_period = local_hour(date(2026, 6, 24), 1);
        let mut readings = readings_from(start, &[(1, 1, 1); 4]);
        readings.anomalies = BTreeMap::from([
            (start, BTreeSet::from([Anomaly::MissingKw])),
            (next_period, BTreeSet::from([Anomaly::MissingKva])),
        ]);

        let values = period_values_at(&readings);
        assert_eq!(values[0].anomalies, [(start, Anomaly::MissingKw)]);
        assert_eq!(values[1].anomalies, [(next_period, Anomaly::MissingKva)]);
        // The tally still says the same thing, one period at a time.
        assert_eq!(values[0].anomaly_counts[&Anomaly::MissingKw], 1);
    }

    /// The rendered section names the file even when nothing is wrong, and says nothing at all
    /// when there is no file either -- which is what a figure that gave its notes up looks like.
    #[test]
    fn the_meter_section_names_its_file_and_is_silent_when_there_is_none() {
        let quiet = MeterNotes {
            source: Some(PathBuf::from("Usage.XML")),
            anomalies: Vec::new(),
        };
        let text = quiet.to_markdown();
        assert!(text.contains("Usage.XML"), "{text}");
        assert!(!text.contains("judgement call"), "{text}");

        assert!(MeterNotes::default().to_markdown().is_empty());
    }

    /// An hour that needed a judgement call is listed, in local time, with what it means.
    #[test]
    fn the_meter_section_lists_the_hours_in_local_time() {
        let at = local_hour(date(2026, 6, 15), 12);
        let notes = MeterNotes {
            source: Some(PathBuf::from("Usage.XML")),
            anomalies: vec![(at, Anomaly::MissingKw)],
        };
        let text = notes.to_markdown();
        // 12:00 local, not the 16:00 the export timestamps it as.
        assert!(text.contains("2026-06-15 12:00"), "{text}");
        assert!(text.contains("MissingKw"), "{text}");
        assert!(text.contains("no kW"), "the glossary is missing:\n{text}");
    }
}
