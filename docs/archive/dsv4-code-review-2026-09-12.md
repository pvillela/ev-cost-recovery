# Code and documentation review — 2026-09-12

Scope: `src/` (26,706 lines of Rust across 83 files), `tests/`, `examples/`, `build.rs`, the
packaging files, and every non-archived document (`README.md`, `CLAUDE.md`, `docs/**` outside
`docs/archive/`).

Revision reviewed: `55e6e26` (`main`, 2026-09-12), "Updated README.md and
docs/Development_Approach_and_Roles.md, archived Questions_for_Evolute.md". Toolchain:
rustc 1.98.1, cargo 1.98.1.

Method: thirteen read-only reviewers, one per module slice, each required to quote verbatim
evidence and to dedupe against `docs/archive/dsv4-code-review-findings.md`,
`docs/archive/fable-code-review-findings.md` and `_todo/_todo.md`; then parent verification of every
finding marked **[reproduced]** or **[read]** below, against the source and, where possible, the
shipped binaries.

Baseline at this revision:

| Check | Result |
| --- | --- |
| `cargo test --all-targets --all-features` | 340 pass, 0 fail, 2 ignored (both data-gated) |
| `cargo clippy --all-targets --all-features` | no warnings |
| `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps --all-features` | clean |
| Links in non-archived documents | 1 broken relative link; 7 stale file paths in 6 documents (see D10) |

**How to read this.** Each finding carries a verification mark:

- **[reproduced]** — reproduced by running a shipped binary on constructed input; the transcript is
  in the finding.
- **[read]** — verified by reading the cited lines; the quote is verbatim.
- **[reported]** — reported by a reviewer with quoted evidence and judged consistent with the code I
  read, but not independently re-run. Treat as a lead, not as established.

Severity: **high** = wrong result, data loss, or a crash on realistic input; **medium** = latent
bug, a document or comment that would cause a wrong change, or a missing test for behaviour that has
already changed once; **low** = real but limited cost.

---

## Summary

The crate is in better shape than either prior review found it. The build, clippy, the 340-test
suite and the intra-doc-link check are all clean; every high and medium item in the two archived
reviews is fixed except two, both in the reimbursement path (§A5, §A6); the structured-error rule in
`CLAUDE.md` is followed everywhere I checked, and `#![deny(private_interfaces, private_bounds,
unnameable_types)]` at the crate root now enforces the visibility rules rather than relying on review.

What remains is concentrated in four places:

1. **One reproducible crash on a malformed row** — the tolerance at the heart of
   `duration_is_consistent` admits a one-second inversion, and the invariant the maintenance manual
   uses to justify a deliberate panic does not hold at that boundary (§A1). This is the only finding
   I would fix before the next release.
2. **Error labelling and report self-contradiction** — a malformed `Charge_Duration` is reported as a
   *write* failure naming two files (§A3), and an excluded session is printed under a heading that
   says it counts towards the figures (§A4).
3. **Money and unit arithmetic that the recovery path got right and the reimbursement path did not**
   — unrounded variance columns (§A5) and unguarded NaN narration (§A6).
4. **Documentation debt with a mechanical cause** — bare `src/...:<line>` citations have rotted;
   `docs/site-specific-constants.md` is wrong in ten of its eleven (§D8), and
   `docs/session/segment-tiling.md` contradicts itself (§D9).

The most common *kind* of defect in the tree is stale prose: a comment or document that was true
before the portal alignment and the removal of the `historic` feature, and was not re-read when the
mechanism it described went away (§D1–§D6). Two of those are visible to users in generated artifacts.

---

## A. Correctness

### A1 — HIGH — A one-second inversion passes the consistency check and panics every estimator **[reproduced]**

`src/session/common.rs:69-79`, `docs/maintenance-manual.md:288-296`, `src/time/base.rs:34-37`

The only duration check is a tolerance comparison:

```rust
let implied_end = conn_start + conn_duration;
...
gap.unsigned_abs() <= DURATION_TOLERANCE
```

`docs/maintenance-manual.md:292-294` states the invariant that makes `Session::intersects`'s
deliberate panic safe:

> Nothing legitimate violates it. `conn_duration` is unsigned, so `conn_start + conn_duration` is
> never before `conn_start`; an inverted span therefore misses `conn_end` by more than
> `DURATION_TOLERANCE` and the record is flagged `InconsistentDuration` and sorted into
> `Sessions::excluded` …

"More than" is false. `DURATION_TOLERANCE` is one second, and the gap is `<=` it, so a record whose
reported end precedes its reported start by exactly one second and whose `Conn_Duration` is
`0:00:00` is **consistent**, is not flagged, is not excluded, and reaches the estimating logic.

Reproduction — one row of `data/evolute/Session_Report_June_1_2026-June_30_2026-seconds.csv`
changed to `Conn_DateTime_End = 2026-06-05 16:25:59` (one second before its start) and
`Conn_Duration = 0:00:00`, with the May report beside it:

