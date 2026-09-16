//! Slow-tier checks against the real 18 MB export.
//!
//! Ignored by default: parsing 41,688 readings on every `cargo test` is how people stop running
//! tests. CI runs the ignored set on every commit, and this test reports a skip where the export is
//! not on disk, so it is meaningful on a machine that has the file and harmless on one that does
//! not. By hand:
//!
//! ```text
//! cargo test --test integration -- green_button::full_feed --ignored --nocapture
//! ```
//!
//! The fast-tier fixtures each prove one rule in isolation; this is the only check that the rules
//! hold together over the whole dataset.

use ev_cost_recovery::green_button::{Anomaly, read_gb_feed};
use std::path::PathBuf;

fn feed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data/green_button/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML")
}

#[test]
#[ignore = "parses the full 18 MB export"]
fn the_real_export_parses_to_three_complete_hourly_series() {
    // The export is not in the repository; this runs in the ignored pass, which CI runs on every
    // commit. It checks the join over the real data wherever the file is on disk, and reports a
    // skip where it is not, so the ignored pass is green on a fresh checkout and meaningful on a
    // machine that has the export.
    let path = feed_path();
    if !path.exists() {
        eprintln!(
            "skipping: {} — put the export there to run this",
            path.display()
        );
        return;
    }
    // `read_gb_feed` names the file in both of its errors, so nothing is prefixed here.
    let feed = read_gb_feed(&path).unwrap_or_else(|e| panic!("{e}"));

    // 579 days x 24 hours, per docs/Toronto_Hydro_Object_Model.md.
    for (name, series) in [("kWh", &feed.kwh), ("kW", &feed.kw), ("kVA", &feed.kva)] {
        assert_eq!(series.values.len(), 13_896, "{name} reading count");
        assert!(
            series.duplicates.is_empty(),
            "{name} has duplicate interval starts"
        );
        assert_eq!(series.power_of_ten, -3, "{name} powerOfTenMultiplier");
        assert_eq!(series.divisor(), 1_000_000.0, "{name} divisor");
    }

    let readings = feed.readings();
    assert_eq!(
        readings.rows.len(),
        13_896,
        "the three series must cover the same hours"
    );
    assert!(
        readings.anomalies.is_empty(),
        "the reference export is clean; found {:?}",
        readings.anomalies.iter().take(5).collect::<Vec<_>>()
    );
    assert!(
        readings.rows.iter().all(|r| !r.is_empty()),
        "no placeholder rows expected"
    );

    // Every hour on the hour, ascending, with no gaps: the property every downstream count rests
    // on. A break here would show up as a wrong Nbr_of_intervals rather than as an error.
    for pair in readings.rows.windows(2) {
        assert_eq!(
            pair[1].start.as_second() - pair[0].start.as_second(),
            3600,
            "gap at {}",
            pair[0].start
        );
    }
    assert!(
        readings
            .rows
            .iter()
            .all(|r| r.start.as_second() % 3600 == 0)
    );
    assert!(
        !readings
            .anomalies
            .values()
            .any(|a| a.contains(&Anomaly::MisalignedInterval))
    );
}
