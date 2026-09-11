EV Peak kW Contribution
=======================

Source: Session_Report_Anomalies.csv
Interval: 2026-06-15 17:00 - 18:00 EDT  (1 hour)


Estimates
---------

| Estimate     | Unit | All-in power | Segment |
|:-------------|:-----|-------------:|:--------|
| Energy-based | kW   |     * 19.080 | 17:15   |
| Energy-based | kVA  |       19.560 | 17:15   |
| Count-based  | kW   |       17.841 | 17:15   |
| Count-based  | kVA  |       18.309 | 17:15   |

"*" - Portion of building's peak kW attributed to EV charging.

2 sessions in the source report were excluded from every figure above,
having reported times that cannot be placed on a timeline. They are listed
under Excluded sessions.


Segments
--------

| Segment | Session count | Session kW |
|:--------|--------------:|-----------:|
| 17:00   |         0.333 |      2.000 |
| 17:15   |         2.667 |     18.800 |
| 17:30   |         1.000 |      6.000 |
| 17:45   |         0.000 |      0.000 |


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

