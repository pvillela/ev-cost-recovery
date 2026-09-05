//! Peak power estimates for one billing period, from a Green Button export and the Evolute session
//! reports covering the period's two ends.

use ev_cost_recovery::api::{PowerEstimates, peak_power};
use jiff::civil::Date;
use std::{env, error::Error, path::Path, process::ExitCode};

const USAGE: &str = "\
peak_power_cli -- peak power estimates for one billing period.

Reports the intervals of interest that maximize kW and kVA in the billing period, one report for
each. A billing period is named by the date it closes on.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

The reports are written to stdout as markdown that also reads as plain text.

Usage:
    peak_power_cli <YYYY-MM-DD> <GREEN_BUTTON.XML> <SESSIONS.csv>...
    peak_power_cli --help

Example:
    peak_power_cli 2026-06-23 data/TH_Electric_Usage.XML data/May.csv data/June.csv
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let [ending, gb_xml, session_csvs @ ..] = args.as_slice() else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if session_csvs.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let session_csvs: Vec<&Path> = session_csvs.iter().map(Path::new).collect();

    match run(ending, Path::new(gb_xml), &session_csvs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(ending: &str, gb_xml: &Path, session_csvs: &[&Path]) -> Result<(), Box<dyn Error>> {
    // The closing date is read before anything else, so omitting it is reported as a date that
    // cannot be read rather than as a missing file. All four arguments are positional and three of
    // them are paths, so a shifted argument list is otherwise hard to tell from a typo.
    let billing_period_ending: Date = ending.parse().map_err(|e| {
        format!("cannot read \"{ending}\" as the billing period's closing date, YYYY-MM-DD: {e}")
    })?;

    let PowerEstimates {
        kw_estimates,
        kva_estimates,
        notes,
        meter,
    } = peak_power(billing_period_ending, gb_xml, session_csvs)?;

    // Written before the reports are printed, so a failure to write one is not buried under them.
    notes.write_logs()?;
    // The meter export's own log, beside the session ones. Without it a command-line run leaves
    // fewer artifacts than the same inputs run through the app, and records no meter-side anomaly
    // for the period priced.
    meter.write_log()?;

    // Each report carries its own heading, so the label above it says only which of the two it is.
    println!("Billing period ending {billing_period_ending} -- interval maximizing kW\n");
    print!("{kw_estimates}");
    println!("\nBilling period ending {billing_period_ending} -- interval maximizing kVA\n");
    print!("{kva_estimates}");

    // The two reports above are each about one interval; what the sessions and the meter export as
    // a whole needed a judgement call about is stated once, after both.
    println!("\n{}", notes.to_markdown());
    print!("{}", meter.to_markdown());
    Ok(())
}
