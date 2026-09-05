//! EV energy cost for one billing period, from a Toronto Hydro bill and the Evolute session reports
//! covering the period's two ends.

use ev_cost_recovery::api::energy_cost;
use std::{env, error::Error, path::Path, process::ExitCode};

const USAGE: &str = "\
energy_cost_cli -- the energy cost attributable to EV charging in one billing period.

Prices the EV share of each time-of-use band at the bill's own rate for that band, and adds the EV
share of the wholesale market service charge, which is levied per kilowatt-hour at one rate across
all three. Every rate and proportion comes off the bill; no tariff is assumed.

No meter export is asked for. Consumption is billed by the kilowatt-hour, so the hour the site
peaked in does not bear on this figure. For the delivery lines, which do, see peak_power_cost_cli.

No closing date is asked for either. The bill states which period it covers.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    energy_cost_cli <BILL.pdf> <SESSIONS.csv>...
    energy_cost_cli --help

Example:
    energy_cost_cli data/bills/TH_2026_06_29.pdf data/May.csv data/June.csv
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let [bill_pdf, session_csvs @ ..] = args.as_slice() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if session_csvs.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let session_csvs: Vec<&Path> = session_csvs.iter().map(Path::new).collect();

    match run(Path::new(bill_pdf), &session_csvs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(bill_pdf: &Path, session_csvs: &[&Path]) -> Result<(), Box<dyn Error>> {
    let cost = energy_cost(bill_pdf, session_csvs)?;
    // Written before the report is printed, so a failure to write one is not buried under it.
    cost.notes.write_logs()?;

    print!("{cost}");
    Ok(())
}
