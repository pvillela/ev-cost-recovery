#!/usr/bin/env python3
# scripts/make-seconds-copies.py
#
# Builds the `-seconds` session reports in `data/evolute` from the minute-precision ones.
#
# The Evolute portal states `Conn_DateTime_Start` and `Conn_DateTime_End` to the second, and
# `Conn_DateTime_Start + Conn_Duration == Conn_DateTime_End`. The two files this reads predate the
# portal: they state a `Conn_Duration` to the second against a start and an end stated only to the
# minute, so almost every row of them misses that invariant and is excluded from every figure. They
# are kept as they were received; these copies are what the app is exercised with.
#
# Each output row is its input row with the start's seconds set to `:00` -- the minute file states
# no others -- and the end set to `start + Conn_Duration`. Nothing else about a record is touched.
#
# One row of each file is moved a second off that invariant, deliberately. Four of the five rows of
# `Session_Report_August_1_2026-September_4_2026.csv`, which is a real portal export, satisfy it
# exactly and the fifth is a second out; `DURATION_TOLERANCE` exists for that second and the sample
# data should exercise it. It is the first data row, so it is easy to find.
#
# One second and no more. A row further out than the tolerance is excluded from every estimate,
# which is a session's worth of energy missing from figures that a reader is checking against the
# bill -- and reading the sample data is not the moment to be teaching that lesson. Both files
# carried such a row until 2026-09-08.
#
# The May-June file is the two months' rows concatenated, under the name of the range they cover.
# It is what a single portal export spanning the whole billing period looks like, which is the case
# the app has to handle as readily as two monthly files -- and there is no such real export.
#
# No output is ever overwritten: move or delete it first. Same rule as `make-may-mock.py`, and for
# the same reason -- `data` is gitignored, so a file there has no history to recover from.
#
# Usage:
#     python3 scripts/make-seconds-copies.py

import csv
import sys
from datetime import datetime, timedelta
from pathlib import Path

EVOLUTE = Path(__file__).resolve().parent.parent / "data" / "evolute"

MAY_IN = EVOLUTE / "Session_Report_May_1_2026-May_31_2026-mock.csv"
JUNE_IN = EVOLUTE / "Session_Report_June_1_2026-June_30_2026.csv"
MAY_OUT = EVOLUTE / "Session_Report_May_1_2026-May_31_2026-seconds.csv"
JUNE_OUT = EVOLUTE / "Session_Report_June_1_2026-June_30_2026-seconds.csv"
BOTH_OUT = EVOLUTE / "Session_Report_May_1_2026-June_30_2026-seconds.csv"

# What the portal writes, and what the reader parses.
STAMP = "%Y-%m-%d %H:%M:%S"
# The jitter the real export shows, applied to the first data row of each month.
JITTER = timedelta(seconds=1)


def read_stamp(text: str) -> datetime:
    """A reported wall time, stated to the minute."""
    return datetime.strptime(text.strip(), "%Y-%m-%d %H:%M")


def read_duration(text: str) -> timedelta:
    """`Conn_Duration` as `H:MM:SS`."""
    hours, minutes, seconds = (int(part) for part in text.strip().split(":"))
    return timedelta(hours=hours, minutes=minutes, seconds=seconds)


def with_seconds(rows: list[dict[str, str]]) -> list[dict[str, str]]:
    """Each row's start and end restated to the second, the end implied by the duration."""
    out = []
    for i, row in enumerate(rows):
        row = dict(row)
        start = read_stamp(row["Conn_DateTime_Start"])
        end = start + read_duration(row["Conn_Duration"])
        if i == 0:
            end += JITTER
        row["Conn_DateTime_Start"] = start.strftime(STAMP)
        row["Conn_DateTime_End"] = end.strftime(STAMP)
        out.append(row)
    return out


def read_report(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open(newline="") as f:
        reader = csv.DictReader(f)
        return reader.fieldnames, list(reader)


def write_report(path: Path, fieldnames: list[str], rows: list[dict[str, str]]) -> None:
    # CRLF, which is what Evolute's own exports use.
    with path.open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames, lineterminator="\r\n")
        writer.writeheader()
        writer.writerows(rows)
    print(f"{path}  ({len(rows)} rows)")


def main() -> int:
    for path in (MAY_OUT, JUNE_OUT, BOTH_OUT):
        if path.exists():
            print(
                f"{path} already exists. Move or delete it first -- this script never overwrites "
                f"its output.",
                file=sys.stderr,
            )
            return 1
    for path in (MAY_IN, JUNE_IN):
        if not path.exists():
            print(f"no such report: {path}", file=sys.stderr)
            return 1

    fieldnames, may = read_report(MAY_IN)
    _, june = read_report(JUNE_IN)
    may, june = with_seconds(may), with_seconds(june)

    write_report(MAY_OUT, fieldnames, may)
    write_report(JUNE_OUT, fieldnames, june)
    write_report(BOTH_OUT, fieldnames, may + june)
    return 0


if __name__ == "__main__":
    sys.exit(main())
