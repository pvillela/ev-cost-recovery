# Kicking the tires on `ev_cost_recovery`

Every figure and message below was produced by running the real files in `data/`. If yours differ,
something has changed.

**You need those files first.** `data/.gitignore` is `*`, so nothing under `data/` is in the
repository — a fresh clone has none of it. The bills, the meter export and the session reports have
to be brought to the checkout by hand.

## Launching it

```sh
cargo run --bin ev_cost_recovery
```

In the devcontainer the display is already set (`DISPLAY=:99`).

The file pickers need a session bus, but you do not have to arrange one: with the portal packages
this image installs, the GTK stack starts a bus itself when none is set, and it exits with the app.
Checked by killing every `dbus-daemon`, running the binary bare, and clicking *Choose…* — the
chooser opened, and two buses appeared that had not been there before.

If a picker ever does nothing at all when clicked, run `bash scripts/run-gui.sh` instead; it says
what it is for and why you should not have needed it.

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

## Running the released Windows build

For someone with no Rust and no clone of this repository — the path the app is actually distributed
by. The figures, the errors and the two tabs are all the same as above; what differs is getting the
program and getting the files to it.

### Getting it

The Windows build is produced by `.github/workflows/release-build.yaml`, which runs when a tag
matching `v*` is pushed. It is a **workflow artifact**, not a GitHub Release:

1. Open the repository on GitHub, then **Actions** → the **Release Binaries** run for that tag.
2. Under **Artifacts**, download **`binary-x86_64-pc-windows-msvc`**.
3. It arrives as a `.zip`. Extract it — Windows will otherwise run the program from inside the
   archive, where it behaves oddly.

Two things about artifacts: you must be signed in to GitHub to download one, and they expire (90
days by default). If the download is not there, the tag build needs re-running.

Extracting gives four files:

| File | What it is |
|:---|:---|
| `ev_cost_recovery.exe` | The program. Nothing else is needed to run it |
| `THIRD-PARTY-NOTICES.md` | Licences of every crate linked in. Also readable inside the app, under **About** |
| `LICENSE-MIT`, `LICENSE-APACHE` | This program's own terms |

No installer, no runtime to put on the machine first, and nothing is written to the registry. To
uninstall, delete the folder.

### Starting it

Double-click `ev_cost_recovery.exe`.

The first launch shows SmartScreen's **"Windows protected your PC"**, because the binary is not
code-signed. Choose **More info**, then **Run anyway**. Later launches are silent.

**There is no console window**, and that is deliberate: the release build sets
`windows_subsystem = "windows"`. Nothing the program might print is visible, so every failure it can
report is shown inside the window instead, in a bordered red block beside the control that caused
it. A window that never appears at all is the one case with nothing to read — and that means the
`.exe` did not start, not that a step failed.

### Getting the files to it

Put the bill, the meter export and the two session reports somewhere writable — your Documents
folder or the Desktop, not `C:\Program Files` and not a folder still inside the downloaded zip.

Writable matters. Every run writes a `.csv.read.log` beside each session report it reads. If that
folder is read-only the run reports it and stops before showing any figures, which reads as a
failure of the calculation when it is a failure to write a log.

The two session reports must keep the names Evolute gave them —
`Session_Report_June_1_2026-June_30_2026.csv` and the like. The name is the only thing that says
which month a file holds, and the app refuses one that does not say, at the moment you pick it. The
bill and the meter export can be named anything.

### What to check

Everything from **The one run that works end to end** onwards applies. The four pickers, the rates,
the expected surplus of **−155.22**, the rate variations and the errors worth provoking are all the
same — the figures come from the files, not from the platform. Ignore the `data/` prefix in those
tables; it is where the files sit on a development machine, and yours are wherever you put them.
Only the file names matter, and only for the two session reports.

Two things from above do **not** apply:

- **Checking it against the command line.** The artifact contains the app and nothing else — no
  `cost_recovery_surplus_cli` — so there is nothing on that machine to diff against. Do that check
  on a development machine.
- **The bad-name test** needs a copy under a name without dates. In File Explorer: copy the CSV,
  paste it in the same folder, and rename the copy to `sessions.csv`.

To see the run logs, open the folder holding the session reports and look at the **Date modified**
column on the two `.csv.read.log` files. Both should change on every run, whether or not you save
anything.
