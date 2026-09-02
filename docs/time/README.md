# `time` module

The `time` module: everything about dates, times and zones that more than one part of this software
needs. Module-specific date arithmetic stays in its own module.

`src/time/` holds the code — `base.rs` for the zone, the grid and intervals, `dst.rs` for resolving
a wall time the zone reads twice or not at all, `excel.rs` for serial-date conversion, `tou.rs` and
`holidays.rs` for Ontario's time-of-use rules.

## What lives here and what does not

| Concern | Where |
|---|---|
| The time zone, and resolving a wall time that is ambiguous or does not exist | here |
| The standard-time clock billing periods are cut on | here |
| Excel serial dates, in both directions | here |
| Ontario time-of-use periods and the holiday calendar | here |
| Truncating an instant to a grid of a given step | here |
| `TIME_GRID_STEP`, the step session boundaries are reported to | `sessions` |
| `METER_INTERVAL`, the interval a Toronto Hydro meter records | `green_button` |

The two steps live with the module that has a reason for their value. Truncation itself is here
because both use it, but neither step is a property of time.

## Boundaries and the time grid

`TIME_GRID_STEP` and the way segments tile an interval of interest are documented in
[`docs/session/README.md`](../session/README.md), under "Boundaries and the time grid". They
belong there: the grid is the session report's reporting resolution, and only `sessions` has one.

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

The session report's timestamps are stated in local time — ET, `America/Toronto` — and every
calculation works in UTC, so each reported wall time has to be placed on the calendar. Almost every
one of them names exactly one instant. Two hours a year do not: the hour repeated when DST ends
names two, and the hour skipped when DST begins names none.

The reader settles both from the record itself, and where the record cannot settle it, says so
rather than guessing.

### One probe, three outcomes

Everything below falls out of one question, asked once per offset in `TZ_OFFSETS`: read the wall
time *as if* at that fixed offset, and check that the zone really is at that offset on the instant
you land on. The readings that survive are what the wall time names, and how many there are is the
classification.

| Readings | What it is | What the reader does |
|---:|---|---|
| 1 | Every wall time but two hours a year | Use it |
| 2 | The **fold**, 01:00:00-01:59:59 when DST ends | Settle it against `Conn_Duration`, below |
| 0 | The **gap**, 02:00:00-02:59:59 when DST begins | Assign no instant at all |

`local_readings` in `src/time/dst.rs` is the probe, and both resolvers sit on it — see "Two
resolvers, deliberately" below.

### Settling the fold

**The assumption it rests on.** `Conn_Duration` is *physical elapsed time*, so it spans the true
start and the true end of the connection. This is what makes the inference possible. Were
`Conn_Duration` instead a naive subtraction of local clock values, a session spanning the fold would
under-report by exactly the repeated hour, and the reported end could not distinguish the two
readings from each other.

**The procedure.** Take every combination of a start reading with an end reading — one or two of
each, so at most four — and keep the combinations satisfying `duration_is_consistent`, the same
three checks any other record is held to and the crate's only statement of them. All of it is
instant arithmetic in UTC; nothing compares wall times.

A mismatched combination — a start read as EDT with an end read as EST — needs no special case. It
implies a duration a whole hour out from the reported one, so the consistency test rejects it on its
own.

The number of survivors is the answer:

- **Exactly one** — that is the session. The ambiguity is resolved.
- **Two** — reachable only when *both* wall times fall in the fold. The hour then cancels on each
  side of the comparison, so the EDT/EDT and EST/EST combinations pass or fail together and the
  record cannot say which it is. This is precisely the case of a session short enough to end inside
  the repeated hour, derived rather than applied as a hardcoded 1-hour threshold. Both are kept —
  see **Duplicated records** below.
- **None** — the evidence has failed: whatever `Conn_Duration` measures on this row, it is not the
  elapsed time the inference assumes. There is nothing left to choose with, so no instant is
  assigned. The row is flagged `DstUnresolvable`, and `InconsistentDuration` with it, since a record
  agreeing with itself under no reading is inconsistent however it is read.

