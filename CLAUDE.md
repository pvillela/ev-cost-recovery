# Working in this repository

## After a rename, a module move, or a new cargo feature

Written 2026-08-26, after a hand refactoring left the default build broken, an error message
missing its filename, 29 broken doc links and five stale documents. Every item below is something
that actually went wrong, not a general principle.

- **Check every feature combination, all targets.**

  ```sh
  cargo check --all-targets
  cargo check --all-targets --features historic
  ```

  A green `cargo build --release` proves nothing once a feature gates whole targets: the release
  build and the test build no longer compile the same code. Three separate breaks hid behind that.

- **A broken build conceals every failure downstream of it.** Two unit tests were failing on a real
  behaviour change, and could not be seen because the default `cargo test` did not compile at all.
  Fix the build first, then believe the test results.

- **Check doc links in every feature combination too.**

  ```sh
  RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps
  RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps --features historic
  ```

  Intra-doc links are the cheapest rename detector this crate has. A link into a feature-gated
  module resolves in one configuration and not the other, so write those as plain code spans.

- **`grep` the prose too, not just the code.** `docs/`, `README.md`, `.github/` and test module
  docs all named things that no longer existed. The release workflow ran a command that had stopped
  compiling.

- **A moved doc comment is a claim about its new home.** Re-read it there. `ReadError`'s comment
  travelled from `api::io` to `error.rs` still saying "this module raises", and still resting on an
  invariant that the same commit had broken.

- **When a field stops being read, nothing tells you.** `ReadError::path` stayed populated while
  the last code that printed it went away. Extracting a function and changing its return type are
  each safe; doing both at once dropped the filename out of every Green Button error. Check what
  falls between two safe edits.

- **Adding a cargo feature is a test-coverage decision.** For every target pushed behind one, ask
  what a default `cargo test` stops checking — and whether it belonged behind the gate at all.

## Rules this repository has settled on

- **Information that reaches a message lives in a field of the error variant, formatted at
  `Display`.** Never `format!("{}: {e}", path.display())` into a `Box<dyn Error>`.
  `BillError` (`src/hydro_bill/bill_pdf.rs`) is the model; `GbReadError`
  (`src/green_button/read_xml.rs`) and `ChargesReportError` (`src/charges_report.rs`) follow it.
  `session::csv::csv_sessions` is the one place that still string-bakes a path, and is a known
  outstanding item.

- **One utility that every caller uses, or none.** A shared helper only half the callers reach is
  two definitions wearing the costume of one. `with_extension("xlsx")` is written out in seven
  places; a session-scoped `workbook_path` reached four of them and left the Green Button
  conversion deriving its own, which is why it was deleted rather than restored. Before extracting
  a utility, list every site that would have to call it.

- **A test's opening claim about which route it takes is an assertion.** Check it against the
  imports directly below it. Three integration tests said "through the public API" and went through
  the workbook round-trip; nobody noticed for months, because prose is not compiled. When the
  shortcut is shorter, say the test is a unit test — do not describe it as the API path.

- **`tests/` should not need `--features historic`.** `grep -rn historic tests/` is expected to find
  nothing. A test that needs it is either testing the legacy path deliberately, in which case it
  belongs in `src/` beside that code, or it is taking a shortcut. See
  [`docs/historic-feature.md`](docs/historic-feature.md).

## When the user is refactoring code Claude wrote

**The prior state is not the baseline.** A test failing after their change may be pinning a kludge
from before. Establish what the behaviour *should* be before reaching for `git show HEAD~1`.

Restoring `format!("{}: {e}", path.display())` was proposed here purely because two tests encoded
it — and that string-formatting was the very thing being refactored out.

## Prefer `Edit`/`Write` over `sed` and heredocs

Two `sed` and `python` rewrites in one session each silently dropped a word from prose they were
editing. They also hide the write from the action-guard hook. Use `Edit` unless it would cost
disproportionate effort.
