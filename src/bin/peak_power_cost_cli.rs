//! EV delivery cost for one billing period, from a Toronto Hydro bill, a Green Button export and
//! the Evolute session reports covering the period's two ends.

use ev_cost_recovery::api::peak_power_cost;
use std::{env, error::Error, path::Path, process::ExitCode};

const USAGE: &str = "\
peak_power_cost_cli -- the delivery cost attributable to EV charging in one billing period.

Prices the EV share of each demand-priced delivery line at the bill's own rate for that line. Each
line is levied on one demand figure, and each demand figure is a maximum over one interval: the
hour the site's kVA peaked in, the hour its kW peaked in, and the hour its kW peaked in within
07:00-19:00. Every rate and proportion comes off the bill; no tariff is assumed.

Only the demand-priced lines are attributed. Consumption is billed by the kilowatt-hour and the
customer charge is fixed, so neither turns on which interval the site peaked in. For the
consumption side, see energy_cost_cli.

No closing date is asked for. The bill states which period it covers.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    peak_power_cost_cli <BILL.pdf> <GREEN_BUTTON.XML> <SESSIONS.csv>...
    peak_power_cost_cli --help

Example:
    peak_power_cost_cli data/bills/TH_2026_06_29.pdf data/TH_Electric_Usage.XML \\
        data/May.csv data/June.csv
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let [bill_pdf, gb_xml, session_csvs @ ..] = args.as_slice() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if session_csvs.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let session_csvs: Vec<&Path> = session_csvs.iter().map(Path::new).collect();

    match run(Path::new(bill_pdf), Path::new(gb_xml), &session_csvs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(bill_pdf: &Path, gb_xml: &Path, session_csvs: &[&Path]) -> Result<(), Box<dyn Error>> {
    let cost = peak_power_cost(bill_pdf, gb_xml, session_csvs)?;
    // Written before the report is printed, so a failure to write one is not buried under it.
    cost.notes.write_logs()?;
    // The meter export's own log, beside the session ones. Without it a command-line run leaves
    // fewer artifacts than the same inputs run through the app, and records no meter-side anomaly
    // for the period priced.
    cost.meter.write_log()?;

    print!("{cost}");
    Ok(())
}
