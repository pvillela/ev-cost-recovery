# `ev_cost_recovery` — a desktop app for the cost-recovery surplus

## Context

`cost_recovery_surplus_cli` is the crate's central operation: what EV cost-recovery rates
recover, less the chargers' share of the delivery and energy lines on the bill. It runs only from
a terminal, with six positional arguments including a `DATE:ON,MID,OFF` rate spec. The person who
needs the answer is not a terminal user.

This builds a GUI for it. Named `ev_cost_recovery`, which is also the name
`.github/workflows/release-build.yaml` already stages — a tag push fails today because no binary
by that name is built. Only this app is distributed; `ev_peak_gui` stays buildable locally until
it is retired.

The app shows two documents. **Cost recovery** is the surplus report, byte-for-byte what the CLI
prints. **Peak power detail** is the three interval estimates the delivery cost was derived from —
the kVA peak hour, the kW peak hour, and the 7-7 kW peak hour.

Those three do not survive today. `peak_power_cost` builds all three, takes one `f64` from each,
and drops them, so `DeliveryCost` records `demand_kva`/`demand_kw`/`peak_7_7_kw` as bare scalars
with no record of which hour, which segment, or which sessions produced them. And no public result
exposes the 7-7 estimate at all — `io::peak_power` returns only kW and kVA. So the app cannot get
what it needs from a second call; the library has to keep what it already builds.

---

## Scope

**In:** one window, two tabs, one library run per click. The library change that makes the detail
tab possible. Doc hygiene around the binary rename.

**Out:** Convert, Green Button to XLSX, standalone peak power, reimbursement reconciliation. The
tab shell is built so each is an addition, not a rewrite.

---

## Commit 1 — Name and doc hygiene

Commit `docs/Prompt_Ev_cost_recovery_gui.md` first; it is untracked and git holds no copy.

**Stale binary name.** Commit `2e4a5f5` renamed the GUI directory back to `ev_peak_gui` without
updating what points at it. After this work `ev_cost_recovery` means the *new* app, so these lines
stop being merely stale and become wrong — a reader following `docs/session/README.md` would
download the surplus app expecting the peak-contribution one:

| File | Lines |
|:---|:---|
| `README.md` | 53 |
| `docs/session/README.md` | 39, 48 |
| `docs/session/Devcontainer_GUI_Options.md` | 164, 170, 218 |

`.github/workflows/release-build.yaml:54,60,66,72` needs **no change** — commit 3 makes it correct.

**Stale log contract.** `src/api/io.rs` lines 101, 156, 214, 253, 303, 350 each say:

```
/// Reading a report writes a `.csv.read.log` beside it, as [`csv::session_list`] always does.
```

Untrue since readers began returning logs instead of writing them. Replace all six with wording
matching `src/session/csv.rs:80-83`:

```
/// Nothing here writes. Each report read returns its `csv.read` log unwritten on the result's
/// `notes` -- see [`csv::session_list`] -- and
/// [`SessionNotes::write_logs`](crate::session::SessionNotes::write_logs) is what a binary calls
/// to put one beside its input.
```

---

## Commit 2 — The library keeps the three priced hours

### `src/api/pure/peak_power.rs`

New type above `DeliveryCost`. The unit label is already in hand at the point the estimates are
built (the closure at :369 takes it only to feed `PeakPowerError::NoPeak`), so carrying it costs
no new literal:

```rust
#[derive(Debug)]
pub struct PricedHour {
    pub unit: &'static str,          // "kVA", "kW", "kW 7-7" -- the charges table's Basis column
    pub estimates: IntervalEstimates,
}
```

New field on `DeliveryCost`, after `peak_7_7_kw` (:72), because field order in that struct mirrors
the report and this is the provenance of the three scalars above it:

```rust
pub priced_hours: [PricedHour; 3],
```

An array, not three named fields: a caller showing all three in report order would otherwise have
to hard-code both the order and the labels. No `Option` — all three `?` run before any charge
arithmetic, so a returned `DeliveryCost` always has all three, and a missing maximum is already
`PeakPowerError::NoPeak`.

In `peak_power_cost` (:369-386, :418-434): the `estimates` closure becomes `priced_hour`, the
three `energy_based` calls take `.estimates`, and the `notes_for_hours` call moves **out** of the
struct literal into a local. That hoist is required, not cosmetic — literal fields evaluate in
source order, so leaving it inline would borrow values already moved into `priced_hours`.

`notes_for_hours` (:454-465) keeps its signature.

Widen the comment at :22-24 — `IntervalEstimates` is now inside `PricedHour` as well as
`PowerEstimates`.

### `src/session/peak.rs`

Add `#[derive(Debug)]` to `IntervalEstimates` (:11) and `EstimateSet` (:71). Required: nine
existing `expect_err` sites need `DeliveryCost: Debug`. Every field already satisfies it;
`Rc<T>: Debug` where `T: Debug`. Do **not** add `Clone` — cloning the `Rc`s would quietly break
the `Rc::ptr_eq` identity matching in `SessionNotes::add_anomalies`
(`src/session/common.rs:1172`).

