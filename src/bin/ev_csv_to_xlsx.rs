use ev_cost_recovery::session::excel::session_csv_to_xlsx;
use std::{env, path::PathBuf, process::ExitCode};

const USAGE: &str = "\
Converts a charging session report from CSV to .xlsx.

Usage: ev_csv_to_xlsx <SESSION_REPORT.csv>...

Each workbook is written beside its input with the extension replaced. Rows needing a judgement
call — an ambiguous DST fold, a wall time in the DST gap, a session with no charge time, one whose
reported duration runs past its reported end — are reported on stderr and recorded in the
workbook's Anomalies column; they do not stop the conversion. Row numbers are rows of the CSV, so a
record duplicated to resolve a DST fold is reported twice against the one row it came from, once
per copy; the -EDT/-EST suffix on the session id tells the two apart.

A .convert.log is written beside the workbook. It lists the same findings, or says there were
none.";

fn main() -> ExitCode {
    let args: Vec<PathBuf> = env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return if args.is_empty() {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }

    let mut failed = false;
    for path in &args {
        match session_csv_to_xlsx(path) {
            Ok(report) => {
                println!("{}", report.output_path.display());
                // The library returns its log rather than writing it, so this is where it lands.
                // A binary is the end of the line: there is nowhere left to return a finding to.
                if let Err(e) = report.log.write() {
                    eprintln!("{}: {e}", report.log.path().display());
                    failed = true;
                }
                for anomaly in &report.anomalies {
                    eprintln!("{}: {anomaly}", path.display());
                }
            }
            // No path prefix: `session_csv_to_xlsx` names the file in every error it returns.
            Err(e) => {
                eprintln!("{e}");
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
