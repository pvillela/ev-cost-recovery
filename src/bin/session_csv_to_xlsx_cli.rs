use ev_cost_recovery::api::{OnExistingWorkbook, session_csv_to_xlsx};
use std::{env, path::PathBuf, process::ExitCode};

const USAGE: &str = "\
Converts a charging session report from CSV to .xlsx.

Usage: session_csv_to_xlsx_cli<SESSION_REPORT.csv>...

Each workbook is written beside its input with the extension replaced. A file already standing
where the workbook would go is refused, not overwritten: move or delete it first.

Rows needing a judgement call — a session with no charge time, one drawing more power than the
breaker should allow, one whose reported start, end and duration contradict each other — are
recorded in the workbook's anomalies column; they do not stop the conversion.

A report of each conversion is written to stdout as markdown that also reads as plain text: the
workbook written, and every row that needed a judgement call, or that none did. Row numbers are rows
of the CSV, counting the header.";

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
    let mut first = true;
    for path in &args {
        // Through the API rather than `session::session_csv_to_xlsx`, which takes no policy and
        // writes unconditionally. This is where the refusal to overwrite an existing workbook
        // lives, and it is the same one the desktop app gets.
        match session_csv_to_xlsx(path, OnExistingWorkbook::Refuse) {
            Ok(report) => {
                // A blank line between reports, so each heading stands clear of the one before.
                if !first {
                    println!();
                }
                first = false;
                print!("{}", report.to_markdown());
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
