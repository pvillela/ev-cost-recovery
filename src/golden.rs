//! The golden-file protocol, for unit tests that pin a rendering.
//!
//! Layout is only judged by looking at it, so a test that cares about column widths, decimal places
//! or wrapping states its expectation as a file that can be read rather than as assertions about
//! substrings. A change shows up as a diff in that file, which is where it should be visible during
//! review.
//!
//! Integration tests reach `tests/fixtures/` through `fixtures_dir_in` in `tests/common` instead;
//! that helper is only visible from `tests/`. This is the same directory, resolved the same way, for
//! a test that has to live in `src/` because the inputs it renders are `#[cfg(test)]` fixtures the
//! public API cannot reach.
//!
//! Nothing under `data/` is used to build one. Those files are gitignored and hold real invoices and
//! real charging records; a golden is built from the crate's own invented fixtures, so it is
//! reproducible on a fresh checkout and carries nothing that was not written for it.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// The environment variable that rewrites a golden instead of checking it.
///
/// The same one the integration-test goldens use, so one command regenerates every golden the crate
/// has.
const UPDATE: &str = "UPDATE_REPORT_GOLDEN";

/// Where a golden lives, given its path under `tests/fixtures/`.
fn path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

/// A fixture *input* under `tests/fixtures/`, for a test in `src/` whose subject is only reachable
/// from inside the crate.
///
/// The same directory [`check`] reads its goldens from, resolved the same way, so an input and the
/// rendering it is expected to produce sit beside each other whichever side of the crate the test
/// lives on.
///
/// # Panics
///
/// When the fixture is not there. That is a broken checkout rather than a condition to handle.
pub(crate) fn fixture(relative: &str) -> PathBuf {
    let p = path(relative);
    assert!(p.exists(), "missing fixture {}", p.display());
    p
}

/// Checks `rendered` against the golden at `tests/fixtures/{relative}`, or rewrites it when
/// [`UPDATE`] is set.
///
/// # Panics
///
/// When they differ, with both texts in the message; and when the golden is missing, saying how to
/// create it.
pub(crate) fn check(relative: &str, rendered: &str) {
    let golden = path(relative);

    if env::var_os(UPDATE).is_some() {
        let dir = golden.parent().expect("a fixture path has a parent");
        fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        fs::write(&golden, rendered).unwrap_or_else(|e| panic!("{}: {e}", golden.display()));
        return;
    }

    let expected = fs::read_to_string(&golden).unwrap_or_else(|e| {
        panic!(
            "{}: {e}\nRun with {UPDATE}=1 to create it.",
            golden.display()
        )
    });

    assert_eq!(
        normalize_eol(&expected),
        normalize_eol(rendered),
        "{} differs from what was rendered. Read the diff, and if the change is intended \
         regenerate with {UPDATE}=1.",
        golden.display()
    );
}

/// Every line terminator written as `\n`, so that CR, LF and CRLF all compare equal.
///
/// A golden is read from a working tree and compared against a string the renderer built in
/// memory. Only the first of those passes through a checkout, and git converts line endings there
/// according to each machine's `core.autocrlf` — so on Windows the file holds CRLF while the
/// renderer emits LF, and every line of a correct golden differs. Normalising both sides settles
/// it wherever the crate is built, without asking `.gitattributes` to pin anything.
///
/// A terminator that is *absent* on one side is still a difference: this collapses the forms of a
/// line ending, it does not discard them. A golden missing its final newline still fails, which is
/// a rendering change worth seeing.
fn normalize_eol(text: &str) -> String {
    // CRLF before a bare CR: the other order turns every `\r\n` into a blank line.
    text.replace("\r\n", "\n").replace('\r', "\n")
}
