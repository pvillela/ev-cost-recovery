# What the binaries, examples and integration tests actually use

Written 2026-08-26, as evidence for a decision about module visibility and re-export policy. It
records what is used, not what should change.

> **The 49 below are necessary but not sufficient. Read "What this method misses" before using this
> document to decide what may be made private.** Applying the list on 2026-08-27 produced nine
> compiler errors from items it does not list.

## Method

Every `.rs` file under `src/bin/`, `tests/` and `examples/` — 26 files that reach the library from
outside — had its `use ev_cost_recovery::…` statements expanded to individual paths, plus any
fully-qualified `ev_cost_recovery::…` used inline. Those three roots are the whole of the external
consumer set: binaries, examples and integration tests each compile against the library as a
separate crate, exactly as a downstream user would.

**49 distinct paths are named.** The rendered public surface, with `--features historic`, is 137
distinct item names.

Reproduce with `cargo doc --no-deps --features historic`, then grep the three roots for
`ev_cost_recovery::`.

## What this method misses

It counts what an external file **names**. A type can be public without any external file naming
it, because a module inside the crate re-exports it — and a `pub use` publishes a type just as
surely as a direct mention does.

That gap was found by using this document, on 2026-08-27, to write the `pub use` lists in each
`mod.rs`. Nine items that appear nowhere below turned out to be public anyway, all of them via
`api::pure` or `api::io`:

| Item | Why it is public |
|---|---|
| `hydro_bill::NotABillingPeriodEnding` | variant payload of four public error enums in `api::pure` |
| `hydro_bill::HydroBill` | parameter of `pure::energy` and `pure::peak_power_cost` |
| `hydro_bill::ZeroDenominator` | variant payload of `EnergyError` and `PeakPowerError` |
| `green_button::GbWriteReport` | return type of `io::gb_xml_to_xlsx` |
| `green_button::PeriodValues` | parameter of `pure::peak_power` |
| `green_button::MeterNotes` | field of `PowerEstimates`, which `pure::peak_power` re-exports |
| `session::RSession` | re-exported by three `pure` modules |
| `session::EstimateSet` | re-exported by `pure::peak_power` |
| `time::Tou` | re-exported by `pure::energy`; `TouKwh` is keyed on it |

The compiler catches every one, as `E0365: only public within the crate, and cannot be re-exported
outside`. So the practical procedure is: take the list below as the floor, write the `pub use`
lists, and let `cargo check` add the rest. It will not let a `pub use` of a `pub(crate)` item
through.

**To find them without the compiler**, grep for re-exports out of one module into another:

```sh
grep -rn "^pub use crate::" src/
grep -rn "pub use" src/api/
```

A second, quieter route exists and the compiler does *not* flag it: a public doc comment linking to
an item that has become private. That is a `rustdoc` warning rather than an error —
`public documentation for X links to private item Y` — and only appears under
`cargo doc`. Twelve of those surfaced in the same pass. They are not visibility bugs; they mean the
prose is pointing at something a reader cannot open.

## The 49, by module

`[bin]`, `[example]`, `[test]` say which kind of consumer names the path. **`[H]`** marks a path
whose only consumers are the `historic` targets — `ev_peak_cli`, `ev_peak_gui`,
`examples/sessions.rs`.

### `io` — 15 paths

The API. Every path here is named by an ordinary binary; none is `historic`.

| Path | Used by |
|---|---|
| `io::cost_recovery` | [bin] `cost_recovery_cli` |
| `io::cost_recovery_surplus` | [bin] `cost_recovery_surplus_cli`, `ev_cost_recovery/state` |
| `io::energy` | [bin] `energy_cli` |
| `io::energy_cost` | [bin] `energy_cost_cli` |
| `io::peak_power` | [bin] `peak_power_cli` |
| `io::peak_power_cost` | [bin] `peak_power_cost_cli` |
| `io::reconcile_evolute_reimbursement` | [bin] `ev_cost_recovery/state` |
| `io::gb_xml_to_xlsx` | [bin] `ev_cost_recovery/state` |
| `io::session_csv_to_xlsx` | [bin] `ev_cost_recovery/state` |
| `io::CostRecoveryRates` | [bin] `cost_recovery_cli`, `cost_recovery_surplus_cli`, `ev_cost_recovery/state` |
| `io::CostRecoverySurplus` | [bin] `ev_cost_recovery/state`, `ev_cost_recovery/surplus` |
| `io::GbWriteReport` | [bin] `ev_cost_recovery/convert`, `ev_cost_recovery/state` |
| `io::OnExistingWorkbook` | [bin] `ev_cost_recovery/convert`, `ev_cost_recovery/state` |
| `io::PowerEstimates` | [bin] `peak_power_cli` |
| `io::ReimbursementReconciliation` | [bin] `ev_cost_recovery/reimbursement`, `ev_cost_recovery/state` |