```
$ ev_csv_to_xlsx Session_Report_June_1_2026-June_30_2026.csv
<path>.xlsx                                     # the conversion succeeds; the row is not flagged
$ cost_recovery_cli 2026-06-23 2026-05-01:0.11,0.09,0.07 <may>.csv <june>.csv
thread 'main' panicked at src/time/base.rs:36:29:
interval ends at 2026-06-05T21:25:59Z before it starts at 2026-06-05T21:26:00Z
```

`Session::interval()` is `Interval::from_start_end(conn_start, conn_end)`, and `peak.rs:223` calls
`s.intersects(&segment.interval)` for every session in `sessions.sessions` and `sessions.spikes`
(`peak.rs:166-171`). The panic lands in `Interval::from_start_end` before anything is reported, so
the app and every CLI die with no output, no exit code of their own, and no record of which row did
it.

The same claim is repeated in `duration_is_consistent`'s own doc (`common.rs:65-68`), so the hole is
described as closed in two places. It is prior-review item C3 of
`docs/archive/fable-code-review-findings.md`, closed there by removing the gap/fold machinery — but
the boundary case the tolerance creates was not considered.

**Suggested fix:** test the inversion before the tolerance, not through it — a record with
`conn_end < conn_start` is `InconsistentDuration` regardless of how small the inversion is — and
correct the manual and the doc comment to say what the code does. Pin the boundary
(`end = start - 1s`, `Conn_Duration = 0:00:00`) with a test beside `inconsistent_duration_is_reported`
in `src/session/csv.rs`.

### A2 — MEDIUM — A negative or non-finite `Energy_Use` is accepted and mispriced **[reproduced]**

`src/session/csv.rs:360`, `src/session/common.rs:487-534`

`Energy_Use` is parsed as a bare `f64` with no sign or finiteness check. The value flows into

```rust
pub fn energy_based_load(&self) -> Load {
    Self::scaled_load(self.agg_kw() / ev_real_power_kw())   // common.rs:487
}
...
let full_panels = (scaling / panel_capacity).floor();       // common.rs:533
```

so a negative `Energy_Use` gives a negative `scaling`, `.floor()` yields **−1** full panels, and
`idle_panels = panel_count - full_panels - partial_panels` gains a panel while the full-panel block
is added negated. `NaN` poisons the whole figure and never raises `ExcessiveAvgKw` (`NaN > x` is
false).

