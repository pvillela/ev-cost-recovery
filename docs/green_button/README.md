# `green_button` module

This module contains functionality related to Toronto Hydro's Green Button export, an ESPI XML feed of hourly meter readings. Notably, it computes the intervals that maximise the building's kW, kVA, and 7-7 kW during a billing period.

This document provides an overview of what the meter export is, when a reading is treated as an anomaly, and when a billing period counts as complete.

**See also:**

- [Notes_on_Green_Button_data.md](Notes_on_Green_Button_data.md) — two facts about the data that everything here rests on.
- [Toronto_Hydro_Object_Model.md](Toronto_Hydro_Object_Model.md) — the ESPI feed's own structure, and how a reading is reached through it.
- [docs/ERRORS.md](../ERRORS.md) -- contains the error messages associated with this module's functionality and where they appear.

## What a reading is

The export carries three series — kWh, kW and kVA — each timestamped in UTC on a one-hour grid. The
kW and kVA figures are not hourly averages: each is the highest 15-minute interval within its hour.

Timestamps are absolute instants, so DST costs nothing here. The clocks going forward or back is a
question for the local-time column a generated workbook renders, and not for the readings themselves. That is why there is no DST anomaly in this vocabulary, where the session side has three.

An hour is **aligned** when it starts on a whole hour. Only aligned hours can be a reported peak. Toronto Hydro's price-period boundaries all fall on the hour, and their UTC offsets are whole hours in both seasons.

## Anomalies

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

### The tokens are a wire format

`Anomaly::as_str` is what a generated workbook cell holds, and what is read back from one. Renaming a variant silently makes every workbook already written deviate from the new naming.

## When a period is complete

A billing period is complete when the number of hourly readings inside it equals the number of hours
between its boundaries. The expected count is derived from the boundary instants rather than stated,
so it stays correct however the boundary is defined — and it is why the periods ending 2026-03-23
and 2025-11-23 hold 672 and 744 hours, matching the day counts their invoices state, rather than the
671 and 745 a prevailing-local-midnight boundary produced.

Two different things follow from an incomplete period, and they are not the same finding:

- **On the Convert tab** it is [worth knowing and nothing more](../ERRORS.md#worth-knowing). An
  export starts and stops where it starts and stops, so the first and last periods it reaches are
  ordinarily partial. The workbook marks them in red on `nbr_of_intervals` so a reader does not
  take their maxima for a whole period's.
- **When a period is priced** it stops the run. Peak demand is the highest reading in the period,
  and a period missing hours may be missing the highest one, so the figure is refused rather than
  estimated. See
  [the meter data covers only part of the period](../ERRORS.md#the-meter-data-covers-only-part-of-the-period).

A period the export does not reach at all is refused when it is read, rather than returned as a row
of zeroes: zeroes would read as a month of no consumption, which is a figure someone could go on to
argue a bill from.