### `session` — 16 paths, 13 of them `historic`-only

| Path | Used by |
|---|---|
| `session::session_csv_to_xlsx` | [bin] `ev_csv_to_xlsx`, `ev_peak_gui/state` |
| `session::site_load_report` | [example] `site_load_report`, [test] `session/site_load_golden` |
| `session::file_name::report_coverage` | [bin] `ev_cost_recovery/state` |
| `session::Bracket` **[H]** | [bin] `ev_peak_gui/estimate` |
| `session::HourEntry` **[H]** | [bin] `ev_peak_gui/state` |
| `session::IntervalEstimates` **[H]** | [bin] `ev_peak_gui/estimate`, `ev_peak_gui/state` |
| `session::IoiLength` **[H]** | [bin] `ev_peak_cli`, `ev_peak_gui/estimate`, `ev_peak_gui/state` |
| `session::LEGAL_START_MINUTES` **[H]** | [bin] `ev_peak_gui/estimate` |
| `session::Segment` **[H]** | [bin] `ev_peak_gui/estimate` |
| `session::Session` **[H]** | [example] `sessions` |
| `session::SessionWriteReport` **[H]** | [bin] `ev_peak_gui/state` |
| `session::Sessions` **[H]** | [bin] `ev_peak_gui/state` |
| `session::checked_interval` **[H]** | [bin] `ev_peak_cli`, `ev_peak_gui/state` |
| `session::hours_of` **[H]** | [bin] `ev_peak_gui/state` |
| `session::xlsx_to_interval_estimates` **[H]** | [bin] `ev_peak_cli`, `ev_peak_gui/state` — already gated |
| `session::xlsx_to_sessions` **[H]** | [bin] `ev_peak_gui/state`, [example] `sessions` — already gated |

**This is the sharpest finding in the document.** Of `session`'s 16 externally-named paths, three
serve the API and the desktop app; the other thirteen exist for `ev_peak_cli` and `ev_peak_gui`.
Only two of the thirteen are behind the `historic` gate today.

### `hydro_bill` — 6 paths

| Path | Used by |
|---|---|
| `hydro_bill::BILL_END_DAY` | [bin] `gb_peak_values`, [test] `green_button/fixtures_golden`, `green_button/read_xml` |
| `hydro_bill::hydro_bill_from_pdf` | [bin] `hydro_bill_dump`, [test] `hydro_bill/all_bills` |
| `hydro_bill::BillError` | [bin] `hydro_bill_dump` — calls `is_layout()` |
| `hydro_bill::bill_start_day` | [bin] `gb_peak_values` |
| `hydro_bill::pdf_text` | [bin] `hydro_bill_dump` (`read_pages`, `write_pages`), [test] `hydro_bill/all_bills` (`read_pages`) |
| `hydro_bill::pdf_text::Line` | [test] `hydro_bill/all_bills` |

### `green_button` — 5 paths

| Path | Used by |
|---|---|
| `green_button::read_gb_feed` | [bin] `gb_peak_values`, [test] `green_button/fixtures_golden`, `green_button/full_feed` |
| `green_button::write_gb_workbook` | [bin] `gb_peak_values`, [test] `green_button/fixtures_golden` |
| `green_button::read_gb_for_billing_period` | [test] `green_button/read_xml` |
| `green_button::Feed` | [bin] `gb_peak_values` |
| `green_button::Anomaly` | [test] `green_button/full_feed` |

### `time` — 5 paths

| Path | Used by |
|---|---|
| `time::time_zone` | [bin] `ev_cost_recovery/detail`, `ev_peak_gui/estimate`, `ev_peak_gui/state` |
| `time::local_date` | [bin] `gb_peak_values`, [example] `gb_trim_fixture` |
| `time::Interval` **[H]** | [bin] `ev_peak_cli`, `ev_peak_gui/state` |
| `time::TZ_OFFSETS` **[H]** | [bin] `ev_peak_cli`, `ev_peak_gui/state` |
| `time::holidays` | [bin] `gb_peak_values` — one call, `holidays::holidays(year)` |

### `charges_report` — 1 path

| Path | Used by |
|---|---|
| `charges_report::charges_report` | [test] `charges_report/real_reports` |

### `pure` — 1 path

