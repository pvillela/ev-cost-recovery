//! What a front-end asks the library, stated in the terms a front-end has.
//!
//! The rest of the crate is organised by the source of data it reads. A caller checking an invoice
//! has none of those in hand — it has a billing period, a meter export and the charging network's
//! monthly reports — so this module is where those become the calls the other modules understand.
//!
//! # How it is arranged
//!
//! [`pure`] computes; the private `io` reads, and its contents are re-exported here. Every `io`
//! function turns paths into values and delegates, so that the reasoning a figure rests on can be
//! exercised without a filesystem, and so that one file answers what the library touches on disk.
//!
//! Inside the crate the split is three files — `io`, `pure` and `error`. From outside it is one
//! flat namespace plus [`pure`]: a caller writes [`peak_power`], [`ApiError`] and [`ReadError`]
//! without choosing which of the three declares them, and reaches the pure counterpart of a
//! reading call as [`pure::peak_power`]. `error` is private because the union it holds belongs to
//! both halves rather than to a subject of its own, so a path segment naming it would say nothing
//! a caller needs to know.
//!
//! [`pure`] is the exception that stays a module, because its whole point is a namespace a caller
//! can reach without a filesystem behind it. Its own submodules are private for the same reason
//! `io` and `error` are: they divide the code by subject, which is a fact about maintaining it
//! rather than about calling it.
//!
//! # What each module re-exports
//!
//! The rule is that a module re-exports every type a caller must be able to *name* in order to use
//! its public items — for a function, its parameters and return type; for an enum, its variant
//! payloads, since matching past the first level forces the caller to write them. Calling a
//! function therefore never requires knowing which module it delegates to.
//!
//! Field types are excluded, because reading or destructuring a field never requires naming it.
//! Probing *into* a returned type may well lead elsewhere in the crate — the two halves of
//! [`PowerEstimates`] are [`session::IntervalEstimates`](crate::session::IntervalEstimates), and
//! reading about them means going there. Re-exporting transitively would put the whole crate in
//! every module.

mod error;
pub use error::*;

mod io;
pub use io::*;

pub mod pure;

/// One rate schedule as the cost-recovery tools take it:
/// `EFFECTIVE_DATE:ON_PEAK,MID_PEAK,OFF_PEAK`, as in `2026-05-01:0.1100,0.0900,0.0700`.
///
/// One argument rather than four, so that the effective date cannot drift away from the rates it
/// belongs to when a second schedule is added to a command line.
///
/// Here rather than in either binary because both take the same argument and have to read it the
/// same way; compiled into two binaries, it would be two definitions that happen to agree today.
/// A schedule a front-end holds as text is what this module is for.
///
/// # Errors
///
/// A message naming the part that would not read, and the form expected. Rates are checked here
/// rather than deeper in, where the band can be named: `"nan"`, `"inf"` and `"-inf"` all parse as
/// `f64` and a negative parses as itself, and a rate that is not a finite, non-negative number
/// prices a band's energy at nothing or less while still producing a report.
pub fn parse_rates(spec: &str) -> Result<CostRecoveryRates, String> {
    const SHAPE: &str = "expected EFFECTIVE_DATE:ON_PEAK,MID_PEAK,OFF_PEAK, \
                         as in 2026-05-01:0.1100,0.0900,0.0700";

    let (date, rates) = spec
        .split_once(':')
        .ok_or_else(|| format!("cannot read \"{spec}\" as a rate schedule: {SHAPE}"))?;

    let effective_date: jiff::civil::Date = date
        .parse()
        .map_err(|e| format!("cannot read \"{date}\" as an effective date, YYYY-MM-DD: {e}"))?;

    let [on_peak, mid_peak, off_peak] = rates.split(',').collect::<Vec<_>>()[..] else {
        return Err(format!("\"{rates}\" is not three rates: {SHAPE}"));
    };
    let rate = |s: &str, band: &str| -> Result<f64, String> {
        let value: f64 = s
            .parse()
            .map_err(|e| format!("cannot read \"{s}\" as the {band} rate: {e}"))?;
        if !value.is_finite() {
            return Err(format!(
                "cannot read \"{s}\" as the {band} rate: it is not a finite number"
            ));
        }
        if value < 0.0 {
            return Err(format!("the {band} rate cannot be negative: \"{s}\""));
        }
        Ok(value)
    };

    Ok(CostRecoveryRates {
        effective_date,
        on_peak: rate(on_peak, "on-peak")?,
        mid_peak: rate(mid_peak, "mid-peak")?,
        off_peak: rate(off_peak, "off-peak")?,
    })
}

// cargo test --lib -- api::test
#[cfg(test)]
mod test {
    use super::*;
    use jiff::civil::date;

    /// The form both cost-recovery tools document, read once here rather than agreeing by
    /// inspection in two binaries.
    #[test]
    fn a_schedule_reads_as_the_tools_write_it() {
        let parsed = parse_rates("2026-05-01:0.1100,0.0900,0.0700").expect("a schedule");
        assert_eq!(parsed.effective_date, date(2026, 5, 1));
        assert_eq!(
            (
                parsed.on_peak,
                parsed.mid_peak,
                parsed.off_peak
            ),
            (0.11, 0.09, 0.07)
        );

        // Zero is a rate. A schedule pricing one band at nothing is odd but not malformed.
        let free = parse_rates("2026-06-01:0,0,0").expect("a schedule");
        assert_eq!((free.on_peak, free.mid_peak, free.off_peak), (0.0, 0.0, 0.0));
    }

    /// Every way the argument can be wrong says which part was wrong, because a command line has
    /// nowhere else to explain itself.
    #[test]
    fn a_schedule_that_will_not_read_names_the_part_that_failed() {
        for (spec, expected) in [
            ("bad", "as a rate schedule"),
            ("2026-05-01:0.11,0.09", "is not three rates"),
            ("2026-05-01:0.11,0.09,0.07,0.05", "is not three rates"),
            ("May 1 2026:0.11,0.09,0.07", "as an effective date"),
            ("2026-05-01:nan,0.09,0.07", "not a finite number"),
            ("2026-05-01:0.11,inf,0.07", "not a finite number"),
            ("2026-05-01:0.11,0.09,-0.07", "cannot be negative"),
        ] {
            let err = parse_rates(spec).expect_err(spec);
            assert!(err.contains(expected), "{spec}: {err}");
        }
    }
}
