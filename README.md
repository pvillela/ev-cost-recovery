# EV Cost Recovery

Works out how much of a building's electricity cost is attributable to EV charging.

A demand charge is levied on the highest 15-minute average the building reached in a billing
period. That figure is metered for the building as a whole, and nothing measures how much of it the
chargers were responsible for. This software estimates that share from the two records that do
exist: the utility's interval data, and the charging network's session report.

## The app

`ev_cost_recovery` answers, for one Toronto Hydro billing period, whether the EV cost-recovery rates
you charge drivers covered what the chargers cost the building. It is the only binary a release
ships: a single self-contained file, with no installer and no runtime to put on the machine first.

Its figures are the same as those from `cost_recovery_surplus_cli`, computed by the same library
code. What differs is only the asking.

### What it needs

Four files and a rate schedule, in the order the window asks for them.

| Input | What it is |
|:---|:---|
| Toronto Hydro bill | The PDF invoice for the period, as issued |
| Green Button export | Toronto Hydro's ESPI XML feed of meter readings |
| Session report 1 | The Evolute monthly session report CSV covering one end of the period |
| Session report 2 | The one covering the other end |

**No closing date is asked for.** The bill states which period it covers, and everything else is read
against that. A bill whose meter reading period does not close a billing period is refused rather
than estimated from.

**Two session reports, because one is not enough.** A billing period runs from the 24th to the 23rd
and so straddles two calendar months, while a session report covers one. Either order will do — the
file names say what each holds. A name that does not say is refused at the moment it is picked,
rather than after four files have been read, because that name is the only thing stating which month
the file holds.

**The rates are yours.** No bill is read for them and no tax is added: what they have to cover is
your decision. Give a second schedule when the rates changed during the period, and the energy is
split at local midnight on its effective date. A blank rate is refused rather than read as zero,
which would price that band's energy at nothing and still produce a report.

### What it shows

**Cost recovery** is the surplus report. A positive surplus means the rates covered the chargers'
share of the bill for that period; a negative one means they fell short. The headline is the four
amounts of the subtraction — recovery, energy cost, delivery cost, surplus — and beneath it the whole
report in collapsible sections. Every figure in the subtraction can be checked, because all three
sub-reports are printed below the summary.

Only the delivery and energy sides are counted as EV cost. The customer charge, the standard supply
administration charge and the wholesale market service charge are in neither, so a surplus of zero is
not breaking even on the whole invoice.

Saved or copied, that report is byte-for-byte what the command line prints. That holds because there
is one rendering rather than two that could drift; see
[`docs/maintenance-manual.md`](docs/maintenance-manual.md), "Where the rendering lives".

**Peak power detail** is what the command line cannot show. Three demand-priced delivery lines are
levied on three different intervals of interest — the ones the *building* peaked in, each in that
line's own unit:

| Bill line | Levied on | Interval |
|:---|:---|:---|
| Distribution Charges | `Adj. kVA` | where the site's kVA peaked |
| Transmission Connection Charge | `Adj. kW` | where its kW peaked |
| Transmission Network Charge | `Adj. Peak kW 7-7` | where its kW peaked within 07:00–19:00 |

The tab shows the full estimate over each: the segment table, both derivations, the sessions that
reached the interval and anything about them that needed a judgement call.

Two maxima are involved, and they are not the same question. **Which interval** comes from the meter
export — where the building peaked. **Which 15-minute segment inside it** comes from the session
records — where the chargers peaked. A demand charge is billed on that segment, which is why the
report names it.

Nothing in the tab is recomputed. These are the estimates the delivery cost was actually priced on,
so a figure there and the charge it produced cannot disagree. The tab is greyed until a run has
succeeded, and a changed input takes it away again: figures describe the inputs that produced them.

### What it writes

Two things, and only when you ask for them:

- **A run log** beside each session report read, named `<report>.csv.read.log`. Written on every run,
  before the report is shown, so a failure to write one is not buried under it. It records what the
  reader found — the rows it accepted, and every row that needed a judgement call.
- **A report**, when you press Save. Each tab saves its own.

Anomalies appear in three places, deliberately: in the run log, in the report's own notes sections,
and — for the intervals the demand charges were priced on — in the Peak power detail tab.

### Running it

**Windows.** Double-click `ev_cost_recovery.exe`. It is not code-signed, so the first run shows
SmartScreen's "Windows protected your PC" — choose *More info* then *Run anyway*. Later runs are
silent.

**Linux.** Mark it executable once (`chmod +x ev_cost_recovery`) and run it.

**From a checkout**, `cargo run --bin ev_cost_recovery`. Nothing under `data/` is in the repository
— `data/.gitignore` is `*` — so the bills, the meter export and the session reports have to be
brought to the checkout before there is anything to run it against.

The app opens files through `rfd`, whose default backend needs a session bus; where a desktop portal
is installed the GTK stack starts one itself, so nothing has to be arranged. Where no portal is
reachable the dialogs fail *silently* — which is easy to misdiagnose as a bug in the app — and
[`scripts/run-gui.sh`](scripts/run-gui.sh) is the fallback for that. See
[`docs/session/Devcontainer_GUI_Options.md`](docs/session/Devcontainer_GUI_Options.md) for the
container setup.

The code is `src/bin/ev_cost_recovery/`. Every decision lives in `state.rs`, which has no `egui` in
it, so what could be *wrong* rather than merely ugly is unit-tested without a window. The widget
modules above it — `surplus.rs` and `detail.rs` — are meant to be thin enough to check by eye.

