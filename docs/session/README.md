# `session` module

This module contains functionality related to the Evolute monthly CSV Session Report. Notably, it computes peak load and energy consumption attributable to EV charging sessions.

The rest of this document describes the computation of peak loads.

## Peak power estimation

### Data sources and intervals of interest

For a given billing period, we can identify the time intervals in which the highest kW, 7-7 kW, and kVA occurred based on the Green Button metering data downloads from Toronto Hydro. The intervals of interest are made available by `green_button` module functionality.

Given a time interval of interest, this module estimates the peak kW and kVA demand associated with EV charging activity during the interval. If the interval of interest is the one where the building's Demand kW or 7-7 kW was highest, then the kW attributable to EV charging activity is the value of interest. If the interval of interest is the one where the building's Demand kVA was highest, then the kVA attributable to EV charging activity is the value of interest.

The data source for EV power demand is the Evolute monthly session report.

#### Interval of interest boundaries

They are constrained as follows:

- The left and right end-points are always of the form HH:00:00 or HH:15:00 or HH:30:00 or HH:45:00.
- The difference between the right end-point and the left end-point can be either:
  - 1 hour -- only if the left end-point is of the form HH:00:00.
  - 15 minutes -- in all four cases.
- The interval is half-open: it includes the left end-point and excludes the right end-point.

### Estimation logic

#### Estimation algorithm overview

Given a time interval of interest **`I`** as described above, the estimation of EV peak power demand during the interval proceeds as follows:

- From the Evolute monthly session report, identify all charging sessions that intersect the interval of interest `I`.
- Partition `I` into 15-minute segments. If `I` is 1-hour long, there will be four segments. If `I` is 15-minutes long, there will only be one segment.
  - The reported peak is therefore **always a 15-minute average**, whatever length of interval was asked for: an hour is reported as the highest of its four segments, never as an average over the whole hour. This is the basis the demand charge is billed on, and it is why the estimates name the segment they came from.
