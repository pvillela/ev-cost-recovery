# App cheat sheet: learning `ev_cost_recovery` with the sample files

Every figure and message below was produced by running the app with the files in the `data` folder. That folder is not in the repository, so its contents need to be provided separately.

The exercises before [Extra credit](#extra-credit) use only the files in four folders inside `data`, and teach everything needed to use the app. Do them in order: later exercises assume the earlier ones.

Folder names are written the **Windows** way, e.g., `data\evolute`. On a Mac or Linux, the same folder is `data/evolute`. Keyboard shortcuts use Ctrl; on a Mac, use Cmd instead.

## Before you start

### Launching the app

See [README.md - Getting and running the software](../README.md#getting-and-running-the-software).

### The sample files

| Folder | File | What it is |
|:---|:---|:---|
| `data\hydro_bills` | `TH_5728140000_2026_06_29.pdf` and six others | Toronto Hydro bills, one per billing period |
| `data\green_button` | `TH_Electric_Usage_23-11-2024_to_24-06-2026.XML` | The building's meter readings, downloaded from Toronto Hydro |
| `data\evolute` | `Session_Report_May_1_2026-May_31_2026-seconds.csv` | Evolute charging sessions for May |
| `data\evolute` | `Session_Report_June_1_2026-June_30_2026-seconds.csv` | Evolute charging sessions for June |
| `data\evolute` | `XX-XX_Charges_June 2026-June 2026.csv` | Evolute's Charges Report for June |
| `data\rates` | `EV_Cost_Recovery_Rates.xlsx` | The rates charged for EV charging |
| `data\rates` | `EV_Cost_Recovery_Rates-change-in-period.xlsx` | Rates that change during the June billing period |

The June session report is real data, with names removed. The May one is made up from June's.

### A practice copy of the rates workbook

Several exercises change the rates workbook. Do not change the original. Instead:

1. In File Explorer, open `data\rates`.
2. Copy `EV_Cost_Recovery_Rates.xlsx` and paste it into the same folder. Windows names the copy `EV_Cost_Recovery_Rates - Copy.xlsx`.
3. Use the copy wherever an exercise says "the practice copy".

Open the practice copy in Excel (or another spreadsheet program). Its `rates` sheet looks like this:

| | A | B | C | D |
|:---|:---|---:|---:|---:|
| **1** | effective_date | on_peak | mid_peak | off_peak |
| **2** | 2026-05-01 | 0.1100 | 0.0900 | 0.0700 |
| **3** | 2026-09-01 | 0.5152 | 0.4740 | 0.4218 |

Each row is a set of rates, used from its `effective_date` until the next row's date. The workbook's format is described in [docs/rates/README.md](rates/README.md).

After each change, **save** the workbook. The app reads it again each time you run, so you do not need to choose it again. Before the next exercise, put the value you changed back the way it was.

### Things the app does not remember

Closing the app loses every file chosen, on every tab, and every result. They all have to be chosen again at the next launch. The text size and the colour theme also go back to where they started.

### The tabs

There are four tabs: **Cost recovery**, **Peak power detail**, **Evolute reimbursement** and
**Convert to workbook**.

## Exercise 1: Text size and colour theme

| Do this | Expect |
|:---|:---|
| Press `Ctrl and +` or `Ctrl and =` | Everything in the window gets larger |
| Press `Ctrl and -` | Everything in the window gets smaller |
| Press `Ctrl and 0` | A standard size, smaller than the one the app starts at |
| Press `Ctrl and +` six times | The theme button is drawn over **Convert to workbook**. Make the window wider to separate them |
| Press **🌙 Dark**, at the top right | The window turns dark, and the button changes to **☀ Light** |
| Press **☀ Light** | The window is light again |

The app starts in the same theme as your computer: light, unless you have set Windows to dark.

## Exercise 2: Work out the surplus for June

This is the app's main job. It compares what the EV chargers cost the building in one billing period with what the rates recovered from the EV charger owners.

Only the **June 2026** billing period can be run with the sample files. The meter readings stop on 24 June, so no later period has them.

1. Open the **Cost recovery** tab.
2. Choose these five files, in this order. The window asks for them from top to bottom.

   | Picker | Folder | File |
   |:---|:---|:---|
   | Toronto Hydro bill | `data\hydro_bills` | `TH_5728140000_2026_06_29.pdf` |
   | Green Button export | `data\green_button` | `TH_Electric_Usage_23-11-2024_to_24-06-2026.XML` |
   | Session report 1 | `data\evolute` | `Session_Report_May_1_2026-May_31_2026-seconds.csv` |
   | Session report 2 | `data\evolute` | `Session_Report_June_1_2026-June_30_2026-seconds.csv` |
   | Rates workbook | `data\rates` | `EV_Cost_Recovery_Rates.xlsx` |

3. Press **Work out the surplus**.

Expect:

```
Billing period ending 2026-06-23

Cost recovery        115.41
EV energy cost      -179.63
EV delivery cost     -92.02
Surplus             -156.24          (red)
```

The June period runs from 24 May to 23 June, so it uses the rates on row 2 of the workbook, effective `2026-05-01`.

A negative surplus, in red, means the rates did not cover the cost.

### What to look at

The report under the figures has sections that collapse and expand when you click their headings. Try it. The **Source Data** section goes into noteworthy details about the source data.

| Sub-section | What to check |
|:---|:---|
| *Session data* | Both session reports, May before June. Beneath them, `2 record(s) repeated a session already read …`: two sessions run past midnight on 31 May, so they appear in both months' reports. Each is counted once |
| *Sessions needing a look* | Four rows, all `DuplicateId`: `S83723` twice in May and `S37487` twice in June, with an explanation beneath |
| *Overall anomalies* | Everything unusual in each file, whether or not it changes the figures: 25 `ExcessiveAvgKw` and 2 `DuplicateId` for each of May and June, with an explanation of both |
| *Meter data* | The Green Button export, and nothing more: no anomalies detected in any of the metering intervals during the billing period. |

## Exercise 3: How the session report pickers behave

In the **Cost recovery** tab, each session report picker stays shut until a required prior selection is made. While shut, it says what it is waiting for:

| Picker           | Shut while                               | It says                                            |
| :--------------- | :--------------------------------------- | :------------------------------------------------- |
| Session report 1 | no bill is chosen                        | `Choose the bill first`                            |
| Session report 2 | Session report 1 is empty                | `Choose Session report 1 first`                    |
| Session report 2 | Session report 1 covers the whole period | `Session report 1 covers the whole billing period` |

To see them:

1. Close and reopen the app, so that nothing is chosen.
2. Open **Cost recovery**. **Session report 1** says `Choose the bill first`.
3. Choose the June bill, `TH_5728140000_2026_06_29.pdf`. The **Session report 1** button is enabled and **Session report 2** says `Choose Session report 1 first`.
4. Choose the June session report in **Session report 1**.
5. **Session report 2** opens: June alone leaves 24 to 31 May uncovered. Choose the May session report.

One report that does not cover the whole period is not an error. That is what **Session report 2** is for.

### Changing the bill

A different bill means a different billing period, so the app empties the session report pickers to match. It always empties **Session report 2**. It keeps **Session report 1** only if its report reaches into the new period.

1. Keep the June bill and the June session report in **Session report 1**.
2. Choose `TH_5728140000_2026_07_28.pdf` as the bill. Its period runs from 24 June to 23 July. The June report reaches a week into it, so it stays.
3. Choose `TH_5728140000_2026_05_28.pdf` as the bill instead. Its period ends on 23 May. Nothing in June belongs to it, so **Session report 1** empties.

## Exercise 4: Try different rates

In the **Cost recovery** tab, choose the practice copy in the **Rates workbook** picker. Choose the other four files as in Exercise 2.

| Do this in the practice copy | Then press **Work out the surplus**, and expect |
| :--- | :--- |
| Set the rates on row 2 to `0.30` / `0.30` / `0.30` | Surplus **136.95**, in teal instead of red, and "covered" in the report |
| Set the rates on row 2 to `0.20` / `0.20` / `0.20` | Surplus **0.75**, in teal: nearly break-even. Try other values to watch the sign flip either way |

Also try:

| Do this | Expect |
| :--- | :--- |
| Choose a rates workbook on the *Evolute reimbursement* tab after a run here | It is chosen here (*Cost recovery tab*) too, the figures here disappear, and the *Peak power detail* tab greys out |
| Change any picker after a run | The figures disappear, and the *Peak power detail* tab greys out |

## Exercise 5: A rate change within the billing period

Rates change from time to time, and a change rarely falls on the day a billing period starts. The app then prices each part of the period at the rates in effect at the time.

`data\rates\EV_Cost_Recovery_Rates-change-in-period.xlsx` shows this. Its `rates` sheet has three rows:

| effective_date | on_peak | mid_peak | off_peak |
|:---|---:|---:|---:|
| `2026-04-01` | `0.1000` | `0.0800` | `0.0600` |
| `2026-06-01` | `0.1200` | `0.1000` | `0.0800` |
| `2026-09-01` | `0.5152` | `0.4740` | `0.4218` |

The June period runs 24 May to 23 June, so it starts at the April rates and changes to the June
rates on 1 June. The September row is not used.

1. In the **Cost recovery** tab, choose the five files of Exercise 2, but choose `EV_Cost_Recovery_Rates-change-in-period.xlsx` in the **Rates workbook** picker.
2. Press **Work out the surplus**.

Expect:

```
Billing period ending 2026-06-23

Cost recovery        124.46
EV energy cost      -179.63
EV delivery cost     -92.02
Surplus             -147.19          (red)
```

The two costs are those of Exercise 2: only the recovery depends on the rates. The
*EV Cost Recovery* section states the total first, then one table for each set of rates. Each table is
headed by its effective date and the dates it priced:

```
| Item                          |      kWh | Recovery |
| At rates effective 2026-04-01 |  228.199 |    17.10 |
| At rates effective 2026-06-01 | 1133.806 |   107.36 |
| Billing period total          | 1362.005 |   124.46 |

EV rates effective 2026-04-01  (2026-05-24 - 2026-05-31)

| TOU      |     kWh | EV rate | Recovery |
| On-peak  |  70.516 | 0.10000 |     7.05 |
| Mid-peak |  29.385 | 0.08000 |     2.35 |
| Off-peak | 128.298 | 0.06000 |     7.70 |
| Total    | 228.199 |         |    17.10 |

EV rates effective 2026-06-01  (2026-06-01 - 2026-06-23)

| TOU      |      kWh | EV rate | Recovery |
| On-peak  |  309.411 | 0.12000 |    37.13 |
| Mid-peak |  214.024 | 0.10000 |    21.40 |
| Off-peak |  610.371 | 0.08000 |    48.83 |
| Total    | 1133.806 |         |   107.36 |
```

The two tables' kWh add up to the `1362.005` kWh total of the *EV Energy Cost* section. Each
session's energy is split before and after midnight on 1 June, so nothing is counted twice and nothing is lost
between them.

## Exercise 6: Mistakes the app catches

In the **Cost recovery** tab, each of these is a mistake that is easy to make. The app refuses to work out a figure from bad input, and says what is wrong.

**The wrong PDF as the bill.** Choose any PDF that is not a Toronto Hydro bill in the **Toronto Hydro bill** picker. As soon as it is chosen, the app says under the picker that it `couldn't parse input`, and **Session report 1** says `The bill above could not be read`.

**The wrong month's session report.** Choose `TH_5728140000_2026_05_28.pdf` as the bill, then the June session report in **Session report 1**:

```
This report covers 2026-06-01 to 2026-06-30, which is outside the billing period
2026-04-24 to 2026-05-23. Choose a report that reaches into the period.
```

The rest are mistakes in the rates workbook. Choose the practice copy and the files from Exercise 2, make the change, save, and press **Work out the surplus**.

**A blank rate.** Delete the mid-peak rate on row 2 (cell C2):

```
… cell C2, the mid_peak rate effective 2026-05-01, is empty
```

A blank is refused rather than read as zero.

**Dates out of order.** Change the date on row 3 to `2026-04-01`, earlier than row 2's:

```
rates workbook …\EV_Cost_Recovery_Rates - Copy.xlsx, sheet "rates": the effective_date on row 3,
2026-04-01, is not after the one on row 2, 2026-05-01. The effective dates must increase
down the sheet, with no date repeated
```

**A date typed as text.** Type `'2026-09-01` into cell A3. The apostrophe at the start makes Excel store it as text:

```
rates workbook …\EV_Cost_Recovery_Rates - Copy.xlsx, sheet "rates": cell A3 holds the text
"2026-09-01". An effective_date must be entered as a date, which the spreadsheet displays in
a date format
```

## Exercise 7: *Peak power detail* tab

***Note:** This exercise is about a function that supports detailed analysis not normally required for routine use of the software.*

Part of the building's bill depends on its highest power use in the period. This tab shows how much of that peak the EV chargers caused.

1. Run Exercise 2 again, with the original rates workbook.
2. Open the **Peak power detail** tab. It is greyed until a *Cost recovery* run succeeds.

The report has three sections, one per peak: *EV Peak kVA Contribution*, *EV Peak kW Contribution*, and *EV Peak kW 7-7 Contribution*. The value used for each charge is the one marked `*` in that section's *Estimates* table. The terms are explained under *Definitions and Conventions*, at the end of the report.

| Section | What to check |
|:---|:---|
| `kVA` | `2026-06-11 19:00 to 20:00 EDT`, energy-based **6.828** kVA |
| `kW` | The **same** hour — in this billing period, the building's kW and kVA peaked in the same 1-hour interval — energy-based **6.402** kW |
| `kW 7-7` | A **different** hour, `14:00 to 15:00 EDT`. It must fall between 07:00 and 19:00, so it cannot be the 19:00 one. Energy-based **1.971** kW |

The same three figures are in the `EV demand` column of *Delivery charges by component*, in the *Cost recovery* report. Compare the two tabs.

In each section, the `Interval` line and the `Segment` column say different things:

- The **interval** is the hour when the *building* peaked, from the meter readings.
- The **segment** is the 15 minutes inside that hour when the *chargers* peaked, from the session reports — `19:00` for the `kW` and `kVA` sections, `14:45` for `kW 7-7`. The charge is based on the segment.

The *Segments* table under each set of estimates gives each segment's `Session count` and
`Session kW`. The *Estimates* table's `All-in power` is worked out from these two;
*Definitions and Conventions* says how. For `kW 7-7`, the first three segments are empty and the whole figure comes from `14:45`. That makes it the clearest one for seeing what a segment contributes.

## Exercise 8: *Evolute reimbursement* tab

A separate question: did Evolute pay what our rates earned over a **calendar month**? A month is not the same as a billing period, so this figure and the surplus are not two views of one number.

Only June can be run with the sample test data. It is the one month with both a session report and a charges report.

1. Open the **Evolute reimbursement** tab.
2. Fill in:

   | Field | Value |
   |:---|:---|
   | Evolute Session Report | `data\evolute\Session_Report_June_1_2026-June_30_2026-seconds.csv` |
   | Evolute Charges Report | `data\evolute\XX-XX_Charges_June 2026-June 2026.csv` |
   | Reimbursement | `246.26` |
   | Rates workbook | `data\rates\EV_Cost_Recovery_Rates.xlsx` — already chosen if you chose it on the *Cost recovery* tab |

3. Press **Reconcile the month**.

Expect:

```
June 2026

Reimbursement received   246.26 $
Charges Report total    -246.26 $
Remittance variance        0.00 $

Reimbursement received   246.26 $
Cost recovery earned    -111.54 $
Dollar variance          134.72 $
```

Two comparisons, not one:

- The first asks whether the money received matches Evolute's own Charges Report. The report says "sent exactly what its own Charges Report comes to".
- The second asks whether it matches what our rates earned. The report says "reimbursed more than the cost-recovery rates come to".

Below the figures, in the sections:

| Section | What to check |
|:---|:---|
| *Cost recovery earned, by time of use* | Total of `1309.965` kWh used by EV chargers per the session report, priced at the EV cost-recovery rates, amounting to a total recovery of `$111.54`. |
| *Energy variance* | `1330.30` kWh on the Charges Report against `1309.97` priced, a difference of `20.33`. The two come from different documents and are not expected to agree exactly |
| *Session data* | The one session report |
| *Sessions needing a look* | Two rows, both `DuplicateId`, both session `S37487` |
| *Overall anomalies* | 25 `ExcessiveAvgKw` and 2 `DuplicateId` |
| *Charges Report* | The file the data was read from, and nothing more |

Also try:

| Do this | Expect |
|:---|:---|
| Type `0` into **Reimbursement** | Both variances negative — `-246.26` and `-111.54` — and "sent less than its own Charges Report" |
| Delete **Reimbursement** (it will show `0.00` after you delete the value in it) and run | `the reimbursement amount is blank` — a blank is refused. A blank field looks the same as a deliberately typed `0.00` but they are different |
| Choose the **May** session report instead of June | `the session reports do not cover the month 2026-06-01 to 2026-06-30:`, followed by the dates the May file does cover |

## Exercise 9: *Convert to workbook* tab

***Note:** This exercise is about functions that supports detailed analysis not normally required for routine use of the software.*

Turns one source file into an Excel workbook, saved in the same folder, for reading by eye. Nothing else in the app uses the workbook: the other tabs read the source files themselves.

**Convert** *Session report* or *Green Button export* chooses which kind of file. Each keeps its own file and its own last result, so switching between them loses nothing.

| Do this | Expect |
|:---|:---|
| Convert `data\evolute\Session_Report_June_1_2026-June_30_2026-seconds.csv` | The workbook `Session_Report_June_1_2026-June_30_2026-seconds.xlsx` appears in `data\evolute`. Anomalies are reported for 25 rows, every one of them `ExcessiveAvgKw` |
| Convert the same file again | A **Replace existing workbook?** box. **Cancel** leaves the workbook alone; **Replace** overwrites it, including anything you may have typed into it |
| Convert the Green Button export | A pause while several years of readings are read. The workbook has two sheets: `Peak_values`, one row per billing period, and `Interval_values`, one row per hour |

The app never overwrites an existing workbook without asking.

## Exercise 10: Saving and copying reports

Every tab has a **Save…** button and a **Copy** button under its report. **Copy** puts the report on the clipboard, to paste into an email or a document. **Save…** offers a name:

| Tab | Offered as |
|---|---|
| Cost recovery | `EV_Cost_Recovery_Surplus_<period ending>.report.md` |
| Peak power detail | `EV_Peak_Power_Detail_<period ending>.report.md` |
| Evolute reimbursement | `Evolute_Reimbursement_<YYYY-MM>.report.md` |
| Convert to workbook | `<workbook name>.conversion.report.md` |

You can save under any name you like. Save the report from Exercise 2: it runs to 165 lines.

Apart from converted workbooks and saved reports, the app writes nothing to your computer.

## Extra credit

The exercises below need files from `data\_archive`, or set up mistakes that are unlikely in normal use. Familiarity with them is not needed to use the app.

### Exercises with files from the archive

#### One session report instead of two

`data\_archive\evolute\Session_Report_May_1_2026-June_30_2026-seconds.csv` covers May and June in one file, as a single Evolute download for both months would. It covers the whole June billing period on its own.

Put it in **Session report 1**, with the other files of Exercise 2. **Session report 2** stays shut. The four amounts are the same as in Exercise 2, to the cent: the same sessions, read from one file. The saved report runs to 158 lines. *Session data* names the one file and reports the same 2 repeated
records, this time repeated within the file itself; *Overall anomalies* counts 50 `ExcessiveAvgKw`
and 4 `DuplicateId`.

On the *Evolute reimbursement* tab, the same file in place of the June session report gives the same figures as June alone. The month comes from the Charges Report; the session report only has to cover it.

#### One session report that contains the other

With the June bill, put `data\evolute\Session_Report_June_1_2026-June_30_2026-seconds.csv` in **Session report 1**. June alone leaves 24 to 31 May uncovered, so **Session report 2** opens. Put
`data\_archive\evolute\Session_Report_May_1_2026-June_30_2026-seconds.csv` in it:

```
Session report 2 covers 2026-05-01 to 2026-06-30, which already includes this
file's 2026-06-01 to 2026-06-30. Move that file to this slot and clear the
second, or choose a report reaching dates it does not.
```

The note appears on the picker that has to change, which here is the first: the wider file is the one to
keep. **Work out the surplus** stays disabled until it is fixed. **Clear** appears beside
**Session report 2** whenever it holds a file.

#### One day of overlap is enough

With the June bill, put `data\_archive\evolute\Session_Report_August_1_2026-September_4_2026.csv` into **Session report 1**:

```
This report covers 2026-08-01 to 2026-09-04, which is outside the billing period
2026-05-24 to 2026-06-23. Choose a report that reaches into the period.
```

One day of overlap is enough to be accepted. `data\_archive\evolute\Session_Report_July_1_2026-July_31_2026-mock.csv` gets the
same message under the June bill, and none at all under `TH_5728140000_2026_07_28.pdf`, whose period
runs 24 June to 23 July.

#### No meter data for the period

Choose `TH_5728140000_2026_07_28.pdf` as the bill, the June session report in **Session report 1**, and `data\_archive\evolute\Session_Report_July_1_2026-July_31_2026-mock.csv` in **Session report 2**. The two cover 24 June to 23 July between them, so the run gets as far as the meter data:

```
the meter data covers 24 of the 720 intervals in the billing period ending 2026-07-23,
so its maxima are not the period's
```

### Unlikely mistakes

The rates workbook changes below are made in the practice copy, with the files of Exercise 2.

**Text in a rate.** Type `eleven cents` into the mid-peak rate on row 2:

```
… cell C2, the mid_peak rate effective 2026-05-01, holds "eleven cents", which is not a number
```

**Bad rates in a row that is not used.** Type `tbd` into a rate on row 3. Nothing changes: row 3 takes effect in September, so the June period does not use it, and its rates are not checked.

**No rates in effect when the period starts.** Change the date on row 2 to `2026-06-10`:

```
rates workbook …\EV_Cost_Recovery_Rates - Copy.xlsx, sheet "rates": no rates are in effect on
2026-05-24: the earliest effective_date is 2026-06-10
```

**Two rate changes within one period.** Change the date on row 3 to `2026-06-01`, and add a row 4 dated `2026-06-15` with any rates:

```
rates workbook …\EV_Cost_Recovery_Rates - Copy.xlsx, sheet "rates": the rates change 2 times
within the billing period 2026-05-24 to 2026-06-23, on 2026-06-01, 2026-06-15. A billing
period can take one change at most
```

**A rate change in the middle of a month, on the *Evolute reimbursement* tab.** Change the date on row 3 to `2026-06-15`, and run Exercise 8 with the practice copy:

```
… the rates change on 2026-06-15 (row 3), within the month 2026-06-01 to 2026-06-30. A month is reconciled at one set of rates, so they can change only on the 1st
```

**A session report with the wrong name.** The app reads a session report's dates from its file name. In File Explorer, copy `Session_Report_June_1_2026-June_30_2026-seconds.csv` in `data\evolute`, and rename the copy `sessions.csv`. (If Windows hides the `.csv` ending, type just `sessions`.) Choose the June bill, then choose `sessions.csv` in **Session report 1**. It is refused on its name alone:

```
sessions is not a session report: the name must be
Session_Report_<Month>_<Day>_<Year>-<Month>_<Day>_<Year>.csv
```

**Work out the surplus** stays disabled until you replace it. Delete the copy afterwards.

**A Charges Report with the wrong name.** The app reads the month from the Charges Report's file name, too. Copy `XX-XX_Charges_June 2026-June 2026.csv` in `data\evolute`, and try these names for the copy on the *Evolute reimbursement* tab:

| Name the copy | Expect |
|:---|:---|
| Anything without `_Charges_<Month Year>-<Month Year>` in it, such as `charges.csv` | `is not a Charges Report`, or `does not state the months it covers`. The app will not read a file it cannot date |
| A name for more than one month, such as `XX-XX_Charges_June 2026-July 2026.csv` | `Only a single calendar month is accepted`. The reconciliation compares one month with one month |

Delete the copy afterwards.

**Two session reports with a gap between them.** Two reports that each reach into the period, but leave days between them uncovered, are refused: `the session reports do not cover the billing period …`, followed by what each file covers. The sample files cannot produce this.

### About the sample files

The Evolute portal states connection times to the second, with
`Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End`. The `-seconds` files were made (with `scripts/make-seconds-copies.py`) from `data\_archive\evolute\Session_Report_June_1_2026-June_30_2026.csv`, a sample provided by Evolute whose connection start and end times were cut to the minute. The May report was made from June's by `scripts/make-may-mock.py`.
