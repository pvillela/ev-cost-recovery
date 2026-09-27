# `time` module

The `time` module holds the date, time and zone code that more than one part of this software uses.
Date arithmetic that only one module needs stays in that module.

`src/time/` holds the code — `base.rs` for the zones and intervals, `format.rs` for rendering an
instant with its offset, `excel.rs` for Excel serial dates, `tou.rs` and `holidays.rs` for
Ontario's time-of-use rules.

## What lives here and what does not

| Concern | Where |
|---|---|
| The zones, and converting a local time to an instant or back | here |
| Rendering an instant as text, with its offset | here |
| The standard-time clock billing periods are cut on | here |
| Excel serial dates | here |
| Ontario time-of-use periods and the holiday calendar | here |
| `METER_INTERVAL`, the interval a Toronto Hydro meter records, and `is_on_grid` | `green_button` |
| `SESSION_OFFSET`, the offset Evolute's session report is stated in, and `session_instant` | `session::common` |
| Whether a session's start, end and duration agree | `session::csv` |

`METER_INTERVAL` is a fact about Toronto Hydro's meters and `SESSION_OFFSET` is a fact about
Evolute's exports, so each lives in the module that reads that source. `time` knows nothing about
either source. It provides the general parts they use: `TZ_OFFSETS`, which `SESSION_OFFSET` is
taken from, and the functions in the table above.

## UTC and two local clocks

A time in this software's inputs and outputs is stated in one of three ways:

- **UTC** — no offset and no daylight saving.
- **Standard time** — EST, a fixed UTC−5 all year.
- **Prevailing local time** — ET, `America/Toronto`, the clock a customer reads: EST (UTC−5) in
  winter and EDT (UTC−4) in summer.

Inside the software every instant is a UTC timestamp. A local clock is used only to convert between an
instant and a time on that clock.

| What | Stated in |
|---|---|
| Green Button timestamps (Unix epoch seconds) | UTC |
| The workbooks' UTC columns | UTC |
| `Conn_DateTime_Start` and `Conn_DateTime_End` in Evolute's session report | standard time |
| A Toronto Hydro billing period's boundaries, and the two dates its bill states | standard time |
| Time-of-Use periods, the 07:00–19:00 demand window, the holiday calendar | prevailing local time |
| Calendar months, and the date a rate change takes effect | prevailing local time |
| Times shown in a report | prevailing local time |
| The Green Button workbook's local-time columns | prevailing local time |

Standard time and prevailing local time agree from November to March and differ by an hour from
March to November. So a time on the wrong clock is wrong only while daylight saving is in force,
and only by an hour, which makes the mistake easy to miss.

In `time`, "local" means prevailing local time: `local_date`, `local_hour`, `local_midnight` and
`local_datetime`. The standard-time functions have "standard" in their names.

## Billing periods are cut on standard time

A Toronto Hydro **billing period** starts and ends at 00:00 EST, all year. `standard_date` and
`standard_midnight` read that clock and `BILLING_OFFSET` is its offset. Only
`hydro_bill::billing_period` calls them.

Cut on prevailing local time, a summer period would start and end an hour early, and a period that
contains a clock change would be an hour too long or too short. Cut that way, the periods reproduce
6 of 19 invoices; cut on standard time, they reproduce all 19 to the milli-kWh. The on-peak and
mid-peak energy on the bills is reproduced only with the Time-of-Use periods on prevailing local
time. The analysis is in
[`../archive/hydro_bill/dst-energy-anomaly-pre-fix.md`](../archive/hydro_bill/dst-energy-anomaly-pre-fix.md).

A standard-time day is always 24 hours, so a billing period is always a whole number of days and
matches the `Number of Days` on its invoice.

The Green Button feed's timestamps are UTC. Its `IntervalBlock`s each hold one day and start at
05:00 UTC all year, which is 00:00 EST, so the feed's days line up with billing-period days. See
`../green_button/Toronto_Hydro_Object_Model.md`, "Fixed daily grid".

### What the bill's two dates mean

A bill states its `Meter Reading Period` as two dates, for example `MAY 23 2026 TO JUN 23 2026`.
**Those dates are in EST, and they name the two meter readings that bound the period, not the days
it covers.** Read as days, `FROM` is excluded and `TO` is included: the period covers all of June
23rd and none of May 23rd.

| Clock | Season | Days covered |
| :--- | :--- | :--- |
| EST | all year | 24 May 00:00:00 → 23 Jun 23:59:59 — the 24th to the 23rd |
| EDT | summer | 24 May 01:00 → 24 Jun 00:59:59 — part of the 24th to part of the 24th |

The bill does not say which endpoint is included. It gives the two dates and a `Number of Days` of
`31`: counting both dates gives 32 and counting neither gives 30, so exactly one is included. The
19-invoice reconciliation in the analysis linked above shows that it is `TO`.

The code does not read these dates. `BillingPeriod` works in instants. The meaning of the dates
matters only when someone compares an invoice with the code.

## Evolute's session report is stated on standard time

Evolute's `Conn_DateTime_Start` and `Conn_DateTime_End` are on standard time all year.
`session::common::SESSION_OFFSET` is that offset, and `session::common::session_instant` converts a
reported time to a UTC instant. Because the offset is fixed, no hour is repeated or skipped at a
clock change, and every reported time names exactly one instant.

`SESSION_OFFSET` and `BILLING_OFFSET` have the same value for unrelated reasons. They are separate
constants so that either can change without the other.

## What a person sees

**A time shown in a report is on prevailing local time and names its offset**:
`2026-08-30 17:57 EDT`, or `18:40 EDT` when the date is not shown. `time::format` renders these.

The offset label matters most for sessions. While daylight saving is in force, the Peak power detail
report shows a session an hour later than Evolute's session report states it: a session Evolute
states at `16:57` shows as `17:57 EDT`. Both name the same instant, and the `EDT` label shows that.

One Peak power detail report can show both labels. It has three intervals of interest — the billing
period's kW, 7-7 kW and kVA peaks — and they can fall on opposite sides of a clock change. Their
`Interval` lines then show different offsets.

The one exception is segment names in the Peak power detail report. They are bare local clock times
such as `16:00`, with no date and no offset. Every segment falls inside its interval of interest,
and that interval's `Interval` line names the offset.

### The workbooks

Excel date/time numbers carry no zone. Each date/time column is stated as follows:

- **Session workbook.** `Conn_DateTime_Start` and `Conn_DateTime_End` are Evolute's reported times:
  standard time. `conn_start_utc` and `conn_end_utc` are UTC.
- **Green Button workbook.** The columns headed `(local time)`, and the interval sheet's `interval`
  column, are prevailing local time. The columns headed `(UTC)`, and `interval_utc`, are UTC.
