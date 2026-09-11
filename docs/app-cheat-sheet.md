# App cheat sheet: kicking the tires on `ev_cost_recovery`

Every figure and message below was produced by running the files in the `data/` folder. That folder is not in the repository, so its contents need to be provided separately.

## Launching it

See [README.md - Getting and running the software](../README.md#getting-and-running-the-software).

## The sample session reports

Three files in `data/evolute`, and every figure below comes from them:

| File | Covers |
|:---|:---|
| `Session_Report_May_1_2026-May_31_2026-seconds.csv` | May, a mock built from June by `scripts/make-may-mock.py` |
| `Session_Report_June_1_2026-June_30_2026-seconds.csv` | June, real anonymized data |
| `Session_Report_May_1_2026-June_30_2026-seconds.csv` | both months in one file, as a single portal export spanning the period would be |

The Evolute portal states connection times to the second, with
`Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End`. `scripts/make-seconds-copies.py` writes
these three, and explains the one row per file that is deliberately a second off that invariant so
that `DURATION_TOLERANCE` is exercised.

## Tabs

The tab bar holds four: **Cost recovery**, **Peak power detail**, **Evolute reimbursement** and
**Convert to workbook**.

## Cost recovery

Only the **June 2026** period can be run with the sample data. The sample Green Button export stops on 24 June, so no later period has meter data. Pick these four, in the order the window asks:

| Picker | File |
|:---|:---|
| Toronto Hydro bill | `data/hydro_bills/TH_5728140000_2026_06_29.pdf` |
| Green Button export | `data/green_button/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML` |
| Session report 1 | `data/evolute/Session_Report_May_1_2026-May_31_2026-seconds.csv` |
| Session report 2 | `data/evolute/Session_Report_June_1_2026-June_30_2026-seconds.csv` |

Then the rates. **Effective from** `2026-05-01`, and `0.1100` / `0.0900` / `0.0700`.

Press **Work out the surplus**. Expect:

```
Billing period ending 2026-06-23

Cost recovery        115.41
EV energy cost      -179.63
EV delivery cost     -92.02
Surplus             -156.24          (red, and "fell short" in the report)
```

The report shown runs to 140 lines.

### One file instead of two

`Session_Report_May_1_2026-June_30_2026-seconds.csv` covers the whole period on its own. Put it in
**Session report 1** and leave **Session report 2** empty. The four amounts are the ones above, to
the cent: the same sessions, read from one file. The report runs to 135 lines, and *Session data*
names the one file.

### The pickers open in order

Each session picker is shut until the thing its file will be judged against is in hand, and says
which it is waiting for where it would otherwise say *None chosen*:

| Slot | Shut while | It says |
|:---|:---|:---|
| Session report 1 | no bill is chosen | `Choose the bill first` |
| Session report 1 | the bill chosen would not read | `The bill above could not be read` |
| Session report 2 | the first slot is empty | `Choose Session report 1 first` |
| Session report 2 | the first slot covers the whole period | `Session report 1 covers the whole billing period` |

The bill is what says which period this is, so a file that is not a readable Toronto Hydro bill is
reported at the bill's own picker, as soon as it is chosen. Pick any other PDF to see it.

### Changing the bill

Choosing a different bill is choosing a different period, and the session slots are emptied to
match: the second always, and the first unless the report in it reaches into the new period. With
the June bill and `Session_Report_June_1_2026-June_30_2026-seconds.csv` in the first slot, swap in
`TH_5728140000_2026_07_28.pdf` — the period ending 23 July starts on 24 June, which that file
reaches into by a week, so it stays. Swap in `TH_5728140000_2026_05_28.pdf` instead and the slot
empties: nothing in June belongs to a period ending 23 May.

### A report from another period

Each file is held against the bill's period the moment it is chosen, at either slot. Put
`Session_Report_August_1_2026-September_4_2026.csv` into **Session report 1** under the June bill:

```
This report covers 2026-08-01 to 2026-09-04, which is outside the billing period
2026-05-24 to 2026-06-23. Choose a report that reaches into the period.
```

One day of overlap is enough to lift it. `Session_Report_July_1_2026-July_31_2026-mock.csv` gets the
same message under the June bill and none at all under `TH_5728140000_2026_07_28.pdf`, whose period
runs 24 June to 23 July.

### One report that already holds the other

With the June bill, put `Session_Report_June_1_2026-June_30_2026-seconds.csv` in the first slot —
June alone leaves 24 to 31 May uncovered, so the second slot opens — and then
`Session_Report_May_1_2026-June_30_2026-seconds.csv` in the second:

```
Session report 2 covers 2026-05-01 to 2026-06-30, which already includes this
file's 2026-06-01 to 2026-06-30. Move that file to this slot and clear the
second, or choose a report reaching dates it does not.
```

The note lands on the slot that has to change, which here is the first: the wider file is the one to
keep. **Work out the surplus** stays disabled until it is answered, and **Clear** appears beside the
second slot whenever it holds a file.

Two reports that each reach the period but leave a day of it between them are refused the same way,
in the run's own words — `the session reports do not cover the billing period …`, followed by what
each file covers. The sample files cannot be made to produce it: every pair of them that both reach
one of the four sample bills' periods either covers it or is the case above.

One report short of the period on its own is never an error. That is the state the second slot
exists for, so nothing is said until a second file arrives.

### What to look at

Collapse and expand the sections. *Session data* should name both CSVs, in the order their names
begin — May before June, whichever picker each went into. *Sessions needing a look* lists four rows,
all `DuplicateId`: `S83723` twice in May and `S37487` twice in June, with a glossary beneath. There
is no *Sessions left out* section: nothing in these files is left out.

### Things worth trying

| Do this                                                      | Expect                                                       |
| :----------------------------------------------------------- | :----------------------------------------------------------- |
| Rates `0.30` / `0.30` / `0.30`                               | Surplus **+136.95**, coloured as the accent rather than red, and "covered" in the report |
| Rates `0.20` / `0.20` / `0.20`                               | Surplus **+0.75** — all but break-even, so you can watch the sign flip either way |
| Tick *rates changed*, leave `0.1100/0.0900/0.0700` from 1 May, add `0.30` flat from `2026-06-01` | Cost recovery **359.52**, surplus **+87.87**; the recovery report gains a second stretches table |
| Change any picker after a run                                | Figures vanish, and the *Peak power detail* tab greys out    |
| Clear the mid-peak rate (delete it, don't set it to zero) and run | `the mid-peak rate is blank` — refused, not read as zero     |
| Type `eleven cents` into the mid-peak rate                   | `cannot read "eleven cents" as the mid-peak rate: …`         |

### Errors worth provoking

All three are real messages from these files, and all three come from the run. What the pickers
refuse before the run is above: a bill that will not read, a report from another period, and one
report that already holds the other.

Choosing the wrong bill is one of those. `TH_5728140000_2026_05_28.pdf` covers 24 April to 23 May,
so the June CSV is refused at its picker.

**No meter data for the period** — `TH_5728140000_2026_07_28.pdf` with the `June` and `July` CSVs.
Choosing that bill empties the second slot, so put July back in it; the two cover 24 June to 23
July between them, and the run gets as far as the meter:

```
the meter data covers 24 of the 720 intervals in the billing period ending 2026-07-23,
so its maxima are not the period's
```

**A rate schedule outside the period** — set *Effective from* to `2026-08-01`:

```
the cost-recovery rates given for the start of the period take effect 2026-08-01,
after it starts on 2026-05-24
```

**A session report whose name says nothing.** The picker filters to `.csv`, so to reach this, pick
the bill first and then copy a report to a name without dates:

```sh
cp data/evolute/Session_Report_June_1_2026-June_30_2026-seconds.csv data/evolute/sessions.csv
```

Picking `data/evolute/sessions.csv` is refused at the picker, on its name alone, and
**Work out the surplus** stays disabled until you replace it. Delete the copy afterwards.

## Peak power detail

The tab is greyed until the *Cost recovery* run succeeds. A report with three sections provides
details on the peak power values that drive the delivery cost portion of *Cost recovery*, one
section per peak — *EV Peak kVA Contribution*, *EV Peak kW Contribution* and *EV Peak kW 7-7
Contribution*. The value used for each delivery charge is the one marked `*` in that section's
*Estimates* table. What the terms mean is under *Definitions and Conventions*, once, at the end.

| Section | What to check |
|:---|:---|
| `kVA` | `2026-06-11 19:00 to 20:00 EDT`, energy-based **6.828** kVA |
| `kW` | The **same** interval — in this period the building's kW and kVA peaked together — energy-based **6.402** kW |
| `kW 7-7` | A **different** interval, `14:00 to 15:00 EDT`, and inside 07:00–19:00. It cannot be the 19:00 one. Energy-based **1.971** kW |

Those three figures are the `EV demand` column of *Delivery charges by component* in the surplus
report, so the two tabs can be read against each other.

Inside each, the `Interval` line and the `Segment` column say different things. The interval is where
the *building* peaked, from the meter. The segment is the 15 minutes inside it where the *chargers*
peaked, from the sessions — `19:00` for the kVA section, `14:45` for `kW 7-7`. The demand charge is
billed on the segment.

The *Segments* table under each set of estimates gives each segment's `Session count` and
`Session kW`, the two figures the *Estimates* table's `All-in power` is worked out from;
*Definitions and Conventions* says how. For `kW 7-7`
the first three segments are empty and the whole figure comes from `14:45`, which is the clearest
of the three for seeing what a segment contributes.

## Evolute reimbursement

A separate run, and a separate question: whether Evolute paid what our rates earned over a
**calendar month**. That is not the billing period the surplus covers, so the two figures are not
two views of one number.

| Input field | Value |
|:---|:---|
| Evolute Session Report | `data/evolute/Session_Report_June_1_2026-June_30_2026-seconds.csv` |
| Evolute Charges Report | `data/evolute/XX-XX_Charges_June 2026-June 2026.csv` |
| Reimbursement | `246.26` |
| Rates | **Effective from** `2026-06-01`, and `0.1100` / `0.0900` / `0.0700` |

Only June works. It is the one month with both a Session Report and a Charges Report.

Press **Reconcile the month**. Expect:

```
June 2026

Reimbursement received   246.26 $
Charges Report total    -246.26 $
Remittance variance        0.00 $

Reimbursement received   246.26 $
Cost recovery earned    -111.54 $
Dollar variance          134.72 $
```

Two subtractions, not one. The first asks whether the money that arrived matches Evolute's own
Charges Report; the second asks whether it matches what our rates earned. The report says
"sent exactly what its own Charges Report comes to" and "reimbursed more than the cost-recovery
rates come to".

Below the headline, in the sections:

| Section | What to check |
|:---|:---|
| *Energy variance* | `1330.30` kWh on the Charges Report against `1309.97` priced, a variance of `20.33` — the two come from different documents and are not expected to agree exactly |
| *Sessions needing a look* | Two rows, both `DuplicateId`, both session `S37487` |
| *Charges Report* | The file it was read from, and nothing more |

Worth trying:

| Do this | Expect |
|:---|:---|
| Type `0` into **Reimbursement** | Both variances negative — `-246.26` and `-111.54` — and "sent less than its own Charges Report" |
| Clear **Reimbursement** and run | `the reimbursement amount is blank` — a blank field is refused, because zero is a real answer and has to be meant |
| Pick the **May** session report against the same Charges Report | `the session reports do not cover the billing period 2026-06-01 to 2026-06-30:`, followed by what the May file does cover |
| Pick `Session_Report_May_1_2026-June_30_2026-seconds.csv` instead | It runs, and gives the same figures as June alone. The month is taken from the Charges Report; the session report only has to reach across it |
| Rename the Charges Report to anything without `_Charges_<Month Year>-<Month Year>` in it | `is not a Charges Report`, or `does not state the months it covers` — the reader will not open a file it cannot date |
| Name a Charges Report for more than one month, e.g. `XX-XX_Charges_June 2026-July 2026.csv` | `Only a single calendar month is accepted` — the reconciliation prices one month against one month |

## Convert to workbook

Turns one source file into an Excel workbook beside it, for reading by eye. Nothing else in the app
depends on the result — the other tabs read the source files themselves.

**Convert** *Session report* or *Green Button export* chooses which; each keeps its own file and its
own last result, so switching between them loses nothing.

| Do this | Expect |
|:---|:---|
| Convert `data/evolute/Session_Report_June_1_2026-June_30_2026-seconds.csv` | A **Replace existing workbook?** modal, because `data/evolute/` already holds that workbook. **Cancel** leaves it alone; **Replace** overwrites it and anything written into it by hand. 25 rows are reported for review, every one of them `ExcessiveAvgKw` |
| Convert `data/evolute/Session_Report_May_1_2026-June_30_2026-seconds.csv` | No modal the first time, because no workbook of that name exists yet; the second run prompts |
| Convert the Green Button export | The same prompt, and a pause while a multi-year export is parsed. The workbook has two sheets: `Peak_values`, one row per billing period, and `Interval_values`, every hour |

An existing workbook is never overwritten silently, in either conversion.

## What it writes

Converted workbooks are, of course, written to the file system. In addition, reports can be optionally saved and logs are automatically written.

### Saving reports

In each tab, pressing the **Save** button saves the displayed report.

### Logs

Each run rewrites a log beside each file it reads. Which tab writes which log, what a log holds, and
how it is named are in [ERRORS.md](ERRORS.md#the-run-logs).

What to check here is that the timestamps move: every log a tab writes should be rewritten on the
run that reads the file, whether or not you save anything.

A session report's log holds `ExcessiveAvgKw` rows and dropped copies, and nothing else. On the
two-file run: 25 items for May, all `ExcessiveAvgKw`, and 27 for
June — the same 25 plus two records dropped as copies of May's rows 32 and 121. Those two are the
sessions that run past midnight on 31 May, so both months' reports carry them and the merge counts
each once; the log names the file and row each repeats. Reading the single May-June file instead
gives one log of 52: 50 `ExcessiveAvgKw` and the same two copies, this time repeating rows of the
file itself.