- For each segment:
  - Identify the charging sessions that intersect the segment.
  - For each session:
    - Compute the average power drawn by the session by dividing its energy consumed by the charge time in hours to obtain `avg_kw`.
    - Compute the overlap ratio `overlap_ratio` of the session over the segment's duration.
    - `avg_kw * overlap_ratio` is the session's contribution to the segment's aggregate kW and `overlap_ratio` is session's contribution to the segment's aggregate session count.

  - Compute the segment's aggregate kW `agg_kw` and `agg_count` by summing the above-described per-session contributions over all sessions.

  - From these two key values, compute the following ones:

    - **`energy_based_kw`**: `agg_kw`.

    - **`energy_based_kva`**: `agg_kw` divided by a power factor that reflects the combination of typical EV chargers and the Evolute infrastructure (~0.98). *(Approximate. The software does not divide by a power factor at all — kVA is a quadrature sum, and ~0.98 is a good figure only near full occupancy. See [kW and kVA calculations](#kw-and-kva-calculations).)*

    - **`count_based_kw`**: `agg_count` multiplied by the average per-EV kW rating of the Evolute infrastructure (~6.7 kW). *(Approximate. The per-EV figure is an average, not a constant; it falls as the site fills. See [kW and kVA calculations](#kw-and-kva-calculations).)*

    - **`count_based_kva`**: `count_based_kw` divided by a power factor that reflects the combination of typical EV chargers and the Evolute infrastructure (~0.98). *(Approximate, for the same reason as `energy_based_kva`.)*


- Identify the one or two *maximal* segments, i.e., segments that have the highest:

  - **`energy_based_kw`**: `agg_kw`.

  - **`count_based_kw`**: `agg_count` multiplied by the average per-EV kW rating of the Evolute infrastructure (~6.7 kW). *(Approximate; see [kW and kVA calculations](#kw-and-kva-calculations).)*
- The identified maximal segments are typically one and the same, but may be distinct in some situations.
- Report on the maximal segment(s).

- The software detects data anomalies in the reported session data. Anomalies associated with every session that **intersects `I`** are reported alongside the estimates, as well as anomalies that caused sessions to be excluded from the analysis. Other sessions elsewhere in the session data are not included in the report.
  - The two listings are scoped differently. The Anomalies table holds sessions reaching `I` and nothing else. The Excluded sessions table covers all the session data, so it carries an `In interval` column saying whether each record *appears* to fall in `I` — see [Anomalies](#anomalies).

#### Sessions and segments

Sessions, segments, and intervals of interest are all **half-open**: each includes its left end-point and excludes its right one. Consecutive segments therefore meet at a single instant belonging to the later one, so no instant falls in two segments, and *abutting* stays distinguishable from *overlapping* — a distinction the estimates count on. See [Half-open boundaries](#half-open-boundaries).

A session occupies `[Conn_DateTime_Start, Conn_DateTime_End)`, exactly as the report states them. The portal states both to the second, so there is nothing to adjust and every overlap has one width.

That was not always so. Reports used to state start and end times truncated to the minute, and the software padded each session's end out to the following minute to bound where the true end might have been. The consequence was that every figure came out as a range rather than a number: where one session was reported to end in the minute another was reported to start, the reported times could not say whether the two overlapped or merely abutted. The padding, and the brackets it forced, are gone.

#### Interval of interest with no EVs charging

In such cases, the EV charging infrastructure still impacts the overall building's peak kW and kVA, but the impact is small (currently ~ 0.20 kW and ~1.51 kVA for the transformer), and the software reports these values.

### Technical Notes

#### Half-open boundaries

Half-open is what makes segments properly cover all of the interval of interest without overlaps between them: consecutive segments meet at a single instant that belongs to the later one, so no instant falls in two segments. Closed intervals (i.e., the end is included) cannot do this — adjacent segments would either share an instant, and so disagree about which sessions were active at it, or leave a one-tick gap. It is also what makes *abutting* distinguishable from *overlapping*, which is significant for the estimates.

It applies to sessions too, and it is what settles the shared-instant case. A session reported to end at `16:34:00` and one reported to start at `16:34:00` abut: that instant belongs to the second alone, so neither counts it twice and the two do not overlap.

The interval of interest must still be a whole number of `SEGMENT_DURATION`s — 15 minutes — or the segments cannot partition it. `estimates_from_sessions` asserts that rather than rounding.

#### kW and kVA calculations

The two formulas above — a per-EV kW rating and a division by a power factor — are a fair
description of the *shape* of the estimates, and a defensible approximation of their values. They
are not what the software computes. Both figures come out of a small electrical model of the site,
described in [Site Model — Level 2 EV Chargers on a Marcus AMTH75A1 75 kVA 600–208 V Transformer](site-model-marcus.md), and implemented in `src/session/site_model.rs`. It is worth knowing where the model and the shorthand part company.

##### The per-EV kW figure is an average, not a constant

A charging station is current-limited rather than
power-limited: the pilot signal caps it at 32 A, so it draws about 6.59 kW whatever else is
happening. What the site draws on top of that is not proportional to the vehicle count. The
transformer's core loss and magnetizing current are a fixed standing block, present whenever it is
energised, and its copper loss rises with the *square* of loading.

Divide the site total by the number of vehicles and those two effects pull opposite ways: the fixed
block is diluted as vehicles are added, while the copper loss grows faster than the count does. The
per-EV share therefore falls, flattens and edges back up — about 6.80 kW at one vehicle, 6.69 at
three, a shallow minimum of 6.68 around four or five, and 6.71 at all ten. The `~6.7 kW` in the
algorithm description is good to within about 1.5% at one vehicle and within 0.2% everywhere from
three vehicles up, which is where this site's peaks have tended to sit. It is worth knowing that
the *lowest* per-EV figure is the one in the middle, not the one at full occupancy.

##### kVA is a quadrature sum, not `kW ÷ PF`

Real power, displacement reactive power and distortion
reactive power are mutually orthogonal, so they combine as the square root of the sum of their
squares rather than by division. Dividing kW by a power factor would imply that current is free to
grow as the power factor degrades, which is exactly what the pilot signal prevents. The `~0.98` is
a good approximation near full occupancy and a poor one at low counts, for the same reason the
per-EV kW figure moves: the transformer's fixed reactive block is diluted as vehicles are added.
At one vehicle the site power factor is about 0.94; by five it is 0.98, and it plateaus a little
above that. With no vehicle charging at all it is far lower still, because the standing block is
then the whole of the load.

##### Past what the panels hold, the model uses full-panel average kW and kVA values

The electrical model describes one panel on one transformer, and the site as built is one such panel of
supporting 10 concurrent charging sessions. The constant `PANEL_COUNT` should be updated when additional panels are installed. Nonetheless, if the constant is not updated, the software will continue to provide reasonable estimates using the full-panel average kW and kVA values for charging sessions above the panels' capacity.

Because the Session Report does not contain panel information, even when there are multiple panels and the constant is up-to-date, the software packs sessions into as few panels as will hold them: panels fill to ten, one at a time, one panel takes the remainder, and the rest stand idle — still drawing their own standing block, because a transformer's core loss and magnetising current
do not wait for a car. Packing rather than spreading gives the larger figure, since a panel's
copper loss and reactance are square-law in its own loading.

##### Where the model is written down

The above-mentioned [electrotechnical document](site-model-marcus.md) derives every constant and every formula, and tabulates the result for each vehicle count from 0 to 10; `cargo run --example site_load_report` prints that same table from the code, and `tests/fixtures/sessions/site_load.report.txt` pins it. `docs/Evolute-Simultaneous_Charging.pdf` is Evolute's own description of how the installation behaves when several vehicles charge at once.

#### Assumptions

- **Reported times are exact.** `Conn_DateTime_Start` and `Conn_DateTime_End` are taken as the instants the connection began and ended, to the second. The one allowance is `DURATION_TOLERANCE`, a second of slack when checking them against `Conn_Duration`, because the source rounds somewhere at second level; see [Anomalies](#anomalies).
- **Breaker ratings are uniform across panels.** `count_based_kw` and `count_based_kva` are an aggregate session count multiplied by a single rating, so an installation mixing breakers of different ratings would skew both. Panels enter the estimates in one other place only — the count at which the site total switches from the one-panel model to proportional scaling, described above — and that too assumes every panel is like the first. Which panel a session ran on is never used: the session report carries no panel ID, and none is needed. A session drawing more than a breaker should allow is flagged `ExcessiveAvgKw`; see [Anomalies](#anomalies).

#### Anomalies

An anomaly is something about a reported session that needed a judgement call. This section says why each one exists and what the software does about it. What the user is shown when one is raised, and where, is in [docs/ERRORS.md](../ERRORS.md).

One of the five excludes a session from every estimate — `InconsistentDuration`. Nothing else removes a session.

It was three of nine until session times were confirmed to be stated on a fixed standard-time offset. At a fixed offset every reported wall time names exactly one instant, so the two daylight-saving exclusions — a wall time that never occurred, and one that occurred twice — cannot arise, and neither can the sentinel timestamps they were given in place of a reading. See `docs/time/README.md`, "Time zone".

Excluded sessions get a section of their own in the report, listing **every** one in the session data rather than only those near the interval of interest, with an `In interval` column saying whether each *appears* to fall in that interval. Appears only: a record whose own fields contradict each other cannot be trusted to say where it belongs, so filtering on that judgement could hide exactly the session a reader most needs to see. Such a record may even report an end before its start, and the column answers for it.

- **`InconsistentDuration`** — the record's reported start, end and duration contradict each other. The invariant is that the three agree, and `duration_is_consistent` in `src/session/common.rs` is the one place it appears in code:

  ```
  | Conn_start + Conn_Duration - Conn_end |  <=  DURATION_TOLERANCE
  ```

  `DURATION_TOLERANCE` is one second. It is not slack chosen for comfort: in the one real portal export, four of five rows satisfy the invariant exactly and one is a second out, and `Active_Charge_Time` misses `Conn_Duration` by a second on three of the five. Something in the source rounds at second level. Exact equality would exclude a fifth of the only genuine export there is; anything wider starts admitting records whose fields really do disagree.

  - An inverted record — one whose end precedes its start — fails this by whatever the inversion is worth, and that is what keeps it out. `Session::intersects` panics on an inverted span and names exclusion by this test as the reason it cannot reach one.
  - A session failing it is excluded from the estimates, in either direction. If a record's own fields disagree by more than the source's rounding explains, neither its duration nor the span the estimating logic would place it on can be relied on.

  This was three checks with a window a whole minute wide, while reported times were truncated to the minute. `docs/archive/session/time-reporting-uncertainty.md` carries that derivation.
- **`DuplicateId`** — another session in the report carries the same `Charge_Session_ID`. `Charge_Session_ID` is **not unique**: Evolute's sample June 2026 report carries `S37487` on two sessions a week apart, within the one file, and reports for adjacent months overlap so a session near the boundary appears in both.

  - Two records stating the same session identically — same start and end, charge time and energy — are one session, and only one copy is kept, whether the two came from different files or from the same one. This is what lets a billing period be estimated from the two monthly reports spanning it without every shared session counting twice. Each dropped copy is noted in the run log of the file it came from, in wording that says the fields were equal, so a collapse cannot be mistaken for a `DuplicateId`.
  - Two records sharing an id but differing in any of those fields are two sessions. Both are kept and both take part in every estimate, and each is flagged.
  - The flag cannot distinguish a reused id from two reports disagreeing about one session; from the merge the two look identical. Neither is treated as fatal, because refusing the first would make June 2026 unestimatable, and the judgement belongs to a reader who can go back to the source rows.
- **`ZeroActiveChargeTime`** — the session delivered energy in no time at all, so its average power is unbounded or undefined. These are designated as *spike*s. Spikes are a theoretical possibility the software must guard against, though it is highly unlikely they would occur in practice.

  - The `csv_sessions` reader function separates spikes from the normal sessions before feeding them all to the peak power estimating logic.
  - If spikes do occur, they are worth reviewing individually for their effect on the building's demand charge.
  - The power estimating logic treats spikes as follows:
    - If `Energy_Use == 0`, set `avg_kw` to 0. These sessions do not contribute to `energy_based_kw` and `energy_based_kva` but they do contribute to `count_based_kw` and `count_based_kva`.
    - Otherwise, set `avg_kw` to the constant `BREAKER_RATING_KW`. These sessions contribute to all four estimate types.
- **`ExcessiveAvgKw`** — the session's own average power exceeds `BREAKER_MAX_NORMAL_KW`, the rating at the top of the normal supply voltage band, which the hardware should not allow. The breaker limits current, so a vehicle draws more kW when the voltage runs high; only a draw above the whole band says something is wrong. It is not excluded, because the figure says something is wrong with `Energy_Use` or `Active_Charge_Time` and not which. See [Assumptions](#assumptions), where the uniform-rating assumption this rests on is stated.
Not an anomaly, but easily mistaken for one: a session with zero `Energy_Use` and non-zero `Active_Charge_Time` is an ordinary record. It does not contribute to `energy_based_kw` or `energy_based_kva`, and it does contribute to `count_based_kw` and `count_based_kva`.