Note the consistency test is a *window*, not an equality, and that is what makes failing it mean
something. Both reported timestamps are truncated while `Conn_Duration` carries seconds, so on a
perfectly sound record the implied end misses the reported one — by up to but never reaching one
truncation step, in either direction. Demanding equal minutes would reject every record whose end
was truncated less than its start: roughly half of them, and 116 of the 238 rows in this project's
`data` directory. The window cannot blur the two readings together, since they lie a full hour
apart. Its derivation is in
[`docs/session/time-reporting-uncertainty.md`](../session/time-reporting-uncertainty.md).

### The gap: no instant is assigned

A wall time in the skipped hour never occurred. There is no instant to record, and shifting it to
either side of the gap would be a guess dressed as a reading — so the reader assigns none.

The session is given two sentinel timestamps, `UNPLACEABLE_START` and `UNPLACEABLE_END`, whose only
property is that they are inverted: any span built from them is impossible, and code that tries to
place such a session on a timeline gets a panic rather than a plausible answer. It is flagged
`FellInDstGap` — once, whichever end it came from — and excluded from every estimate.

The gap is settled before anything else and settles the record on its own. No fold work is done, and
no test that reads the instants runs: `duration_is_consistent` and the grid check would both be
reporting the sentinels rather than the record.

What survives is what the record actually said. The reported wall times are written to the
workbook's `Conn_DateTime_Start` and `Conn_DateTime_End` columns verbatim, from the CSV text rather
than re-derived, and the row and file name where the record is. Every column derived from the
instants is left empty, and the workbook reader puts the sentinels back from the `anomalies` column
rather than parsing those cells — a serial carries whole seconds and the sentinels do not sit on
one, so reading them back could not reproduce them.

`DstUnresolvable` reaches the same state by the other route. The two kinds are kept apart because
they say different things about *why* the record could not be placed.

### Duplicated records

**Duplicated records** are given distinct ids — `<id>-EDT` and `<id>-EST` — because the peak power
contribution logic keys `Session` on its id alone and holds sessions in a `BTreeSet`. With identical
ids the second copy would be silently discarded on insertion, defeating the purpose of duplicating
it. Note also that **both copies carry the full `Energy_Use`**, so a duplicated session contributes
to the peak in both candidate hours.

## Truncating to a grid

`truncate_to(ts, step)` rounds an instant down to the nearest multiple of `step`, counting from the
Unix epoch, and `is_on_grid(ts, step)` says whether it was already there. The property everything
else rests on is

```text
truncate_to(ts, step) <= ts < truncate_to(ts, step) + step
```

which is what makes `adj_conn_start <= real_start` true in
[`docs/session/time-reporting-uncertainty.md`](../session/time-reporting-uncertainty.md).

Truncation is always **backwards**, including before 1970. The implementation uses `rem_euclid`
rather than `%` for that reason: `%` gives a negative remainder for a negative timestamp, which
would round towards zero — forwards — and break the bound above.

## Two resolvers, deliberately

Two functions resolve an ambiguous local time, and they must not be merged. They **share the probe**
that enumerates the readings, `local_readings`, and differ in what they do with more than one of
them, because they are asked different questions:

- **`map_local`** (`time::dst`) is asked *what could this wall time mean?* by a user choosing an
  interval of interest. It has nothing but the wall time, so it reports every reading and lets the
  caller choose, or name `EST`/`EDT`. `session::ioi` is its only caller, which is why it is gated
  behind `historic`.
- **`CsvSession::resolve`** (`session::csv`) is asked *which reading was this session actually
  at?* and has evidence the other lacks: `Conn_Duration`, untruncated elapsed time. That usually
  settles it; duplication and the sentinels are the fallbacks when it does not.

Their tie-breaks differ for the same reason. Giving the first the second's behaviour would have it
invent evidence it does not have; giving the second the first's would throw evidence away.

They sit in the same module so that this warning is read where both of them are. The split of labour
is the other thing to keep: `time::dst` owns the zone arithmetic and knows nothing about sessions,
while `session::csv` owns the policy — which reading the record's own fields support, which
`AnomalyKind` to raise, and what a record gets when no reading fits.
