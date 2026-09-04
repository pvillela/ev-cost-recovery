# `green_button` module

This module contains functionality to:
- Read downloaded Toronto Hydro Green Button XML files of hourly meter readings.
- Identify the intervals that maximise the building's kW, kVA, and 7-7 kW during a billing period.
- Compute total kWh consumed during a billing period (to cross-check with a bill).
- Convert the downloaded XML data to an Excel workbook.

## The data

### The feed

A file downloaded from Toronto Hydro's [Green Button](https://www.torontohydro.com/for-home/green-button) site is a standard [ESPI (Energy Services Provider Interface)](https://www.naesb.org/espi_standards.asp) XML feed of hourly meter readings. The download spans a date range specified by the user.

### What a reading is

The export carries three series — kWh, kW and kVA — each timestamped in UTC on a one-hour grid. The
kW and kVA figures are not hourly averages: each is the highest 15-minute interval within its hour.

Timestamps are UTC instants, so DST has no impact here. The clocks going forward or back is a
question for the local-time column a generated workbook renders, and not for the readings themselves.

An hour is **aligned** when it starts on a whole hour. All intervals in the feed are expected to be aligned. Only aligned hours can be a reported peak. Toronto Hydro's price-period boundaries all fall on the hour, and their UTC offsets are whole hours in both seasons.

### See also

- [Toronto_Hydro_Object_Model.md](Toronto_Hydro_Object_Model.md) — the ESPI feed's structure, and how a reading is reached through it.

## Billing periods

As much of this module's functionality involves billing periods, it is important to characterize billing periods precisely.

A billing period spans 00:00:00 EST (inclusive) on the 24th of a month to 00:00:00 EST (exclusive) on the 23rd of the following month. Notice that billing periods are always defined in terms of EST (Eastern **Standard** Time), which does not change with DST. 

A data feed contains readings for a full billing period when the number of hourly readings inside it equals the number of hours between the billing period boundaries.

## Anomalies

(See also [docs/ERRORS.md](../ERRORS.md) -- contains the error messages associated with this module's functionality and where they appear.)

None of these is fatal. A generated workbook is still written and the figures are still produced; an anomaly
says a reading needed review, not that the run failed.

Each is recorded three ways: counted in the run log, listed on the Convert tab as `<token> x<count>`,
and highlighted in the generated workbook against the reading it concerns.

| Token | What it says |
| --- | --- |
| `MissingKwh` | The hour carried a kW or kVA reading but no kWh. |
| `MissingKw` | The hour carried a kWh or kVA reading but no kW. |
| `MissingKva` | The hour carried a kWh or kW reading but no kVA. |
| `MissingInterval` | No series carried this hour, though the hours around it imply it should exist. |
| `DuplicateInterval` | The same interval start appeared more than once within one series. |
| `MisalignedInterval` | The hour does not start on a whole hour, so it is left out of peak selection and can never be a reported maximum. |
| `ImplausibleGap` | The hole before this hour was too large to be an outage, so it was left unfilled rather than expanded into placeholder rows. |

The three `Missing…` kinds are about a reading present in some series and absent from others.
`MissingInterval` is about an hour absent from all three.

### Why an implausible gap is not filled

A gap in the readings is ordinarily made visible by writing one empty row per missing hour, which
is what puts a power cut in front of a reader rather than leaving the rows either side of it looking
adjacent. That is only sane while the hole is a plausible one. A single corrupt `<espi:start>` can
place a reading thousands of years away, and filling to it would mean millions of rows — an
out-of-memory kill or an apparent hang, neither of which tells anyone what is wrong. Past a
plausible size the gap is recorded and left as a gap.

### Incomplete periods

A billing period is incomplete when the data feed does **not** contains readings for the full billing period.

Two different things follow from an incomplete period, and they are not the same finding:

- **When a period is priced** it stops the run. Peak demand is the highest reading in the period,
  and a period missing hours may be missing the highest one, so the figure is refused rather than
  estimated. See
  [the meter data covers only part of the period](../ERRORS.md#the-meter-data-covers-only-part-of-the-period).
- **On the Convert tab** it is [worth knowing and nothing more](../ERRORS.md#worth-knowing). An
  export starts and stops where it starts and stops, so the first and last periods it reaches may
  be incomplete. The workbook marks them in red on `nbr_of_intervals` so a reader does not
  take their maxima for a whole period's.

### The tokens are a wire format

`Anomaly::as_str` is what a generated workbook cell holds, and what is read back from one. Renaming a variant silently makes every workbook already written deviate from the new naming.

