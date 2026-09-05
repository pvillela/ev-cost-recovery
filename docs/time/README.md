# `time` module

The `time` module: everything about dates, times and zones that more than one part of this software
needs. Module-specific date arithmetic stays in its own module.

`src/time/` holds the code — `base.rs` for the zones and intervals, `format.rs` for rendering an
instant with the zone it is read in, `excel.rs` for serial-date conversion, `tou.rs` and
`holidays.rs` for Ontario's time-of-use rules.

## What lives here and what does not

| Concern | Where |
|---|---|
| The zones, and converting a wall time to an instant or back | here |
| Rendering an instant for a person, with the zone it is read in | here |
| The standard-time clock billing periods are cut on | here |
| Excel serial dates, in both directions | here |
| Ontario time-of-use periods and the holiday calendar | here |
| `METER_INTERVAL`, the interval a Toronto Hydro meter records, and `is_on_grid` | `green_button` |

The meter interval and the predicate that tests against it live together, in the module with a
reason for the value. The session reader had a grid of its own — the resolution its timestamps were
reported at — until the portal confirmed they are stated to the second.

## Two clocks, and which is which

Almost everything here means **prevailing local time** — the clock a customer reads, which moves
twice a year. `local_date`, `local_hour` and `local_midnight` are that clock, and Time-of-Use
periods, the 07:00–19:00 demand window and the holiday calendar all run on it.

One thing does not. A Toronto Hydro **billing period** is cut on **standard time**, at 00:00 EST all
year round, and does not move when the clocks do. `standard_date` and `standard_midnight` are that
clock, `BILLING_OFFSET` is the offset, and `hydro_bill::BillingPeriod` is the only caller.

The two coincide from November to March and differ by an hour from March to November, which is what
makes the distinction easy to lose and expensive to get wrong: a summer period cut on the wrong
clock is an hour out at each end, and a period containing a clock change is an hour out overall.
Cutting on prevailing local time reproduced 6 of 19 invoices; cutting on standard time reproduces
all 19 to the milli-kWh. The derivation is in
[`../archive/hydro_bill/dst-energy-anomaly-pre-fix.md`](../archive/hydro_bill/dst-energy-anomaly-pre-fix.md).

Two consequences worth knowing:

- A standard-time day is always 24 hours, so a billing period is always a whole number of days and
  matches the `Number of Days` its invoice states. Periods of 671 and 745 hours were what the
  prevailing-local boundary produced, and they are gone.
- `standard_midnight` cannot fail, where `local_midnight` can in principle: a fixed offset has no
  gap for a wall time to fall into and no fold for it to be ambiguous in.

The Green Button feed itself keeps the same standard-time day — its `IntervalBlock`s start at 05:00
UTC year-round. See `../green_button/Toronto_Hydro_Object_Model.md`, "Fixed daily grid".

### What the bill's two dates mean

A bill states its `Meter Reading Period` as two dates — `MAY 23 2026 TO JUN 23 2026`. **Those dates
are in EST, and they name the two meter readings that bound the period, not the days it covers.**
Read as days, `FROM` is exclusive and `TO` is inclusive: the period covers all of June 23rd and none
of May 23rd.

The label is easy to misread as "the 23rd to the 23rd", which no clock makes true:

| Clock | Season | Days actually covered |
| :--- | :--- | :--- |
| EST | either | 24 May 00:00:00 → 23 Jun 23:59:59 — the 24th to the 23rd |
| EDT | summer | 24 May 01:00 → 24 Jun 00:59:59 — part of the 24th to part of the 24th |

This is inferred rather than stated. The bill gives only the two dates and a `Number of Days` of
`31`; counting both dates would give 32 and counting neither 30, so exactly one endpoint is
included. Which one is settled by the reconciliation of 19 invoices with Green Button data, in
[`../archive/hydro_bill/dst-energy-anomaly-pre-fix.md`](../archive/hydro_bill/dst-energy-anomaly-pre-fix.md).

None of the arithmetic depends on the label. `BillingPeriod` works in instants and never parses it;
the reading matters only when someone compares an invoice to the code and has to decide whether the
two agree.

## Time zone

Two clocks are in play, and keeping them apart is the whole of this section.

**The session report is stated on standard time, all year.** Evolute's `Conn_DateTime_Start` and
`Conn_DateTime_End` do not observe daylight saving. `time::SESSION_OFFSET` names that offset and
`time::session_instant` does the conversion, which cannot fail: a fixed offset has no hour that
occurs twice and none that is skipped, so every reported wall time names exactly one instant. There
is nothing for the reader to infer, and no anomaly it can raise about placing a record.

**Everything shown to a person is on prevailing local time** — ET, `America/Toronto`, the clock a
customer reads. Time-of-Use periods, the 07:00-19:00 demand window and the holiday calendar are all
stated on it, and so is every rendered time.

The consequence is that a session displays an hour later than the portal states it, right through
the summer. A row the portal shows at `16:57` appears in a report as `17:57 EDT`. That is not a
discrepancy — it is one instant on two clocks — but nothing on the page would say so, which is why
every rendered time names its offset. `time::format` does that and nothing else does; see its own
docs.

One report can carry both labels. The kW and kVA peaks of a billing period can fall on opposite
sides of a transition, and then two headings in the same document differ by an hour of offset.

### Where the two meet

The workbook could show both clocks in one row, so it shows only one. Its `Conn_DateTime_Start` and
`Conn_DateTime_End` are the CSV's own text, copied verbatim, and it derives no local column of its
own — the padded `adj_conn_*` pair it used to carry is gone with the padding. Its UTC columns are
instants. So nothing in the sheet is on prevailing time, and a workbook that carries no zone labels
stays honest.

**Before the portal.** Until the offset was confirmed, the reader read session times as prevailing
local and had to resolve the two hours a year that are ambiguous or absent: it enumerated readings
at each offset, settled the fold against `Conn_Duration`, duplicated a record no reading could
choose between, and assigned sentinel timestamps where a wall time named nothing. All of it is gone,
along with the four anomaly kinds it raised. The history is in
[`docs/archive/dst-gap-plan.md`](../archive/dst-gap-plan.md).

## Where the labour divides

`time` owns the zone arithmetic and knows nothing about sessions: `session_instant` converts a
reported wall time to an instant, and `format` renders one for a reader. `session::csv` owns the
policy — whether a record's own three fields agree, and which `AnomalyKind` to raise when they do
not.
