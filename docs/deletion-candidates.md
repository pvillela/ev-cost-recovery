# Public surface: who reaches what

Written 2026-08-26, as the survey the refactoring brief asked for and did not get:

> Any library code that does not support the API will be identified and:
> - If the code does not support any binary, it should be marked for deletion.
> - If the code supports a binary but not the API or `ev_cost_recovery` GUI app, it should be gated
>   with the new feature "historic".

**This is a list, not a set of deletions.** What goes is a judgement call; this says what the
evidence is.

## Method

Every item rustdoc renders a page for (285 of them, with `--features historic`) was matched by name
against `src/api/`, `src/bin/`, `src/` outside those two, and `tests/` + `examples/`, and sorted by
the narrowest reader that reaches it. Name matching is coarse — a short name can collide — so treat
each row as a lead, not a verdict.

To reproduce, after `cargo doc --no-deps --features historic`, grep each rendered item name across
those four roots.

## Result

| Category | Count | Meaning |
|---|---|---|
| API | 72 | reached from `src/api/` |
| lib-internal | 46 | reached only from elsewhere in `src/`, not from `api` or `bin` |
| binary-only | 16 | reached from `src/bin/` but not from `src/api/` |
| test-only | 5 | reached only from `tests/`, `examples/` or a `#[cfg(test)]` module |
| **unreached** | **0** | — |

**Nothing is unreached, so nothing is marked for deletion.** That is the survey's main finding, and
it is worth having stated rather than assumed.

## Binary-only items, and which binaries reach them

The interesting column is whether a binary-only item is reached *only* by `ev_peak_cli` and
`ev_peak_gui`. Those two are already behind `historic`; an item reached only by them is a candidate
for the gate that the gate has not yet caught.

| Item | Reached by | Gate it? |
|---|---|---|
| `session::HourEntry` | `ev_peak_gui` | **gated 2026-08-27** |
| `session::IoiLength` | `ev_peak_cli`, `ev_peak_gui` | **gated 2026-08-27** |
| `session::LEGAL_START_MINUTES` | `ev_peak_gui` | **gated 2026-08-27** |
| `session::checked_interval` | `ev_peak_cli`, `ev_peak_gui` | **gated 2026-08-27** |
| `session::hours_of` | `ev_peak_gui` | **gated 2026-08-27** |
| `time::TZ_OFFSETS` | `ev_peak_cli`, `ev_peak_gui` | **gated 2026-08-27** |
| `session::xlsx_to_interval_estimates` | `ev_peak_cli`, `ev_peak_gui` | **already gated** |
| `session::xlsx_to_sessions` | `ev_peak_cli`, `ev_peak_gui` | **already gated** |
| `hydro_bill::BillError` | `hydro_bill_dump` | no — also `api::io`'s `ReadError::Bill` cause |
| `hydro_bill::read_pages`, `write_pages` | `hydro_bill_dump` | no — the PDF dump tool is not historic |
| `hydro_bill::bill_start_day` | `gb_peak_values` | no |
| `green_button::Feed` | `gb_peak_values` | no — also `api::io::gb_xml_to_xlsx` |
| `time::holidays`, `time::local_date` | `gb_peak_values` | no |
| `time::time_zone` | `ev_cost_recovery`, `ev_peak_gui` | no — the desktop app uses it |

### On the six candidates

They are `src/session/ioi.rs` almost entirely, plus `TZ_OFFSETS`. That module is the interval-of-
interest vocabulary: which interval starts are legal, how long an interval may be, what hours a
report covers. Only the two historic binaries ask those questions, because only they let a user
*choose* an interval — the API derives its intervals from the meter peak instead.

**Settled 2026-08-27: all six are gated,** along with the whole of `src/session/ioi.rs` and the
crate-private `time::TIME_ZONE_NAME`, which nothing but `ioi` reached. Both arguments recorded
against doing it were answered rather than overruled:

1. *"`ioi` is pure, and gating a module of types and predicates is a different kind of quarantine
   from gating an I/O module."* True, and it turned out to be the wrong axis. What the feature
   sorts by is **who calls it**, not what it touches — the name `historic` says so. `ioi` is
   reached by the same two binaries as the workbook reader and by nothing else, which is the only
   test the gate applies.
