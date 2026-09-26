//! `docs/ERRORS.md` names every anomaly the app can report.
//!
//! The document is the only place a user can look a message up, and prose is not compiled: an
//! anomaly added to either vocabulary would otherwise reach a workbook column, a run log and the
//! Convert tab while the document that explains it says nothing.
//!
//! The anomalies are checked two ways: that each token has an entry, and that the entry's block
//! quote is the description the software actually prints. The descriptions are placeholder-free
//! static strings, so the comparison is exact.
//!
//! The quote is worth pinning because it is what a user reads. `ZeroActiveChargeTime`'s description
//! said the estimating logic substitutes an average power, which it does not and never did; the
//! sentence sat in `AnomalyKind::Display`, in this document quoting it, and in the session report
//! rendering it, and none of the three could tell the others were wrong.
//!
//! The *error variants* are not checked here. Their `Display` output carries placeholders, so a
//! test over fragments of it would fail on innocent rewording; what keeps those entries honest is
//! the procedure in `docs/maintenance-manual.md`.
//!
//! Both tests open `docs/ERRORS.md`, and a release build must not touch `docs/`, so both are
//! `#[ignore]`d. `cargo test` skips them with no flag involved; `ci.yml` asks for them by name on
//! every push, which is where the document is actually held to the code.
//!
//! cargo test --test docs_errors -- --ignored

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
        ExcessiveAvgKw,
        DuplicateId,
    ]
    .into_iter()
    .map(|kind| {
        let expected = match kind {
            ZeroActiveChargeTime => "ZeroActiveChargeTime",
            InconsistentDuration => "InconsistentDuration",
            ExcessiveAvgKw => "ExcessiveAvgKw",
            DuplicateId => "DuplicateId",
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

/// The block quote under a token's entry, unwrapped to one line and stripped of its backticks.
///
/// Two differences are expected and are not drift. The document wraps its quote across several `>`
/// lines where the software holds it as one string, so both sides are joined on single spaces. And
/// the document marks up column names -- `` `Active_Charge_Time` `` -- which a message written to a
/// log file or a terminal cannot carry, so the backticks come off. Everything else has to match.
fn quoted_description(doc: &str, token: &str) -> Option<String> {
    let heading = format!("### `{token}`");
    let mut lines = doc.lines().skip_while(|line| line.trim() != heading);
    lines.next()?;
    let quote: Vec<&str> = lines
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| line.trim_start().starts_with('>'))
        .map(|line| line.trim_start().trim_start_matches('>').trim())
        .collect();
    let quote: Vec<String> = quote.into_iter().map(|l| l.replace('`', "")).collect();
    match quote.is_empty() {
        true => None,
        false => Some(quote.join(" ")),
    }
}

/// The same joining, applied to what the software prints.
fn unwrapped(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
#[ignore = "reads docs/ERRORS.md"]
fn every_session_anomaly_has_an_entry_in_the_errors_document() {
    let doc = errors_doc();
    for (kind, token) in session_tokens() {
        assert_eq!(
            kind.as_str(),
            token,
            "the wire format differs from its token"
        );
        assert!(
            has_entry(&doc, token),
            "docs/ERRORS.md has no `### `{token}`` entry"
        );
        let quoted = quoted_description(&doc, token)
            .unwrap_or_else(|| panic!("`{token}`'s entry has no block quote"));
        assert_eq!(
            quoted,
            unwrapped(&kind.to_string()),
            "`{token}`'s block quote is not what the software prints"
        );
    }
}

#[test]
#[ignore = "reads docs/ERRORS.md"]
fn every_meter_anomaly_has_an_entry_in_the_errors_document() {
    let doc = errors_doc();
    for (kind, token) in meter_tokens() {
        assert_eq!(
            kind.as_str(),
            token,
            "the wire format differs from its token"
        );
        assert!(
            has_entry(&doc, token),
            "docs/ERRORS.md has no `### `{token}`` entry"
        );
        let quoted = quoted_description(&doc, token)
            .unwrap_or_else(|| panic!("`{token}`'s entry has no block quote"));
        assert_eq!(
            quoted,
            unwrapped(kind.description()),
            "`{token}`'s block quote is not what the software prints"
        );
    }
}
