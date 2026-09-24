//! What the EV cost-recovery rates recover for one billing period, less what the chargers' share of
//! the Toronto Hydro bill cost, from the bill, a Green Button export, the rates workbook and the
//! session reports covering the period's two ends.

use ev_cost_recovery::api::cost_recovery_surplus;
use std::{env, error::Error, ffi::OsString, path::Path, process::ExitCode};

const USAGE: &str = "\
cost_recovery_surplus_cli -- EV cost recovery against EV cost, for one billing period.

Reports what the EV cost-recovery rates recover, what the chargers' share of the bill cost, and the
difference. A positive surplus means the rates covered that share; a negative one means they fell
short.

The three parts are printed in full beneath the summary, so every figure in the subtraction can be
checked against the report it came from.

No closing date is asked for: the bill states which period it covers, and everything else is
fetched for that period.

The rates workbook is an .xlsx file with a sheet named \"rates\" (or \"Sheet1\") whose first row
names the columns effective_date, on_peak, mid_peak and off_peak; README.md describes it in full.
The period is priced at the rates in effect on its first day, and changes once, at local midnight
starting the date of a row dated within it. More than one such row is refused.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

Only the delivery and energy sides of the bill are counted as EV cost. The energy side covers the
three time-of-use lines and the wholesale market service charge; the customer charge and the
standard supply administration charge are flat and are left out of both sides.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    cost_recovery_surplus_cli <BILL.pdf> <GREEN_BUTTON.XML> <RATES.xlsx> <SESSIONS.csv>...
    cost_recovery_surplus_cli --help

Example:
    cost_recovery_surplus_cli data/June.pdf data/TH_Electric_Usage.XML \\
        data/EV_Cost_Recovery_Rates.xlsx data/May.csv data/June.csv
";

fn main() -> ExitCode {
    // `args_os`, not `args`: `env::args()` panics on an argument that is not valid
    // Unicode, and a path need not be.
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let [bill, meter, rates, reports @ ..] = &args[..] else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if reports.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let reports: Vec<&Path> = reports.iter().map(Path::new).collect();

    match run(
        Path::new(bill),
        Path::new(meter),
        Path::new(rates),
        &reports,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    bill_pdf: &Path,
    gb_xml: &Path,
    rates_xlsx: &Path,
    session_csvs: &[&Path],
) -> Result<(), Box<dyn Error>> {
    let surplus = cost_recovery_surplus(bill_pdf, gb_xml, session_csvs, rates_xlsx)?;

    // Written before the report is printed, so a failure to write one is not buried under it.
    surplus.notes.write_logs()?;
    // The meter export's own log, beside the session ones. Without it a command-line run leaves
    // fewer artifacts than the same inputs run through the app, and records no meter-side anomaly
    // for the period priced.
    surplus.meter.write_log()?;

    print!("{surplus}");
    Ok(())
}
