# App cheat sheet: kicking the tires on `ev_cost_recovery`

Every figure and message below was produced by running the files in the `data/` folder. That folder is not in the repository, so its contents need to be provided separately.

## Launching it

See [README.md - Getting and running the software](../README.md#getting-and-running-the-software).

## Tabs

The tab bar holds four: **Cost recovery**, **Peak power detail**, **Evolute reimbursement** and
**Convert to workbook**.

## Cost recovery

Only the **June 2026** period can be run with the sample data. The sample Green Button export stops on 24 June, so no later period has meter data. Pick these four, in the order the window asks:

| Picker | File |
|:---|:---|
| Toronto Hydro bill | `data/hydro_bills/TH_5728140000_2026_06_29.pdf` |
| Green Button export | `data/green_button/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML` |
| Session report 1 | `data/evolute/Session_Report_May_1_2026-May_31_2026-mock.csv` |
| Session report 2 | `data/evolute/Session_Report_June_1_2026-June_30_2026.csv` |

Then the rates. **Effective from** `2026-05-01`, and `0.1100` / `0.0900` / `0.0700`.

Press **Work out the surplus**. Expect:

```
Billing period ending 2026-06-23

Cost recovery        118.09
EV energy cost      -186.02
EV delivery cost     -91.52
Surplus             -159.45          (red, and "fell short" in the report)
```

The report shown runs to 142 lines. May's file is a mock built by `scripts/make-may-mock.py`; only
June's is based on real anonymized data.

### What to look at

Collapse and expand the sections. *Session data* should name both CSVs. *Sessions needing a look* should list a handful of rows with a glossary beneath.

### Things worth trying

| Do this                                                      | Expect                                                       |
| :----------------------------------------------------------- | :----------------------------------------------------------- |
| Rates `0.30` / `0.30` / `0.30`                               | Surplus **+131.06**, coloured as the accent rather than red, and "covered" in the report |
| Rates `0.20` / `0.20` / `0.20`                               | Surplus **-5.14** — near break-even, so you can watch the sign flip either way |
| Tick *rates changed*, leave `0.1100/0.0900/0.0700` from 1 May, add `0.30` flat from `2026-06-01` | Cost recovery **359.95**, surplus **+82.41**; the recovery report gains a second stretches table |
| Change any picker after a run                                | Figures vanish, and the *Peak power* tab greys out           |
| Clear the mid-peak rate (delete it, don't set it to zero) and run | `the mid-peak rate is blank` — refused, not read as zero     |
| Type `eleven cents` into the mid-peak rate                   | `cannot read "eleven cents" as the mid-peak rate: …`         |

### Errors worth provoking

All four are real messages from these files.

**Wrong bill for the reports** — pick `TH_5728140000_2026_05_28.pdf` with the same two CSVs:

```
the session reports do not cover the billing period 2026-04-24 to 2026-05-23:
```

**Reports that miss the start** — the June bill with `June` and `July` CSVs:

```
the session reports do not cover the billing period 2026-05-24 to 2026-06-23:
```

**No meter data for the period** — `TH_5728140000_2026_07_28.pdf` with `June` and `July` CSVs:

```
the meter data covers 24 of the 720 intervals in the billing period ending 2026-07-23,
so its maxima are not the period's
```

**A rate schedule outside the period** — set *Effective from* to `2026-08-01`:

```
the cost-recovery rates given for the start of the period take effect 2026-08-01,
after it starts on 2026-05-24
```

**A session report whose name says nothing.** The picker filters to `.csv`, so to reach this, copy
one to a name without dates:

```sh
cp data/evolute/Session_Report_June_1_2026-June_30_2026.csv data/evolute/sessions.csv
```

Picking `data/evolute/sessions.csv` is refused at the picker, before anything is read, and
**Work out the surplus** stays disabled until you replace it. Delete the copy afterwards.

## Peak power

The tab is greyed until the *Cost recovery* run succeeds. A report with three sections provides details on the peak power values that drive the delivery cost portion of *Cost recovery*. The values used for delivery cost calculation are the mid-points of the energy-based ranges presented in the report:

| Section | What to check |
|:---|:---|
| `kVA` | `2026-06-11 19:00 to 20:00` |
| `kW` | The **same** interval — in this period the building's kW and kVA peaked together |
| `kW 7-7` | A **different** interval, and inside 07:00–19:00. It cannot be the 19:00 one |

Inside each, the `Interval` line and the `Segment` column say different things. The interval is where
the *building* peaked, from the meter. The segment is the 15 minutes inside it where the *chargers*
peaked, from the sessions — for the kVA section, `19:45`. The demand charge is billed on the segment.

## Evolute reimbursement

A separate run, and a separate question: whether Evolute paid what our rates earned over a
**calendar month**. That is not the billing period the surplus covers, so the two figures are not
two views of one number.

| Input field | Value |
|:---|:---|
| Evolute Session Report | `data/evolute/Session_Report_June_1_2026-June_30_2026.csv` |
| Evolute Charges Report | `data/evolute/XX-XX_charges_2026-06-01T00_00_00-04_00.csv` |
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
Cost recovery earned    -114.67 $
Dollar variance          131.59 $
```

Two subtractions, not one. The first asks whether the money that arrived matches Evolute's own
Charges Report; the second asks whether it matches what our rates earned. The report says
"sent exactly what its own Charges Report comes to" and "reimbursed more than the cost-recovery
rates come to".

Below the headline, in the sections:

| Section | What to check |
|:---|:---|
| *Energy variance* | `1330.30` kWh on the Charges Report against `1315.73` priced, a variance of `14.57` — the two come from different documents and are not expected to agree exactly |
| *Sessions needing a look* | Two rows, both `DuplicateId`, both session `S37487` |
| *Charges Report* | `Bill_Status tally: Issued 38.` |

Worth trying:

| Do this | Expect |
|:---|:---|
| Type `0` into **Reimbursement** | Both variances negative — `-246.26` and `-114.67` — and "sent less than its own Charges Report" |
| Clear **Reimbursement** and run | `the reimbursement amount is blank` — a blank field is refused, because zero is a real answer and has to be meant |
| Pick the **May** session report against the same Charges Report | `no row of the Charges Report falls in 2026-05-01 to 2026-05-31, which is the month the session report is for; its rows cover 2026-06-01 to 2026-06-30. This is usually the wrong file.` |

## Convert to workbook

Turns one source file into an Excel workbook beside it, for reading by eye. Nothing else in the app
depends on the result — the other tabs read the source files themselves.

**Convert** *Session report* or *Green Button export* chooses which; each keeps its own file and its
own last result, so switching between them loses nothing.

| Do this | Expect |
|:---|:---|
| Convert `data/evolute/Session_Report_June_1_2026-June_30_2026.csv` | A **Replace existing workbook?** modal, because `data/evolute/` already holds that workbook. **Cancel** leaves it alone; **Replace** overwrites it and anything written into it by hand |
| Convert the Green Button export | The same prompt, and a pause while a multi-year export is parsed. The workbook has two sheets: `Peak_values`, one row per billing period, and `Interval_values`, every hour |

An existing workbook is never overwritten silently, in either conversion.

## What it writes

Converted workbooks are, of course, written to the file system. In addition, reports can be optionally saved and logs are automatically written.

### Saving reports

In each tab, pressing the **Save** button saves the displayed report.

### Logs

Each run rewrites a log beside each file it reads, named after the file with the kind of read appended:

| Tab | Logs written |
|:---|:---|
| Cost recovery | `*.session.csv.read.log` beside each of the two session reports, and `*.meter.xml.read.log` beside the Green Button export |
| Evolute reimbursement | `*.session.csv.read.log` beside the session report, and `*.charges.csv.read.log` beside the Charges Report |
| Convert to workbook | The workbook itself, and `*.session.convert.log` or `*.meter.convert.log` beside the source |

Every one of those timestamps should move on the run that reads the file, whether or not you save
anything.

