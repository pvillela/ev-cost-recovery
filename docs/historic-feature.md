# The `historic` feature

Written 2026-08-26, when the feature was added.

## What it is

`historic` gates the code whose only callers are `ev_peak_cli` and `ev_peak_gui`. The library API
does not use it, the `ev_cost_recovery` desktop app does not use it, and a default `cargo build`
does not compile it.

```toml
[features]
historic = [] # code that does not support library API
```

## What is behind it

Two bodies of code, each with its own reason.

**Reading a workbook back.**

| Behind `historic` | Where |
|---|---|
| `session::excel::historic` — the whole module | `src/session/excel.rs` |
| `session::xlsx_to_sessions` | re-exported from that module |
| `session::xlsx_to_interval_estimates` | re-exported from that module |
| `time::duration_of_serial`, `time::instant_of_serial` | `src/time/excel.rs`, crate-private |

**Choosing an interval of interest.** Added 2026-08-27.

| Behind `historic` | Where |
|---|---|
| `session::ioi` — the whole module, and its ten tests | `src/session/ioi.rs` |
| `session::checked_interval`, `IoiLength`, `LEGAL_START_MINUTES`, `HourEntry`, `hours_of` | re-exported from that module |
| `time::TZ_OFFSETS` | `src/time/base.rs`; labels an ambiguous wall time with the zone it was read in |
| `time::TIME_ZONE_NAME` | crate-private, reached only by `ioi`'s doc links |

**The targets that need the feature to build.**

| Behind `historic` | Where |
|---|---|
| `ev_peak_cli` | `src/bin/ev_peak_cli.rs`, `required-features` in `Cargo.toml` |
| `ev_peak_gui` | `src/bin/ev_peak_gui/`, `required-features` in `Cargo.toml` |
| `examples/sessions.rs` | `[[example]]` `required-features` in `Cargo.toml` |

Everything else is unconditional, including the *writing* direction — `session_csv_to_xlsx` is API,
used by the Convert tab.

## Why the split falls there

The API reaches sessions from the CSV, through the crate-private `session::csv::csv_sessions`. The
workbook is a
faithful rendering of that same CSV, so reading one back gets you to the same place by a longer
road, plus one thing the CSV route cannot offer: a comparison of the workbook's stored derived
columns against the recomputed values, logged to `<stem>.xlsx.read.log`. That comparison is the
whole point of `ev_peak_gui`'s Estimate tab, and it is worth keeping. It is not worth carrying in
the library API, which has the source in hand and nothing to compare against.

Naming it `historic` rather than `workbook-reading` says which way this is going: the code stays
because the workflow it serves is still in use, not because the API is expected to grow into it.
The name earned itself on 2026-08-27, when `session::ioi` went behind the same gate for a reason
that has nothing to do with workbooks.

`ioi` answers one question: is this interval of interest a *legal* one — does it start on a legal
minute, is it one of the two legal lengths, and which zone offset does an ambiguous wall time mean.
That question arises only where someone picks the window by hand. The API is handed an interval;
the desktop app derives one from a billing period. So the module's only callers are the same two
binaries, and it belongs on the same side of the gate as the code that reads workbooks, despite
having nothing else in common with it.

## What a default `cargo test` stops checking

Adding to a feature is a test-coverage decision, and this one has a cost worth naming: `ioi`'s ten
tests now run only under `--features historic`. They cover the DST-ambiguity rules — which offsets
a wall time can mean on a fall-back morning, and how the interval is labelled — which is real
logic, not front-end presentation.

They go behind the gate anyway, for the reason the rest of this document gives: nothing else can
reach the code they test, so a default `cargo test` would be exercising a module the default build
does not compile. The mitigation is the same one `session::excel::test_historic` relies on — CI
runs both `cargo test` lines, not one.

## Building and testing

```sh
# The desktop app. Does NOT build ev_peak_gui or ev_peak_cli -- that is the segregation working.
cargo build --release

# The two historic binaries.
cargo build --features historic --bin ev_peak_cli --bin ev_peak_gui

# Tests. BOTH are needed: neither covers the other. Without the feature, the historic modules are
# not compiled; with it, the default build is never exercised.
cargo test
cargo test --features historic
```

`.github/workflows/release-build.yaml` runs both `cargo test` lines for the same reason.

## Nothing in `tests/` needs the feature

That is deliberate, and worth checking:

```sh
grep -rn historic tests/    # expect no hits
```

A test that needs `historic` is either testing the workbook round-trip on purpose — in which case
it belongs beside that code in `src/`, as `session::excel::test_historic` does — or it is reaching
for the workbook as a shortcut to something the API offers directly. Three integration tests were
in the second category when this feature was introduced; they claimed in their own opening lines to
run "through the public API" and did not. They now read the CSV and call the estimating function,
and they live in `src/session/` where that function is reachable. See
`docs/Plan_repair_after_manual_refactoring.md`, Part 2F.
