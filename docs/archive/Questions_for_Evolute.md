# Questions for Evolute

## Not asked

### Session report

- How are sessions that start in one month and end in the next reported? Does the report always include an extra day at each end to ensure all sessions that touch the month are fully reported?
- Can you provide `Energy_Use` with 3 decimal places instead of just 1?
- The Charges Report contains a panel ID column. Can you also provide the panel ID in the session report?

### Charges report

- I understand that the kWh and cost of a session that starts in month M and ends in month M+1 are fully captured in the charges report for month M+1. Please confirm.

## Answers received

### 14 Sep 2026

1. **Q:** You mentioned "differences in how idle usage is captured". Can you elaborate on that?
   - **A:** Building Report (Monthly Charges Report) is a running total, while some trickle data may not be captured in sessions. This relates to question 4.
2. **Q:** Can you briefly describe how your system selects session information from your database to include in a monthly charges report?
   - **A:** I'm not sure what you mean - the Monthly Charges Report is not related to the Sessions Report. Monthly Charges Report is calculated based on a running total that is updated daily. The sessions are referencing the same raw data as the Monthly Charges Reports, but that is where their relationship ends. The data is similar, because they use the same data base, there are discrepancies because their calculations/function/mechanisms are different. As said - the sessions report was developed for a specific purpose - it is not related to billing, even though it does have cost information.
3. **Q:** Does the charges report define the calendar month based on local time (with daylight savings) or EST (always UTC-5)? Local Time.
   - **A:** This is how I caught my mistake when comparing Sessions to billing after I downloaded the report using UTC-5. They were off by an hour.
4. **Q:** I noticed some sessions that last only a few seconds and have total energy use of 0 or 0.1 kWh. What causes those sessions?
   - **A:** Trickle charging that IS captured in sessions. Not all of it is. Some of it is lost due to rounding or not meeting minimum thresholds.

### 13 Sep 2026

- **Q:** As you can see from the summary [spreadsheet not attached], the numbers from my app agree pretty closely with the totals from the session reports, as they should because my app uses the session report as the input for the cost recovery calculation. You will notice, however, that while the total kWh numbers from the session and charges reports are close (equal in August, off by 0.5 in September), there is a difference between the total costs from the session reports and the charges reports that cannot be explained by rounding error. These numbers also make me wonder why the rounding of kWh values would be different between the two reports. I think this warrants further investigation.

  - **A:** To clarify our earlier guidance, the Session Report is the most useful for reviewing individual charging sessions, and for tracking TOU usage and peaks. It is the best report we can generate right now for your needs. The Building Report is the appropriate reference for billed amounts.

    The two reports serve different purposes and use different calculation methods. Billing is calculated daily from recorded usage and the applicable time-of-use rates, then aggregated into the monthly totals shown in the Building Report. The Session Report presents usage by session, so rounding and differences in how idle usage is captured can prevent the reports from matching exactly. Generally, the session report will have slightly less kWh totals than the building report, over time (not every month).

    With only one full month of operation, we recommend allowing six months of data to accumulate before undertaking a broader comparison. If we see issues after six months, we can correct them and issue reimbursements if needed.

### 9 Sep 2026

#### Session Report

- **Q:** We would like to confirm the time zone used for the session start and end times in the report. In a previous email, you said that those timestamps were in local time (ET), which implies they would be subject to DST changes. However, the portal gives the clear impression that the timestamps are in Standard Time, with a fixed UTC offset that does not change with DST. Please confirm that indeed the UTC offset is fixed and the timestamps in the report are always in EST (no DST change) if I select the EST time zone.
  - **A:** Time zone / UTC offset. Yes, the portal's impression is the correct one. The session report uses a fixed UTC offset that never changes with DST. Selecting EST (UTC-5) gives you UTC-5 year-round, including June and July.

- **Q:** The sample session report you sent us for June had session start and end timestamps truncated to minutes. The session report I downloaded from the portal shows start and end times with seconds precision and `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End`. Please confirm we can rely on the seconds precision going forward.
  - **A:** Seconds precision. Yes, you can rely on it. Timestamps are always emitted as YYYY-MM-DD HH:MM:SS,. Conn_DateTime_Start, Conn_DateTime_End and Conn_Duration are all derived from the same two underlying instants, so Start + Duration == End holds by construction, including for merged continuous sessions.

