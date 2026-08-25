# The two file conversions in `api::io`

Written 2026-08-25, when `api::io::session_csv_to_xlsx` and `api::io::gb_xml_to_xlsx` were added.

## What they are

```rust
pub fn session_csv_to_xlsx(session_csv: &Path) -> Result<ConversionReport, ApiError>
pub fn gb_xml_to_xlsx(gb_xml: &Path) -> Result<GbConversionReport, ApiError>
```

`GbConversionReport` pairs the output path with `green_button::WriteReport`, which is the workbook
writer's own account of what went into the sheets and knows nothing about where the file went.

`gb_xml_to_xlsx` takes no `bill_end_day`. It uses `BILL_END_DAY`, for the reason the reading
functions in the same module take none: the billing period boundary is a fact about Toronto Hydro,
not a choice a caller makes.

Failures are a new `ConversionError` — `OutputWouldBeInput`, `OutputExists`, `Write` — collapsed
into a new `ApiError::Conversion`. Read failures reuse `ReadError`.

## Why there is no pure counterpart

`api::pure` exists so that arithmetic and judgement can be exercised without a filesystem in the
way. Neither conversion has any. The input is a path, the product *is* a file, and everything in
between is already reachable on its own: `session::csv::session_rows` for one,
`green_button::parse` and `period_values` for the other.

A pure counterpart would have to return an in-memory `Workbook`, and the only thing any caller does
with a workbook is write it. It would exist to be tested and for nothing else.

**The seam, if one is ever wanted.** `green_button::excel::write_workbook` currently builds the
workbook *and* writes it. Splitting those two is where a pure version would begin — to assert on
cell contents without a temp file, say. Not worth doing speculatively, but that is the place.

## Three adjustments this forced on existing code

### The module doc was wrong the moment these landed

`api::io` opened with "The half of the API that reads", and each function's doc says "Nothing here
writes." Two functions that write make that false. The module doc now says most of it reads, names
the two that do not, and says why they have no pure counterpart — rather than leaving the contradiction
for a reader to find.

### `session::excel::workbook_path` is now public

The api has to know where the workbook will go *before* the conversion runs, in order to refuse.
Deriving that name a second time in the api would be two definitions free to drift, so there is now
one, and `the_session_workbook_is_named_in_one_place` asserts the api asks for it rather than
computing it.

### Both conversions refuse to overwrite — an open decision

`gb_peak_values` already refuses, with the reasoning written out in its help text: the figures in
these workbooks get reconciled against real invoices by hand, and a silent overwrite is how that
work is lost. `session::excel::session_csv_to_xlsx` does not refuse.

The guard was applied **in the api layer only**, so nothing existing changed: `ev_csv_to_xlsx` and
`ev_peak_gui` still overwrite as they did. That leaves one operation with two behaviours depending
on which layer you call, which is a real cost. The alternatives are to push the guard down into
`session::excel` — which would change the CLI and the GUI, where re-converting an edited CSV would
start failing — or to drop it from the api. Undecided; the api-only guard is the reversible choice.

## A follow-up not taken

`gb_peak_values` has its own `output_path` with the same `.xlsx` derivation and the same two
refusals. It could call `api::io::gb_xml_to_xlsx` and lose the duplication. It was left alone
because it also prints the holiday calendar, which is CLI presentation and has no place in the api.

## What was and was not checked

Checked: `cargo test` (258 passing), `cargo clippy --all-targets`, `cargo fmt --check`. Two new
tests cover the guard's two refusals and its passing case, settled from paths alone so they write
nothing.

**Not checked: neither conversion has been run against a real file.** Both are thin wrappers over
code the CLIs already exercise, but that is an argument rather than a check, and it should not be
mistaken for one.
