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
    let first_csv = args.iter().position(|a| is_csv(a));
    let (ending, rates1, rates2, session_csvs) = match (args.as_slice(), first_csv) {
        ([ending, rates1, csvs @ ..], Some(2)) => (ending, rates1, None, csvs),
        ([ending, rates1, rates2, csvs @ ..], Some(3)) => (ending, rates1, Some(rates2), csvs),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    if session_csvs.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let session_csvs: Vec<&Path> = session_csvs.iter().map(Path::new).collect();

    match run(ending, rates1, rates2.map(String::as_str), &session_csvs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
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
