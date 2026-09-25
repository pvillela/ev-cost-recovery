//! Cost recovery for one billing period, at the EV cost-recovery rates in the rates workbook, from
//! the Evolute session reports covering the period's two ends.

use ev_cost_recovery::api::cost_recovery;
use jiff::civil::Date;
use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    path::Path,
    process::ExitCode,
};

const USAGE: &str = "\
cost_recovery_cli -- EV cost recovery for one billing period.

Applies the EV cost-recovery rates in the rates workbook to the energy the chargers drew in each
time-of-use band, and reports what that recovers. A billing period is named by the date it closes
on.

The rates are yours rather than Toronto Hydro's, so no bill is read and no tax is added: the report
is the rate times the kilowatt-hours it was charged on.

The rates workbook is an .xlsx file with a sheet named \"rates\" (or \"Sheet1\") whose first row
names the columns effective_date, on_peak, mid_peak and off_peak; docs/rates/README.md describes
it in full. The period is priced at the rates in effect on its first day, and changes once, at
local midnight starting the date of a row dated within it. More than one such row is refused.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    cost_recovery_cli <YYYY-MM-DD> <RATES.xlsx> <SESSIONS.csv>...
    cost_recovery_cli --help

Example:
    cost_recovery_cli 2026-06-23 data/rates/EV_Cost_Recovery_Rates.xlsx data/May.csv data/June.csv
";

fn main() -> ExitCode {
    // `args_os`, not `args`: `env::args()` panics on an argument that is not valid
    // Unicode, and a report path need not be. The date is read as text below, where failing to be
    // text is an argument error rather than a crash in `std::env`.
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let [ending, rates, reports @ ..] = &args[..] else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    if reports.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let reports: Vec<&Path> = reports.iter().map(Path::new).collect();

    // The date is the only argument that has to be text: a path may be any bytes the platform
    // allows, `YYYY-MM-DD` may not. Saying so is an argument error.
    let Some(ending) = OsStr::to_str(ending) else {
        eprintln!("error: the closing date must be valid text");
        return ExitCode::FAILURE;
    };

    match run(ending, Path::new(rates), &reports) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(ending: &str, rates_xlsx: &Path, session_csvs: &[&Path]) -> Result<(), Box<dyn Error>> {
    // The closing date is read before anything else, so a typo in it is reported as such rather
    // than as a missing file. Every argument is positional, so a shifted argument list is otherwise
    // hard to tell from a typo.
    let billing_period_ending: Date = ending.parse().map_err(|e| {
        format!("cannot read \"{ending}\" as the billing period's closing date, YYYY-MM-DD: {e}")
    })?;

    let recovery = cost_recovery(billing_period_ending, session_csvs, rates_xlsx)?;

    // Written before the report is printed, so a failure to write one is not buried under it.
    recovery.notes.write_logs()?;

    print!("{recovery}");
    Ok(())
}
