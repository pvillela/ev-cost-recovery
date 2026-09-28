# Plan: remove the run logs

Agreed 2026-09-28, in a design interview. Recorded here as the decisions were made; the code and
the living documents are the description of what was built.

## Decision

Run logs are removed from the desktop app and from every CLI. Anything noteworthy that only a log
said is shown in the regular report instead. Saving a conversion's findings becomes optional,
through the same **Copy / Save…** row the other tabs have.

What the logs held, against what the screen showed:

| Log | Against the screen |
| --- | --- |
| `meter.xml.read` (Cost recovery) | Same. The meter notes are on screen. |
| `charges.csv.read` (Reimbursement) | Same. One function writes both. |
| `session.csv.read` | More: every anomaly in the whole CSV, where the screen shows only those in the interval of interest that bear on the figure. |
| `session.csv.read`, collapsed duplicates | Only in the log. |
| `session.convert` | Same lines as the screen. The workbook's Anomalies column holds them too. |
| `meter.convert` | Slightly more: example hours and a description per anomaly kind. The workbook highlights every hour. |

## Scope

- The app and the CLIs follow one policy.
- The whole log mechanism goes: `src/log.rs`, the `logs` fields, every `write_log(s)`, the app's
  "log could not be written" messages, the log sentences in the CLI `--help` texts.
- `.log` files already in users' folders are left alone. No cleanup code.
- Every living markdown document that mentions logs is updated: `README.md`,
  `docs/maintenance-manual.md`, `docs/session/README.md`, `docs/green_button/README.md`,
  `docs/app-cheat-sheet.md` and `docs/ERRORS.md` throughout, plus the module comment of
  `tests/docs_errors.rs`. `docs/archive/` and `_todo/` are not touched.

## Analysis reports

Applies to Cost recovery and Evolute reimbursement. Peak power detail is an extension of Cost
recovery and gets nothing of its own.

- Everything about the inputs goes at the end, under an `h1` **Source Data**. The Reimbursement
  report gains that heading.
- A third heading level is added, to the markdown helpers and to the app's section splitter.
- Under **Session data**, as sub-sections: *Sessions left out*, *Sessions needing a look*, and a
  new **Overall anomalies**:
  - present only when a file has anomalies;
  - one line per file, a count per anomaly kind, counting every anomaly in the file — the
    sessions listed above included;
  - ending in one tool-neutral sentence inviting the reader to convert the session report to a
    workbook to see every anomaly with its row.
- Under the list of session files, one line counting the records dropped as identical copies,
  e.g. "4 sessions appeared in more than one file and were counted once."
- Meter data and the Charges Report get nothing new. The meter log covered only the billing
  period, which the screen shows; the Charges Report is read all-or-nothing.

## Conversion reports

- No *Source Data* or *Overall anomalies* headings: the whole report is about its one source.
- When there are anomalies, the report says so and points to the workbook's anomalies column.
- The meter conversion report carries what its log did: example hours per anomaly kind and each
  kind's description.
- Markdown, with the **Copy / Save…** row. Saved into the workbook's folder, offered as
  `<workbook stem>.conversion.report.md`.
- The conversion CLIs print that same markdown to stdout, so a saved report and a piped one are
  the same file.
