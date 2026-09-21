use std::path::{Path, PathBuf};

pub fn fixtures_dir_in(project: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(project);
    assert!(p.exists(), "missing fixtures directory {p:?}");
    p
}

/// Resolve a fixture path, failing loudly if it is not there.
pub fn fixture_in(project: &str, name: &str) -> PathBuf {
    let p = fixtures_dir_in(project).join(name);
    assert!(p.exists(), "missing fixture {p:?}");
    p
}

/// Every line terminator written as `\n`, so that CR, LF and CRLF all compare equal.
///
/// A golden is read from a working tree and compared against a string built in memory. Only the
/// first of those passes through a checkout, and git converts line endings there according to each
/// machine's `core.autocrlf` — so on Windows the file holds CRLF while the renderer emits LF, and
/// every line of a correct golden differs. Normalising both sides settles it wherever the crate is
/// built, without asking `.gitattributes` to pin anything.
///
/// A terminator that is *absent* on one side is still a difference: this collapses the forms of a
/// line ending, it does not discard them. A golden missing its final newline still fails, which is
/// a rendering change worth seeing.
///
/// `src/golden.rs` carries the same function for the goldens pinned from inside the crate. The two
/// are not one because `mod golden` is private and its items are `pub(crate)`, so nothing under
/// `tests/` can name it — the duplication is the visibility boundary, not an oversight, and
/// collapsing it would mean publishing a test helper on the crate's public surface.
pub fn normalize_eol(text: &str) -> String {
    // CRLF before a bare CR: the other order turns every `\r\n` into a blank line.
    text.replace("\r\n", "\n").replace('\r', "\n")
}
