# Questions for Evolute

## Session Report

- How are sessions that start in one month and end in the next reported? In such cases, do the duration and energy use fields contain only the amounts that land on month the report is for?
- Session start and end times as currently reported are truncated to minutes.
  - Can the reporting of session start and end times be modified to include the seconds? (This would allow us to have a clear view of whether sessions overlapping over a period of 1 minute really overlap or abut each other.)
  - Either way: does the reported session end time denote the last second during which the EV was drawing power, or the first second during which it was not? We currently assume the former, and pad the reported end accordingly; under the latter, no padding would be needed. We can work with either, but the two call for different arithmetic, so we would rather not guess.
- Can you provide `Energy_Use` with 3 decimal places instead of just 1?
- The Charges Report contains a panel ID column. Can you also provide the panel ID in the session report?


## Charges Report

- What do `Start_Date` and `End_Date` mean on each row? In every Charges Report we have seen, every row carries the first and last day of the month the report is for. Two readings fit that, and they call for different handling:
  - They always state the report's own month, identically on every row, whatever the breaker's history.
  - They state the span the breaker was actually subscribed for, which happens to be the whole month whenever nobody joined or left.

  If the second is right: for a breaker whose subscription started or ended part-way through the month, is the span clipped to the month the report covers, or does it carry the subscription's own dates — which for a long-standing subscriber would be a start date years in the past? And can rows within one report differ from each other?
- What values can `Bill_Status` take? We have only ever seen `Issued`. For each of the others, does the row's `kWh` and `Cost` represent an amount actually billed to the building? We currently count every row towards the month's totals whatever its status, since dropping one on a status we have not met would quietly change a figure.


## Answers received

### 22 Jul 2026 — the three duration fields

> **Q:** What is the difference between Conn_Duration, Charge_Duration, and Active_Charge_Time?
>
> **A:** All 3 will show as almost the same, with Active charging being off by maybe 1 second due
> to rounding as it is on a slightly different timer. These fields are here for grant reporting,
> but for our system we do not track them differently.

Consequences for the software:

- The three fields do **not** distinguish connected time from charging time. A car that stays
  plugged in without drawing power is not why they differ; the difference is a rounding artefact of
  roughly a second. `Session::charge_time`'s doc comment carried the wrong reason and has been
  corrected.
- A zero `Active_Charge_Time` on a session that reports energy is therefore not "energy delivered
  in no time at all" but a reporting fault. Such rows are still surfaced rather than dropped — see
  `Sessions::spikes`.