## The four modules

Three read a source of data. The fourth is the calendar they all run on.

| Module | Reads | Answers |
|---|---|---|
| [`green_button`](docs/green_button/README.md) | Toronto Hydro's Green Button export, an ESPI XML feed of hourly meter readings | When did the building peak, and at what kW and kVA? Which billing period was that in? |
| [`sessions`](docs/session/README.md) | The Evolute monthly session report, a CSV of charging sessions | Over a chosen interval, how much of the demand was EV charging — as a bracket, not a point? |
| [`hydro_bills`](docs/hydro_bill/) | The PDF invoices Toronto Hydro issues | What did the utility charge, and on what terms? |
| [`time`](docs/time/README.md) | Nothing of its own — the other three call it | What does a timestamp mean here, and what does the Ontario calendar say about it? |

`time` is a module rather than a scattering of helpers because the same calendar has to be applied
identically by all three. Getting it subtly different in two places is a class of error nothing else
here would catch.

`sessions` reports a **bracket** rather than a single number, and that is deliberate. Session start
and end times are reported only to the minute, so a session's true extent is known to within a
minute at each end. Every figure therefore runs from what the reported times least support to what
they most support. See [`docs/session/time-reporting-uncertainty.md`](docs/session/time-reporting-uncertainty.md)
for the derivation.

`hydro_bills` is the least complete of the four. It reads a bill and it defines the billing period
boundary, since that is a fact about the bill; the step from *how much power* to *how much money* is
still to be written.

## Documentation

- [`docs/app-cheat-sheet.md`](docs/app-cheat-sheet.md) — trying the app against the files in `data/`:
  which to pick, what to expect, and the errors worth provoking
- [`docs/session/README.md`](docs/session/README.md) — the estimation logic, the workbook, the
  interval-of-interest rules
- [`docs/green_button/README.md`](docs/green_button/README.md) — the ESPI feed, the peak values, the
  workbook layout
- [`docs/hydro_bill/green-button-vs-bills.md`](docs/hydro_bill/green-button-vs-bills.md) — the
  export checked against every invoice, period by period
- [`docs/time/README.md`](docs/time/README.md) — the two clocks and which is for what, the DST fold
  and how it is resolved, the time grid
- [`docs/maintenance-manual.md`](docs/maintenance-manual.md) — what to check before changing a
  constant, how to regenerate the golden files, the invariants nothing enforces
- [`docs/session/time-reporting-uncertainty.md`](docs/session/time-reporting-uncertainty.md) — the
  specification the consistency checks are derived from

## Building, and the command line

```sh
cargo build --release      # the desktop app, ev_cost_recovery
cargo test                 # everything
cargo run --example sessions -- <workbook.xlsx>
```

The command-line tools are `ev_csv_to_xlsx` (session report to workbook), `ev_peak_cli` (estimate
over an interval), `gb_peak_values` (Green Button feed to workbook) and `hydro_bill_dump` (a bill
PDF's figures). Each prints its usage when run with no arguments.

Six more report on one billing period. `peak_power_cli` gives the kW and kVA peaks, estimated from
a Green Button export and the two session reports spanning the period. `energy_cli` gives the
kilowatt-hours drawn, split by time-of-use band. Two of them price those against a Toronto Hydro
bill: `energy_cost_cli` for the consumption lines and `peak_power_cost_cli` for the three
demand-priced delivery lines. Every rate they use is read off the bill; no tariff is assumed.

The two costing tools ask for no closing date, because the bill states which period it covers.

`cost_recovery_cli` is the other side of the ledger: it prices the same kilowatt-hours at EV
cost-recovery rates you give on the command line, and reports what they recover. No bill is read
and no tax is added — the rates are yours, and what they have to cover is your decision. A schedule
is written `EFFECTIVE_DATE:ON_PEAK,MID_PEAK,OFF_PEAK`; give a second one when the rates changed
during the period, and the energy is split at local midnight on its effective date.

`cost_recovery_surplus_cli` puts the two sides together: what the rates recover, less the delivery
and energy costs, and the difference. A positive surplus means the rates covered the chargers'
share of the bill; a negative one means they fell short. It prints all three reports beneath the
summary, so every figure in the subtraction can be checked. Only the delivery and energy sides are
counted as EV cost — the customer charge, the standard supply administration charge and the
wholesale market service charge are in neither — so a surplus of zero is not breaking even on the
whole invoice.

These six need two adjacent months' session reports, since a billing period runs from the
24th to the 23rd. Only June's is real, so `scripts/make-may-mock.py` builds the May half from it —
shifted dates, fresh ids, and two deliberate quirks that exercise the merge: a session at the end
of May that both reports carry, and a reused `Charge_Session_ID` of May's own. The script explains
both, and refuses to overwrite an output that already exists.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.

Released binaries link third-party crates. Their licences and copyright notices
are generated at release time as `THIRD-PARTY-NOTICES.md`, which ships in each
release archive and is readable from the app's About window. It is not committed
here, since it goes stale as soon as a dependency moves. To produce a copy:

```
bash scripts/gen-notices.sh
```

A release build will not compile without it: `build.rs` checks that the notices
were generated from the current `Cargo.lock`, so a release binary cannot carry a
list that has fallen behind what is linked into it. Debug builds do not need it.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
