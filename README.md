# EV Cost Recovery

This software supports the calculation of the impact of EV charging activity on the building's finances.

## Contents

- [Background](#background)
- [What the software does](#what-the-software-does)
- [Getting and running the software](#getting-and-running-the-software)
- [Software inputs and outputs](#software-inputs-and-outputs)
- [Error reporting and logging](#error-reporting-and-logging)
- [License](#license)
- [Additional documentation](#additional-documentation)
- [Appendix](#appendix)

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

**Linux notes:** The app opens files through `rfd`, which on Linux asks the XDG desktop portal. The
desktop puts up its own dialog, in its own process — the Qt one on Plasma, the GTK one on GNOME —
so the app looks native on either without linking a toolkit. Nothing but glibc is needed to build
it, and `ldd` on the result names neither GTK nor Qt.

What the machine does need at run time is a portal service and a session bus. Every desktop
environment provides both; a bare window manager assembled from a netinstall may not, and there
the dialog does not open while the rest of the app still runs. A portal older than 1.17 — Ubuntu
22.04 ships 1.14, 26.04 ships 1.21 — opens the dialog in its own default folder instead of the one
the app names.

## Software inputs and outputs

### Inputs and outputs for `Cost recovery` <small>(and `Peak power detail`)</small>

#### Inputs

Five files:

| Input                      | What it is                                                   |
| :------------------------- | :----------------------------------------------------------- |
| Toronto Hydro bill         | The PDF invoice for the billing period.                      |
| Green Button export        | Toronto Hydro's ESPI XML feed of meter readings for a date range. The data must cover at least the full billing period. |
| Session report 1           | An Evolute Session Report CSV. The reports given must cover the whole billing period between them, without a gap. |
| Session report 2           | A second Session Report CSV, if one report does not cover the whole period. Optional. |
| Rates workbook             | The Excel workbook of TOU EV cost-recovery rates, described in [The rates workbook](#the-rates-workbook). The period is priced at the rates in effect on its first day, with at most one change within it. |

#### Outputs

On-screen reports for the cost recovery surplus calculation and peak power details. The reports can be saved and/or copied to the clipboard.

### Inputs and outputs for `Evolute reimbursement`

#### Inputs

Three files and a remittance amount:

| Input                      | What it is                                                   |
| :------------------------- | :----------------------------------------------------------- |
| Session report             | An Evolute Session Report CSV covering the calendar month the Charges Report is for. |
| Charges report             | The Evolute Charges Report CSV file for the calendar month. Its file name is what states the month, and only a single month is accepted. |
| Remittance                 | The reimbursement received from Evolute for the calendar month. |
| Rates workbook             | The same workbook as for `Cost recovery`; choosing it on one tab chooses it on both. The month is priced at the rates in effect on the 1st, and the rates may not change within the month. |

#### Outputs

On-screen report for the Evolute reimbursement reconciliation. The report can be saved and/or copied to the clipboard.

### Inputs and outputs for `Convert to workbook`

#### Inputs

When converting an Evolute Session Report:

| Input          | What it is                                                   |
| :------------- | :----------------------------------------------------------- |
| Session report | An Evolute Session Report CSV, covering whatever date range its file name states. |

When converting a Green Button export:

| Input               | What it is                                                   |
| :------------------ | :----------------------------------------------------------- |
| Green Button export | Toronto Hydro's ESPI XML feed of meter readings for a date range. |

#### Outputs

Converted files are written to the same folder as the input files. Each converted file has the same name as its input file, but with the ".xlsx" file type.

### The rates workbook

The EV cost-recovery rates are read from an Excel workbook (`.xlsx`), which can have any name. The `Cost recovery` and `Evolute reimbursement` tabs share it: choosing it on one chooses it on both. It is read each time the figures are worked out, so a change saved in the spreadsheet is used on the next run.

**The sheet.** The rates are on the sheet named `rates`. If there is none, the sheet named `Sheet1` is used. Capitals and surrounding spaces in the sheet name do not matter. Other sheets are ignored.

**The columns.** Row 1 names the columns. It must hold these four names, spelled exactly as shown, in any order:

| Column           | Contents of the rows below                                   |
| :--------------- | :----------------------------------------------------------- |
| `effective_date` | The first day the rates on that row apply. It must be an Excel date — a date the spreadsheet shows in a date format — not text, and with no time of day. |
| `on_peak`        | The on-peak rate, in dollars per kilowatt-hour. A number greater than zero. |
| `mid_peak`       | The mid-peak rate, in dollars per kilowatt-hour. A number greater than zero. |
| `off_peak`       | The off-peak rate, in dollars per kilowatt-hour. A number greater than zero. |

Other columns are ignored, and can hold notes.

**The rows.**

- The rates start on row 2 and end at the first row with an empty `effective_date`. Nothing may follow that row in the four columns.
- The effective dates must increase down the sheet: each one later than the one above it.
- The effective dates are checked on every run. A rate is checked only when a run uses its row, so an old row with a rate missing does not stop a run that does not reach it.

**Which rows are used.** The rates in effect on a date are those on the last row whose `effective_date` is on or before that date.

- `Cost recovery` uses the rates in effect on the billing period's first day. If a row's `effective_date` falls within the period, the period is split at local midnight at the start of that date, and the rest of it is priced at that row's rates. At most one row may fall within a billing period.
- `Evolute reimbursement` uses the rates in effect on the 1st of the month. No row may fall within the month after the 1st.

An example sheet:

| effective_date | on_peak | mid_peak | off_peak |
| :------------- | ------: | -------: | -------: |
| 2026-05-01     |  0.1100 |   0.0900 |   0.0700 |
| 2026-09-01     |  0.5152 |   0.4740 |   0.4218 |

**A known limit.** Dates are read in the 1900 date system, which every current version of Excel and LibreOffice uses by default. A workbook saved in the 1904 date system, an option in old versions of Excel for Mac, would read every date four years and one day early.

## Error reporting and logging

The software checks for problems when reading the inputs. 

When an input file is ingested by the software, any problems are reported on-screen and a log file is created in the same folder as the input file. If no problems are detected when reading the input file, the log file will say so. Otherwise, the log file will contain a description of problems.

Some problems are temporary, e.g., due to an oversight by the user. Such cases may just merit an on-screen message.

Serious errors block the performance of the desired function. Less severe anomalies do not block function execution, but must still be reported on-screen and logged for the user's awareness. Some of those anomalies change the figures a function produces -- by leaving a session out of them, for instance -- while others do not impact the calculations. Certain anomalies are additionally included in the functional reports produced by the software functions.

Every message the app can show or log is described in [docs/ERRORS.md](docs/ERRORS.md), grouped by what happened to your work, with what each one means and what to do about it.

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
- [docs/ERRORS.md](docs/ERRORS.md) -- Every error and anomaly the app reports on-screen or logs: what each message means, and what to do about it.
- [docs/maintenance-manual.md](docs/maintenance-manual.md) -- What to check before changing a constant, how to regenerate the golden files, the invariants nothing enforces.
- [docs/site-specific-constants.md](docs/site-specific-constants.md) -- Constants specific to this site at the present time. Other sites using this repo's code will likely need to change some of them.
- [docs/session/Rough_kW_kVA_Table.xlsx](docs/session/Rough_kW_kVA_Table.xlsx) -- Spreadsheet comparing rough kW and kVA estimates per charger with the values resulting from the electrotechnical site model (see also [docs/session/site-model-marcus.md](docs/session/site-model-marcus.md)).
- [docs/session/README.md](docs/session/README.md) -- The estimation logic and the generated workbook layout.
- [docs/session/Google_Voltage_Fluctuations_in_Building.md](docs/session/Google_Voltage_Fluctuations_in_Building.md) -- Normal voltage fluctuations to be expected.
- [docs/session/site-model-marcus.md](docs/session/site-model-marcus.md) -- Electrotechnical model of an Evolute panel, transformer, and connected EV charging stations. Values are based on an Evolute 20-breaker panel and a Marcus AMTH75A1 transformer.
- [docs/session/site-load-report-marcus.txt](docs/session/site-load-report-marcus.txt) -- Table, generated by the software, corresponding to the site model.
- [docs/session/Google_Transformer_characteristics.md](docs/session/Google_Transformer_characteristics.md) -- Technical characteristics of the Marcus AMTH75A1 transformer.
- [docs/session/transformer-glossary.md](docs/session/transformer-glossary.md) -- Glossary of electrotechnical terms related to transformers.
- [docs/session/Evolute-Simultaneous_Charging.pdf](docs/session/Evolute-Simultaneous_Charging.pdf) -- Evolute technical documentation about simultaneous charging limits in terms of voltages, currents, kW, kVA, transformer parameters, and number of charging stations.
- [docs/green_button/README.md](docs/green_button/README.md) -- What the meter export is, when a reading is treated as an anomaly, and when a billing period counts as complete.
- [docs/green_button/Toronto_Hydro_Object_Model.md](docs/green_button/Toronto_Hydro_Object_Model.md) -- The conceptual domain model for the Green Button ESPI XML feed.
- [docs/time/README.md](docs/time/README.md) -- Date-time-related functions and constants.
- [docs/Development_Approach_and_Roles.md](docs/Development_Approach_and_Roles.md) -- How the software was developed.

## Appendix

### Software architecture direction

The software was designed with these architectural goals:

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

#### Challenge 3: Toronto Hydro billing period, Evolute reporting, time zones

A Toronto Hydro bill for the building states that it covers from the 23rd of a month to the 23rd of the following month. In practice, the bill covers the period from 00:00:00 (inclusive) EST (Eastern Standard Time, not Eastern Time) on the 24th of a month to 00:00:00 EST (exclusive) on the 24th of the following month. In other words, all of the 24th of a month (EST) to all of 23rd of the following month (EST). So, while DST (daylight saving time) impacts the TOU periods, it does not impact billing period determination. That is good, because the billing period stays stable throughout the year, regardless of DST.

Toronto Hydro Green Button metering data is reported in UTC (Coordinated Universal Time = EST + 5h, no DST).

Evolute's session reports state their times in EST as well, all year round, so they are not impacted by DST either. This was confirmed after gaining access to the Evolute portal; the software previously read them as ET and had to disambiguate the hour that repeats when DST ends.

Evolute's charges reports cover calendar months, not Toronto Hydro billing periods.

Reports produced by the application to be shown to the user are stated in prevailing local time (ET), so a summer session appears an hour later than the portal shows it — which is why every displayed time names its zone.

#### Challenge 4: Time resolution

While Toronto Hydro measures peak power using 15-minute intervals, its comprehensive Green Button metering data provides data in 1-hour intervals. So, the software has to figure out which of the four 15-minute intervals in an hour is the one that maximises a particular power value.

Evolute's Session Report provides crucial data for the calculations performed by this software. Reported session start and end times are stated to the second, and are taken at face value. They were truncated to whole minutes before the Evolute portal was available, which put an inherent 1-in-15 uncertainty on where each session fell within a 15-minute Toronto Hydro interval of interest, and every estimate was reported as a range for that reason. Only one allowance survives: a second of slack when checking a session's reported duration against its reported span, because the source rounds somewhere at second level.

#### Challenge 5: Non-linearity

The maximum power draw of one EV connected to the Evolute system is 6.589 kW and 6.656 kVA at the vehicle, which comes to 6.797 kW and 7.218 kVA measured at the transformer primary — see [docs/session/site-load-report-marcus.txt](docs/session/site-load-report-marcus.txt). The figure at the primary varies with how many cars are charging concurrently. The impact is not highly significant but the software includes an electrotechnical model that estimates the power draw more accurately.

### Module structure

The software is structured as top-level library modules, each of which may have sub-modules. Those
marked *private* are not part of the public surface; they are listed because a reader of the tree
meets them.

| Top-level module | Purpose                                                      |
| ---------------- | ------------------------------------------------------------ |
| `api`            | Functions and types that represent the majority of the software functionality. Builds on all the other modules. The binaries call primarily functions in this module, although they may also call functions in the other modules. |
| `green_button`   | Functionality related to Toronto Hydro's Green Button export, an ESPI XML feed of hourly meter readings. Notably, computes the intervals that maximise the building's kW, kVA, and 7-7 kW during a billing period. |
| `hydro_bill`     | Functionality to read the PDF invoices Toronto Hydro issues. |
| `session`        | Functionality related to the Evolute monthly CSV Session Report. Notably, computes peak load and energy consumption attributable to EV charging sessions. |
| `time`           | Date-time-related constants and functions.                   |
| `charges_report` | Functionality to read the Evolute monthly CSV Charges Report. |
| `csv`            | Common CSV reading logic.                                    |
| `error`          | Error types used by multiple other modules.                  |
| `golden` (private, test-only) | A consistent mechanism for **unit** tests to check their output against a golden file. Integration tests use `fixtures_dir_in` in `tests/common` instead, and nothing under `tests/` reaches this. |
| `log`            | Common functionality to produce read logs.                   |
| `markdown` (private) | Common functionality to produce markdown reports.        |
| `number` (private) | What a number written for a person looks like: the rule both document readers apply before stripping thousands separators. |
| `rates_workbook` (private) | Reads the [rates workbook](#the-rates-workbook) and checks its effective dates. Which rows price a period is decided in `api`. |

### Building the GUI app and command line tools

```sh
bash scripts/gen-notices.sh    # once, and again whenever Cargo.lock changes
cargo build --release          # every binary: the desktop app and the command-line tools below
cargo test --all-targets       # every test, examples included
```

The first line is a prerequisite of the second, not an optional step: a release build checks that
`THIRD-PARTY-NOTICES.md` was generated from the current `Cargo.lock`, and panics without it, so that
a release binary cannot carry a list that has fallen behind what is linked into it. Debug builds do
not need it.

The command-line tools are listed below. Each prints its usage when run with no arguments.

-  `ev_csv_to_xlsx` -- session report to workbook.
- `gb_peak_values` -- Green Button feed to workbook.
- `hydro_bill_dump` -- a bill PDF's figures.

- `peak_power_cli` -- gives the kW and kVA peaks for a billing period, estimated from a Green Button export and the session reports spanning the period.
- `energy_cli` -- gives the kilowatt-hours drawn by EV charging sessions during a billing period, split by time-of-use band.
- `energy_cost_cli` -- gives the energy-related costs attributable to EV charging sessions for a billing period.
- `peak_power_cost_cli` -- gives the peak power-related costs attributable to EV charging sessions for a billing period.
- `cost_recovery_cli` -- is the other side of the ledger: it prices the kilowatt-hours consumed by EV charging activity during a billing period, using the cost-recovery rates in the [rates workbook](#the-rates-workbook).

- `cost_recovery_surplus_cli` -- puts the two sides together: what the rates recover, less the delivery and energy costs, and the difference. A positive surplus means the rates covered the chargers' share of the bill; a negative one means they fell short.

