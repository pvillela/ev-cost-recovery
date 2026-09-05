use std::ops::Deref;

use super::Session;
use crate::time::{Interval, Tou, tou_partition};

/// Energy split across the three Ontario time-of-use bands, in kilowatt-hours.
///
/// The three are disjoint and, over any interval this crate produces one for, exhaustive: every
/// hour of the year falls in exactly one band, so [`Self::total_kwh`] is the whole of the energy
/// and not a subset of it.
///
/// A band's figure is what was drawn *while that band was in force*, not what a session drawing
/// across a boundary is nominally assigned to — see the crate-private `tou_kwh`, which cuts
/// sessions at the
/// boundary rather than filing each one under a single band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouKwh {
    pub on_peak: f64,
    pub mid_peak: f64,
    pub off_peak: f64,
}

impl TouKwh {
    /// The energy across all three bands.
    pub fn total_kwh(&self) -> f64 {
        self.on_peak + self.mid_peak + self.off_peak
    }
}

/// The energy `sessions` drew within `time_range`, split by time-of-use band.
///
/// Each session is spread evenly over its own span and the part lying inside `time_range` is kept,
/// then divided among the bands that part covers. So a session is cut twice — once at the
/// interval's edges and once at each price-period boundary it crosses — and a session running from
/// off-peak into mid-peak contributes to both, in proportion to the time it spent in each. Nothing
/// is filed whole under the band it started in.
///
/// Evenly, because a session report states energy and duration and nothing about how the draw was
/// shaped in between. That is an assumption, and it is the only one the data supports; it is also
/// why a figure over a short interval is worth less than the same figure over a long one.
///
/// The span a session is spread over is [`Session::conn_span`], the reported start to the reported
/// end, taken at face value: the portal states both to the second.
///
/// Sessions wholly outside `time_range` contribute nothing, so a caller may hand over more than the
/// interval needs. **A session reported to start and end at the same instant contributes nothing
/// either**, because there is no time to spread its energy over — see the comment on that branch.
pub fn tou_kwh(time_range: Interval, sessions: &[impl Deref<Target = Session>]) -> TouKwh {
    let mut on_peak_kwh = 0.0;
    let mut mid_peak_kwh = 0.0;
    let mut off_peak_kwh = 0.0;

    for s in sessions {
        // A zero span would make the rate infinite or NaN and poison every bucket it touched.
        //
        // Reachable, and it was not before. While reported times were truncated to the minute the
        // span was padded out to a whole grid step, so it was never empty; the padding is gone and
        // a record naming the same instant for its start and its end now has no width at all. Such
        // a record is consistent -- start + 0 == end -- so it is not excluded, and if it reports
        // zero `Active_Charge_Time` with it, it is bucketed as a spike and surfaced there.
        //
        // Its energy is dropped rather than attributed, because there is no interval to attribute
        // it to and choosing one would invent a span the report does not state. See
        // `a_zero_length_session_contributes_no_energy` below.
        let conn_span = s.conn_span();
        if conn_span.is_zero() {
            continue;
        }
        let session_interval = Interval::new(s.conn_start, conn_span);
        let kwh_per_sec = s.energy_use / conn_span.as_secs_f64();
        let overlap = session_interval.intersection(&time_range);
        let partition = tou_partition(overlap);
        for (tou, itvl) in partition {
            let tou_kwh = kwh_per_sec * itvl.duration.as_secs_f64();
            match tou {
                Tou::OnPeak => on_peak_kwh += tou_kwh,
                Tou::MidPeak => mid_peak_kwh += tou_kwh,
                Tou::OffPeak => off_peak_kwh += tou_kwh,
            }
        }
    }

    TouKwh {
        on_peak: on_peak_kwh,
        mid_peak: mid_peak_kwh,
        off_peak: off_peak_kwh,
    }
}

// cargo test --lib -- session::energy::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::session::test_support::session;

    /// A record naming one instant for both its ends has no span, so its energy reaches no band.
    ///
    /// This reverses what the padding used to do. The adjusted end was a whole grid step past the
    /// reported one, so such a record was spread over a minute and its energy counted; with the
    /// reported times taken at face value there is no minute to spread it over.
    #[test]
    fn a_zero_length_session_contributes_no_energy() {
        // 02:00 EDT on 10 June, off-peak, reported as a single instant.
        let s = session("June.csv", 2, "INSTANT", "2026-06-10T06:00:00Z", 0, 3.0);
        let day = Interval::from_start_end(
            "2026-06-10T04:00:00Z"
                .parse()
                .expect("an RFC 3339 timestamp"),
            "2026-06-11T04:00:00Z"
                .parse()
                .expect("an RFC 3339 timestamp"),
        );
        let kwh = tou_kwh(day, &[s]);
        assert_eq!(kwh.total_kwh(), 0.0, "{kwh:?}");
    }
}
