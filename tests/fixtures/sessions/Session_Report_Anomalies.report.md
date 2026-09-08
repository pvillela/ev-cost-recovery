EV Peak Power Contribution
==========================

Source     Session_Report_Anomalies.csv
Interval   2026-06-15 17:00 - 18:00 EDT  (1 hour)


Estimates
---------

| Estimate     | Unit |  Value | Segment |
|:-------------|:-----|-------:|:--------|
| Energy-based | kW   | 19.080 | 17:15   |
| Energy-based | kVA  | 19.560 | 17:15   |
| Count-based  | kW   | 17.841 | 17:15   |
| Count-based  | kVA  | 18.309 | 17:15   |

"Energy-based" is derived from the sessions' own consumption, "Count-based"
from how many of them were charging and the per-EV rating of the
infrastructure. "Segment" names the 15-minute segment the figure was drawn
from - the one where that derivation peaks, which the two need not agree on.
Each figure is a single value: the reported session times are stated to the
second and taken as given, so an overlap has one width. They were a range
while those times were stated only to the minute.

The peak is always a 15-minute average, whatever the length of the interval
asked for, because that is the basis the demand charge is billed on. An hour
is reported as the highest of its four segments, not as an average over the
whole hour.

2 sessions in the source report were excluded from every figure above,
having reported times that cannot be placed on a timeline. They are listed
under Excluded sessions.


Segments
--------

| Segment | Count-based (EVs) | Energy-based (kW) |
|:--------|------------------:|------------------:|
| 17:00   |             0.333 |             2.000 |
| 17:15   |             2.667 |            18.800 |
| 17:30   |             1.000 |             6.000 |
| 17:45   |             0.000 |             0.000 |

The two columns are what the estimates above are computed from: each
estimate is the site load implied by the column of its own name, taken from
the segment where that column peaks. "Count-based" is how much of the
segment was occupied - each session contributes the fraction of the segment
its connection covers, so one connected throughout adds 1 and one connected
for half of it adds 0.5, which makes the column a fractional count of
vehicles. "Energy-based" is the average power over the segment: each
session's reported energy is spread evenly over its own connection span, the
part of it falling in this segment is taken, and the sum is divided by the
segment's length.

The two weight a session differently, and neither can be read off the other.
The count divides a session's overlap with the segment by the segment's
length, which is what makes it a fraction of the segment; the energy divides
that same overlap by the session's own length, which is what makes it that
session's share of its own energy. A short heavy session and a long light
one can therefore rank differently in the two columns.

Times are local, on the zone the Interval line above names, and each segment
is 15 minutes long, named by the minute it starts on. That is an hour later
than the session report states the same instants, which are on standard time
all year. Segments are half-open: each runs from its own start up to but not
including the next one's, so no instant falls in two of them and they tile
the interval exactly.


Sessions by segment
-------------------

- 17:00 - N1
- 17:15 - N1, N2, EXCESS, SPIKE
- 17:30 - N1, N2
- 17:45 - none


Excluded sessions
-----------------

| Row | Session  | From                 | To        | In interval | Anomaly              |
|----:|:---------|:---------------------|:----------|:------------|:---------------------|
|   4 | BAD      | 2026-06-15 17:05 EDT | 17:30 EDT | yes         | InconsistentDuration |
|   8 | REVERSED | 2026-06-15 17:30 EDT | 17:20 EDT | yes         | InconsistentDuration |

These sessions take no part in any estimate. Times are local and name the
zone they are read in, which through the summer is an hour later than the
session report states them; the report is on standard time all year. The
list covers the whole source report rather than the interval estimated, so
"From" carries its date and "To" carries one only when the session crosses
midnight. "In interval" is whether the session appears to fall in the
interval - appears only, because a record whose own fields contradict each
other cannot be trusted to say where it belongs. It reads the same doubtful
times, so no row was dropped on its say-so.

- InconsistentDuration - reported start, end and duration contradict each
  other by more than a second, which is the rounding the source does; the
  session is excluded from every estimate.


Anomalies
---------

| Row | Session | Anomaly               |
|----:|:--------|:----------------------|
|   6 | SPIKE   | ZeroActiveChargeTime  |
|   9 | EXCESS  | ExcessiveAvgKw(7.200) |

Row numbers are rows of the source data file named above, so each one can be
looked up directly. Only sessions reaching the interval of interest are
listed here. The Excluded sessions table above is scoped differently - it
covers the whole source report, and carries an "In interval" column for that
reason.

- ZeroActiveChargeTime - zero Active_Charge_Time, so the session delivered
  its energy in no time at all and has no finite average power; the
  estimating logic substitutes one, and the session is worth reviewing
  individually.
- ExcessiveAvgKw - average kilowatts above the Evolute breaker rating at the
  top of the normal voltage band, which the hardware should not allow; the
  session still counts towards every estimate.

