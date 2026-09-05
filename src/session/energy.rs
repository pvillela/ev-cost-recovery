use super::Session;
use crate::time::{Interval, Tou, tou_partition};
use jiff::Timestamp;
use std::{ops::Deref, time::Duration};

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
/// interval needs. A session reported to start and end at the same instant has no span to spread
/// over, so its energy is filed whole under the band that instant falls in — see the comment on
/// that branch.
pub fn tou_kwh(time_range: Interval, sessions: &[impl Deref<Target = Session>]) -> TouKwh {
    let mut on_peak_kwh = 0.0;
    let mut mid_peak_kwh = 0.0;
    let mut off_peak_kwh = 0.0;

    for s in sessions {
        // A record naming the same instant for its start and its end has no span to divide by, and
        // dividing by it would give an infinite rate that poisons every bucket it touched. The
        // energy is still energy, so it is filed whole under the band its one instant falls in.
        //
        // Reachable, and it was not before: while reported times were truncated to the minute the
        // span was padded out to a whole grid step and was never empty. Such a record is consistent
        // -- start + 0 == end -- so nothing excludes it, and its energy has to reach a band the way
        // every other session's does.
        //
        // `sub_second_span` stands a second in for the missing span, so the arithmetic below runs
        // unchanged: the rate is the energy over that second and the overlap is that second, so the
        // whole of the energy lands in whichever band it falls in. Ontario's price-period
        // boundaries are all on the hour, so one second never straddles two of them.
        let conn_span = s.conn_span();
        let session_interval = match conn_span.is_zero() {
            true => sub_second_span(s.conn_start),
            false => Interval::new(s.conn_start, conn_span),
        };
        let conn_span = session_interval.duration;
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

/// The one-second span standing in for a session reported to start and end at the same instant.
///
/// A stand-in for the arithmetic only. It is never widened into a claim about how long the session
/// ran: the whole of the energy lands in it either way, because the rate is the energy divided by
/// this second and the overlap is this second.
fn sub_second_span(at: Timestamp) -> Interval {
    Interval::new(at, Duration::from_secs(1))
}

// cargo test --lib -- session::energy::test
#[cfg(test)]
mod test {
    use super::*;
    use crate::session::test_support::session;

    /// A record naming one instant for both its ends still contributes all of its energy, under
    /// the band that instant falls in.
    ///
    /// The padding used to give such a record a minute to be spread over. Without it there is no
    /// span, but the energy is not thereby less real — dropping it would take kilowatt-hours out of
    /// a total silently, which is the one thing a figure drawn from these sessions must not do.
    #[test]
    fn a_zero_length_session_contributes_all_its_energy() {
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
        assert!((kwh.total_kwh() - 3.0).abs() < 1e-9, "{kwh:?}");
        // 02:00 local on 10 June is off-peak, and all of it lands there rather than being split.
        assert!((kwh.off_peak - 3.0).abs() < 1e-9, "{kwh:?}");
    }
}
