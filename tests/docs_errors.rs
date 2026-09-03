//! `docs/ERRORS.md` names every anomaly the app can report.
//!
//! The document is the only place a user can look a message up, and prose is not compiled: an
//! anomaly added to either vocabulary would otherwise reach a workbook column, a run log and the
//! Convert tab while the document that explains it says nothing.
//!
//! Only the anomaly tokens are checked. They are a stable wire format — `AnomalyKind::as_str` and
//! `Anomaly::as_str` both say so — where the error variants' `Display` output carries placeholders,
//! and a test over fragments of those would fail on innocent rewording. What keeps the error
//! entries honest is the procedure in `docs/maintenance-manual.md`.
//!
//! cargo test --test docs_errors

use ev_cost_recovery::{green_button::Anomaly, session::AnomalyKind};

/// Every session anomaly, and the token each must appear under.
///
/// A `match` rather than a bare array: adding a variant fails to compile here, where an array would
/// silently go on passing a test that had stopped covering it. The tokens are repeated rather than
/// taken from `as_str`, so that renaming one -- which the wire format forbids -- is caught too.
fn session_tokens() -> Vec<(AnomalyKind, &'static str)> {
    use AnomalyKind::*;
    [
        ZeroActiveChargeTime,
        InconsistentDuration,
        DstAmbiguousDuplicated,
        FellInDstGap,
        DstUnresolvable,
        ExcessiveAvgKw,
        DuplicateId,
        OffGridTimes,
        WorkbookDiscrepancy,
    ]
    .into_iter()
    .map(|kind| {
        let expected = match kind {
            ZeroActiveChargeTime => "ZeroActiveChargeTime",
            InconsistentDuration => "InconsistentDuration",
            DstAmbiguousDuplicated => "DstAmbiguousDuplicated",
            FellInDstGap => "FellInDstGap",
            DstUnresolvable => "DstUnresolvable",
            ExcessiveAvgKw => "ExcessiveAvgKw",
            DuplicateId => "DuplicateId",
            OffGridTimes => "OffGridTimes",
            WorkbookDiscrepancy => "WorkbookDiscrepancy",
        };
        (kind, expected)
    })
    .collect()
}

/// Every Green Button anomaly, and the token each must appear under. See [`session_tokens`].
fn meter_tokens() -> Vec<(Anomaly, &'static str)> {
    use Anomaly::*;
    [
        MissingKwh,
        MissingKw,
        MissingKva,
        MissingInterval,
        DuplicateInterval,
        MisalignedInterval,
        ImplausibleGap,
    ]
    .into_iter()
    .map(|kind| {
        let expected = match kind {
            MissingKwh => "MissingKwh",
            MissingKw => "MissingKw",
            MissingKva => "MissingKva",
            MissingInterval => "MissingInterval",
            DuplicateInterval => "DuplicateInterval",
            MisalignedInterval => "MisalignedInterval",
            ImplausibleGap => "ImplausibleGap",
        };
        (kind, expected)
    })
    .collect()
}

fn errors_doc() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/ERRORS.md");
    std::fs::read_to_string(path).expect("docs/ERRORS.md is part of the repository")
}

/// Heading, not a passing mention: an entry is what a reader searching for the token needs to
/// arrive at, and the token appearing inside somebody else's paragraph is not one.
fn has_entry(doc: &str, token: &str) -> bool {
    doc.lines()
        .any(|line| line.trim() == format!("### `{token}`"))
}

#[test]
fn every_session_anomaly_has_an_entry_in_the_errors_document() {
    let doc = errors_doc();
    for (kind, token) in session_tokens() {
        assert_eq!(kind.as_str(), token, "the wire format was renamed");
        assert!(
            has_entry(&doc, token),
            "docs/ERRORS.md has no `### `{token}`` entry"
        );
    }
}

#[test]
fn every_meter_anomaly_has_an_entry_in_the_errors_document() {
    let doc = errors_doc();
    for (kind, token) in meter_tokens() {
        assert_eq!(kind.as_str(), token, "the wire format was renamed");
        assert!(
            has_entry(&doc, token),
            "docs/ERRORS.md has no `### `{token}`` entry"
        );
    }
}
