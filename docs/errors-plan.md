# Document every error and anomaly the app reports

## Context

The app has no document that says what its messages mean. `README.md` states the policy — problems
are reported on screen and logged, serious ones block, lesser ones do not — and stops there.
`docs/app-cheat-sheet.md` quotes four messages as test recipes. Everything else lives only in the
code: 14 error enums, 57 variants that can reach the screen, 16 anomaly kinds in two unrelated
vocabularies, and one non-enum finding.

The gap this closes is a user who reads a message and has nowhere to look it up.

Two things were found while surveying, and both are settled here rather than left for later.

- **A failed log write throws the result away.** On the Cost recovery and Evolute reimbursement
  tabs, a run whose figures were worked out shows no report at all if a log file could not be
  written. The Convert tab, faced with the same failure, keeps its workbook and says so. One
  behaviour cannot be documented while the app has two.
- **A run log names an anomaly in prose and never in its token.** The workbook's `anomalies` column
  holds `FellInDstGap`; the log beside it says `row 42 (S37487): reported start or end is a local
  time that never occurred…`. The report's glossary already pairs the two (`report.rs:116`); the
  log and the Convert tab do not.

## The design

### `docs/ERRORS.md`

For the `ev_cost_recovery` app. Written for the person running it; each entry carries a source
reference at the end for whoever maintains it.

#### Three tiers

Entries are grouped by what happened to the user's work, most severe first:

- **Stops the run** — the function did not produce its result.
- **Changes the figures** — the result is there, and something in the data moved it or was left
  out of it.
- **Worth knowing** — the result stands and nothing in it moved.

The third tier opens by saying what it ranks. It ranks impact on the work, not how loud the message
is: a red block on screen can sit in it.

#### Two tags

A tag says what the reader should do. Tags appear **only** in *Stops the run*, because that is the
only tier where the answer varies:

- **Get the data** — the files given do not span what was asked for. The remedy may be re-exporting
  from Evolute, re-downloading the Green Button export from Toronto Hydro, or picking a file
  already on disk.
- **Tell the maintainer** — the app could not make sense of a file, and nothing the user types or
  re-picks changes that.

An entry in that tier with no tag carries its remedy in its own prose; the tier's opening line says
so. The two lower tiers state once, at the top, that they need no action.

#### An entry

```
### already exists

**Stops the run** — *Tell the maintainer*

> <file name> already exists. Move or delete it first, or ask for it to be
> replaced -- a conversion never overwrites its output unless told to.

**Where** Convert to workbook.

<what it means, and what to do about it>

`src/error.rs:68`
```

Placeholders are written as `<file name>`, not as the code's `{path}`. Where a template is hard to
recognise — the multi-line coverage failures — one real example is shown as well.

#### Order

Tier, then alphabetical on the first literal fragment of the message, ignoring a leading
placeholder: `"{path} already exists…"` files under *already exists*. Anomaly entries are headed by
their stable token instead, since that is the string the user meets — in the workbook's `anomalies`
column, and in the Convert tab's `MissingKwh x3` lines.

A full contents list, grouped by tier, sits at the top. Alphabetical order only pays for itself if
there is an index to scan.

#### Logs

One short section: a log is named `<stem>.<suffix>.log`, is written beside the file it concerns,
is **overwritten every run**, and says either `Nothing to report. No errors, warnings or anomalies.`
or `N item(s) to review, in the order found:` and a list. Then the table of which tab writes which
log, moved here from `docs/app-cheat-sheet.md`.

The overwrite behaviour especially: a user comparing two runs loses the first log and cannot tell
from looking.

#### Text the crate does not own

Four `ReadError` variants (`api/error.rs:51,57,63,76`), both `ConversionError` variants that carry
a cause (`error.rs:42,47`) and `GbReadError::Malformed` (`read_xml.rs:29`) hold a boxed cause. What
reaches the screen through them is wording from `csv`, `lopdf`, `roxmltree`, `jiff` or the operating
system. Those entries say so and give one real example. Cataloguing the third-party strings
themselves would make the document a hostage to `cargo update`.

#### What is left out, and why it is not said there

Two families are not catalogued, and `docs/ERRORS.md` does not mention them:

- The six field-validation messages — a blank rate, `cannot read "x" as the off-peak rate`, not a
  finite number, cannot be negative, a blank amount.
- The five file-choice refusals that state their whole remedy in their own text — `already exists.
  Move or delete it first…`, `is already an .xlsx file…`, `does not say what it covers…`, `is not a
  whole calendar month`, and the Charges Report month mismatch.

