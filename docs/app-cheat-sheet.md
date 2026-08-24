# Kicking the tires on `ev_cost_recovery`

Every figure and message below was produced by running the real files in `data/`. If yours differ,
something has changed.

## Launching it

```sh
bash scripts/run-gui.sh
```

Use the script rather than `cargo run`. The file pickers need a desktop portal, and without one they
fail **silently** — clicking *Choose…* does nothing at all, with no error and no panic.

In the devcontainer the display is already set (`DISPLAY=:99`). Add `--release` after the script name
for a release build.

## The one run that works end to end

Only the **June 2026** period can be run. The Green Button export stops on 24 June, so no later
period has meter data. Pick these four, in the order the window asks:

| Picker | File |
|:---|:---|
| Toronto Hydro bill | `data/hydro_bills/TH_5728140000_2026_06_29.pdf` |
| Green Button export | `data/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML` |
| Session report 1 | `data/Session_Report_May_1_2026-May_31_2026-mock.csv` |
| Session report 2 | `data/Session_Report_June_1_2026-June_30_2026.csv` |

Then the rates. **Effective from** `2026-05-01`, and `0.1100` / `0.0900` / `0.0700`.

Press **Work out the surplus**. Expect:

```
Billing period ending 2026-06-23

Cost recovery        118.09
EV energy cost      -179.37
EV delivery cost     -93.94
Surplus             -155.22          (red, and "fell short" in the report)
```

The report below runs to 130 lines. May's file is a mock built by `scripts/make-may-mock.py`; only
June's is real.

## What to look at

**Cost recovery tab.** The four headline amounts are the report's own summary table — check they
agree. Collapse and expand the sections. *Session data* should name both CSVs. *Sessions needing a
look* should list a handful of rows with a glossary beneath, not a wall of them.

**Peak power detail tab.** Greyed until the run succeeds. Three sections:

| Section | What to check |
|:---|:---|
| `kVA` | `2026-06-11 19:00 to 20:00` |
| `kW` | The **same** interval — in this period the building's kW and kVA peaked together |
| `kW 7-7` | A **different** interval, and inside 07:00–19:00. It cannot be the 19:00 one |

Inside each, the `Interval` line and the `Segment` column say different things. The interval is where
the *building* peaked, from the meter. The segment is the 15 minutes inside it where the *chargers*
peaked, from the sessions — for the kVA section, `19:45`. The demand charge is billed on the segment.

## Things worth trying

| Do this | Expect |
|:---|:---|
| Rates `0.30` / `0.30` / `0.30` | Surplus **+135.29**, coloured as the accent rather than red, and "covered" in the report |
| Rates `0.20` / `0.20` / `0.20` | Surplus **−0.91** — near break-even, so you can watch the sign flip either way |
| Tick *rates changed*, leave `0.1100/0.0900/0.0700` from 1 May, add `0.30` flat from `2026-06-01` | Cost recovery **359.95**, surplus **+86.64**; the recovery report gains a second stretches table |
| Change any picker after a run | Figures vanish, and the detail tab greys out and drops you back |
| Clear one rate and run | `the mid-peak rate is blank` — refused, not read as zero |
| Type `eleven cents` into a rate | `cannot read "eleven cents" as the mid-peak rate: …` |

## Errors worth provoking

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
cp data/Session_Report_June_1_2026-June_30_2026.csv data/sessions.csv
```

Picking `data/sessions.csv` is refused at the picker, before anything is read, and
**Work out the surplus** stays disabled until you replace it. Delete the copy afterwards — nothing
else needs it, and a session report under a name that says nothing is exactly what the check exists
to catch.

## What it writes

A run rewrites a log beside each session report it read:

```sh
ls -l data/*.csv.read.log
```

Both timestamps should move on every run, whether or not you save anything. Nothing else is written
unless you press **Save…**, and each tab saves its own document.

## Checking it against the command line

The saved Cost recovery report is byte-for-byte what the CLI prints. Save it as `/tmp/gui.md`, then:

```sh
cargo run --bin cost_recovery_surplus_cli -- \
  data/hydro_bills/TH_5728140000_2026_06_29.pdf \
  data/TH_Electric_Usage_23-11-2024_to_24-06-2026.XML \
  2026-05-01:0.1100,0.0900,0.0700 \
  data/Session_Report_May_1_2026-May_31_2026-mock.csv \
  data/Session_Report_June_1_2026-June_30_2026.csv > /tmp/cli.md

diff /tmp/cli.md /tmp/gui.md
```

`diff` must print nothing. The Peak power detail tab has no command-line equivalent to compare
against.
