# Questions for Evolute

## Charges report

- What are the cut-offs for the monthly charges data? Are they based on midnight on Standard Time all year (no change with DST) or midnight local time (changes with DST)?

## Not asked

### Session report

- How are sessions that start in one month and end in the next reported? Does the report always include an extra day at each end to ensure all sessions that touch the month are fully reported?
- Can you provide `Energy_Use` with 3 decimal places instead of just 1?
- The Charges Report contains a panel ID column. Can you also provide the panel ID in the session report?

### Charges report

- What values can `Bill_Status` take and what do they mean? In the sample report, all lines show `Issued`.

## Answers pending

### Session Report

- We would like to confirm the time zone used for the session start and end times in the report. In a previous email, you said that those timestamps were in local time (ET), which implies they would be subject to DST changes. However, the portal gives the clear impression that the timestamps are in Standard Time, with a fixed UTC offset that does not change with DST. Please confirm that indeed the UTC offset is fixed and the timestamps in the report are always in EST (no DST change) if I select the EST time zone.
- The sample session report you sent us for June had session start and end timestamps truncated to minutes. The session report I downloaded from the portal shows start and end times with seconds precision and `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End`. Please confirm we can rely on the seconds precision going forward.

### Charges Report

- What do `Start_Date` and `End_Date` mean on each row? In the sample Charges Report we have seen, every row carries the first and last day of the month the report is for. (**BTW**, I downloaded a charges report from the portal but it was empty because the billing account has not yet been set up.)
- What values can `Bill_Status` take and what do they mean? In the sample report, all lines show `Issued`.
- When the charges report spans two months, will the charges for a user be combined in one line or will there be two lines for each user? And what will the start and end dates be in that case?

### Report naming and formatting

- The software depends on the formats of the Session Report and Charges Report CSV files. Please give us reasonable advance notice if you make any changes at all to the formatting of data in the files or the naming convention of those files.


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