#### Charges Report

- **Q:** What do `Start_Date` and `End_Date` mean on each row? In the sample Charges Report we have seen, every row carries the first and last day of the month the report is for. (**BTW**, I downloaded a charges report from the portal but it was empty because the billing account has not yet been set up.)

  - **A:** Start_Date and End_Date. These are report-level, not row-level: they describe the billing period the whole report
    covers, and are repeated identically on every row. Start_Date is the first day of the period and End_Date is the last
    day, inclusive. In steady state that's the first and last day of the month, which matches what you observed.

    Two exceptions: a building's very first report starts on its first day of recorded data rather than the 1st, and a multi-
    month selection in the portal spans the whole range you selected. The date format is Jan 2, 2006 style — e.g. Jun 1,
    2025 / Jun 30, 2025 — note this is a different format from the ISO timestamps in the Session Report.
  
- **Q:** What values can `Bill_Status` take and what do they mean? In the sample report, all lines show `Issued`.

  - **A:** Bill_Status. Exactly four values are possible:

    Issued : Charge processed successfully (also used for a $0 total).
    Pending - No MOP : No payment method on file for the resident. Retried every 24h; the resident and building contacts are  emailed after 3 and 6 failures.
    Pending - Error : The card was declined or the charge errored. Same 24h retry and notification schedule.
    Bill Status Unknown : Defensive fallback for an unrecognized state. Should not appear in practice, but please handle it rather than failing.

    Important: the status is a snapshot taken when the report was issued, not live. A bill that read Pending - Error at month close may since have been collected on a retry; the CSV won't reflect that unless the report is regenerated. If you need settled figures, pulling the report a week or two after month-end is the safer pattern. We are happy to discuss if you need something more real-time.

#### Report naming and formatting

- **Q:** The software depends on the formats of the Session Report and Charges Report CSV files. Please give us reasonable advance notice if you make any changes at all to the formatting of data in the files or the naming convention of those files.

  - **A:** Understood, and agreed. Current conventions, for the record (both filenames are generated at download time):

    Session Report: Session_Report_June_1_2025-June_30_2025.csv
    Charges Report: \<building address>_Charges_June 2025-June 2025.csv
    Note the Charges filename contains the building address verbatim, so it may include spaces, commas and periods.

    We will give you advance notice before changing the column set or order, the formatting of any value, or the filename convention of either report.

    Charges Cost includes a leading $ (e.g. $41.20).
    Charges kWh can be the literal string N/A for periods before April 2022.
    Charges Name is prefixed with [deleted] for residents removed during the period (e.g. [deleted]Jane Smith).
    Session User_ID is the last 7 characters of the internal user ID, not a full identifier.
    Session Total_Fee is in dollars with 2 decimals and no currency symbol.
    Also note the Session Report request is capped at a two-year range.

#### Charges report (follow-up)

- **Q:** When the charges report spans two months, will the charges for a user be combined in one line or will there be two lines for each user? And what will the start and end dates be in that case?

  - **A: **Combined in one row.

    One additional point I want to mention: Building Charges/Monthly Invoicing can be affected by sessions that span the
    end of one month and the beginning of the next.

    For example, a session might start at 23:30:30 on the 31st and end at 01:30:30 on the 1st. The total session cost is calculated accurately, but the way cost for the session is allocated between billing periods means the split may not precisely reflect the amount incurred in each month. As a result, more (or less) of the cost may appear on the previous or following month’s bill than expected. Without getting into too much depth of the mechnism - it was originally designed to avoid crashing the system at 00:00:00 at the end of every month with calculations - but we have identified a better method to avoid these issues which will be implemented in the future.

    This is worth considering if you're going to use Building Charges for (precise) monthly cost analysis. For real precise
    and detailed analysis of individual sessions, I strongly recommend using Session Reports, which provide the session
    timestamps and associated costs.

    As you know, we're developing Evolute PRO, and this "cost-splitting" issue (for lack of a better name) will be addressed in our future billing mechanisms - hence Building Charges reports and Monthly Invoicing will be more accurate as it relates to exact month end.

    We will update you with any new reporting styles/formats etc. before they take effect.

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
