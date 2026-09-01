# EV Cost Recovery

This software supports the calculation of the impact of EV charging activity on the building's finances.

## Background

The building is on a time-of-use (**TOU**) billing plan with Toronto Hydro.

We define a set of TOU **EV cost-recovery rates** that are meant to defray the incremental electricity costs the building incurs as a result of EV charging activity.

The building has installed an Evolute EV charging system. Evolute's smart panels provide Evolute with information that allows them to measure in detail how much energy each EV charging session consumes and when that consumption occurs. We provide Evolute with our TOU EV cost-recovery rates, which are the basis for Evolute's billing of EV charger owners. Each calendar month, Evolute reimburses the building for the total energy consumed by EV charging sessions during the month, priced at our EV cost-recovery rates.

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

## Getting and running the software

### Getting the executable

Download the latest version of the software (or a prior version if required) for your operating system (e.g., Windows, Linux) from the [Releases](https://github.com/pvillela/ev-cost-recovery/releases) section of the GitHub repo. Extract the executable file from the downloaded archive file, and place the executable file anywhere you choose on your file system.

### Running the executable

Before running the software, understand the [software inputs and outputs](#software-inputs-and-outputs).

**Windows** -- Double-click the executable file. It is not code-signed, so the first run shows
SmartScreen's "Windows protected your PC" — choose *More info* then *Run anyway*. Later runs are
silent.

**Linux** -- Run it by double-clicking or from the command line.

### Running from sources

If you want to execute from the source code, clone the repo and run `cargo run --bin ev_cost_recovery`.

**Linux notes:** The app opens files through `rfd`, configured in `Cargo.toml` to use GTK directly on Linux rather than the desktop portal `rfd` would reach for by default. The dialogs are therefore ordinary GTK
windows in the app's own process: nothing has to be arranged, and no session bus has to be running.
Building on Linux needs `libgtk-3-dev` and `pkg-config`; running needs the GTK 3 runtime, which
any GTK desktop already has (`libgtk-3-0` on Ubuntu 22.04, renamed `libgtk-3-0t64` by the 64-bit
`time_t` transition in 24.04 and later).

## Software inputs and outputs

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

## Error reporting and logging

The software checks for problems when reading the inputs. 

When an input file is ingested by the software, any problems are reported on-screen and a log file is created in the same folder as the input file. If no problems are detected when reading the input file, the log file will say so. Otherwise, the log file will contain a description of problems.

Some problems are temporary, e.g., due to an oversight by the user. Such cases may just merit an on-screen message.

Serious errors block the performance of the desired function. Less severe anomalies do not block function execution, but must still be reported on-screen and logged for the user's awareness. Certain anomalies are additionally included in the functional reports (as opposed to error reports) produced by the software functions.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Released binaries link third-party crates. Their licences and copyright notices are generated at release time as `THIRD-PARTY-NOTICES.md`, which ships in each release archive and is readable from the app's About window. It is not committed here, since it goes stale as soon as a dependency moves. To produce a copy:

```
bash scripts/gen-notices.sh
```

A release build will not compile without it: `build.rs` checks that the notices were generated from the current `Cargo.lock`, so a release binary cannot carry a list that has fallen behind what is linked into it. Debug builds do not need it.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Additional documentation

Much, but not all, of this documentation pertains to software structure or electrotechnical concerns. Some portions are useful to end-users and administrators.

- [docs/app-cheat-sheet.md](docs/app-cheat-sheet.md) -- Steps for trying the app against data files in `data/` directory (not available in the repo): which to pick, what to expect, and the errors worth provoking.
- [docs/historic-feature.md](docs/historic-feature.md) -- Describes the "historic" cargo feature.
- [docs/maintenance-manual.md](docs/maintenance-manual.md) -- What to check before changing a constant, how to regenerate the golden files, the invariants nothing enforces.
- [docs/Questions_for_Evolute.md](docs/Questions_for_Evolute.md) -- Questions to ask Evolute;  answers may result in software changes.
- [docs/site-specific-constants.md](docs/site-specific-constants.md) -- Constants specific to this site at the present time. Other sites using this repo's code will likely need to change some of them.
- [docs/session/Rough_kW_kVA_Table.xlsx](docs/session/Rough_kW_kVA_Table.xlsx) -- Spreadsheet comparing rough kW and kVA estimates per charger with the values resulting from the electrotechnical site model (see also [docs/session/site-model-marcus.md](docs/session/site-model-marcus.md)).
- [docs/session/README.md](docs/session/README.md) -- The estimation logic and the generated workbook layout.
- [docs/session/Google_Voltage_Fluctuations_in_Building.md](docs/session/Google_Voltage_Fluctuations_in_Building.md) -- Normal voltage fluctuations to be expected.
- [docs/session/segment-tiling.md](docs/session/segment-tiling.md) -- Worked-out example of the decomposition of a 1-hour interval of interest (e.g., from the Green Button feed) into four 15-minute segments and how the EV session power demand impact is calculated.
- [docs/session/site-model-marcus.md](docs/session/site-model-marcus.md) -- Electrotechnical model of an Evolute panel, transformer, and connected EV charging stations. Values are based on an Evolute 20-breaker panel and a Marcus AMTH75A1 transformer.
- [docs/session/site-load-report-marcus.txt](docs/session/site-load-report-marcus.txt) -- Table, generated by the software, corresponding to the site model.
- [docs/session/Google_Transformer_characteristics.md](docs/session/Google_Transformer_characteristics.md) -- Technical characteristics of the Marcus AMTH75A1 transformer.
- [docs/session/transformer-glossary.md](docs/session/transformer-glossary.md) -- Glossary of electrotechnical terms related to transformers.
- [docs/session/time-reporting-uncertainty.md](docs/session/time-reporting-uncertainty.md) --  The impact of the truncation of reported session start and end times.
- [docs/session/Evolute-Simultaneous_Charging.pdf](docs/session/Evolute-Simultaneous_Charging.pdf) -- Evolute technical documentation about simultaneous charging limits in terms of voltages, currents, kW, kVA, transformer parameters, and number of charging stations.
- [docs/green_button/Notes_on_Green_Button_data.md](docs/green_button/Notes_on_Green_Button_data.md) -- Brief notes about Green Button data.
- [docs/green_button/Toronto_Hydro_Object_Model.md](docs/green_button/Toronto_Hydro_Object_Model.md) -- The conceptual domain model for the Green Button ESPI XML feed.
- [docs/time/README.md](docs/time/README.md) -- Date-time-related functions and constants, the time grid, the DST fold and how it is resolved.
- [docs/Development_Approach_and_Roles.md](docs/Development_Approach_and_Roles.md) -- How the software was developed.

## Appendix

### Architectural goals

The software architecture was designed with these goals:

- Easy-to-use -- Simple user interface, as self-explanatory as possible.
- Multi-target -- Able to run on Windows, Linux, and Mac. Written in Rust, a multi-target language.
- Zero operations -- No deployment or installation required -- Just download and double-click to run.
- No configuration -- No configuration files. Configuration is baked-in. Configuration changes imply a new release (see below). As a result, configuration and code are always kept in sync.
- Coexistence of multiple releases -- Over time, source documents and EV charging infrastructure will likely change and new releases will be required. But old releases should continue to work and coexist with new ones, so that the software can be applied to past data without bloating over time.
- Runs fast -- Written in Rust, a language that operates at a low level under the hood.
- Maintainable -- Modular structure, written in Rust, a modern high-level language.

### Calculation challenges

We need a way to calculate the impact of EV charging activity on the total electricity cost billed by Toronto Hydro, but there are challenges.  EV charging activity is combined with the building's other electric energy consumption in Toronto Hydro's electric bills, i.e., there is no separate Toronto Hydro meter dedicated to the EV charging infrastructure.

#### Challenge 1: Energy <u>and</u> power

The TOU cost-recovery rates apply to EV charger energy consumption, but electricity costs in the Toronto Hydro bill depend not only on energy consumption but also, significantly, on the building's peak power draw during the billing period. So, the cost recovery mechanism based on rates applied to energy consumption may over- or under-recover the total costs attributable to EV charging activity.

#### Challenge 2: Multiple intervals

There are three bill components that depend on peak power. One depends on the 15-minute interval with the highest kW in the billing period, another depends on the 15-minute interval with the highest kVA in the billing period, and the third depends on the 15-minute interval with the highest kW *not* in an off-peak hour during the billing period.

#### Challenge 3: Toronto Hydro billing period vs. Evolute reporting

A Toronto Hydro bill for the building states that it covers from the 23rd of a month to the 23rd of the following month. In practice, the bill covers the period from 00:00:00 (inclusive) EST (Eastern Standard Time, not Eastern Time) on the 24th of a month to 00:00:00 EST (exclusive) on the 24th of the following month. In other words, all of the 24th of a month (EST) to all of 23rd of the following month (EST). So, while DST (daylight saving time) impacts the TOU periods, it does not impact billing period determination. That is good, because the billing period stays stable throughout the year, regardless of DST.

Toronto Hydro Green Button metering data is reported in UTC (Coordinated Universal Time = EST + 5h, no DST).

On the other hand, Evolute's reports are based on calendar months, running from the 1st to the last day of each month, in ET (not EST), so they **are** impacted by DST. A side-effect of reporting in ET is that some information on the "DST fold" day (when DST transitions back to Standard Time) may be ambiguous and the software needs to use a heuristic approach to disambiguate.

Correlating a Toronto Hydro bill with Evolute's reports is challenging as they cover different periods and use different time standards.

#### Challenge 4: Time resolution

While Toronto Hydro measures peak power using 15-minute intervals, its comprehensive Green Button metering data provides data in 1-hour intervals. So, the software has to figure out which of the four 15-minute intervals in an hour is the one that maximises a particular power value.

Evolute's Session Report provides crucial data for the calculations performed by this software. Reported session start and end times are truncated to whole minutes rather than being provided with 1-second resolution. As a result, there is an inherent (7% = 1/15) uncertainty associated with the actual start and end of each session when comparing with a 15-minute Toronto Hydro interval of interest.

#### Challenge 5: Non-linearity

The actual maximum power draw of an EV connected to the Evolute system is approximately 6.7 kW and 6.8 kVA, but it can actually vary somewhat depending on how many cars are concurrently charging. The impact is not highly significant but the software includes an electrotechnical model that estimates the power draw more accurately.

### Module structure

The software is structured as top-level library modules, each of which may have sub-modules.

| Top-level module | Purpose                                                      |
| ---------------- | ------------------------------------------------------------ |
| `api`            | Functions and types that represent the majority of the software functionality. Builds on all the other modules. The binaries call primarily functions in this module, although they may also call functions in the other modules. |
| `green_button`   | Functionality related to Toronto Hydro's Green Button export, an ESPI XML feed of hourly meter readings. Notably, computes the intervals that maximise the building's kW, kVA, and 7-7 kW during a billing period. |
| `hydro_bill`     | Functionality to read the PDF invoices Toronto Hydro issues. |
| `session`        | Functionality related to the Evolute monthly CSV Session Report. Notably, computes peak load and energy consumption attributable to EV charging sessions. |
| `time`           | Date-time-related constants and functions.                   |
| `charges_report` | Functionality to read the Evolute monthly CSV Charges Report. |
| `error`          | Error types used by multiple other modules.                  |
| `golden`         | Defines a consistent mechanism for integration tests to check their outputs against their golden files. |
| `log`            | Common functionality to produce read logs.                   |
| `markdown`       | Common functionality to produce markdown reports.            |

### Building and the command line

```sh
cargo build --release      # the desktop app, ev_cost_recovery -- and nothing else
cargo test                 # the default build
cargo test --features historic   # and the legacy workbook-reading half
```

Two of the binaries and one example sit behind the `historic` feature, so `cargo build --release`
does not produce them and `cargo test` does not compile them. That is deliberate — see
[`docs/historic-feature.md`](docs/historic-feature.md) — and it is why the tests are two commands:
neither covers the other.

```sh
cargo build --features historic --bin ev_peak_cli --bin ev_peak_gui
cargo run --features historic --example sessions -- <workbook.xlsx>
```

The command-line tools are `ev_csv_to_xlsx` (session report to workbook), `ev_peak_cli` (estimate
over an interval, `historic`), `gb_peak_values` (Green Button feed to workbook) and
`hydro_bill_dump` (a bill PDF's figures). Each prints its usage when run with no arguments.

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
counted as EV cost — the customer charge and the standard supply administration charge are in
neither, being flat.

These six need two adjacent months' session reports, since a billing period runs from the
24th to the 23rd.
