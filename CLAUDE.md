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

- **A subject document describes its subject. Packaging facts go in the packaging document.**
  `docs/session/README.md` is about session reports, interval-of-interest rules and workbook
  columns. Twice I put `historic` explanations in it — a six-line block quote about what a default
  `cargo build` produces, and later a paragraph on why `ioi` is gated, naming the API and the
  desktop app. Both belonged in `docs/historic-feature.md`, which already said them. The one
  mention that survives is `Needs --features historic` on the command-table row, where a reader is
  about to type the command. When a change spans a feature, the pull is to mention it everywhere
  the feature touches; the test is whether a reader of *this* document needs it here.

## Rules this repository has settled on

- **Information that reaches a message lives in a field of the error variant, formatted at
  `Display`.** Never `format!("{}: {e}", path.display())` into a `Box<dyn Error>`.
  `BillError` (`src/hydro_bill/bill_pdf.rs`) is the model; `GbReadError`
  (`src/green_button/read_xml.rs`), `SessionCsvError` (`src/session/csv.rs`) and
  `ChargesReportError` (`src/charges_report.rs`) follow it, as does `PdfTextError`
  (`src/hydro_bill/pdf_text.rs`). One place still formats a path into a message: the `historic`
  workbook reader, `session::excel::historic::xlsx_to_sessions`, where a comment says it is legacy
  and exempt. Being structured is separate from being public — `SessionCsvError` is `pub(crate)`,
  because the function that returns it is.

- **A wrapper that adds the path must not wrap a cause that already carries one.** When
  `SessionCsvError` gained its `path` field, `ConversionError::Write` — which prints the path,
  because workbook writers name no file of their own — started printing it twice. The fix was to
  split the variant: `Input` defers to a cause that names its own file, `Write` adds the path to
  one that does not. Convert a reader to a typed error and the wrappers above it need re-reading.

- **One utility that every caller uses, or none.** A shared helper only half the callers reach is
  two definitions wearing the costume of one. `with_extension("xlsx")` is written out in seven
  places; a session-scoped `workbook_path` reached four of them and left the Green Button
  conversion deriving its own, which is why it was deleted rather than restored. Before extracting
  a utility, list every site that would have to call it.

- **A test's opening claim about which route it takes is an assertion.** Check it against the
  imports directly below it. Three integration tests said "through the public API" and went through
  the workbook round-trip; nobody noticed for months, because prose is not compiled. When the
  shortcut is shorter, say the test is a unit test — do not describe it as the API path.

- **An error type belongs where it is *raised*, not where it is rendered.** `crate::error` holds
  what a library module returns — `ConversionError`, raised by both workbook writers, neither of
  which depends on the API. `ReadError` is built only by the API's `io` half, so it lives in
  `api::error` beside the union it collapses into. Moving both together looked tidy and was half
  wrong.

- **Where a module sits is settled by what its contents must be nameable as, not by what reads
  tidily in `lib.rs`.** `api::error` was kept private and its siblings re-exported one level up, to
  stop `api::error` colliding with `crate::error`. That left `ReadError` — `ApiError::Read`'s
  payload — with no path at all: private module, and `io` no longer re-exporting it. A caller could
  match `ApiError::Read(_)` and not one thing further. The fix was `pub mod api` with `io` and
  `error` private inside it, so there is no second `error` to collide and no module segment a
  caller has to guess. **`cargo doc` is where this shows up** — "public documentation for `io`
  links to private item `ReadError`" is a *warning*, not an error, so `cargo check` stays green
  while the type is unreachable.

- **What a cargo feature sorts by is who calls the code, not what the code touches.** `historic`
  began as "reads a workbook back" and now also holds `session::ioi`, which opens nothing and is
  pure — it is there because its only callers are `ev_peak_cli` and `ev_peak_gui`, the same two the
  workbook reader serves. Do not argue from the kind of code: `docs/deletion-candidates.md`
  recorded "gating a module of types and predicates is a different kind of quarantine" as a reason
  to hold off, and that was the wrong axis.

- **"Reached only by gated callers" is necessary but not sufficient for gating a public item.**
  The second question is whether an *ungated* public signature or field is typed with it. Six of
  `session`'s thirteen historic-only paths could not be gated for this reason: `IntervalEstimates`
  types `PowerEstimates.kw_estimates`, and gating it would make that field unnameable while it
  stays readable — the `ReadError` hole again. The compiler will not ask this question; a public
  field of a type with no public path compiles clean.

- **A `pub use` publishes a type as surely as a `pub struct` does.** Surveying what external files
  *name* will not find it. See `docs/public-surface-usage.md`, whose list of 49 was nine short for
  exactly this reason. To decide what may go private, take such a list as the floor and let
  `cargo check` add the rest: it refuses a `pub use` of a `pub(crate)` item (`E0365`).

- **An error type is only as public as the most public function that returns it, directly or indirectly.** Of the four
  readers, only `BillError` has a real external consumer — `hydro_bill_dump` calls `is_layout()` to
  change its advice. The other three are reached by nothing outside the crate, so their visibility
  follows their function's: `csv_sessions` is `pub(crate)`, so `SessionCsvError` is too. Do not
  publish an error type on the theory that someone might downcast to it; `ReadError` carried that
  promise for months and nothing ever did.

- **A public item whose only external callers are tests is a test hatch, named like one or not.**
  Before adding an export so a test can reach something, check what the test actually needs — the
  four `green_button` tests behind `for_test` were re-implementing `read_gb_feed`'s two lines by
  hand, and the export that really held the API open was the unmarked `period_values`. The fix is
  to move the test into `src/` beside what it tests, not to widen the API so it can stay outside.

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
