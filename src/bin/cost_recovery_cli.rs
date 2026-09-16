//! Cost recovery for one billing period, at the EV cost-recovery rates given, from the Evolute
//! session reports covering the period's two ends.

use ev_cost_recovery::api::{cost_recovery, parse_rates};
use jiff::civil::Date;
use std::{env, error::Error, path::Path, process::ExitCode};

const USAGE: &str = "\
cost_recovery_cli -- EV cost recovery for one billing period.

Applies the EV cost-recovery rates given to the energy the chargers drew in each time-of-use band,
and reports what that recovers. A billing period is named by the date it closes on.

The rates are yours rather than Toronto Hydro's, so no bill is read and no tax is added: the report
is the rate times the kilowatt-hours it was charged on.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

A rate schedule is written EFFECTIVE_DATE:ON_PEAK,MID_PEAK,OFF_PEAK. Give one schedule for a period
the rates held through, and two when they changed during it -- the first being the rates in effect
on the period's first day, whose effective date may be well before the period, and the second the
rates it changed to. A change takes effect at local midnight starting its date.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    cost_recovery_cli <YYYY-MM-DD> <RATES> [RATES] <SESSIONS.csv>...
    cost_recovery_cli --help

Examples:
    cost_recovery_cli 2026-06-23 2026-05-01:0.1100,0.0900,0.0700 \\
        data/May.csv data/June.csv

    cost_recovery_cli 2026-06-23 2026-05-01:0.1100,0.0900,0.0700 \\
        2026-06-01:0.1200,0.1000,0.0800 data/May.csv data/June.csv
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    // The optional schedule sits between the closing date and the reports, and the reports are now
    // however many the caller has, so a count no longer tells the two shapes apart. The first
    // argument ending `.csv` is where the reports begin: a rate schedule is
    // `DATE:ON,MID,OFF` and never ends that way.
    let Some(Args {
        ending,
        rates_at_start,
        rates_at_end,
        reports,
    }) = shape(&args)
    else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let reports: Vec<&Path> = reports.iter().map(Path::new).collect();

    match run(ending, rates_at_start, rates_at_end, &reports) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A command line this tool can read: the closing date, one or two rate schedules, then one or more
/// session reports.
struct Args<'a> {
    ending: &'a str,
    rates_at_start: &'a str,
    /// Absent unless the caller gave a second schedule, which is what a rate change inside the
    /// period looks like.
    rates_at_end: Option<&'a str>,
    reports: &'a [String],
}

/// Reads the arguments, or `None` for a list this tool cannot read — one where the reports do not
/// begin at the argument a rate schedule's position implies. Every argument is positional, so
/// guessing at a shifted list would price one file's energy at another's rates; the usage is
/// printed instead.
fn shape(args: &[String]) -> Option<Args<'_>> {
    match args.iter().position(|a| is_csv(a))? {
        2 if args.len() > 2 => Some(Args {
            ending: &args[0],
            rates_at_start: &args[1],
            rates_at_end: None,
            reports: &args[2..],
        }),
        3 if args.len() > 3 => Some(Args {
            ending: &args[0],
            rates_at_start: &args[1],
            rates_at_end: Some(&args[2]),
            reports: &args[3..],
        }),
        _ => None,
    }
}

/// Whether an argument names a session report rather than a rate schedule.
///
/// The extension, case-insensitively. A rate schedule is `DATE:ON,MID,OFF`, so nothing but a path
/// ends in `.csv`, and the first argument that does is where the reports begin.
fn is_csv(arg: &str) -> bool {
    Path::new(arg)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"))
}

fn run(
    ending: &str,
    rates1: &str,
    rates2: Option<&str>,
    session_csvs: &[&Path],
) -> Result<(), Box<dyn Error>> {
    // The closing date and the rates are read before anything else, so a typo in either is reported
    // as such rather than as a missing file. Every argument is positional, so a shifted argument
    // list is otherwise hard to tell from a typo.
    let billing_period_ending: Date = ending.parse().map_err(|e| {
        format!("cannot read \"{ending}\" as the billing period's closing date, YYYY-MM-DD: {e}")
    })?;
    let recovery_rates_at_start = parse_rates(rates1)?;
    let recovery_rates_at_end = rates2.map(parse_rates).transpose()?;

    let recovery = cost_recovery(
        billing_period_ending,
        session_csvs,
        recovery_rates_at_start,
        recovery_rates_at_end,
    )?;

    // Written before the report is printed, so a failure to write one is not buried under it.
    recovery.notes.write_logs()?;

    print!("{recovery}");
    Ok(())
}

#[cfg(test)]
// cargo test --bin cost_recovery_cli -- test --nocapture
mod test {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    /// Where the reports begin decides what the other arguments are, and the rule is the extension
    /// rather than a count: a schedule is `DATE:ON,MID,OFF` and never ends `.csv`.
    #[test]
    fn the_arguments_are_split_where_the_reports_begin() {
        let one = args(&["2026-06-23", "2026-05-01:0.11,0.09,0.07", "June.csv"]);
        let Args {
            ending,
            rates_at_start,
            rates_at_end,
            reports,
        } = shape(&one).expect("one schedule and one report");
        assert_eq!(ending, "2026-06-23");
        assert_eq!(rates_at_start, "2026-05-01:0.11,0.09,0.07");
        assert_eq!(rates_at_end, None);
        assert_eq!(reports, ["June.csv"]);

        let two = args(&[
            "2026-06-23",
            "2026-05-01:0.11,0.09,0.07",
            "2026-06-01:0.12,0.10,0.08",
            "May.csv",
            "June.csv",
        ]);
        let shape = shape(&two).expect("two schedules and two reports");
        assert_eq!(shape.rates_at_end, Some("2026-06-01:0.12,0.10,0.08"));
        assert_eq!(shape.reports, ["May.csv", "June.csv"]);
    }

    /// The extension is read in either case, as a name from a Windows machine will spell it.
    #[test]
    fn the_extension_is_read_whatever_its_case() {
        assert!(is_csv("/data/Reports.CSV"));
        assert!(is_csv("June.csv"));
        assert!(!is_csv("2026-05-01:0.11,0.09,0.07"));
        assert!(!is_csv("/data/anything"));
    }

    /// A list this tool cannot read is refused rather than guessed at: every argument is
    /// positional, so a shifted list would price one file against another's rates.
    #[test]
    fn a_list_that_does_not_fit_the_shape_is_refused() {
        for list in [vec!["2026-06-23"],
            vec!["2026-06-23", "June.csv"],
            vec!["2026-06-23", "2026-05-01:0.11,0.09,0.07"],
            vec!["2026-06-23", "r1", "r2", "r3", "June.csv"],] {
            assert!(shape(&args(&list)).is_none(), "{list:?} should be refused");
        }
    }
}
