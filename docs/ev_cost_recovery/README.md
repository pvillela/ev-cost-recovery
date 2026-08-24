# EV Cost Recovery — the desktop app

Works out, for one Toronto Hydro billing period, whether the EV cost-recovery rates you charge
drivers covered what the chargers cost the building. The figures are the same as those from
`cost_recovery_surplus_cli`, computed by the same library code; what differs is only the asking.

The app is a single self-contained binary — no installer, no runtime to put on the machine first.

## What it needs

Four files and a rate schedule, in the order the window asks for them.

| Input | What it is |
|:---|:---|
| Toronto Hydro bill | The PDF invoice for the period, as issued |
| Green Button export | Toronto Hydro's ESPI XML feed of meter readings |
| Session report 1 | The Evolute monthly session report CSV covering one end of the period |
| Session report 2 | The one covering the other end |

**No closing date is asked for.** The bill states which period it covers, and everything else is
read against that. A bill whose meter reading period does not close a billing period is refused
rather than estimated from.

**Two session reports, because one is not enough.** A billing period runs from the 24th to the 23rd
and so straddles two calendar months, while a session report covers one. Either order will do — the
file names say what each holds. A name that does not say is refused at the moment it is picked,
rather than after four files have been read, because that name is the only thing that states which
month the file holds.

**The rates are yours.** No bill is read for them and no tax is added: what they have to cover is
your decision. Give a second schedule when the rates changed during the period, and the energy is
split at local midnight on its effective date. A blank rate is refused rather than read as zero,
which would price that band's energy at nothing and still produce a report.

## What it shows

### Cost recovery

The surplus report. A positive surplus means the rates covered the chargers' share of the bill for
that period; a negative one means they fell short.

The headline is the four amounts of the subtraction — recovery, energy cost, delivery cost, surplus
— and beneath it the whole report, split into sections you can collapse. Every figure in the
subtraction can be checked, because all three sub-reports are printed below the summary.

Only the delivery and energy sides are counted as EV cost. The customer charge, the standard supply
administration charge and the wholesale market service charge are in neither, so a surplus of zero
is not breaking even on the whole invoice.

**Saved or copied, it is byte-for-byte what the command line prints.** That holds because there is
one rendering rather than two that could drift; see
[`docs/maintenance-manual.md`](../maintenance-manual.md), "Where the rendering lives".

### Peak power detail

Three demand-priced delivery lines are levied on three different intervals of interest — the ones
the *building* peaked in, each in that line's own unit:

| Bill line | Levied on | Interval |
|:---|:---|:---|
| Distribution Charges | `Adj. kVA` | where the site's kVA peaked |
| Transmission Connection Charge | `Adj. kW` | where its kW peaked |
| Transmission Network Charge | `Adj. Peak kW 7-7` | where its kW peaked within 07:00–19:00 |

This tab shows the full estimate over each: the segment table, both derivations, the sessions that
reached the interval and anything about them that needed a judgement call. It has no command-line
equivalent.

Two maxima are involved and they are not the same question. **Which interval** comes from the meter
export — where the building peaked. **Which 15-minute segment inside it** comes from the session
records — where the chargers peaked. A demand charge is billed on that segment, which is why the
report names it.

The tab is greyed until a run has succeeded, and a changed input takes it away again: figures
describe the inputs that produced them.

Nothing here is recomputed. These are the estimates the delivery cost was actually priced on, so a
figure in this tab and the charge it produced cannot disagree.

## What it writes

Two things, and only when you ask for them:

- **A run log** beside each session report read, named `<report>.csv.read.log`. Written on every
  run, before the report is shown, so a failure to write one is not buried under it. It records
  what the reader found — the rows it accepted, and every row that needed a judgement call.
- **A report**, when you press Save. Each tab saves its own.

Anomalies appear in three places, deliberately: in the run log, in the report's own notes sections,
and — for the intervals the demand charges were priced on — in the Peak power detail tab.

## Running it

**Windows.** Double-click `ev_cost_recovery.exe`. It is not code-signed, so the first run shows
SmartScreen's "Windows protected your PC" — choose *More info* then *Run anyway*. Later runs are
silent.

**Linux.** Mark it executable once (`chmod +x ev_cost_recovery`) and run it.

**From a checkout**, use [`scripts/run-gui.sh`](../../scripts/run-gui.sh) rather than a bare
`cargo run`. The app opens files through `rfd`, whose default backend needs a desktop portal; with
none reachable the dialogs fail *silently*, which is easy to misdiagnose as a bug in the app. The
script supplies a DBus session. See
[`docs/session/Devcontainer_GUI_Options.md`](../session/Devcontainer_GUI_Options.md) for the
container setup.

## Where the code is

`src/bin/ev_cost_recovery/`. Every decision lives in `state.rs`, which has no `egui` in it, so what
could be *wrong* rather than merely ugly is unit-tested without a window. The widget modules above
it — `surplus.rs` and `detail.rs` — are meant to be thin enough to check by eye.
