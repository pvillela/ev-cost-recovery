use ev_cost_recovery::api::{OnExistingWorkbook, session_csv_to_xlsx};
use std::{env, path::PathBuf, process::ExitCode};

const USAGE: &str = "\
Converts a charging session report from CSV to .xlsx.

Usage: ev_csv_to_xlsx <SESSION_REPORT.csv>...

Each workbook is written beside its input with the extension replaced. A file already standing
where the workbook would go is refused, not overwritten: move or delete it first.

Rows needing a judgement call — a session with no charge time, one drawing more power than the
breaker should allow, one whose reported start, end and duration contradict each other — are
reported on stderr and recorded in the workbook's Anomalies column; they do not stop the
conversion. Row numbers are rows of the CSV, counting the header.

A .session.convert.log is written beside the workbook. It lists the same findings, or says there
were none.";

fn main() -> ExitCode {
    let args: Vec<PathBuf> = env::args_os().skip(1).map(PathBuf::from).collect();
    // Asked for, so it is the output: stdout, exit 0. Not asked for, so it is a refusal: stderr,
    // exit 1. The other nine binaries split it the same way, and a shell redirecting stdout should
    // not capture a complaint.
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    for path in &args {
        // Through the API rather than `session::session_csv_to_xlsx`, which takes no policy and
        // writes unconditionally. This is where the refusal to overwrite an existing workbook
        // lives, and it is the same one the desktop app gets.
        match session_csv_to_xlsx(path, OnExistingWorkbook::Refuse) {
            Ok(report) => {
                println!("{}", report.output_path.display());
                // A binary is the end of the line: there is nowhere left to return a finding to.
                // Reported, not fatal. The workbook is on disk and its figures are right; exiting
                // non-zero would tell a script the conversion failed when only its log did.
                // `gb_peak_values` and the desktop app treat it the same way.
                if let Err(e) = report.log.write() {
                    eprintln!("{}: {e}", report.log.path().display());
                }
                for anomaly in &report.anomalies {
                    eprintln!("{}: {anomaly}", path.display());
                }
            }
            // `error: ` as the other nine binaries write it; no path prefix beyond that, because
            // `session_csv_to_xlsx` names the file in every error it returns.
            Err(e) => {
                eprintln!("error: {e}");
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
