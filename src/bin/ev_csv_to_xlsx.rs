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
        // Through the API rather than `session::session_csv_to_xlsx`, which takes no policy and
        // writes unconditionally. This is where the refusal to overwrite an existing workbook
        // lives, and it is the same one the desktop app gets.
        match session_csv_to_xlsx(path, OnExistingWorkbook::Refuse) {
            Ok(report) => {
                println!("{}", report.output_path.display());
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
