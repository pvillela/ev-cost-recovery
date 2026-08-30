# EV Charging Cost Recovery

This software supports the calculation of the impact of EV charging activity on the building's finances.

## Background

The building is on a time-of-use (**TOU**) billing plan with Toronto Hydro.

We define a set of TOU **EV cost-recovery rates** that are meant to defray the incremental electricity costs the building incurs as a result of EV charging activity.

The building has installed an Evolute EV charging system. Evolute's smart panels provide Evolute with information that allows them to measure in detail how much energy each EV charging session consumes and when that consumption occurs. We provide Evolute with our TOU EV cost-recovery rates, which are the basis for Evolute's billing of EV charger owners. Each calendar month, Evolute reimburses the building for the total energy consumed by EV charging sessions during the month, priced at our EV cost-recovery rates.

## Calculation challenges

We need a way to calculate the impact of EV charging activity on the total electricity cost billed by Toronto Hydro, but there are challenges.  EV charging activity is combined with the building's other electric energy consumption in Toronto Hydro's electric bills, i.e., there is no separate Toronto Hydro meter dedicated to the EV charging infrastructure.

### Challenge 1: Energy <u>and</u> power

The TOU cost-recovery rates apply to EV charger energy consumption, but electricity costs in the Toronto Hydro bill depend not only on energy consumption but also, significantly, on the building's peak power draw during the billing period. So, the cost recovery mechanism based on rates applied to energy consumption may over- or under-recover the total costs attributable to EV charging activity.

### Challenge 2: Multiple intervals

There are three bill components that depend on peak power. One depends on the 15-minute interval with the highest kW in the billing period, another depends on the 15-minute interval with the highest kVA in the billing period, and the third depends on the 15-minute interval with the highest kW *not* in an off-peak hour during the billing period.

### Challenge 3: Toronto Hydro billing period vs. Evolute reporting

A Toronto Hydro bill for the building states that it covers from the 23rd of a month to the 23rd of the following month. In practice, the bill covers the period from 00:00:00 (inclusive) EST (Eastern Standard Time, not Eastern Time) on the 24th of a month to 00:00:00 EST (exclusive) on the 24th of the following month. In other words, all of the 24th of a month (EST) to all of 23rd of the following month (EST). So, while DST (daylight saving time) impacts the TOU periods, it does not impact billing period determination. That is good, because the billing period stays stable throughout the year, regardless of DST.

Toronto Hydro Green Button metering data is reported in UTC (Coordinated Universal Time = EST + 5h, no DST).

On the other hand, Evolute's reports are based on calendar months, running from the 1st to the last day of each month, in ET (not EST), so they **are** impacted by DST. A side-effect of reporting in ET is that some information on the "DST fold" day (when DST transitions back to Standard Time) may be ambiguous and the software needs to use a heuristic approach to disambiguate.

Correlating a Toronto Hydro bill with Evolute's reports is challenging as they cover different periods and use different time standards.

### Challenge 4: Time resolution

While Toronto Hydro measures peak power using 15-minute intervals, its comprehensive Green Button metering data provides data in 1-hour intervals. So, the software has to figure out which of the four 15-minute intervals in an hour is the one that maximises a particular power value.

Evolute's Session Report provides crucial data for the calculations performed by this software. Reported session start and end times are truncated to whole minutes rather than being provided with 1-second resolution. As a result, there is an inherent (7% = 1/15) uncertainty associated with the actual start and end of each session when comparing with a 15-minute Toronto Hydro interval of interest.

### Challenge 5: Non-linearity

The actual maximum power draw of an EV connected to the Evolute system is approximately 6.7 kW and 6.8 kVA, but it can actually vary somewhat depending on how many cars are concurrently charging. The impact is not highly significant but the software includes an electrical engineering model that estimates the power draw more accurately.

## Architectural goals

The software architecture was designed with these goals:

- Easy-to-use -- Simple user interface, as self-explanatory as possible.
- Multi-target -- Able to run on Windows, Linux, and Mac. Written in Rust, a multi-target language.
- Zero operations -- No deployment or installation required -- Just download and double-click to run.
- No configuration -- No configuration files. Configuration is baked-in. Configuration changes imply a new release (see below). As a result, configuration and code are always kept in sync.
- Coexistence of multiple releases -- Over time, source documents and EV charging infrastructure will likely change and new releases will be required. But old releases should continue to work and coexist with new ones, so that the software can be applied to past data without bloating over time.
- Runs fast -- Written in Rust, a language that operates at a low level under the hood.
- Maintainable -- Modular structure, written in Rust, a modern high-level language.

## What the software does

The **`ev_cost_recovery`** application performs the following functions:

- **Cost recovery** -- Calculate the net financial impact of EV charging sessions during a billing period:
  - Energy consumption and regulatory wholesale market costs attributable to EV charging. These are energy-based.
  - Delivery charges attributable to EV charging. These are based on peak power demand.
  - Gross cost recovery from TOU EV cost-recovery rates.
  - Net recovery surplus or deficit.