2. *"A future API entry point taking a caller-chosen interval would want these rules."* If one is
   ever added, ungating is one `#[cfg]` line, and the rules will be intact because the module was
   gated whole rather than picked apart. Gating says the entry point does not exist today, not that
   it never will.

The cost is real and is recorded in [`historic-feature.md`](historic-feature.md): `ioi`'s ten
tests, which cover the DST-ambiguity rules, now run only under `--features historic`.

## test-only items

| Item | Note |
|---|---|
| `session::site_load_report`, `session::site_load` | rendered by `examples/site_load_report.rs` and pinned by a golden. Public on purpose: the table is a deliverable |
| `time::holidays::Holiday`, `time::tou_of` | used by tests and by `gb_peak_values`'s holiday report |
| `hydro_bill::pdf_text::Line` | the PDF layout type; `hydro_bill_dump` prints them |

None of these is a deletion candidate. They are public because something outside the crate renders
them.

## `green_button::for_test`, and what came of it

A first version of this document said `for_test` existed so that four integration tests could
"build a `Feed` by hand", and left open whether it should exist at all. The first half was wrong
and the second is now settled. Recording both, because the reasoning generalises.

**What the tests were actually doing.** All five call sites read a fixture file and parsed it:

```rust
let xml = fs::read_to_string(<a fixture path>).unwrap();
let feed = parse_espi_xml(&xml).unwrap();
```

None built a feed by hand and none edited the XML in memory. That is `read_gb_feed`'s whole body,
and `read_gb_feed` was already public. So the export was not letting the tests reach something
otherwise unreachable — it was letting them re-implement, by hand, a function sitting next to the
one they imported. Removing it *deleted* duplication rather than creating any.

**What actually kept it alive.** `period_values` was public, and it takes a `&Readings` — a type
reachable only through `for_test`. Nothing in `src/api/`, `src/bin/` or the desktop app called it;
its only external callers were three test sites. It was a test hatch with no marking on it, and
`for_test` was quietly holding its signature together.

**Settled 2026-08-26.** `period_values` is `pub(crate)`, `Readings` stays private, `for_test` is
gone, and the three test sites moved into `src/green_button/` as `invoice_tests` and
`pipeline_tests` — the same treatment three session tests got the same day, for the same reason.
The two integration tests that only needed a parsed feed stayed in `tests/` and call
`read_gb_feed`.

The alternative was to widen: export `Readings` beside `Feed` and keep `period_values` public. It
was rejected because nothing in this crate or its five binaries wants every period as values, and
because widening the API so that a test can stay outside is the opposite of the judgement applied
everywhere else in that day's work. When a real caller appears, exporting it then is a two-line
change with a reason behind it.

**The general shape**, worth recognising next time: a public item whose only external callers are
tests is a test hatch, whether or not it is named like one. `for_test` was the marked case;
`period_values` was the unmarked one, and it was the one doing the damage.

## The same question, asked of `session::csv`

Settled the same way, the same day, once `csv_sessions` had a structured error type to return.

`session::csv` was a public module whose one remaining public item was `csv_sessions`; everything
else in it — `SessionRows`, `Row`, `csv_session_rows` — was already `pub(super)`. Its callers are
`api::io` and `session::excel`, both inside the crate, and four `#[cfg(test)]` modules. Nothing
outside reaches it: the API takes paths and hands back figures, never a `Sessions`.

The module is now `pub(crate)`, and `csv_sessions` and `SessionCsvError` with it. That last part is
the point worth keeping: a reader's error type is only as public as the function returning it needs
it to be. Of the four readers, only `BillError` has a genuine external consumer —
`hydro_bill_dump` calls `is_layout()` to change the advice it prints. `GbReadError` and
`ChargesReportError` are named by nothing outside their own modules, and are public only because
`read_gb_feed` and `charges_report` are.

`ReadError`'s doc used to offer a downcast to `BillError` "for a caller that wants to ask". Nothing
in the crate or its binaries ever downcast to anything. The doc now records that as a fact about
the implementation rather than as a contract.

Rendered public items: 285 at the time of the survey, 281 after this and the `for_test` removal.