Reproduction: `Energy_Use = -5` in row 3 of the June report is accepted by `ev_csv_to_xlsx` with
**no anomaly naming that row** (only the file's pre-existing `ExcessiveAvgKw` rows appear), and
`cost_recovery_cli` prices the month without comment. This is prior-review item #13 of
`docs/archive/dsv4-code-review-findings.md`, still open.

**Suggested fix:** refuse non-finite or negative energy where the cell is parsed, the way rates and
amounts are already refused (`checked_figure` in `src/bin/ev_cost_recovery/state.rs:233`), or assert
`scaling >= 0.0` in `load_over_panels` — and add a row for it to the reader's test set.

### A3 — MEDIUM — A malformed `Charge_Duration` is reported as a write failure, naming two files **[reproduced]**

`src/session/excel.rs:150-165`, `src/session/csv.rs:176-190`

`session_csv_to_xlsx` splits its two halves deliberately (`excel.rs:154-158`): reading failures go
into `ConversionError::Input`, which adds nothing, because `SessionCsvError` already names its own
file; only the writing half goes into `ConversionError::Write`, which adds the workbook path.

`Charge_Duration` is never parsed by the reading half — `csv_session_rows` resolves only
`Conn_Duration` and `Active_Charge_Time` — and is parsed lazily at write time by
`SessionRows::duration`, which raises a `SessionCsvError` carrying the CSV path and row. That error
is then wrapped by the *write* arm:

```
$ ev_csv_to_xlsx Session_Report_June_1_2026-June_30_2026.csv   # row 3, Charge_Duration = "not-a-duration"
<path>.xlsx: Session Report <path>.csv: row 3, column `Charge_Duration`: cannot read
"not-a-duration": expected an elapsed time as H:MM:SS
```

Both paths appear, the write is blamed for a read failure, and — the part that matters — the same
file is accepted by the reader the API exposes (`csv_sessions` priced the whole month without
complaint), so the two halves disagree about what a valid session report is.

This is the rule in `CLAUDE.md` ("A wrapper that adds the path must not wrap a cause that already
carries one") being defeated by a column read late.

**Suggested fix:** validate `Charge_Duration` in the reading half (it is one of the columns
`CsvSession` already parses neighbours of), or route the lazy read's error back into `Input`.

### A4 — MEDIUM — An excluded session is printed under "These sessions count towards the figures" **[reproduced]**

`src/session/report.rs:200-210`, `src/session/common.rs:610-622`

`energy()` builds its notes with `AnomalyKind::bears_on_energy`, which matches
`InconsistentDuration | DuplicateId`; `Sessions::notes` chains the excluded bucket in as well. The
rendered report therefore prints one record in two sections that contradict each other. From the
shipped `energy_cli` on the repo's own `-bad` fixture:

```
Sessions left out
These records cannot be placed on a timeline, so they take no part in any figure above …
| Session_Report_June_1_2026-June_30_2026.csv |   3 | S39090  | InconsistentDuration |

Sessions needing a look
These sessions count towards the figures above, and something about them needed a judgement call …
| Session_Report_June_1_2026-June_30_2026.csv |   3 | S39090  | InconsistentDuration |
```

The footnote under the second table then says the opposite of its own heading: "the session is
excluded from every estimate". A reader reconciling the month is told the same row both counts and
does not.

**Suggested fix:** partition the two lists — an `excludes_session` kind belongs to "left out" only —
or state in the "needing a look" intro that a row may also appear above.

### A5 — MEDIUM — The reimbursement variance columns are not rounded to the cent, against their own comment **[read]**

`src/api/pure/reimbursement.rs:234-235, 305-311`, versus `src/api/pure/recovery.rs:498-530`

```rust
dollar_variance: reimbursed - cost_recovery_amount,
remittance_variance: reimbursed - charges_report_amount,
```

Both are stored unrounded and printed through `amounts`, which formats every cell with `{:.2}`
(`src/markdown.rs:138-144`). The comment above the "Reimbursement received / Charges Report total /
Remittance variance" block claims:

> // Negative so the column adds down to the variance, which is a subtraction and
> // cannot be checked against two positive numbers.

Each cell is rounded independently, so the column does not always add down: `1.005` and `1.000` print
as `1.00` and `1.00` while their difference prints `0.01`. The recovery path hit this exact defect and
fixed it — `to_the_cent` (`recovery.rs:525`) exists solely so the printed surplus agrees with its own
column, and carries the argument:

> A surplus that disagreed with its own column in that case would be the one defect this rounding
> exists to prevent.

with a test sweeping the printed amounts against the printed surplus. The reimbursement tables were
never migrated: they carry the generic `rounding_note()`, which disclaims the very discrepancy their
comment promises against.

**Suggested fix:** round the two variance figures through `to_the_cent`, as the surplus is, and align
the note with what the column then guarantees.

### A6 — MEDIUM — The reimbursement verdicts narrate a non-finite variance as a definite outcome **[read]**

`src/api/pure/reimbursement.rs:247-258, 261-273`; prior review #4, still open

```rust
if variance.abs() < 0.005 { … } else if variance > 0.0 { … } else { "… less than …" }
```

Every comparison against `NaN` is false, so a `NaN` variance takes the `else` branch and the report
asserts Evolute underpaid, beside a column printing `NaN`. The sibling `recovery::verdict` guards
exactly this (`recovery.rs:737`, with a test that passes `NaN` and `±inf`).

Nuance that changes the fix's urgency but not its correctness: the shipped GUI can no longer produce
it — both the rates (`state.rs:206-217`) and the reimbursement amounts (`state.rs:860-869`) now pass
through `checked_figure`, which rejects non-finite and negative values — but
`reconcile_evolute_reimbursement` is public API taking raw `f64`, and the verdict functions remain
unguarded and untested.

**Suggested fix:** return a neutral sentence when `!variance.is_finite()`, in both functions, and
extend `a_variance_below_a_printed_cent_is_called_neither_way` with `NaN`/`±inf`.

### A7 — LOW — The Charges Report number parser accepts `NaN` and infinities **[read]**

`src/charges_report.rs:613-636`

`parse_number` strips thousands separators and then `cleaned.parse()` — which accepts `NaN`, `inf`,
`-inf` and returns `inf` for `1e999`. A cell carrying any of those is summed into the month's kWh
or amount and reaches the reconciliation as a non-finite figure, in a reader whose stated posture is
"an error rather than a partial sum".

**Suggested fix:** reject a non-finite parse through `bad_value`, as the comma check beside it
already does.

### A8 — LOW — `markdown::table` enforces none of its contracts, and has no tests **[read]**

`src/markdown.rs:25-66`

Three unguarded assumptions in the renderer every report goes through: `r[i]` panics on a row
shorter than `headers`; the final `.zip(align)` silently truncates a row when `align` is shorter
than the headers; and no test in the crate touches this module (it is one of the few files with no
`#[cfg(test)]` at all). A caller passing a short row gets a panic in a report; a caller passing a
short `align` gets a quietly narrower table.

**Suggested fix:** an assertion or a documented `# Panics` section on `table`, and a unit test over
the three shapes (short row, short align, empty rows).

### A9 — LOW — `report_sections` measures bytes where the headings are generated in characters **[read]**

`src/bin/ev_cost_recovery/state.rs:1156` versus `src/markdown.rs:96-102`

The library underlines a heading to `s.chars().count()`; the GUI recognises a heading with
`rule.len() != title.len()`, which is bytes. For any heading containing a non-ASCII character the
two rules disagree, the heading is not recognised, and the section is silently swallowed into the
previous one — no error, no visible mark. Latent today (no heading is non-ASCII), but the two copies
of one rule have already diverged.

**Suggested fix:** compare `chars().count()` on both sides, or exporting the rule from `markdown`.

---

## B. Testing gaps

The suite is strong where it has been aimed: 299 library unit tests, 27 in the app, 12 across the
integration targets, and goldens for the Green Button workbook, the interval report, the site-load
report and the session report tables. What follows is what it does not defend.

| # | Gap | Evidence |
| --- | --- | --- |
| B1 | **The session workbook's cell values are unpinned.** `round_trip_produces_the_expected_workbook` (`src/session/excel.rs:519`) asserts the header order, one UTC cell, two formulas, some formats, and a couple of values. A writer bug transposing `Energy_Use`/`Total_Fee`, or a wrong duration, passes: both the header and the rows are driven by the same `COLUMNS` table. The Green Button writer has a committed golden; this one has none. | [read] |
| B2 | **The bill PDF parse is only exercised behind `#[ignore]`.** `hydro_bill_from_pdf`'s sole end-to-end test, `tests/hydro_bill/all_bills.rs:39`, is ignored and reads `data/hydro_bills`, which is not in the repository. No bill fixture is committed. A moved label or reordered column ships undetected by the default `cargo test` — which is what CI runs. | [read] |
| B3 | **The real ESPI export is only exercised behind `#[ignore]`.** `tests/green_button/full_feed.rs:23` is the only check that the join rules hold together on the 18 MB export; the fast tests use a two-reading synthetic feed. | [read] |
| B4 | **`definitions()` is unpinned.** The section that explains the model to users — half-open intervals, the All-in power model, how to read the times — is rendered by `definitions()` (`src/session/report.rs:606`) into the CLI report (`peak_power_cli.rs:98`) and into the GUI's Detail tab and saved document (`detail.rs:68,102`). No test references it, and no golden covers it, so it can drift from `docs/session/README.md` silently. | [read] |
| B5 | **The non-finite branch of the reimbursement verdicts is untested** — see A6. The recovery sibling has both the guard and the test. | [read] |
| B6 | **Seven of the ten binaries have no test at all.** From the suite's own output: `cost_recovery_cli`, `cost_recovery_surplus_cli`, `energy_cli`, `energy_cost_cli`, `ev_csv_to_xlsx`, `peak_power_cli`, `peak_power_cost_cli` each report `running 0 tests`. `gb_peak_values`, `hydro_bill_dump` (2 each) and the app (27) have some. `src/markdown.rs` is likewise untested. | [read] |

---

## C. Modularity and consistency

### C1 — `Session.conn_duration` is populated and never read **[read]**

`src/session/common.rs:164` is written on every parsed session (`csv.rs:419`) and read by nothing in
production: the consistency check consumes `CsvSession.conn_duration` *before* the `Session` is
built (`csv.rs:395`), and the workbook writer reads the raw column from the source table. Its only
reader is `test_support.rs:100`. This is the case `CLAUDE.md` names — "when a field stops being read,
nothing tells you" — on a public field every external caller must fill in for no effect.

### C2 — `charges_month` is public, has no caller, and two doc comments say the reader uses it **[read]**

`src/charges_report.rs:179` defines it; the reader calls `parse_charges_report_name(stem)` directly
(`:494`), while the module doc (`:472`) and `ChargesReport::month`'s doc (`:189-190`) both attribute
the work to `charges_month`. Either route the reader through it or delete it and correct both docs.

### C3 — The time-of-use band row is defined twice **[reported]**

`recovery.rs:557` (`band_row`) and `reimbursement.rs:335-341` (an inline closure) format the same
four cells with the same widths, under the same header and alignment. A change to one (rate
precision, a new column) drifts the other. `CLAUDE.md`'s "one utility that every caller uses, or
none" points at extraction into `markdown` or `api::pure`.

### C4 — `commas_group_thousands` is written out twice, and says so **[read]**

`src/charges_report.rs:637-650` and `src/hydro_bill/bill_pdf.rs` carry the same rule, with a comment
that reads "The same rule is written out in … Change one and change the other." The earlier
`with_extension` duplication was resolved by deleting the half-shared helper, and the argument there
was that only two of seven callers reached it; here both callers are in-crate and the rule has real
subtlety (digit triples, not just "delete commas"), which is the case for one definition rather than
a note.

### C5 — The `energy` test module redefines shared fixtures that have already drifted **[reported]**

`src/api/pure/energy.rs` re-declares `period_ending_date()` (:517), `bill()` (:623) and `close()`
(:673), all of which exist in `src/api/pure/test_support.rs` — the module whose doc says a second
definition "would be a second place for the two to drift apart". The two `bill()` fixtures already
disagree (`loss_factor_adjustment: 1.05` / `on_peak_kwh: 10000.0` against `1.0295` / `13000.0`), so
the drift is no longer hypothetical.

### C6 — `parse_rates` is duplicated verbatim between two CLIs **[read]**

`src/bin/cost_recovery_cli.rs` and `src/bin/cost_recovery_surplus_cli.rs` each define it, with a
comment acknowledging the duplication and asking that both be changed together. It parses into the
library's own `CostRecoveryRates`, so the library is the natural home.

### C7 — Two doc/code mismatches in `peak_power`, one of which invites a wrong "fix" **[read]**

- `src/api/pure/peak_power.rs:152` — the `delivery_cost` field is documented as "net of HST and
  OER", but the figure is `charges + hst - ontario_electricity_rebate`: HST is *added*, and only the
  rebate is netted. A reader trusting the doc would subtract HST and produce a wrong delivery cost.
  The sibling `HydroBill::bill_total_amount` states the same arithmetic correctly ("+ hst - rebate").
- `src/api/pure/peak_power.rs:443` — "Taken before the maxima are read off, since those move
  `gb_period_values` field by field" justifies an ordering that needs no justification: `Peak`
  derives `Copy` (`src/green_button/peaks.rs:18`), so nothing is moved.

---

## D. Comments and documentation

The dominant failure mode here is prose that survived the mechanism it described. Two of these are
visible to users inside generated artifacts.

### D1 — The workbook tells its reader that a cell is read back, and nothing reads it **[read]**

`src/session/excel.rs:433` writes this comment into the `anomalies` column of **every** generated
workbook:

> Empty means the row needed no judgement call. This cell IS read back, and InconsistentDuration is
> what removes a session from every estimate — so editing it changes the figures. The adjusted
> columns are not read back: they are recomputed, and a disagreement is written to the
> `.session.xlsx.read.log` rather than obeyed.

The module doc two screens above says the opposite (`excel.rs:11`: "Nothing reads the column back"),
`csv_sessions`'s doc says the workbook "is never read back", and the `historic` reader that once did
so has been removed. A user is told that hand-editing the cell changes the figures; a maintainer is
told a read-back path needs preserving. Neither is true.

### D2 — `csv_sessions` promises an off-grid warning that no longer exists **[reported]**

`src/session/csv.rs:114-118` lists "the off-grid warning if it applies" among what the run log
carries. The session grid was removed when the portal confirmed second-resolution times
(`green_button/common.rs:23-25`); `csv_session_rows` only records anomalies.

### D3 — `TIME_ZONE_NAME` names a module that does not exist **[read]**

`src/time/base.rs:14`: "Referenced by `session::ioi` and several doc comments". There is no
`session::ioi` — `src/session/` holds `common`, `csv`, `energy`, `excel`, `file_name`, `peak`,
`report`, `site_model` — and the constant is now referenced only inside `time::base` itself.

### D4 — `lib.rs` says `ConversionError` is public "only through this module" **[read]**

`src/lib.rs:13`. It is also the public return type of `session::session_csv_to_xlsx`
(`session/excel.rs:153`) and `green_button::write_gb_workbook` (`green_button/excel.rs:295`). The
module's visibility is justified by all three routes, not by the API alone.

### D5 — The `spikes` field doc contradicts the code on both of its claims **[read]**

`src/session/common.rs:745-756`:

- "Kept out of `session` because those values would swamp or poison any segment they entered" — the
  estimate chain puts them into the segments on the same footing as any other session
  (`peak.rs:166-171`, `.chain(&sessions.spikes)`), and what a session contributes is energy prorated
  over its connection span, not `avg_kw`.
- "[`Session::avg_kw`] substitutes a finite figure so the row can still be listed" — `avg_kw`
  performs no substitution, and its own doc (`common.rs:259-260`) says "Non-finite when `charge_time`
  is zero, and left that way".

### D6 — The `parse_session_report_name` doc example is a name the function rejects **[read]**

`src/session/file_name.rs:87` documents the form `Session_Report_June_1_2026-June_30_2026.csv`, but
the function does not strip an extension: `report_date("June_30_2026.csv")` fails, so passing that
name returns `BadDate`. Every in-crate caller passes `file_stem` first (`report_coverage`,
`state.rs:418`), so the only person this bites is a caller who follows the public doc — or the error
message, which repeats the same `.csv` form.

### D7 — The maintenance manual's safety argument is off by the tolerance **[read]**

`docs/maintenance-manual.md:288-296` — see A1. The section exists precisely to explain why the
panic is safe, and the case it excludes on paper is the case that crashes.

### D8 — `docs/site-specific-constants.md`: ten of eleven line citations point at the wrong line **[read]**

Verified citation by citation. `TIME_ZONE_NAME` (`:16`) is exact; the other ten drift by one to a few
lines or land on an unrelated construct:

| Citation | Cited line holds | The constant is at |
| --- | --- | --- |
| `billing_period.rs:70` | the doc of the *next* constant | `:68` (`BILL_END_DAY`) |
| `time/base.rs:88` | blank | `:87` (`BILLING_OFFSET`) |
| `green_button/common.rs:23` | prose inside the rationale | `METER_INTERVAL` below it |
| `hydro_bill/bill_pdf.rs:41` | `MONTHS`' doc | `:39` (`CHARGE_COLUMN_RIGHT`) |
| `hydro_bill/pdf_text.rs:37` | `ROW_TOLERANCE`'s doc | `:38` |
| `session/csv.rs:54` | the closing `];` of the array | header of the array |
| `charges_report.rs:27` | a `use` statement | `REQUIRED_HEADERS` below |
| `session/excel.rs:418` | prose inside a cell comment | `SESSION_REPORT_PREFIX` |
| `green_button/espi.rs:52` | blank | `:50-51` (`UOM_KW`, `UOM_KVA`) |
| `session/common.rs:59` | blank | `SEGMENT_DURATION` |

The document is a list of pointers; every pointer but one has rotted. Citing the symbol
(`green_button::METER_INTERVAL`) rather than the line would make the document survive the next edit.

`docs/ERRORS.md` is in visibly better shape — of the nine `src/...:line` references I sampled, all
land on the construct their entry describes, several with one to three lines of drift (e.g. `:133`
cites `charges_report.rs:305-311`; the code is at `:303-307`). The same advice applies: the
citations are bare line numbers with no symbol, so they cannot be checked mechanically.

### D9 — `docs/session/segment-tiling.md` contradicts itself, and the defects the author filed are still open **[reported]**

`_todo/_todo.md` lists this document as "seriously messed-up". Confirmed, and one item is worse than
described:

- **The membership table contradicts the prose six lines below it.** The table (`:73`) lists
  `A, B, C, D, E` in the 16:15 segment; `:79` says "**`B` is not in the 16:15 segment.** `B` ends at
  16:15, exactly where the segment starts." The machine-checked artefacts agree with the prose:
  `src/session/segment_tiling_tests.rs`'s `MEMBERSHIP` holds `("17:15", &["A", "C", "D", "E"])`, and
  the committed fixture prints the same. Every `agg_count` in the document is computed without `B`.
  A reader who trusts the table would "fix" the tiling code to reintroduce the minute padding the
  portal alignment removed.
- **The "Span used" column and the ASCII sketch still carry that padding.** Each row pads its end by
  a minute (`A` → 17:04, `B` → 16:16, `C`/`F` → 16:43 …) where the fixture reports 17:03, 16:15,
  16:42, while the document's own text says twice that the reported times are exact and nothing is
  adjusted. The same stale sketch survives in the test module that owns the fixture:
  `src/session/segment_tiling_tests.rs:13-21` draws `A` ending at `17:04`, `B` at `16:16`, `C` and
  `F` at `16:43`, `D`/`E` at `16:35`, `G` at `16:56` — each end a minute past what the fixture
  reports — while the prose nine lines below (`:25-27`) says "Spans are `[conn_start, conn_end)` —
  the reported times, taken at face value … The right edge used to be padded a minute past the
  reported end, and that padding is what made them overlap." `MEMBERSHIP` (`:62-71`) is correct and
  matches the golden, so only the two pictures are wrong — which is why they mislead rather than
  fail.
- The author-tracked defects (clock times labelled standard while the diagram is local; a sentence
  naming a `17:15` value that appears in no table; "report" used for both the session report and the
  rendered report) are still present.

### D10 — Broken and stale pointers in the non-archived documents **[read]**

Verified by resolving every relative link and backticked path in the non-archived documents:

- `docs/ERRORS.md:613` links `Questions_for_Evolute.md`, which the reviewed commit moved to
  `docs/archive/`. The link is broken; the sentence still sends the reader to `docs/`.
- `docs/session/README.md:135` cites `docs/Evolute-Simultaneous_Charging.pdf`; the file is
  `docs/session/Evolute-Simultaneous_Charging.pdf`.
- `docs/session/segment-tiling.md:4,6` cite `tests/fixtures/Session_Report_Diagram.csv` — the file
  is at `tests/fixtures/sessions/Session_Report_Diagram.csv`, so the path is missing its `sessions/`
  segment — and `tests/session/segment_tiling.rs`, which does not exist; the tiling tests are
  `src/session/segment_tiling_tests.rs`, and the rendered report it points at is
  `tests/fixtures/sessions/Session_Report_Diagram.report.md`.
- `docs/session/site-model-marcus.md:349` cites `docs/session/archive/…`; it is
  `docs/archive/session/ev-charger-power-factor-and-kva-allocation-20260828.md`.
- `docs/maintenance-manual.md:563,612` cite
  `docs/green_button/reference/Green_Button_Peak_Values-python-2026-07-16.xlsx`; the file lives at
  `data/reference/green_button/…`.
- `docs/Evolute_portal_alignment.md:16,44` cite `examples/sessions.rs` (the examples are
  `site_load_report.rs` and `gb_trim_fixture.rs`) and `docs/session/time-reporting-uncertainty.md`
  (archived under `docs/archive/session/`).
- `docs/maintenance-manual.md:73-75` names three fixtures with no directory at all, where all three
  live under `tests/fixtures/sessions/` — the same missing segment as the `segment-tiling`
  reference above.

### D11 — `README.md` misstates what `cargo build --release` builds **[read]**

`README.md:244`: "the desktop app, `ev_cost_recovery` — and nothing else". With no `[[bin]]`
sections, cargo builds all ten binaries under `src/bin/`. The sibling line `cargo test # everything`
is also imprecise: it does not build the examples, and one example is currently uncompilable without
`--all-targets` being exercised anywhere (see E2).

### D12 — User-visible help text breaks a phrase mid-line **[read]**

`src/bin/ev_csv_to_xlsx.rs:12-13` hard-wraps "judgement call" across two source lines, so the shipped
help prints:

```
Rows needing a judgement
call — a session with no charge time, …
```

---

## E. Packaging and CI

### E1 — No CI on push or pull request **[read]**

`.github/workflows/release-build.yaml:2-4` triggers on `v*` tags only. Ordinary commits run no
tests, no clippy and no doc-link check, so a broken `cargo test` is discovered when a release is
tagged.

### E2 — The examples are never compiled by CI **[read]**

The workflow's only test and build steps are `cargo test --verbose` (`:71`) and
`cargo build --release …` (`:82`). Neither passes `--all-targets`, so `examples/site_load_report.rs`
and `examples/gb_trim_fixture.rs` are compiled by no job. This repository's own `CLAUDE.md` opens
with the lesson ("Check all targets, not just the library"), and the local pre-release command
`cargo test --all-targets --all-features` is what the memory summary for this repo records.

### E3 — The third-party notices stamp authenticates the inputs, not the text **[reported]**

`build.rs:79-97` — `release_notices()` accepts `THIRD-PARTY-NOTICES.md` whenever the appended stamp
matches `input_hash()` (`:90`), and `input_hash()` (`:97`) hashes `Cargo.lock`, `about.toml` and
`about.md.hbs` — never the generated notices. Removing a licence section while leaving the stamp
intact passes a release build, contrary to `about.md.hbs`'s claim that an edited copy fails to build.

### E4 — `about.toml`'s two comments disagree about `r-efi` **[read]**

`about.toml:10-12` says the GPL/LGPL exclusions are "what makes self_cell resolve to Apache-2.0 and
r-efi to MIT", while the `targets` comment above (`:1-2`) says r-efi is dropped by target filtering
because it is reached only for UEFI targets. One of the two describes work that never happens.

---

## F. Prior reviews: what is fixed, what remains

Re-checked against `docs/archive/dsv4-code-review-findings.md` (2026-08-31) and
`docs/archive/fable-code-review-findings.md` (2026-08-31). Fixed items I verified personally:

| Prior item | Status at `55e6e26` |
| --- | --- |
| dsv4 #1 / fable — panic on an inverted session sharing an id | **partially fixed**; the tolerance-boundary case is A1 |
| dsv4 #3 — Charges Report kWh column did not strip thousands separators | fixed: `number` and `money` share `parse_number` (`charges_report.rs:604-636`) |
| dsv4 #5 / fable — a blank line aborted the whole CSV read | fixed: verified by inserting a blank line mid-file and at EOF; the read is identical and row numbering is unaffected |
| dsv4 #6 / fable C3 — a session end in the DST gap shifted silently | fixed by design: the session clock is a fixed offset and there is no gap to fall in (`csv.rs:604-607`) |
| dsv4 #14 — `from_source` clippy warning | fixed: clippy is clean |
| fable C1 — `ev_csv_to_xlsx` overwrote silently | fixed: `OnExistingWorkbook::Refuse` (`ev_csv_to_xlsx.rs:37`) |
| fable C2 — a case-variant extension bypassed the input-is-the-output guard | fixed: the guard tests the input's extension, case-insensitively, rather than comparing the two paths (`api/io.rs:597-609`) |
| fable C7 — within-file duplicate collapse contradicted its own doc | fixed: the doc now states that one code path serves both and says how it logs (`session/common.rs:362-365`) |
| fable C5 — three CLIs wrote no meter log | fixed: `peak_power_cli`, `peak_power_cost_cli`, `cost_recovery_surplus_cli` each call `meter.write_log()` |
| fable C6 — rates/amounts accepted `NaN`, `inf`, negatives | fixed in the GUI: `checked_figure` (`state.rs:233-243`) |
| fable C8 — the charges cross-check test split lines naively | fixed: `cells()` is quote-aware and documents why |
| fable C9 — the anomaly-token round-trip test omitted a variant | fixed: all seven are listed (`green_button/common.rs:234-246`) |
| fable C18 — `PowerEstimates` lacked `Debug` | fixed (`peak_power.rs:42`) |
| fable C12 — an unresolvable `Tf` dropped all following text silently | fixed: `PdfTextCause::UndeclaredFont` is now raised with a message naming the font (`pdf_text.rs:165-170`) |
| fable C13 — `BillingPeriod::ending_on` did not bound `bill_end_day` | fixed: the range is asserted first, with the reason stated (`billing_period.rs:116-119`) |
| fable — missing meter log in `cost_recovery_cli` (raised as a question) | not a gap: `meter` lives on `CostRecoverySurplus` (`recovery.rs:177`), and that CLI's `CostRecovery` carries none |

Still open, confirmed here: dsv4 #4 (A6), dsv4 #13 (A2), dsv4 #12 (the CMap `bfrange` list-length
check still truncates silently, `pdf_text.rs:425-433`), and the bill-parser
`expect("just matched")` at `bill_pdf.rs:429`, which panics on layout drift where every other shape
mismatch in that parser returns a `BillError`.

---

## G. Suggested order of work

Every finding in this report appears in exactly one tier below; between them the tiers name all
thirty-two. Ordering is by risk first, cost second — the first tier is what I would not ship
without, and the last is investment rather than repair. Nothing here is recommended to be left
alone.

**Tier 1 — before the next release**

1. **A1**, with its documentation half **D7** — test the inversion before the tolerance, correct the
   manual and `duration_is_consistent`'s doc, and pin the boundary case with a test. A shipped crash
   on a malformed row, and a small fix.
2. **A3, A4, A2** — validate `Charge_Duration` in the reading half (or route its error back into
   `Input`); stop printing an excluded session under the heading that says it counts; refuse
   negative and non-finite `Energy_Use` before it reaches `load_over_panels`.
3. **A5, A6 (+ B5), A7** — round the reimbursement variance columns through `to_the_cent`, guard the
   two verdicts against a non-finite variance, and bound the Charges Report number parser with
   `is_finite`. The tests for A6's branch belong with the fix: the recovery sibling has both.

**Tier 2 — correctness and robustness cleanup**

4. **A8** — give `markdown::table` an assertion or a `# Panics` section, and unit-test the three
   shapes (short row, short `align`, empty rows); it is the renderer every report passes through, and
   one of the few files in the crate that no test touches at all.
5. **A9** — make `report_sections` measure characters, as `h1`/`h2` do, or export the rule from
   `markdown` so there is only one.
6. **C7** — correct the `delivery_cost` doc ("including HST and net of the rebate") and delete the
   "field by field" justification; the first is the one doc in the tree that would cause a wrong
   arithmetic change.

**Tier 3 — documentation**

7. **D1, D2, D3, D4, D5, D6** — the stale prose, cheapest first. D1 is user-visible in every
   generated workbook; the other five are a paragraph each, and each names a mechanism that no
   longer exists.
8. **D8, D9, D10, D11, D12** — rebuild `site-specific-constants.md` and `ERRORS.md` around symbols
   rather than line numbers; rewrite `segment-tiling.md` (and the sketch in
   `segment_tiling_tests.rs`) from the fixture it claims to describe; fix the six stale paths and the
   broken link; correct `README.md`'s build description and the wrapped help text.
9. **E4** — settle which of `about.toml`'s two `r-efi` comments is true and delete the other.

**Tier 4 — structure, once the above is quiet**

10. **C1, C2** — delete `Session.conn_duration` and `charges_month`, or give each a reader; both are
    public surface that nothing reads, and C2's two doc comments currently point at the dead one.
11. **C3, C4, C6, C5** — one definition each for the TOU band row, `commas_group_thousands` and
    `parse_rates`; drop the `energy` test module's private copies of the shared fixtures. All four are
    drift hazards that have already drifted at least once (C5's two bills disagree today).

**Tier 5 — test and CI investment**

12. **B1, B2, B3, B4, B6** — a golden for the session workbook; a committed bill fixture; a decision
    on the two ignored data-gated tests (nightly job, or promote them with trimmed fixtures); a pin
    for `definitions()`; and tests for the seven binaries that have none, starting with the argument
    and rate parsing in the pair that duplicate each other (C6).
13. **E1, E2, E3** — a push/PR workflow running `cargo test --all-targets --all-features`, clippy and
    the doc-link check; `--all-targets` on the release workflow's two commands so the examples compile
    somewhere; and a notice stamp that covers the generated text rather than only its inputs.
