# The `historic` feature

Written 2026-08-26, when the feature was added.

## What it is

`historic` gates the code reached only by targets that themselves require the feature —
`ev_peak_cli`, `ev_peak_gui` and `examples/sessions.rs`. The library API does not use it, the
`ev_cost_recovery` desktop app does not use it, and a default `cargo build` does not compile it.

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
| the `time::TZ_OFFSETS` **re-export** | the constant itself is unconditional — see below |
| the `time::TIME_ZONE_NAME` **re-export** | crate-private; likewise unconditional |

Those last two rows are a different kind of entry from the others, and the distinction matters.
`TZ_OFFSETS` and `TIME_ZONE_NAME` are declared unconditionally in `src/time/base.rs` and are
load-bearing in **every** build: `time_zone()` resolves `TIME_ZONE_NAME` on every call, and
`BILLING_OFFSET` is an entry of `TZ_OFFSETS`. What is gated is only the path out of `time` — the
`pub use` and `pub(crate) use` lines in `src/time/mod.rs` — because outside `base` the only caller
of either is `session::ioi`, plus the two binaries for `TZ_OFFSETS`. Gating a re-export says
nothing about whether the item is compiled or used.

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
columns against the recomputed values, logged to `<stem>.session.xlsx.read.log`. That comparison
is the whole point of `ev_peak_gui`'s Estimate tab, and it is worth keeping. It is not worth
carrying in the library API, which has the source in hand and nothing to compare against.

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

Adding to a feature is a test-coverage decision. `ioi`'s ten tests now run only under
`--features historic`, five of them about DST.

**This is a smaller loss than it first looks, and the distinction is worth being exact about.**
The crate resolves DST ambiguity in two places, deliberately separate, and only one of them moved:

| | `session::csv`, `CsvSession::resolve` | `session::ioi`, `map_local` |
|---|---|---|
| Question | an Evolute record reports a local start in the fall-back hour — which instant is it? | a *user* names a wall time — what could it mean? |
| Evidence | the reported end and an untruncated `Conn_Duration` | none; a wall time is all there is |
| Outcome | resolves it, or duplicates the record as EDT and EST and flags it | reports the ambiguity for the front-end to put to the user |
| Bears on a figure | **yes — every kWh and kW attribution** | no |
| Gated | no | yes |
| Tests | 7, ungated | 5, now gated |

`ioi`'s doc comment states the split: *"The session reader faces the same ambiguity with more
evidence — an untruncated `Conn_Duration` — and settles it."* So a default `cargo test` still
covers DST everywhere a bill figure depends on it: the fold resolved from the reported end, the
hour-early candidate rejected, the unresolvable record duplicated, the gap shifted forward and
reported, and a fold-spanning session's true elapsed duration. Those are `session::csv`'s seven,
and they are where the correctness of the numbers lives.

What went behind the gate is the *input-validation* half — refusing a start in the DST gap,
requiring a designator on a fold start, checking a designator against its date, and which hours a
picker offers. Real logic, and its own implementation rather than a wrapper over the other one, so
it is not covered by proxy. But nothing outside the two front-ends can reach it, and no figure
rests on it.

The mitigation is the same one `session::excel::test_historic` relies on — CI runs both
`cargo test` lines, not one.

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