- **Peak power detail** -- Report on peak power demand details -- At what intervals was peak power attained and what was the portion attributable to EV charging. This is an **optional function**, not required for routine monthly or quarterly financial reporting and review of EV charging activity.
- **Evolute reimbursement** -- Reconcile Evolute reimbursement -- Match actual amount received from Evolute for a calendar month with calculated energy consumption and amounts from two Evolute reports.
- **Convert to workbook** -- Convert source data files from Toronto Hydro metering and Evolute sessions to Excel workbooks to facilitate data review and exploration. This is an **optional function**, not required for routine monthly or quarterly financial reporting and review of EV charging activity.

### Inputs and outputs for `Cost recovery` <small>(and `Peak power detail`)</small>

#### Inputs

Four files and a rate schedule:

| Input                      | What it is                                                   |
| :------------------------- | :----------------------------------------------------------- |
| Toronto Hydro bill         | The PDF invoice for the billing period.                      |
| Green Button export        | Toronto Hydro's ESPI XML feed of meter readings for a date range. The data must cover at least the full billing period. |
| Session report 1           | The Evolute monthly Session Report CSV file covering one end of the billing period. |
| Session report 2           | The Session Report CSV covering the other end of the billing period. |
| TOU EV cost-recovery rates | TOU EV cost-recovery rates in effect at the beginning of the billing period. If the cost-recovery rates change during the billing period then a second set of rates also needs to be provided. |

#### Outputs

On-screen reports for the cost recovery surplus calculation and peak power details. The reports can be saved and/or copied to the clipboard.

### Inputs and outputs for `Evolute reimbursement`

#### Inputs

Two files, a remittance amount, and a rate schedule:

| Input                      | What it is                                                   |
| :------------------------- | :----------------------------------------------------------- |
| Session report             | The Evolute monthly Session Report CSV file for the calendar month. |
| Charges report             | The Evolute Charges Report CSV file for the calendar month.  |
| Remittance                 | The reimbursement received from Evolute for the calendar month. |
| TOU EV cost-recovery rates | TOU EV cost-recovery rates in effect during the calendar month. |

#### Outputs

On-screen report for the Evolute reimbursement reconciliation. The report can be saved and/or copied to the clipboard.

### Inputs and outputs for `Convert to workbook`

#### Inputs

When converting an Evolute Session Report:

| Input          | What it is                                                   |
| :------------- | :----------------------------------------------------------- |
| Session report | The Evolute monthly Session Report CSV file for a calendar month. |

When converting a Green Button export:

| Input               | What it is                                                   |
| :------------------ | :----------------------------------------------------------- |
| Green Button export | Toronto Hydro's ESPI XML feed of meter readings for a date range. |

#### Outputs

Converted files are written to the same folder as the input files. Each converted files has the same name as its input file, but with the ".xlsx" file type.

### Error reporting and logging

The software checks for problems when reading the inputs.

Some problems are temporary, e.g., due to an oversight by the user. Such cases just merit an on-screen message.

Other problems are more persistent. Such problems are reported on-screen and also logged to a log file created in the same folder as the input file.

Serious errors block the performance of the desired function. Less severe anomalies do not block function execution, but must still be reported on-screen and logged for the user's awareness. Certain anomalies are additionally included in the functional reports (as opposed to error reports) produced by the software functions.

## Quick start guide



### Running it

**Windows.** Double-click `ev_cost_recovery.exe`. It is not code-signed, so the first run shows
SmartScreen's "Windows protected your PC" — choose *More info* then *Run anyway*. Later runs are
silent.

**Linux.** Mark it executable once (`chmod +x ev_cost_recovery`) and run it.

**From a checkout**, `cargo run --bin ev_cost_recovery`. Nothing under `data/` is in the repository
— `data/.gitignore` is `*` — so the bills, the meter export and the session reports have to be
brought to the checkout before there is anything to run it against.

The app opens files through `rfd`, configured in `Cargo.toml` to use GTK directly on Linux rather
than the desktop portal `rfd` would reach for by default. The dialogs are therefore ordinary GTK
windows in the app's own process: nothing has to be arranged, and no session bus has to be running.
Building on Linux needs `libgtk-3-dev` and `pkg-config`; running needs the GTK 3 runtime, which
any GTK desktop already has (`libgtk-3-0` on Ubuntu 22.04, renamed `libgtk-3-0t64` by the 64-bit
`time_t` transition in 24.04 and later). See
[`docs/session/Devcontainer_GUI_Options.md`](docs/session/Devcontainer_GUI_Options.md) for the
container setup.

The code is `src/bin/ev_cost_recovery/`. Every decision lives in `state.rs`, which has no `egui` in
it, so what could be *wrong* rather than merely ugly is unit-tested without a window. The widget
modules above it — `surplus.rs`, `detail.rs`, `reimbursement.rs` and `convert.rs`, one per tab —
are meant to be thin enough to check by eye.

