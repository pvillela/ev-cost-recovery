# The two file conversions in `api::io`

Written 2026-08-25, when `api::io::session_csv_to_xlsx` and `api::io::gb_xml_to_xlsx` were added.
Revised the same day, when the GUI tab for them settled the overwrite question left open below.

## What they are

```rust
pub fn session_csv_to_xlsx(session_csv: &Path, on_existing: OnExistingWorkbook)
    -> Result<ConversionReport, ApiError>
pub fn gb_xml_to_xlsx(gb_xml: &Path, on_existing: OnExistingWorkbook)
    -> Result<GbConversionReport, ApiError>
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

### An existing workbook is the caller's decision — `OnExistingWorkbook`

`gb_peak_values` refuses, with the reasoning written out in its help text: the figures in these
workbooks get reconciled against real invoices by hand, and a silent overwrite is how that work is
lost. `session::excel::session_csv_to_xlsx` does not refuse.

The guard first went in as api-only and unconditional, which was recorded here as undecided. The
GUI settled it. `ev_peak_gui`'s Convert tab does not refuse an existing workbook — it asks, with a
modal — and a tab built on an api that refuses outright could not offer that. Refusing and
replacing are both right, for different callers, so which one happens is now an argument:

```rust
pub enum OnExistingWorkbook { Refuse, Replace }
```

`Replace` waives only the existing-file refusal. An input that is its own output — anything already
named `.xlsx` — stays refused either way, since there would be nothing left to read.

The guard is still api-only: `ev_csv_to_xlsx` and `ev_peak_gui` call `session::excel` directly and
overwrite as they always did. One operation with two behaviours by layer remains a real cost, but
it is now a cost paid for a reason rather than an accident, and the api's behaviour is no longer
the stricter of the two by fiat — a caller can ask for either.

## A follow-up not taken

`gb_peak_values` has its own `output_path` with the same `.xlsx` derivation and the same two
refusals. It could call `api::io::gb_xml_to_xlsx` and lose the duplication. It was left alone
because it also prints the holiday calendar, which is CLI presentation and has no place in the api.

## The GUI tab

`ev_cost_recovery` has a fourth tab, `Convert to workbook`, holding both conversions one above the
other. It is last, and produces nothing the other three read — they parse the source files
themselves — so it is a place someone goes on purpose rather than on the way to something else.

Both halves are the same widget, `convert::picker::<C>`, generic over a `state::Conversion` trait
whose three items are all that differ between them: where the workbook goes, how to run it, and
what a finished one has to show. Everything the tab does around that — pick, ask, run, report — is
written once.

Picking a file that already has a workbook beside it opens the "Replace existing workbook?" modal,
the same question `ev_peak_gui` asks. Answering it is the only thing in the codebase that passes
`OnExistingWorkbook::Replace`.

## What was and was not checked

Checked: `cargo test` (259 passing), `cargo clippy --all-targets`, `cargo fmt --check`. Three tests
cover the guard's refusals, the case that passes, and what `Replace` does and does not waive —
settled from paths alone, so they write nothing.

Checked in the running app, on a virtual display, against the repository's own test fixtures copied
to a scratch folder: both conversions wrote a workbook, the session one wrote its run log beside it
and listed its five anomalies, the Green Button one reported 3 billing periods over 792 intervals,
and re-converting raised the modal and replaced the file on Replace.

**Not checked: neither conversion has been run against a production file** — a real Evolute report
or a real multi-year Toronto Hydro export. The fixtures are small and were built for other tests.