### What must not change

`Display for DeliveryCost` (:467-563) and `Display for CostRecoverySurplus`
(`src/api/pure/recovery.rs:656-705`) are **not touched**. Neither reads the new field. In
particular `Display` must not start reading `PricedHour::unit` for its `Basis` column even though
the strings are identical — provably unchanged source is worth more here than one fewer literal,
and a test pins the equality instead. No `Display` on `PricedHour`; `IntervalEstimates` already
has one, and `docs/maintenance-manual.md:315` says there is one rendering.

### The notes hoist needs no change

`cost_recovery_surplus` (`recovery.rs:489-495`) empties three `SessionNotes` and takes
`delivery.meter`. `IntervalEstimates` holds no `SessionNotes` — `session_anomalies`,
`excluded_sessions` and `logs` are its own fields — so the hoist cannot reach the retained
estimates. They come through intact, which is what the detail tab needs.

`session_anomalies` is already built per-interval by `collect_session_anomalies`
(`peak.rs:135`), so "anomalies to the extent they impact these three intervals" needs no
filtering. And the two tabs cannot disagree: `notes_for_hours` folds these same three lists into
`delivery.notes`, which `cost_recovery_surplus` then unions into `surplus.notes`.

**Run logs.** A `CostRecoverySurplus` will hold the same `Vec<SourceLog>` in four places —
`notes.logs` plus three `priced_hours[i].estimates.logs`. Not new: `PowerEstimates` already ships
three copies and `peak_power_cli.rs:78` writes only `notes.write_logs()`. Nothing renders logs, so
drawing the detail tab writes nothing. Leave the copies; clearing them would make
`priced_hours[0].estimates.write_logs()` a silent no-op on a value a caller legitimately holds.

### No new re-export

`src/api/mod.rs` states the rule: field types are excluded, because reading a field never requires
naming it. `PricedHour` is a field type of `DeliveryCost`, exactly as `IntervalEstimates` is of
`PowerEstimates`. The GUI reaches both through `pure::peak_power::PricedHour` and
`session::IntervalEstimates`.

### Tests

In `peak_power.rs`:

- `each_priced_hour_holds_the_estimate_its_own_figure_was_taken_from` — interval start matches the
  fixture peak hour, and `energy_based(...).mid()` equals the matching scalar. The three fixture
  hours hold different sessions, so a swap shows as a wrong number.
- `the_priced_hours_name_the_basis_the_charges_table_prints` — `["kVA", "kW", "kW 7-7"]`, and each
  appears in the corresponding `Display` row. This is what lets `Display` keep its own literals.
- `the_delivery_report_says_nothing_about_the_hours_it_kept` — retaining is not rendering.
- `each_priced_hour_lists_only_the_anomalies_that_reached_it` — a hot session in the kW peak hour
  appears in that hour's list and neither other.
- `the_notes_are_the_anomalies_of_the_hours_the_cost_keeps` — the detail tab can never show a row
  the summary omits.

In `recovery.rs`:

- `the_surplus_keeps_the_three_priced_hours_the_delivery_cost_was_drawn_from`
- `hoisting_the_notes_leaves_each_priced_hours_own_lists_alone` — catches someone "tidying up" by
  clearing the estimates in the hoist and silently emptying the detail tab.
- `the_surplus_names_one_place_to_write_the_run_logs`
- `the_surplus_report_is_unchanged_by_the_hours_it_now_keeps`

**One gap worth knowing:** there is no golden file for the surplus report. The only goldens are
`tests/fixtures/sessions/*.report.md`, covering `IntervalEstimates::to_markdown` and the site-load
table. Byte-parity for `CostRecoverySurplus` currently rests on substring assertions. I propose
adding one as a unit test in `recovery.rs` writing to `tests/fixtures/` under the existing
`UPDATE_REPORT_GOLDEN=1` convention — the fixtures are `#[cfg(test)] pub(crate)` so it cannot live
in `tests/`. Flagged rather than assumed.

---

## Commit 3 — The app

`src/bin/ev_cost_recovery/`, mirroring `ev_peak_gui`'s split: widgets thin enough to check by eye,
every decision in an egui-free `state.rs` that is unit-tested without a window.

| File | Contents |
|:---|:---|
| `main.rs` | window options, icon, zoom, `theme::apply`, `run_native` |
| `app.rs` | `APP_NAME = "EV Cost Recovery"`, the tab strip, About |
| `state.rs` | `AppState`, `SurplusState`, `RatesForm`, outcomes — no egui |
| `surplus.rs` | the Cost recovery tab |
| `detail.rs` | the Peak power detail tab |
| `theme.rs`, `widgets.rs`, `about.rs` | copied verbatim from `ev_peak_gui` |

The three copied files are ~310 lines of duplication with a known end date: they go when
`ev_peak_gui` retires. Deliberate, per your YAGNI call.

