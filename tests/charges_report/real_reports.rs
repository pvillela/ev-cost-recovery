//! Slow-tier check against the real Charges Reports in `data/evolute`.
//!
//! Ignored by default: those files are not in the repository. Run explicitly with:
//!
//! ```text
//! cargo test --test integration -- charges_report::real_reports --ignored --nocapture
//! ```
//!
//! Parsing without error is the weaker half. The stronger half is that the file is one period,
//! that every row carries a status this code has seen before, and that the two totals are the ones
//! a person adding the columns by hand would get -- which is the whole reason to read the file
//! rather than have someone type its totals in.

use ev_cost_recovery::charges_report::charges_report;
use std::{fs, path::PathBuf};

/// Evolute's own files, both reports, beside each other -- see `crate::charges_report`.
fn evolute_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/evolute")
}

/// Charges Reports are named `<building>_charges_<start timestamp>.csv`.
fn is_charges_report(path: &std::path::Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.contains("_charges_") && name.ends_with(".csv")
}

#[test]
#[ignore = "reads the Charges Report CSVs in data/evolute"]
fn every_charges_report_parses_and_totals_its_own_columns() {
    let mut paths: Vec<PathBuf> = fs::read_dir(evolute_dir())
        .expect("the sample data is not in the repository")
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|p| is_charges_report(p))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no Charges Report found in {}",
        evolute_dir().display()
    );

    for path in &paths {
        let report = charges_report(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        println!(
            "{}\n  {} to {}, {} rows, {:.3} kWh, ${:.2}",
            path.display(),
            report.from,
            report.to,
            report.rows,
            report.total_kwh,
            report.total_amount,
        );
        for (status, count) in &report.statuses {
            println!("  Bill_Status {status:?}: {count}");
        }
        // Printed rather than asserted. Every report seen so far carries one span on every row,
        // but whether a row may state its breaker's own subscription span is an open question with
        // Evolute -- see docs/Questions_for_Evolute.md -- so a file with several is not a failure.
        // This is how a new shape gets noticed.
        for ((from, to), rows) in &report.spans {
            println!("  {from} to {to}: {} row(s)", rows.len());
        }

        assert!(report.rows > 0, "{}: no rows", path.display());
        assert!(
            report.from <= report.to,
            "{}: dates reversed",
            path.display()
        );

        // Totalled against a second, independent pass over the file. Not a tautology: this one is
        // written the way a person would add the columns up, while the reader accumulates as it
        // parses and could in principle skip or double-count a row.
        let (kwh, amount) = totals_by_hand(path);
        assert!(
            (report.total_kwh - kwh).abs() < 1e-9,
            "{}: kWh total {} does not match {kwh}",
            path.display(),
            report.total_kwh
        );
        assert!(
            (report.total_amount - amount).abs() < 1e-9,
            "{}: cost total {} does not match {amount}",
            path.display(),
            report.total_amount
        );

        // Only `Issued` has ever been seen. This is not a rule the reader enforces -- every row
        // counts whatever its status -- so failing here means a new status has appeared and
        // somebody has to decide whether it should still count.
        for status in report.statuses.keys() {
            assert_eq!(
                status,
                "Issued",
                "{}: unfamiliar Bill_Status; decide whether it counts towards the totals",
                path.display()
            );
        }
    }
}

/// The two column totals, read straight out of the text.
fn totals_by_hand(path: &std::path::Path) -> (f64, f64) {
    let text = fs::read_to_string(path).expect("a readable CSV");
    let mut lines = text.lines();
    let header = cells(lines.next().expect("a header row"));
    let kwh_at = header.iter().position(|h| h.trim() == "kWh").expect("kWh");
    let cost_at = header
        .iter()
        .position(|h| h.trim() == "Cost")
        .expect("Cost");

    let mut kwh = 0.0;
    let mut amount = 0.0;
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let cells = cells(line);
        kwh += cells[kwh_at]
            .trim()
            .replace(',', "")
            .parse::<f64>()
            .expect("a kWh figure");
        amount += cells[cost_at]
            .trim()
            .replace(['$', ','], "")
            .parse::<f64>()
            .expect("a cost figure");
    }
    (kwh, amount)
}

/// One CSV line's cells, with quoted cells kept whole and their quotes removed.
///
/// A naive split on `,` would be wrong on the very reports this check exists for: a busy month's
/// kWh reaches four figures, a four-figure cell is written `"1,234.5"`, and splitting inside those
/// quotes yields `"1` for that cell and shifts every column after it. The check would then fail on
/// its own parsing rather than on a discrepancy.
///
/// Deliberately not a CSV library. This is the independent reading that
/// `charges_report::Charges::read` is compared against, and sharing a parser with it would make
/// the two agree by construction.
fn cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => cells.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    cells.push(cell);
    cells
}
