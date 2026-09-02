//! Slow-tier checks against the real 18 MB export.
//!
//! Ignored by default: parsing 41,688 readings on every `cargo test` is how people stop running
//! tests. Run explicitly with:
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
    // `read_gb_feed` names the file in both of its errors, so the message below adds only what it
    // cannot know: that this particular file is expected to be absent from most checkouts.
    let feed = read_gb_feed(&feed_path()).unwrap_or_else(|e| {
        panic!(
            "{e}\nThe sample export is not in the repository: put \
             data/green_button/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML in place before \
             running this."
        )
    });

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
