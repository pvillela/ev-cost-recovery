//! What the EV cost-recovery rates recover for one billing period, less what the chargers' share of
//! the Toronto Hydro bill cost, from the bill, a Green Button export and the session reports
//! covering the period's two ends.

use ev_cost_recovery::api::{cost_recovery_surplus, parse_rates};
use std::{env, error::Error, path::Path, process::ExitCode};

const USAGE: &str = "\
cost_recovery_surplus_cli -- EV cost recovery against EV cost, for one billing period.

Reports what the EV cost-recovery rates recover, what the chargers' share of the bill cost, and the
difference. A positive surplus means the rates covered that share; a negative one means they fell
short.

The three parts are printed in full beneath the summary, so every figure in the subtraction can be
checked against the report it came from.

No closing date is asked for: the bill states which period it covers, and everything else is
fetched for that period.

A billing period straddles two calendar months, so it usually takes two session reports -- but the
portal exports any date range, and one report covering the whole period is enough on its own. Give
as many as it takes, in any order. What is refused is a gap: a set of reports whose names leave any
day of the period unaccounted for.

A rate schedule is written EFFECTIVE_DATE:ON_PEAK,MID_PEAK,OFF_PEAK. Give one schedule for a period
the rates held through, and two when they changed during it -- the first being the rates in effect
on the period's first day, whose effective date may be well before the period, and the second the
rates it changed to. A change takes effect at local midnight starting its date.

Only the delivery and energy sides of the bill are counted as EV cost. The energy side covers the
three time-of-use lines and the wholesale market service charge; the customer charge and the
standard supply administration charge are flat and are left out of both sides.

The report is written to stdout as markdown that also reads as plain text.

Usage:
    cost_recovery_surplus_cli <BILL.pdf> <GREEN_BUTTON.XML> <RATES> [RATES] \\
        <SESSIONS.csv>...
    cost_recovery_surplus_cli --help

Examples:
    cost_recovery_surplus_cli data/June.pdf data/TH_Electric_Usage.XML \\
        2026-05-01:0.1100,0.0900,0.0700 data/May.csv data/June.csv

    cost_recovery_surplus_cli data/June.pdf data/TH_Electric_Usage.XML \\
        2026-05-01:0.1100,0.0900,0.0700 2026-06-01:0.1200,0.1000,0.0800 \\
        data/May.csv data/June.csv
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    // The optional schedule sits between the meter export and the reports, and the reports are now
    // however many the caller has, so a count no longer tells the two shapes apart. The first
    // argument ending `.csv` is where the reports begin: a rate schedule is `DATE:ON,MID,OFF` and
    // never ends that way.
    let first_csv = args.iter().position(|a| is_csv(a));
    let (bill, gb_xml, rates1, rates2, session_csvs) = match (args.as_slice(), first_csv) {
        ([bill, gb, r1, csvs @ ..], Some(3)) => (bill, gb, r1, None, csvs),
        ([bill, gb, r1, r2, csvs @ ..], Some(4)) => (bill, gb, r1, Some(r2), csvs),
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

    match run(
        Path::new(bill),
        Path::new(gb_xml),
        rates1,
        rates2.map(String::as_str),
        &session_csvs,
    ) {
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
    bill_pdf: &Path,
    gb_xml: &Path,
    rates1: &str,
    rates2: Option<&str>,
    session_csvs: &[&Path],
) -> Result<(), Box<dyn Error>> {
    // The rates are read before anything is opened, so a typo in one is reported as such rather
    // than after a bill and a year of meter readings have been parsed.
    let recovery_rates_at_start = parse_rates(rates1)?;
    let recovery_rates_at_end = rates2.map(parse_rates).transpose()?;

    let surplus = cost_recovery_surplus(
        bill_pdf,
        gb_xml,
        session_csvs,
        recovery_rates_at_start,
        recovery_rates_at_end,
    )?;

    // Written before the report is printed, so a failure to write one is not buried under it.
    surplus.notes.write_logs()?;
    // The meter export's own log, beside the session ones. Without it a command-line run leaves
    // fewer artifacts than the same inputs run through the app, and records no meter-side anomaly
    // for the period priced.
    surplus.meter.write_log()?;

    print!("{surplus}");
    Ok(())
}
