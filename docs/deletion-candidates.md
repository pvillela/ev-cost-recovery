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
| `session::HourEntry` | `ev_peak_gui` | candidate |
| `session::IoiLength` | `ev_peak_cli`, `ev_peak_gui` | candidate |
| `session::LEGAL_START_MINUTES` | `ev_peak_gui` | candidate |
| `session::checked_interval` | `ev_peak_cli`, `ev_peak_gui` | candidate |
| `session::hours_of` | `ev_peak_gui` | candidate |
| `time::TZ_OFFSETS` | `ev_peak_cli`, `ev_peak_gui` | candidate |
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

Gating them is defensible and was not done. Two things argue against doing it now without a
decision from you:

1. `src/session/ioi.rs` is pure. Gating it means a `#[cfg]` on a module of types and predicates,
   which is a different kind of quarantine from gating an I/O module.
2. `checked_interval` and `LEGAL_START_MINUTES` encode the interval rules that
   `docs/session/README.md` specifies and that a future API entry point taking a caller-chosen
   interval would want. Gating them says that will not happen.

## test-only items

| Item | Note |
|---|---|
| `session::site_load_report`, `session::site_load` | rendered by `examples/site_load_report.rs` and pinned by a golden. Public on purpose: the table is a deliverable |
| `time::holidays::Holiday`, `time::tou_of` | used by tests and by `gb_peak_values`'s holiday report |
| `hydro_bill::pdf_text::Line` | the PDF layout type; `hydro_bill_dump` prints them |

None of these is a deletion candidate. They are public because something outside the crate renders
them.

## One thing this survey does not cover

`green_button::for_test` re-exports all of `espi` so four integration tests can build a `Feed` by
hand. It is `#[doc(hidden)]` as of 2026-08-26, so rustdoc renders no page for it and the survey
above cannot see it. Whether it should exist at all — rather than the tests reaching a feed through
`read_gb_feed` and a fixture file — is a real question and is not answered here.