Also out: the `widgets::note` text that describes a rule rather than a finding, and the Convert
tab's overwrite confirmation.

The reason is recorded in `docs/maintenance-manual.md`, beside the completeness procedure, where
only a maintainer meets it. A reader looking up a blank-rate message should not find a paragraph
explaining why it is absent.

### `docs/green_button/README.md`

New, and deliberately narrow: the seven `green_button::Anomaly` kinds and the period-coverage rules,
in domain terms, with pointers to `Notes_on_Green_Button_data.md` and
`Toronto_Hydro_Object_Model.md`.

The session anomalies have their rationale in `docs/session/README.md`. The meter ones have none
anywhere. This closes that, at the size the gap actually is — not a module README mirroring
session's, most of whose content already sits in `Toronto_Hydro_Object_Model.md`.

### Behaviour change: a failed log write

`SurplusState` and `ReimbursementState` gain a `log_failure: Option<String>`, mirroring
`SessionWorkbook::log_failure` (`state.rs:659`). The four early returns go
(`state.rs:352-356, 359-363, 551-555, 560-566`), so the outcome is set and the report is shown.
Each tab writes two logs, so the field holds both failures when both fail.

`error` keeps meaning *the run failed*. A separate field is what stops a missing log from looking
like a failed calculation, and it is the shape the Convert tab already has.

The calculation tabs get their own message; Convert keeps *"The workbook was written, but its run
log was not"*, which tells the user something true and specific that a shared wording would throw
away. Both end by saying to check the folder's permissions and free space.

The comment at `state.rs:351` argues only that a log failure should not be buried under a report.
Showing both satisfies it — `surplus.rs:33-43` already renders `error` and `outcome` in independent
blocks.

### Behaviour change: the anomaly token

`Anomaly`'s `Display` (`common.rs:1026-1034`) becomes `row {row} ({id}) {token}: {prose}`, in the
token-then-prose order `report.rs:116` already uses. One edit reaches both surfaces that lack the
token: the run log (`csv.rs:238`) and the Convert tab's list (`convert.rs:205-217`).

`OffGridTimes` is summarised rather than listed per row, so it does not pass through `Display`; its
line at `csv.rs:276` names the token itself.

`AnomalyKind::as_str()` is the stable string — its own rustdoc says the `Display` prose "may be
reworded at will". That is why the token is what the document is keyed on and what the test asserts.

### `tests/docs_errors.rs`

Asserts that every `AnomalyKind::as_str()` and every `Anomaly::as_str()` appears in
`docs/ERRORS.md`. Both enums are already publicly exported (`session/mod.rs:59`,
`green_button/mod.rs:29`), so this needs no new export and no `historic`.

A plain array of tokens goes stale silently, which is the failure it exists to prevent. It is paired
with a `match` on each kind returning its expected token, so adding a variant fails to compile
rather than passing a shrunken test.

The error variants are not tested. Their `Display` output carries placeholders, and a test over
fragments would fail on innocent rewording. They are covered by the procedure below instead.

## What else changes

- `README.md` — the error section gains a clause for the third tier and a link; the document list
  gains `docs/ERRORS.md` and `docs/green_button/README.md`.
- `docs/maintenance-manual.md` — a **Shared** section, *Messages the user sees*, stating once that
  user-visible text is catalogued and that changing a message means changing the catalogue; an
  `ERRORS.md` step in *Adding an `AnomalyKind`*; a matching *Adding an `Anomaly`* under **Green
  Button**; the exclusion list and the completeness procedure. *Boundaries and the time grid* and
  *If the session report starts stating seconds* are untouched.
- `docs/session/README.md` — one link to `docs/ERRORS.md`.
- `docs/app-cheat-sheet.md` — the `Logs` table becomes a pointer. `Errors worth provoking` keeps its
  four recipes and gains none: provoking a log-write failure needs a read-only folder, which most
  users of this app cannot arrange.

## Two commits

1. The behaviour changes, the token, and the golden files they move. Reviewable without 800 lines
   of prose in the diff.
2. The documentation and its test.

Golden files are regenerated by the procedure in `docs/maintenance-manual.md`, not by hand.

## Checks

Before each commit:

```sh
cargo check --all-targets
cargo check --all-targets --features historic
cargo test
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps
RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps --features historic
```

And a grep over `docs/`, `README.md` and `.github/` for anything the edits renamed.
