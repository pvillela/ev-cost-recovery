# Constants to revisit when another building uses this crate

What a second site has to change. The tariff numbers are not on this list: rates, charges and the
loss factor are read out of the bill PDF at run time, never declared. What *is* declared is the
electrical installation, the jurisdiction's pricing rules, and the layout of the input files.

Grouped by how likely a change is, not by module.

## Almost certain to change: the electrical installation

`src/session/site_model.rs`. These are the "free constants" of
[`maintenance-manual.md`](maintenance-manual.md), "Which constants are free, and which are derived"
— declared outright, and the ones to edit to describe a different installation.

| Constant | Here | Meaning |
|---|---|---|
| `PANEL_VOLTAGE_V` | 208.0 | Secondary line-to-line voltage |
| `NORMAL_VOLTAGE_FLUCTUATION_FACTOR` | 0.05 | Normal supply voltage band, either way (ANSI C84.1 Range A) |
| `BREAKER_RATING_A` | 40.0 | Rating of each EVSE branch breaker |
| `CONTINUOUS_DUTY_DERATE` | 0.80 | Continuous-load derating, CEC Rule 8-104 |
| `PANEL_COUNT` | 1 | Installed panels, each on a transformer of its own |
| `PANEL_BREAKER_COUNT` | 10 | EVSE breakers in one panel |
| `EV_TRUE_POWER_FACTOR` | 0.99 | Onboard charger's true power factor at full current |
| `EV_CURRENT_THD` | 0.045 | Onboard charger's input current distortion |
| `XFMR_RATING_KVA` | 75.0 | Transformer nameplate |
| `XFMR_NO_LOAD_LOSS_KW` | 0.197 | Core loss, constant whenever energised |
| `XFMR_FULL_LOAD_LOSS_KW` | 1.293 | Copper loss at rated load |
| `XFMR_MAGNETIZING_PU` | 0.02 | Magnetizing current, per unit of rating |
| `XFMR_REACTANCE_PU` | 0.0383 | Leakage reactance, per unit of rating |

The four transformer figures are the installed Marcus AMTH75A1's. Two are datasheet values, one is
derived from its nameplate impedance, and the magnetizing current is a typical figure for the
class; [`session/site-model-marcus.md`](session/site-model-marcus.md) §3 shows each derivation, and
a different unit means going back to its own datasheet rather than scaling these.

`PANEL_COUNT` multiplies the whole installation, transformer included. A site whose second panel
hangs off the *same* transformer is not `PANEL_COUNT = 2`; it needs a different model.

Do not edit the derived values to a literal — `ev_pilot_current_a()`, `ev_apparent_power_kva()`,
`ev_real_power_kw()`, `single_panel_load()`, and `BREAKER_RATING_KW` and `BREAKER_MAX_NORMAL_KW` in
`src/session/common.rs`. Edit the constant behind them. The manual lists them in full.

## Likely to change: a different utility or province

- `src/hydro_bill/billing_period.rs:70` — `BILL_END_DAY = 23`, the day of the month a period closes
  on. Changing it moves every billing period boundary in the crate. `MAX_BILL_END_DAY = 27` bounds
  what the type will accept.
- `src/time/base.rs:16` — `TIME_ZONE_NAME = "America/Toronto"`, and `TZ_OFFSETS`, which names the
  two offsets `EST` and `EDT` as a bill reader would recognise them.
- `src/time/base.rs:88` — `BILLING_OFFSET`, the standard-time entry of `TZ_OFFSETS`. Toronto Hydro
  cuts a billing period on standard time all year round; another utility may not.
- `src/time/tou.rs` — `SUMMER_WEEKDAY` and `WINTER_WEEKDAY`, the 07:00 / 11:00 / 17:00 / 19:00
  boundaries, and the May–October summer window. Ontario Energy Board policy, quoted in the module
  header. A utility whose demand window is not the complement of weekday off-peak also splits
  `is_off_peak`, which currently serves both questions.
- `src/time/holidays.rs` — the OEB Time-of-Use holiday calendar, deliberately not the Employment
  Standards Act list. The August Civic Holiday is on it; leaving it out reclassifies a summer
  weekday's 07:00–19:00 block.
- `src/green_button/common.rs:23` — `METER_INTERVAL`, one hour. Checked against the feed rather than
  assumed, so a 15-minute export fails in `parse_espi_xml` rather than reading wrongly.
- `src/hydro_bill/bill.rs` — the *set* of `HydroBill` fields, not a constant but the same question. A
  utility with different line items needs new fields, and `bill_pdf.rs` refuses a charge line it
  does not recognise rather than dropping it from a total.

## Likely to change: a different vendor or file layout

- `src/hydro_bill/bill_pdf.rs:41` — `CHARGE_COLUMN_RIGHT = 360.0`, in PDF points, the vertical cut
  that removes the promotional column. Tied to where this issuer prints things.
- `src/hydro_bill/bill_pdf.rs` — every charge label matched there, and `MONTHS`, the month
  abbreviations as the bill spells them.
- `src/hydro_bill/pdf_text.rs:37` — `ROW_TOLERANCE = 1.5`, how far off a shared baseline a label and
  its value may sit.
- `src/session/csv.rs:54` — `REQUIRED_HEADERS`, the charger vendor's session-export columns.
- `src/charges_report.rs:27` — `REQUIRED_HEADERS`, and `DATE_FORMAT = "%d-%b-%y"` on line 33.
- `src/session/excel.rs:418` — `SESSION_REPORT_PREFIX`, and `src/session/file_name.rs`, which reads
  `Session_Report_June_1_2026-June_30_2026.csv` with English month names spelled in full.
- `src/green_button/espi.rs:52` — the unit-of-measure codes 72, 38 and 61 for kWh, kW and kVA.
  Standard ESPI, but a feed that omits kVA changes what can be reported.

## Probably fine as they are

Properties of the reporting grid rather than of the site:

- `src/session/common.rs:46` — `TIME_GRID_STEP`, 60 seconds, the resolution session timestamps are
  truncated to.
- `src/session/common.rs:59` — `SEGMENT_DURATION`, 15 minutes.
- `src/session/ioi.rs:31` — `LEGAL_START_MINUTES`.
- Spreadsheet cosmetics in `src/green_button/excel.rs` and `src/session/excel.rs`: fonts, column
  widths, number formats.

## What editing these costs

A rule already in force: **no test may depend on the numeric value of any freely-declared
constant.** Relationships may be relied on; values may not. See
[`maintenance-manual.md`](maintenance-manual.md), "The rule the tests are written to".

Expect fixture churn anyway. A CSV fixture states `Energy_Use` and `Active_Charge_Time` as fixed
text, so whether the average power they imply clears `BREAKER_MAX_NORMAL_KW` depends on
`BREAKER_RATING_A` and `NORMAL_VOLTAGE_FLUCTUATION_FACTOR`. Lower the breaker rating and every
fixture session picks up an `ExcessiveAvgKw` flag; widen the voltage band and the sessions built to
carry that flag lose it. The manual works through that exact experiment at 40 A → 32 A and names
the tests it moves.
