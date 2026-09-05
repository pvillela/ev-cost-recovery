//! EV energy for one billing period, from the Evolute session reports covering the period's two
//! ends.

use ev_cost_recovery::api::energy;
use jiff::civil::Date;
use std::{env, error::Error, path::Path, process::ExitCode};

const USAGE: &str = "\
energy_cli -- EV energy for one billing period, split by time-of-use band.

Reports the kilowatt-hours drawn by EV charging within the period, in each of the three price
bands. A billing period is named by the date it closes on.

No meter export and no bill are asked for. Consumption is billed by the kilowatt-hour, so neither
the hour the site peaked in nor any rate on the bill bears on this figure. For what the energy
cost, see energy_cost_cli.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    energy_cli <YYYY-MM-DD> <SESSIONS.csv>...
    energy_cli --help

Example:
    energy_cli 2026-06-23 data/May.csv data/June.csv
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let [ending, session_csvs @ ..] = args.as_slice() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if session_csvs.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let session_csvs: Vec<&Path> = session_csvs.iter().map(Path::new).collect();

    match run(ending, &session_csvs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(ending: &str, session_csvs: &[&Path]) -> Result<(), Box<dyn Error>> {
    // The closing date is read before anything else, so omitting it is reported as a date that
    // cannot be read rather than as a missing file. All three arguments are positional and two of
    // them are paths, so a shifted argument list is otherwise hard to tell from a typo.
    let billing_period_ending: Date = ending.parse().map_err(|e| {
        format!("cannot read \"{ending}\" as the billing period's closing date, YYYY-MM-DD: {e}")
    })?;

    let energy = energy(billing_period_ending, session_csvs)?;
    // Written before the report is printed, so a failure to write one is not buried under it.
    energy.notes.write_logs()?;

    print!("{energy}");
    Ok(())
}