### Opening view

No landing screen. Both tabs from the start, **Peak power detail** greyed until a run succeeds.

```
┌────────────────────────────────────────────┐
│ Cost recovery │ Peak power detail │  About │
│ ═══════════════                            │
└────────────────────────────────────────────┘
```

### The Cost recovery tab

Inputs in your order — bill, meter export, two session reports, then rates:

```
Toronto Hydro bill        [ Choose... ]  TH_Bill_Jun2026.pdf
Green Button export       [ Choose... ]  TH_Electric_Usage.XML
Session report 1          [ Choose... ]  Session_Report_May_1_2026-May_31_2026.csv
Session report 2          [ Choose... ]  Session_Report_June_1_2026-June_30_2026.csv

Cost-recovery rates
  Effective  [ 2026-05-01 ▼ ]
  On-peak  [0.1100]   Mid-peak  [0.0900]   Off-peak  [0.0700]   $/kWh
  ☐ Rates changed during the period

                        [ Work out the surplus ]
```

- **Pickers** go through the copied `widgets::dialog`, which carries the Linux portal workaround.
  One shared `WorkingDir` for the whole app.
- **CSV validation at pick time.** Reuse the existing name-based check in
  `src/api/pure/coverage.rs` rather than writing a parser. A report whose name carries no dates is
  reported on the field, not on submit. Full coverage is checked once the bill and both CSVs are
  in hand, since the period comes from the bill.
- **No closing date field.** The bill states which period it covers.
- **Rates** are held as `String`, not `f64`, so a half-typed `0.` is not mangled;
  `RatesForm::parse() -> Result<CostRecoveryRates, String>` runs on submit. The date is
  `egui_extras::DatePickerButton`, already a dependency. The second schedule appears only when the
  checkbox is ticked — the CLI encodes that positionally, which a form should not.
  `CostRecoveryError::RatesNotYetInEffect` and `RateChangeOutsidePeriod` are the two library errors
  that point at rate fields rather than pickers.
- **On success**: `surplus.notes.write_logs()` first, so a log failure is not buried under the
  report — the same order `cost_recovery_surplus_cli.rs:113` uses. Then a headline grid (recovery,
  energy cost, delivery cost, surplus, and the verdict sentence), Copy/Save, and the report split
  into collapsing sections by the same setext-heading rule `ev_peak_gui`'s `report_sections`
  uses.
- **Errors** are the library's own message, unaltered, in `widgets::error_block` — `format!("{e}")`
  on `ApiError` is already a complete one-line message naming the file.
- Every setter clears the results, so stale figures never survive an input change.

### The Peak power detail tab

Three collapsing sections, one per `PricedHour`, headed by unit and the hour in local time, each
body `estimates.to_markdown()` in `widgets::monospace_block`. Its own Save button.

Anomalies appear here per-interval (already in `to_markdown`) *and* in the surplus report's hoisted
section *and* in the `.csv.read.log` files. The excluded-session table repeats across the three
sections; that rendering carries an `In interval` column the hoisted section does not, so the
repetition carries information.

### Tests in `state.rs`

- `the_app_produces_the_same_report_as_the_command_line` — the saved text is `surplus.to_string()`.
- `a_session_report_with_no_dates_in_its_name_is_refused_at_pick_time`
- `the_second_rate_schedule_is_sent_only_when_it_is_asked_for`
- `a_rate_that_is_not_a_number_names_the_band_it_belongs_to`
- `changing_an_input_discards_the_figures_it_produced`
- `the_detail_tab_offers_nothing_until_a_run_has_succeeded`

### Icon and window

900×700, min 560×420, zoom 1.15, `assets/icon.png` — same as `ev_peak_gui`. Both apps sharing one
icon is a wrinkle only while both exist.

---

## Commit 4 — Documentation

- `README.md` — the app under "Building and running"; what it takes and what the two tabs show.
- `docs/maintenance-manual.md:177` — the parity test is named under the crate, not the binary;
  add the new app's parity test beside it.

---

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
cargo test
```

**Byte-parity, by hand** — the point of the whole exercise:

```sh
cargo run --bin cost_recovery_surplus_cli -- \
  data/TH_Bill_Jun2026.pdf data/TH_Electric_Usage.XML \
  2026-05-01:0.1100,0.0900,0.0700 \
  data/Session_Report_May_1_2026-May_31_2026-mock.csv \
  data/Session_Report_June_1_2026-June_30_2026.csv > /tmp/cli.md
```

Run the app with the same five inputs, Save, `diff` the two. Must be empty.

**The app itself** — `scripts/run-gui.sh` wraps `cargo run` in `dbus-run-session`, which rfd 0.17
needs on Linux; add the new binary to it. Check by eye: the detail tab is greyed before a run; the
three sections name the three hours; both `.csv.read.log` files are rewritten on each run.

**That the library change moved nothing:** the June surplus report hashed
`71aec1fed30063b68a97970362190f58` at the end of the anomaly work. It must still.