| Path | Used by |
|---|---|
| `pure::peak_power::PricedInterval` | [bin] `ev_cost_recovery/detail` |

## Module paths named as paths

Only four, and they are what decides which modules must stay `pub mod`:

| Module path | Why it is named | Could it be avoided? |
|---|---|---|
| `hydro_bill::pdf_text` | `pdf_text::read_pages`, `pdf_text::write_pages`, and `pdf_text::{self, Line}` | Not without re-exporting three items |
| `time::holidays` | `holidays::holidays(year)` — the module qualifies a function whose bare name would read badly | Re-exporting `holidays::holidays` at `time` would collide with the module name |
| `session::file_name` | `file_name::report_coverage`, one call site | Yes — re-export `report_coverage` from `session` |
| `pure::peak_power` | `peak_power::PricedInterval`, one call site | Yes — `io` could re-export the type |

Never named as a path from outside: `session::site_load`, `api::error`, and four of the five
`api::pure` submodules (`additional`, `coverage`, `energy`, `recovery`).

## The 88 public names nothing outside writes

**Read this number carefully.** "Never named" is not "unused". `api/mod.rs` states the rule the
crate follows: a module re-exports what a caller must be able to *name* — parameters, return types,
variant payloads — and excludes field types, "because reading or destructuring a field never
requires naming it".

So the 88 divide into at least three kinds, and only a per-item check separates them:

- **Reached by field access, never written.** `PowerEstimates.kw_estimates` is an
  `IntervalEstimates`; `DeliveryCost`, `Energy`, `EnergyCost`, `SessionNotes`, `MeterNotes`,
  `TouKwh`, `Peak`, `PeriodValues` are all read this way. Public on purpose.
- **Returned but never named.** `ApiError` is what every `io::` function returns, and a binary can
  `?` it or print it without ever writing the type. Same for most of the narrow error types.
- **Genuinely unreferenced.** The site-model constants (`XFMR_*`, `EV_*`, `PANEL_VOLTAGE_V`,
  `BREAKER_*`, `CONTINUOUS_DUTY_DERATE`, `VA_PER_KVA`) are the clearest block — they are inputs to
  `site_load_report`, which external code calls, but the constants themselves are named by nothing
  outside the crate.

`docs/deletion-candidates.md` sorts the same surface by a different question — which *part of the
crate* reaches each item — and found nothing unreached from anywhere. The two documents together
say: everything is used from somewhere inside, and roughly a third of it is named from outside.

## Two facts relevant to any policy decision

**`api::error` was unreachable from outside, and worked by accident.** `lib.rs` had
`mod api; pub use api::*;` and also `pub mod error;`. The glob would publish `api::error` at the
crate root; the explicit `pub mod error` shadowed it. So `ev_cost_recovery::error` resolved to the
module holding `ConversionError`, and `ApiError` and `ReadError` were reachable only because
`api::io` re-exports them. No consumer noticed, because all 15 `io` paths above go through `io`.

*Settled 2026-08-27.* The glob is gone; `lib.rs` re-exports `api`'s children by name and leaves
`api::error` out, so the collision cannot arise and `ev_cost_recovery::io::…` still resolves.
`ConversionError` was added to `io`'s re-exports, being `ApiError::Conversion`'s payload — the rule
in `api/mod.rs` had always asked for that, and the type was nameable only because `pub mod error`
happened to be there.

**`pure` is `pub mod` per operation by design, and one caller uses it that way.** `api/mod.rs`
argues that "the unit of import is the operation, not the type", which is why
`pure::{additional, coverage, energy, peak_power, recovery}` are each `pub mod`. One import out of
26 files takes that route. Either the design stands and usage has not caught up, or it should be
revisited; the evidence alone does not settle it. **Still open.**

## What was done with this

The module cleanup of 2026-08-26/27 (`efa0427`, `950c940`) used the list above to write a named
`pub use` tier in each `mod.rs`, with a `pub(crate) use` tier beneath it for what only the rest of
the crate reaches. `session::csv` and `session::file_name` became private modules; their two
externally-used items are re-exported from `session` instead. Rendered public items went from 137
to 106.

One finding here is not yet acted on. Of the 16 paths external code names in `session`, thirteen
have no consumer but `ev_peak_cli`, `ev_peak_gui` and `examples/sessions.rs`, and only two of those
are behind the `historic` gate. Gating the rest would take `session`'s public tier down to the
three paths that serve the API and the desktop app. `src/session/mod.rs` carries a note pointing
here; see also `docs/deletion-candidates.md`, which reached six of the same items by a different
route.
