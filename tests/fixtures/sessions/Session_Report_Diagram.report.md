EV Peak Power Contribution
==========================

Source     Session_Report_Diagram.csv
Interval   2026-06-15 17:00 - 18:00 EDT  (1 hour)


Estimates
---------

| Estimate     | Unit |  Value | Segment |
|:-------------|:-----|-------:|:--------|
| Energy-based | kW   | 17.751 | 17:15   |
| Energy-based | kVA  | 18.218 | 17:15   |
| Count-based  | kW   | 20.500 | 17:15   |
| Count-based  | kVA  | 20.996 | 17:15   |

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


Segments
--------

| Segment | Count-based (EVs) | Energy-based (kW) |
|:--------|------------------:|------------------:|
| 17:00   |             2.467 |            14.553 |
| 17:15   |             3.067 |            17.482 |
| 17:30   |             2.867 |            16.165 |
| 17:45   |             1.467 |             8.440 |

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

- 17:00 - A, B, C
- 17:15 - A, C, D, E
- 17:30 - A, C, D, E, F
- 17:45 - A, G


Anomalies
---------

None. Every session considered for this interval was well formed.

