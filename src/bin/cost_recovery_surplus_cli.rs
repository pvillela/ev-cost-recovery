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
    // argument ending `.csv` is where the reports begin: a rate schedule is
    // `DATE:ON,MID,OFF` and never ends that way.
    let Some(Args {
        bill,
        meter,
        rates_at_start,
        rates_at_end,
        reports,
    }) = shape(&args)
    else {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let reports: Vec<&Path> = reports.iter().map(Path::new).collect();

    match run(
        Path::new(bill),
        Path::new(meter),
        rates_at_start,
        rates_at_end,
        &reports,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A command line this tool can read: the bill, the meter export, one or two rate schedules, then
/// one or more session reports.
struct Args<'a> {
    bill: &'a str,
    meter: &'a str,
    rates_at_start: &'a str,
    /// Absent unless the caller gave a second schedule, which is what a rate change inside the
    /// period looks like.
    rates_at_end: Option<&'a str>,
    reports: &'a [String],
}

/// Reads the arguments, or `None` for a list this tool cannot read — one where the reports do not
/// begin at the argument a rate schedule's position implies. See `cost_recovery_cli::shape`, which
/// is the same rule two arguments shorter.
fn shape(args: &[String]) -> Option<Args<'_>> {
    match args.iter().position(|a| is_csv(a))? {
        3 if args.len() > 3 => Some(Args {
            bill: &args[0],
            meter: &args[1],
            rates_at_start: &args[2],
            rates_at_end: None,
            reports: &args[3..],
        }),
        4 if args.len() > 4 => Some(Args {
            bill: &args[0],
            meter: &args[1],
            rates_at_start: &args[2],
            rates_at_end: Some(&args[3]),
            reports: &args[4..],
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

#[cfg(test)]
// cargo test --bin cost_recovery_surplus_cli -- test --nocapture
mod test {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    /// Where the reports begin decides what the other arguments are, and the rule is the extension
    /// rather than a count: a schedule is `DATE:ON,MID,OFF` and never ends `.csv`.
    #[test]
    fn the_arguments_are_split_where_the_reports_begin() {
        let one = args(&[
            "bill.pdf",
            "usage.xml",
            "2026-05-01:0.11,0.09,0.07",
            "June.csv",
        ]);
        let Args {
            bill,
            meter,
            rates_at_start,
            rates_at_end,
            reports,
        } = shape(&one).expect("the bill, the meter export, one schedule and one report");
        assert_eq!(bill, "bill.pdf");
        assert_eq!(meter, "usage.xml");
        assert_eq!(rates_at_start, "2026-05-01:0.11,0.09,0.07");
        assert_eq!(rates_at_end, None);
        assert_eq!(reports, ["June.csv"]);

        let two = args(&[
            "bill.pdf",
            "usage.xml",
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
        for list in [vec!["bill.pdf"],
            vec!["bill.pdf", "usage.xml", "June.csv"],
            vec!["bill.pdf", "usage.xml", "2026-05-01:0.11,0.09,0.07"],
            vec!["bill.pdf", "usage.xml", "r1", "r2", "r3", "June.csv"],] {
            assert!(shape(&args(&list)).is_none(), "{list:?} should be refused");
        }
    }
}
