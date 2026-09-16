# Code and documentation review — 2026-09-14

Reviewed by Claude Fable 5.1, as asked in `Prompt_Code_and_docs_review-20260914.md`.

Scope, in two parts:

1. The code (`src/`, `tests/`, `examples/`, `build.rs`, packaging, CI) and every non-archived
   document, on `main` at `382f2c5`.
2. The review in `dsv4-code-review-2026-09-12.md` and the five commits on branch `dsv4-fixes`
   (`c289b4a` … `3ce26ce`, branched from `5c621cd`) that implement it: whether each
   recommendation was sound, whether the implementation is right, and which to adopt.

Toolchain: rustc 1.98.1, cargo 1.98.1.

Method: the branch diff (3,880 lines against `main`) was read in full by me. The independent review
of `main` was split into seven read-only slices, each reviewer required to quote verbatim evidence
with `path:line` and to dedupe against the DSV4 review; every finding kept below was then checked
against the source by me. Findings I could not confirm myself are marked **[reported]**.

**How the labels work.** Two sets of findings appear here and they are numbered differently on
purpose. The DSV4 review's findings keep that review's own letters, and outside Part 1 they are
always cited with its name: `DSV4 C7`, `DSV4 A1`. Findings and sections of my own are numbered by
the part they sit in: `2.2` is the second section of Part 2, and `3.4.6` is the sixth finding in
the fourth group of Part 3. A bare letter therefore always means DSV4, and a number always means
this document.

Baseline checks:

| Check | `main` (`382f2c5`) | `dsv4-fixes` (`3ce26ce`) |
| --- | --- | --- |
| `cargo test --all-targets --all-features` | 299 lib, 27 app, 2 + 2 bin, 12 integration pass; 2 ignored | 318 lib, 28 app, 3 + 3 + 2 + 2 bin, 12 integration pass; 2 ignored |
| `cargo test … -- --ignored` (real data present) | not run | both pass |
| `cargo clippy --all-targets --all-features -- -D warnings` | clean | clean |
| `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps --all-features` | clean | clean |
| same, with `--document-private-items` | **10 broken links** (3.2.1) | not run |
| `cargo fmt --check` | clean | **11 files not formatted** (2.1) |

The branch was built from a copy of its tree in a scratch directory, because this session may not
switch branches.

---

## Summary

**The DSV4 review is sound, with one exception.** I re-derived every one of its thirty-two findings
against the source. Thirty are correct as stated. One is overstated (B6, the binaries "with no
tests"), one understates its own case (D8 sampled nine `ERRORS.md` citations and found small drift;
of 33 I checked, 16 land on unrelated code), and **one is wrong**: E2 says the examples "are
compiled by no job", but `cargo test` builds examples by default. `cargo test --no-run -v` on this
tree issues rustc invocations for both `site_load_report` and `gb_trim_fixture`, so the release
workflow's own test step already compiles them. Its tiering is right: A1 is the only shipped crash,
and it is a two-line fix.

**The branch implements all of it, and most of it well.** The five commits close every finding
in the report. The correctness fixes (A1–A9) are correct and each carries a test pinned at the
boundary it draws. The two goldens it adds (the session workbook and the definitions section) are
the best testing investment on the branch.

**Three things stop it being merged as it stands:**

1. It is not `rustfmt`-clean. Eleven files, all in code the branch touched. `main` is clean.
2. It conflicts with `main`: the branch rewrites `docs/session/segment-tiling.md`, which `main`
   deleted in `3aaf32b` two days later. The rewrite corrects the citations and the padded spans,
   but it settles the clock question the wrong way, so the conflict is a decision and the deletion
   is the better side of it. See 2.1.2.
3. Two placement choices the author of `_todo/_todo.md` has already objected to: `parse_rates` in
   `src/api/mod.rs`, and four helpers in `src/api/pure/mod.rs`. Both belong elsewhere, and
   `parse_rates` also returns a `String` error, which `CLAUDE.md` forbids in library code.

**Two changes on the branch should be rejected.** The first is the rewrite of
`docs/session/segment-tiling.md`, for the reason under blocker 2 above. The second is making the
two data-gated integration tests pass silently when their data is absent, and then running them in
CI where the data can never be present. The Rust harness has no runtime skipped outcome, so such a
test reports success, and `cargo test` captures the explanation, leaving nothing on screen to say
it did not run. `main` already carries four tests that skip silently (3.1.4), and all four have
been passing on paths that moved, which is the argument against adding two more rather than for it.

**The independent review of `main` found no new defect that produces a wrong figure.** The code is
in good shape: the money arithmetic, the ESPI join, the time-of-use and holiday rules, the billing
period arithmetic and the tiling all held up against every input the reviewers could construct.
What it did find is in Part 3: a handful of medium code items (a panic that reaches jiff before the
assertion meant to stop it, a public conversion that overwrites without asking, four data-gated
tests that pass without running, an error message that calls a calendar month a billing period),
three user-visible
statements that are false, one systematic comment problem the repository's own rules forbid, and —
the largest single item — **a maintenance manual whose first half names error variants, anomaly
kinds and methods that do not exist**. That manual is what a maintainer reads first, and three of
its claims would send one looking for mechanisms the crate does not have.

---

## Part 1 — The DSV4 findings, one by one

Verdict on the finding, then on the branch's implementation. **Adopt** = merge as is; **Adopt with
change** = merge after the change named; **Reject** = do not merge this part.

### A. Correctness

| # | Finding correct? | Branch fix | Verdict |
| --- | --- | --- | --- |
| A1 inversion of exactly one second passes the tolerance | Yes. `gap.unsigned_abs() <= DURATION_TOLERANCE` admits it; `Interval::from_start_end` panics downstream. | `duration_is_consistent` refuses `conn_end < conn_start` before the tolerance (`common.rs`); boundary test `10:00:00 → 09:59:59, 0:00:00` added; `AnomalyKind::Display`, the fixture report, `ERRORS.md` and the manual all updated. | **Adopt.** |
| A2 negative or non-finite `Energy_Use` accepted | Yes. | `parse_energy` refuses `!is_finite() \|\| < 0.0` at the cell; four-value test plus zero and an ordinary figure. | **Adopt.** |
| A3 malformed `Charge_Duration` reported as a write failure | Yes. | `CsvSession::parse` validates the column when non-blank; the writer still parses it on demand. Two tests, for a malformed cell and a blank one; the malformed one also asserts the message names no `.xlsx`. | **Adopt.** The value is parsed twice (validated, discarded, re-parsed by the writer). Acceptable; a `Row` field would avoid it but widens the type for one column. |
| A4 excluded session printed under "count towards the figures" | Yes. | `Sessions::notes` drops anomalies whose session is in `excluded`, by `Rc::ptr_eq`. Test added. | **Adopt with change.** (1) `own` still chains `&self.excluded` in and then filters them out again; drop the chain and filter only `self.anomalies`. (2) The doc comment on `notes` still says "Excluded sessions and sources are not filtered" and says nothing about the anomalies list no longer carrying an excluded session; the contract changed and the `///` did not. |
| A5 reimbursement variances not rounded to the cent | Yes. | `to_the_cent(to_the_cent(a) - to_the_cent(b))` for both; 40-step half-cent sweep reading the rendered report. | **Adopt.** The sweep test is the right shape. |
| A6 `NaN` variance narrated as a shortfall | Yes. | Both verdicts return a neutral sentence when `!is_finite()`; test extended with `NaN`, `±inf`. | **Adopt.** |
| A7 Charges Report parser accepts `NaN`/`inf` | Yes. | `parse_number` refuses `!is_finite()`; test over `NaN`, `inf`, `-inf`, `1e999`, and `1e3` still reads. | **Adopt.** |
| A8 `markdown::table` unguarded, untested | Yes. | Two `assert_eq!` with messages, a `# Panics` section, four tests. | **Adopt.** |
| A9 `report_sections` measures bytes | Yes. | `chars().count()` both sides; test with `Période — notes`. | **Adopt.** The DSV4 alternative — export the rule from `markdown` — was not taken; two copies of the rule remain, now agreeing. Acceptable. |

### B. Testing gaps

| # | Finding correct? | Branch fix | Verdict |
| --- | --- | --- | --- |
| B1 session workbook cells unpinned | Yes. | `the_written_workbook_matches_its_golden` dumps every cell (value or formula, number format, alignment), widths, heights and comments for two fixtures. | **Adopt.** Best single addition on the branch. It also pins a comment sentence that should not exist (3.4.4 below): "The sheet used to carry a padded pair of columns beside these…". Fix the sentence, regenerate the golden. |
| B2 bill PDF parse only behind `#[ignore]` | Yes. | Five unit tests over `Charges::read` and `Usage::read` on constructed `Line`s; no committed bill fixture. | **Adopt.** The unit tests cover the row logic, which is what drifts. `pdf_text::page_fragments` is still untested (3.2.6). A committed synthetic bill PDF is still worth having but is not owed to this branch. |
| B3 real ESPI export only behind `#[ignore]` | Yes, as a fact. | See E1. | **Reject the skip-on-absent change** (E1). |
| B4 `definitions()` unpinned | Yes. | Golden `tests/fixtures/sessions/definitions.txt`. | **Adopt.** |
| B5 non-finite verdict branch untested | Yes. | With A6. | **Adopt.** |
| B6 "Seven of the ten binaries have no test at all." | **Overstated.** Seven had no *unit* test; the argument-parsing in the two cost-recovery CLIs was the only logic worth one. | `shape()` extracted in both CLIs and tested (three tests each); `is_csv` tested. | **Adopt.** The remaining five binaries are `main` plus one library call each; no test is owed. |

### C. Modularity and consistency

| # | Finding correct? | Branch fix | Verdict |
| --- | --- | --- | --- |
| C1 `Session.conn_duration` never read | Yes. | Field deleted; five constructors updated. | **Adopt.** |
| C2 `charges_month` uncalled, two docs cite it | Yes. | Deleted; both docs now cite `parse_charges_report_name`. | **Adopt.** |
| C3 TOU band row defined twice | Yes. | `BAND_HEADERS`, `BAND_ALIGNMENT`, `band_row` in `src/api/pure/mod.rs`, `pub(super)`. | **Adopt with change.** Wrong home — see 2.2 below. |
| C4 `commas_group_thousands` written twice | Yes. | One `pub(crate)` copy in `src/csv.rs`. | **Adopt the dedup; change the home before merging.** The dedup is right. The placement breaks two written rules: `csv.rs` never calls the function, while `CLAUDE.md` says a helper lives where its callers are, and `csv.rs:13-14` says value parsers belong to the format that writes the values. A bill reader now imports from the CSV module. Move it to a neutral leaf. The new doc comment also adds a history sentence ("Each used to carry its own copy…") that the repository's rules forbid. See 2.2.3. |
| C5 `energy` test module redefines drifted fixtures | Yes. | `period_ending_date` and `close` taken from `test_support`; the private bill renamed `round_bill` with a doc saying why it is a second fixture; `test_support`'s module doc amended. | **Adopt.** The right call: the two bills serve different purposes, and the rename says so. |
| C6 `parse_rates` duplicated between two CLIs | Yes. | Moved to `src/api/mod.rs` as `pub fn parse_rates(&str) -> Result<CostRecoveryRates, String>`, with two tests. | **Adopt with change.** See 2.2: wrong home and a `String` error in the library. |
| C7 `delivery_cost` doc says "net of HST"; stale "moved field by field" | Yes, both. | Doc corrected and expanded; comment deleted. | **Adopt, and do 3.4.6 in the same commit.** Both halves of C7 are fixed correctly. But `src/api/pure/energy.rs:87` carries the identical sentence over the identical arithmetic, DSV4 did not find it, so the branch had no reason to touch it. Merged alone, this fix leaves that line the only place in the crate still saying "net of HST and OER", beside a field that now spells out the opposite. A reader would take the difference for a deliberate one and conclude that energy cost really is net of HST. Fixing one of a matched pair is what makes the other misleading. |

### D. Comments and documents

| # | Finding correct? | Branch fix | Verdict |
| --- | --- | --- | --- |
| D1 workbook comment says the cell is read back | Yes. | Comment rewritten. | **Adopt.** The sentence "editing a cell here changes the sheet and nothing else" is the one users need. |
| D2 `csv_sessions` promises an off-grid warning | Yes. | Sentence removed. | **Adopt.** |
| D3 `TIME_ZONE_NAME` names `session::ioi` | Yes. | Rewritten. | **Adopt.** |
| D4 `lib.rs` says `ConversionError` is public "only through this module" | Yes. | Rewritten. | **Adopt.** |
| D5 `spikes` doc wrong on both claims | Yes. | Rewritten; the "That is a correction: the reason given here used to be…" paragraph gone. | **Adopt.** The same false claim in the user-visible `ZeroActiveChargeTime` text, `ERRORS.md` and `docs/session/README.md` was not touched (3.4.1). |
| D6 `parse_session_report_name` example is a name it rejects | Yes. | Doc says "name, not a file name", explains `2026.csv`. | **Adopt.** The Charges Report twin has the same defect (3.1.8). |
| D7 manual's safety argument off by the tolerance | Yes. | Paragraph rewritten around the two checks. | **Adopt.** |
| D8 `site-specific-constants.md` line citations rotted | Yes (I checked three: `billing_period.rs:70`, `base.rs:88`, `common.rs:59`). | Every `:line` dropped; symbol names kept. | **Adopt.** |
| D9 `segment-tiling.md` contradicts itself | Yes. | Rewritten: padded spans removed, membership table now matches `MEMBERSHIP`, the two wrong paths fixed, `17:15` sentence redirected, "report" disambiguated, and the `segment_tiling_tests.rs` sketch corrected. Every arithmetic figure matches the fixture. | **Reject the document rewrite; adopt the sketch correction.** The mechanical fixes are right, but the rewrite resolves the document's clock contradiction by adopting the wrong half: it calls the interval of interest the session report's and states the whole document on standard time, where the interval is a metering interval rendered in local time. `main` deleted this document; the deletion is the better side of the conflict. See 2.1.2. |
| D10 broken and stale paths | Yes. | All fixed. | **Adopt with change.** In the entry "a breaker billed for part of the month", the link still reads `[docs/Questions_for_Evolute.md](archive/Questions_for_Evolute.md)`: the target was corrected, the link text was not. Four more stale citations of the same file are in 3.4.7. |
| D11 README misstates what `cargo build --release` builds | Yes. | Fixed. | **Adopt.** |
| D12 help text wraps "judgement call" | Yes. | Fixed. | **Adopt.** |

### E. Packaging and CI

| # | Finding correct? | Branch fix | Verdict |
| --- | --- | --- | --- |
| E1 no CI on push | Yes. | `.github/workflows/ci.yml`: clippy `-D warnings`, `cargo test --all-targets --all-features`, the ignored set, doc links. | **Adopt three of the four steps; reject the fourth.** The "Run the data-gated tests too" step depends on the two tests below skipping when their data is absent. In CI the data is never present, so the step always passes and tests nothing, while its comment says it "checks the real parse wherever the data is present". Drop that step and restore the panics: the two tests are meant to be run by name, as their own module docs prescribe, and under that invocation a panic is the correct answer to a named request that cannot be met. See 2.2.4 for the harness reason a skip cannot be told from a pass, and for the two conditions running by name depends on. Add `--document-private-items` to the doc step (3.2.1). |
| E2 examples never compiled by CI | **No — this is the one DSV4 finding I believe is wrong.** `cargo test` builds examples by default; `cargo test --no-run -v` at `382f2c5` issues rustc invocations for both `site_load_report` and `gb_trim_fixture`, and the full suite passes with both compiled. The release workflow's `cargo test --verbose` therefore already compiled them, and "one example is currently uncompilable" cannot have been true while `cargo test` was green. | `--all-targets` on the release workflow's test step; CI job compiles them. | **Adopt the change, discard the reason.** `--all-targets` is what `CLAUDE.md` prescribes and it does add the bench and test targets, so the edit is right; the comment the branch adds beside it ("which is exactly how a broken example or test target once reached a tag") states something that did not happen and should go. |
| E3 notices stamp authenticates inputs, not text | Yes, as stated. | Second hash over the body; `build.rs` reads the stamp's fields by name; the script and template updated. | **Adopt.** Slightly more than the defect needed (a body hash alone would do; the inputs hash is now redundant with it), but the field-by-name parser is clean and `about.md.hbs` now says what the check does. |
| E4 `about.toml` comments disagree on `r-efi` | Yes. | The `accepted` comment now defers to `targets`. | **Adopt.** |

### F. Prior-review items DSV4 left open

The two it named — the CMap `bfrange` length check (`pdf_text.rs:425-433`) and
`expect("just matched")` at `bill_pdf.rs:429` — are still open on the branch. Both are real and
both are small. Its table also marks fable C13 (`BillingPeriod::ending_on` bounds `bill_end_day`)
as fixed; that is true of `ending_on` and not of `containing` (3.1.6).

---

## Part 2 — The branch as an implementation

### 2.1 Blocking before merge

1. **Not formatted.** `cargo fmt --check` on the branch tree reports diffs in `build.rs`,
   `src/api/mod.rs`, `src/bin/cost_recovery_cli.rs`, `src/bin/cost_recovery_surplus_cli.rs`,
   `src/charges_report.rs`, `src/hydro_bill/bill_pdf.rs`, `src/session/csv.rs`,
   `src/session/excel.rs` (which also has a doubled blank line after `dump_workbook`). Every one
   is in code the branch wrote. `main` is clean. One `cargo fmt` fixes it; the user's rule is that
   every touched file is left formatted.

2. **Conflicts with `main`, and the deletion is the better side of it.** `main` deleted
   `docs/session/segment-tiling.md` and its README line in `3aaf32b` ("Deleted confusing …"); the
   branch rewrites the document. Git will report a modify/delete conflict.

   The rewrite does fix the mechanical defects `_todo/_todo.md` lists: the two wrong file paths, the
   padded spans in the table and the sketch, the membership table that disagreed with `MEMBERSHIP`,
   the `17:15` sentence that pointed at tables holding no such value, and the references to what was
   there before. Every arithmetic figure in it agrees with the pinned fixture.

   **But it settles the clock question the wrong way, and that is the defect the document was
   deleted for.** Three problems, in descending order:

   - **It says the interval of interest is the session report's, and it is not.** The rewrite opens
     its worked example with "The interval of interest is 16:00–17:00 as the session report states
     it". A session report states session start and end times and nothing else. The interval comes
     from the meter: `peak_power` derives it from the Green Button period values
     (`src/api/pure/peak_power.rs:333`), and the crate's own definitions section calls it "a
     metering interval during which a particular power metric peaks" (`src/session/report.rs:618`).
   - **It puts the whole document on the wrong clock.** An interval of interest is rendered in
     prevailing local time: `interval_line` goes through `zoned_span`, which names the offset in
     force (`src/session/report.rs:582-585`), and the fixture prints `2026-06-15 17:00 - 18:00 EDT`.
     The rewrite instead states every table, every segment name and the sketch on the portal's fixed
     standard-time offset, so every segment name in the document disagrees with the report a user
     reads. `_todo/_todo.md` flagged the original document for holding both clocks at once; the
     rewrite made it consistent by adopting the false half, which is harder to catch than the
     contradiction it replaced.
   - **The cross-reference has no address where it is used.** The sentence that tells a reader the
     segment is named `17:15` elsewhere names the fixture by bare file name. The full path appears
     once, in the opening paragraph. The document contains no links at all, while its sibling
     `docs/session/README.md` uses relative ones. A fixture a reader cannot find is no use to them.

   The underlying tension is real and the document has to choose: its session table can match the
   input CSV or the rendered output, not both. The fixture CSV states session `A` as 15:54–17:03;
   the report names that hour 17:00–18:00 EDT. The report is the better anchor, because segments are
   named by the report, the golden pins it, and it is what the user has in front of them. Written
   that way, the interval reads 17:00–18:00 and the segments 17:00 through 17:45, matching the
   report name for name, and the cross-clock section shrinks to one sentence where the CSV is
   introduced.

   **Recommendation:** keep `main`'s deletion unless the document is rewritten from the meter's
   clock. Take the branch's `segment_tiling_tests.rs` sketch correction either way, and delete that
   module's citation of the document at `:10-11` while the document does not exist.

   The two `docs/archive` renames on `main` (`…-20260831.md`, `…-20260912.md`) do not conflict;
   the branch did not touch them.

3. **A citation in `docs/ERRORS.md` lost half of what it pointed at.** In the entry headed
   "periods that do not hold a full billing period's intervals", under the anomalies that leave the
   figures standing.

   The branch's headline change to this document is converting every citation from a line number to
   a symbol, and that is right. This entry described two places, one in the app and one in the
   library, and the conversion produced the app one twice:

   | Version | What the entry cites |
   | --- | --- |
   | `main` | `` `src/bin/ev_cost_recovery/convert.rs:236-247` ``, `` `src/green_button/excel.rs:305` `` |
   | `dsv4-fixes` | `` `src/bin/ev_cost_recovery/convert.rs`: `gb_outcome` ``, then the same again |

   So the library half is gone. The two were not redundant: `gb_outcome`
   (`src/bin/ev_cost_recovery/convert.rs:218`) is where the Convert tab shows the count, while
   `src/green_button/excel.rs:305` is where `write_gb_workbook` computes
   `incomplete_periods` and marks the row. A reader tracing why a period is flagged needs the
   second. The fix is to cite `` `src/green_button/excel.rs`: `write_gb_workbook` `` as the second
   location.

### 2.2 Design objections

   Both of the objections below are already the author's own. `_todo/_todo.md:6` reads:

   > Move consts and functions defined in src/api/mod.rs and src/api/pure/mod.rs to appropriate
   > file(s).

   That item can only be about this branch. On `main` neither file defines a single constant or
   function; both are module declarations and re-exports. The branch puts `parse_rates` in the
   first and four items in the second, which is exactly what the note asks to be moved. So this is
   not a preference of mine that happens to agree with a list. It is the same conclusion, reached
   independently. (`_todo/` is gitignored, so there is no history to date the note by.)

1. **`parse_rates` in `src/api/mod.rs`, returning `Result<_, String>`.** Two problems.
   - The home. `api/mod.rs`'s own doc describes the module as `io` re-exported plus `pure`;
     a text parser is neither. The idiomatic home is
     `impl FromStr for CostRecoveryRates` in `src/api/pure/recovery.rs`, beside the type, which
     both CLIs then reach as `spec.parse()`.
   - The error. `CLAUDE.md`: "Information that reaches a message lives in a field of the error
     variant, formatted at `Display`." A `String` was tolerable in a binary; in the library it is
     the pattern the rest of the crate was converted away from. A small
     `RateScheduleError { spec, part, reason }` with the messages at `Display` follows
     `SessionCsvError` and `ChargesReportError`.
2. **`BAND_HEADERS`, `BAND_ALIGNMENT`, `band_row`, `to_the_cent` in `src/api/pure/mod.rs`.** These
   are the four the note above names in that file. `reimbursement.rs` already imports from
   `recovery` (`CostRecoveryRates`), so the four can stay in `recovery.rs` as `pub(super)` with no
   new module and no change to the "by subject, not by call" rule the `pure` doc states.
   `to_the_cent` in particular is a money rule, and `recovery` is where the argument for it is
   written.
3. **`commas_group_thousands` in `src/csv.rs`. Fix before merging.** Removing the duplicate is
   right and is DSV4 C4's whole point. The home is wrong, and not as a matter of taste: it breaks
   two rules this repository has written down.

   - **The module it sits in never calls it.** On the branch, `src/csv.rs` names the function once,
     at its definition. Both callers are elsewhere: `charges_report.rs:615` and
     `hydro_bill/bill_pdf.rs:543`. `CLAUDE.md` states the rule and the precedent for exactly this
     shape: "Where a helper lives is settled by who calls it, not by what kind of code it is."
     `is_on_grid` was moved out of `time` once both its callers were in `green_button`. Here the
     callers are in two modules, neither of them `csv`.
   - **The module's own doc excludes it.** `src/csv.rs:13-14`: "What is *not* here is anything that
     depends on which document was opened. The value parsers belong to the format that writes the
     values." A digit-grouping predicate is what the two value parsers are built from, and the two
     formats in question are a CSV and a PDF. The branch added it to the one file whose doc says it
     does not belong there.

   The branch's own doc comment argues only the negative: "Here rather than in either reader,
   because both need it over the same question about a different document." That establishes it
   should not live in either reader. It does not establish that it should live in `csv`.

   The consequence today is that `hydro_bill::bill_pdf`, which reads no CSV, imports from the CSV
   module. Leaving it in `charges_report` instead would be worse, making a bill reader depend on a
   charges-report reader. So it wants a neutral leaf: a small `src/number.rs` holding the predicate
   and its tests, which is the shape `markdown.rs` and `log.rs` already have. Both callers reach
   it, so the "one utility that every caller uses, or none" test is met either way.

   That the two copies now agree is not the test. Agreement is what DSV4 C4 asked for; where the
   single copy lives is a separate question, and the answer the branch gives contradicts two
   written rules at a cost of one small module to fix.

   The cost of leaving it is not only this one import. `csv.rs` is where the next shared string
   helper will land once one module has become the place such things go, and each one widens the
   same false coupling.
4. **Skip-on-absent in the two ignored tests** (`tests/green_button/full_feed.rs`,
   `tests/hydro_bill/all_bills.rs`). On `main` each panics with a message saying where to put the
   data. The branch makes each `return` after an `eprintln!`, and adds the step that runs them:
   "Run the data-gated tests too" in `.github/workflows/ci.yml:39-40`, a file the branch creates
   and `main` does not have.

   **A skip is spelled "pass", and the explanation is swallowed.** The Rust harness has no runtime
   skipped outcome: `#[ignore]` is decided statically, before the body runs, so a test that finds
   at run time that it cannot run can only report success. Worse, `cargo test` captures output from
   passing tests, so the `eprintln!` never appears. Run against the branch with the sample invoices
   moved aside:

   ```text
   test hydro_bill::all_bills::every_bill_parses_and_its_figures_agree_with_each_other ... ok
   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out
   exit: 0
   ```

   The "skipping" line shows only under `--nocapture`. An ordinary run is indistinguishable from
   one that parsed twenty-five real invoices. **This repository has already been bitten by it:** the
   four tests in 3.1.4 skip silently on paths that moved, so they have been passing without running
   and nobody noticed, while these two have not rotted because absence fails immediately.

   **Why the panic is the right answer here.** Neither test was meant to be reached by a bulk
   `--ignored` run. Each module doc names its own invocation, by test name and with capture off:

   ```text
   cargo test --test integration -- hydro_bill::all_bills --ignored --nocapture
   cargo test --test integration -- green_button::full_feed --ignored --nocapture
   ```

   Run that way, a panic is not a false alarm. One named test was asked for, it could not run, and
   `--nocapture` means its message is actually read. The bulk `--ignored` run is what the branch
   introduced so that CI would have something to call, and it is the only thing that makes the
   panic look like noise.

   **So: restore the panic, drop the "Run the data-gated tests too" step**, and keep the two
   conditions that make running by
   name workable. Both need attention, and one of them is a live defect:

   - **The module doc must state the precondition.** `all_bills.rs:1-4` does: it names
     `data/hydro_bills` and says the bills are not in the repository. `full_feed.rs:1-8` does not.
     It says only that the test is slow, and the file it needs, the fact that no checkout carries
     it, and where to put it live in a function and a panic string further down. A reader of the
     module doc alone cannot tell the test has a precondition at all. Add one sentence.
   - **The failure must say the data is absent, and must not say it when something else broke.**
     `all_bills.rs` is right on both counts: the `expect` on `read_dir` names the directory and the
     remedy, and an existing but empty directory is caught separately by the
     `assert!(!paths.is_empty())` below it.
     `full_feed.rs:26-32` is wrong on the second. It appends "The sample export is not in the
     repository" to **every** error `read_gb_feed` returns, and that function returns
     `GbReadError::Unreadable` for a file it cannot open *and* `GbReadError::Malformed` for a file
     that will not parse (`src/green_button/read_xml.rs:170-179`). So a genuine parse regression,
     which is the whole reason this test exists, would be announced as missing data and dismissed.
     Suffix the advice to the unreadable case only.
5. **`Sessions::notes` filters by identity after chaining the same sessions in.** See DSV4 A4 above.
   Also, the change belongs in the `///` contract, not only in a `//` body comment.

### 2.3 Small defects in the branch

- `docs/ERRORS.md`, entry "a breaker billed for part of the month": the link text still reads
  `docs/Questions_for_Evolute.md` although the target was corrected to `archive/`. Correct the text
  to match the target.
- `docs/ERRORS.md`, entry "saving a report or a workbook failed": mixed style —
  `` `surplus.rs`: `export_row`, `detail.rs:107`, `reimbursement.rs:272` `` — two of the four
  citations are still bare line numbers. Cite all four by symbol, which is the style the rest of
  the branch's change adopts.
- `src/csv.rs` (branch, `commas_group_thousands` doc): narrates the duplication it replaced
  ("Each used to carry its own copy under a comment asking…"). A doc comment carries the contract;
  the history belongs in the commit message.
- `src/csv.rs` (branch, `Table` doc): "An open CSV file, read whole" — "open" is new and means
  nothing here. Keep `main`'s "One CSV file, read whole", which was right.
- `src/session/excel.rs`, workbook golden: pins the `conn_span` column comment that ends "The sheet
  used to carry a padded pair of columns beside these…". This is user-visible text about a version
  of the sheet the user never had. Cut the sentence and regenerate both `.workbook.txt` files.
- `docs/ERRORS.md`, `InconsistentDuration` entry: the new paragraph "the estimating logic panics on
  an inverted span rather than answering for one" is maintainer reasoning in the user-facing
  document. Cut the paragraph; the first one already says what a user needs.
- `src/api/mod.rs` and `src/api/pure/mod.rs` (branch): `jiff::civil::Date` and
  `crate::markdown::{Align, Left, Right}` are written inline rather than imported at the top,
  against the import style the rest of the crate keeps. Move both to `use` declarations at the top
  of each file.
- `AnomalyKind::Display` for `InconsistentDuration` now reads "…contradict each other by more than a
  second, which is the rounding the source does, or report an end before the start; …". The
  subject of "report" is three nouns back. "…, or the end is before the start; …" reads.

### 2.4 What the branch does well

- Every correctness fix is pinned by a test at the exact boundary the finding named.
- The two goldens (workbook, definitions) turn two silent drift paths into diffs.
- The `Charges::read` / `Usage::read` unit tests are the first tests of the bill parser that run
  without a PDF.
- The half-cent sweep in `the_money_columns_add_down_to_their_variances` reads the rendered report
  rather than the struct, which is the comparison a reader makes.
- The commit messages state what changed and why in one sentence each.

---

## Part 3 — Independent findings on `main`

Everything below is new relative to the DSV4 review and to `_todo/_todo.md`, except where a todo
item is named. Each was verified by me against the source unless marked **[reported]**. Severity
uses the DSV4 scale: **high** = wrong result, data loss or a crash on realistic input;
**medium** = latent bug, a document or comment that would cause a wrong change, or a missing test
for behaviour that has already changed once; **low** = real but limited cost.

### 3.1 Correctness

- **3.1.1 MEDIUM — `session::session_csv_to_xlsx` is public and overwrites without asking.**
  `src/session/excel.rs:153-175`, exported at `src/session/mod.rs:34` under "Named outside the
  crate". It writes `path.with_extension("xlsx")` unconditionally. The refusal that
  `src/error.rs:29-30` calls "the default rather than a courtesy" lives only in
  `api::session_csv_to_xlsx`
  (`src/api/io.rs:482-493`), which is this function's sole caller; `src/bin/ev_csv_to_xlsx.rs:34-35`
  says so in as many words. A library caller who reaches for the `session` path destroys a
  hand-reconciled workbook; one who passes the `.xlsx` itself truncates it after reading it. By the
  visibility rule in `CLAUDE.md` (as public as its most public caller) this should be `pub(crate)`.

- **3.1.2 LOW — `HydroBill::metering_adj` is read and used by nothing, and no comment says so.**
  `src/hydro_bill/bill.rs:55` is populated by `bill_pdf.rs:478` and read by no calculation in the
  crate. Reading it is correct: `HydroBill` models the bill, the bill's usage row states the
  column, and a struct that silently dropped a stated figure would be the harder thing to notice.
  What is missing is a line saying the field is deliberately recorded and deliberately unused.
  Without it, the next reader has to search the crate to find out, and `CLAUDE.md` names the
  reverse case ("when a field stops being read, nothing tells you") as a defect this project has
  already been bitten by. The field sits in a block of bare `pub` fields, so the comment is the
  only thing that can carry the fact.

  Suggested wording for the comment: the figure is taken from the bill because the bill states it;
  no calculation uses it; every bill read so far states `1.0`; and it is not to be used until
  someone establishes what the utility does with it.

  **Not to be confused with the day-count proration**, which is a separate question and is not in
  doubt. `src/api/pure/peak_power.rs:384-388` argues that pricing at a blended rate makes the
  `days / 30` factor cancel, because the rate is a quotient against the adjusted demand and the EV
  figure is adjusted by the same day count. That argument is about the day count and it holds.
  Whether this column could ever be anything but `1.0`, and what the utility would do with it if it
  were, is unknown and unanswerable from the bills on hand: all twenty-five state `1.0`.

- **3.1.3 MEDIUM — `CoverageError::PeriodNotCovered` says "billing period" for a calendar month.**
  `src/api/pure/coverage.rs:76-80` renders "the session reports do not cover the billing period
  {period_start} to {period_ending}:" unconditionally; `src/api/io.rs:421` raises it for the
  reimbursement's
  calendar month (`charges.month … last_of_month()`), which `coverage.rs:122-124` itself says "is
  not a billing period". A user on the reimbursement tab is told a billing period runs 1–30 June.
  `docs/ERRORS.md:441-445` lists the entry under "Where: Cost recovery" only and explains it as the
  24th-to-23rd period. The one test of this route asserts `.is_err()` and never reads the wording.
  Give the variant a field saying which span it is about, and render "billing period" or "month"
  from it; assert the reimbursement wording in that test.

- **3.1.4 MEDIUM — Data-gated tests point at paths that no longer exist, and pass.**
  `src/bin/ev_cost_recovery/state.rs:1214-1225` (`real_inputs`) looks for
  `data/TH_Electric_Usage_…XML` and `data/Session_Report_…csv`; the files are under
  `data/green_button/` and `data/evolute/` (as `docs/app-cheat-sheet.md:35-38` says). The three
  tests it gates — including the one the module calls "the contract the whole app rests on" —
  return early and count as passes. `src/green_button/read_xml.rs:280-283` has the same stale path
  and the same silent `return`; `src/bin/gb_peak_values.rs:44-45` prints it as its usage example.
  This is the failure mode the branch's skip-on-absent change (2.2.4) would extend to the two
  integration tests: fix the paths, and make a skip visible (`#[ignore]` with a reason) rather than
  a pass.

- **3.1.5 LOW — `build.rs` declares an input that dev builds never read, so in a checkout without
  the notices file every local build recompiles the crate.** `build.rs:56` emits
  `cargo:rerun-if-changed=THIRD-PARTY-NOTICES.md` unconditionally. That file is gitignored, so a
  fresh checkout has none, and cargo treats a watched path that does not exist as permanently
  stale. Two consecutive `cargo build --bin energy_cli` on an unchanged tree both recompile, and
  cargo names the reason itself:

  ```text
  stale: missing "/workspaces/ev-cost-recovery/THIRD-PARTY-NOTICES.md"
  dirty: FsStatusOutdated(StaleItem(MissingFile { path: ".../THIRD-PARTY-NOTICES.md" }))
  ```

  Put a file at that path and the rebuilds stop: the next build reruns the script, because the
  path it watches has appeared, and the two after it are incremental. Remove it and every build
  recompiles again. What recompiles is this crate's own library and binary, not the dependency
  tree, which is why the effect is a steady tax on iteration rather than something obvious.

  **The file is not an input to a dev build at all.** `build.rs:62-66` reads it only when
  `PROFILE` is `release`; otherwise it writes `PLACEHOLDER` and never opens it. So on the builds
  where the path is missing, nothing depended on it in the first place.

  **Where this does and does not cost anything.** It costs local iteration, where a warm `target`
  is the point. It costs nothing in either workflow: both check out fresh, so there is no
  incremental build to spoil. That is presumably why it has gone unnoticed.

  **The fix is to emit the directive when the file is actually an input, which is release builds**,
  rather than when it happens to exist. Tying it to the `released` flag already computed on line 62
  makes the rule match the read. Release builds keep their guard either way, and for two reasons
  that hold together: the release workflow runs `scripts/gen-notices.sh` immediately before
  `cargo build --release` (`release-build.yaml:78-82`), so the file is present by the time cargo
  builds; and a release build that lacks it cannot succeed, because `release_notices()` panics. So
  every release build that produces a binary is one where the file existed, was read, and was
  tracked.

- **3.1.6 MEDIUM — `BillingPeriod::containing` panics inside jiff before the assertion meant to
  stop it.** `src/hydro_bill/billing_period.rs:99-101` calls `period_ending(…, bill_end_day)`,
  which calls `date(y, m, bill_end_day)` (`:143-150`), *before* `ending_on`'s range assertion
  runs. With `bill_end_day = 31` and a 15 April instant, the panic is jiff's `invalid date`, not the
  message `ending_on`'s doc (`:112-113`) says the check exists to give. `containing` has no
  `# Panics` section. DSV4 §F marks fable C13 as fixed; it is fixed for one of the two entry points.
  Move the assertion into a shared check both call. **[reported; reproduced by the reviewer with a
  probe crate]**

- **3.1.7 MEDIUM — Two tables in the interval report cite a row number without saying which file
  it is in.** `src/session/report.rs:549-552` renders the Anomalies table as
  `["Row", "Session", "Anomaly"]`, and `:489` renders Excluded sessions as
  `["Row", "Session", "From", "To", "In interval", "Anomaly"]`. Neither carries the file.
  `api/io.rs:654` builds the estimate from `Sessions::merge(reports)`, and a billing period
  straddles two monthly reports by construction, so more than one source is the normal case rather
  than an edge. With `May.csv` and `June.csv` both listed in the header,
  `| 3 | S123 | DuplicateId |` is not a row anyone can look up. The note asserts the opposite:
  "Row numbers are
  rows of the source data file named above, so each one can be looked up directly."

  **The obvious two fixes both fail, which is why this needs a decision rather than a patch.**

  Reusing `by_source_table` (`report.rs:267-271`), which does have a `File` column, would lose
  information. It renders the bare token `kind.as_str()`, while the Anomalies table renders
  `anomaly_cell(a.kind, a.session.avg_kw())`, which is why the fixture shows
  `ExcessiveAvgKw(7.200)` rather than `ExcessiveAvgKw`. The comment at `report.rs:542-543` says
  that figure is deliberate: "the number that fed the totals is the one worth seeing beside the
  flag."

  Adding a `File` column does not fit either. The rendered report is asserted at 90 columns
  (`tests/session/report_rendering.rs:43`), and the two tables have very different headroom:

  | Table | Widest row today | With a `File` column |
  | --- | --- | --- |
  | Anomalies | 41 | 87, using a report name from `data/` |
  | Excluded sessions | 90 | over the limit; there is no room at all |

  The Excluded table is already at exactly 90. A longer report name, and the repository holds one
  of 66 characters, breaks the Anomalies table too.

  **So carry the file without a per-row column: precede each table with the file name its rows
  belong to.** This is settled — the report may grow taller, which is the only cost. It costs no
  width, works for both tables, and keeps `anomaly_cell`'s figure. `by_source_table` already
  establishes the ordering that would feed it.

  It also repairs the note rather than forcing a rewrite of it. "Row numbers are rows of the source
  data file named above, so each one can be looked up directly" is false today because the only
  thing named above is a header listing every file. With the file name immediately above each
  table, it is true as written, once "named above" is made to point at that name rather than at the
  header.

- **3.1.8 MEDIUM — `parse_charges_report_name` documents a form it rejects, and the alignment
  document repeats the example.** `src/charges_report.rs:120-121` gives
  `123 Foo Bar Road_Charges_November 2026-January 2027.csv`; `NAME_FORM` (`:57`) ends `.csv` and is
  quoted in every error; `docs/Evolute_portal_alignment.md:118-124` calls the function on that
  name. `month_start` (`:168-172`) parses the year from `January 2027.csv` and returns `BadMonth`.
  Every in-crate caller passes `file_stem` first and every test omits the extension. The
  charges-side twin of DSV4's D6, which the branch fixed only on the session side. Correct the two
  examples to the stem form the parser accepts, and add a test that passes a name ending `.csv`.

- **3.1.9 LOW — `-0.00` in money tables.** `src/markdown.rs:141` formats with `{:.2}` and Rust
  prints negative zero with its sign, so every negated amount that is zero —
  `-self.tou_kwh.total_kwh()` for a month with no sessions (`reimbursement.rs:390`),
  `-self.energy.energy_cost` before the chargers ran (`recovery.rs:685`),
  `-self.ontario_electricity_rebate` on a bill with no rebate — prints `-0.00`.
  `to_the_cent(-0.001)` also yields `-0.0`, so a surplus that rounds to nothing prints `-0.00`
  beside the "covered … with the surplus above left over" verdict. Normalise zero in `amounts` and
  `to_the_cent`; pin with a `markdown` test.

- **3.1.10 LOW — Eight of ten binaries panic on a non-UTF-8 argument.** `env::args()` at
  `energy_cli.rs:34` and seven siblings; only `ev_csv_to_xlsx.rs:22` uses `args_os()`. Running
  `energy_cli 2026-06-23 $'\xff.csv'` panics in `std::env` with exit 101. Collect `args_os()`,
  convert only the date and rate arguments, and report a failure as an argument error.
  **[reported; reproduced by the reviewer]**

- **3.1.11 LOW — `report_sections` can slice backwards on a constructed input.**
  `src/bin/ev_cost_recovery/state.rs:1155-1156` accepts a rule line as a title when the line after
  it is another rule of the same length; `"abc\n===\n===\nbody\n"` panics at `lines[2..1]`. Latent
  (the library never emits that shape), but it is a `pub fn` over any `&str`. Require the title line
  to be non-empty and not itself a rule before taking it, and pin that input in a test.
  **[reported]**

- **3.1.12 LOW — An empty `session_csvs` is accepted everywhere and produces a message that ends in
  a colon.** `reports_cover(first, last, &[])` fails, so `energy(date(2026,6,23), &[])` yields
  "the session reports do not cover the billing period 2026-05-24 to 2026-06-23:" with nothing
  after it. No entry point tests the empty slice. Refuse an empty slice at each entry point with a
  message that says so, and test one entry point for it.

- **3.1.13 LOW — `parse_duration` overflows on a large hour field and accepts a leading `+`.**
  `src/session/csv.rs:295` `h * 3600 + m * 60 + sec` on `u64`; the doc at `:273-275` says the sign
  is rejected, but `u64::from_str` accepts `+5:07:53`. Use `checked_mul`/`checked_add` into
  `bad()`, and refuse a sign explicitly.

- **3.1.14 LOW — `ToUnicode::decode` drops a trailing partial code, against its own doc.**
  `src/hydro_bill/pdf_text.rs:351-356`: the doc says an uncovered code "shows up as visible damage
  … rather than as a quietly shortened label"; `chunks_exact(self.code_len)` discards the
  remainder, so `[0x00, 0x25, 0x00]` with two-byte codes decodes to one character and no U+FFFD.
  Append U+FFFD when `remainder()` is non-empty.

- **3.1.15 LOW — `GbReadError::NotABillingCalendar` is unreachable through the API, and
  `ERRORS.md` lists it under "Cost recovery".** Every `read_gb_for_billing_period` call in
  `src/api/io.rs` (`:106, :165, :350`) follows `check_reports_cover_period`, whose
  `billing_period_dates` refuses the same condition first (`billing_period.rs:206`). The manual's
  list of unreachable variants (`docs/maintenance-manual.md:261-263`) omits it and names
  `ReimbursementError::NotOneSessionReport`, which no longer exists in `src/`. Move the entry out of
  "Cost recovery" in `ERRORS.md`, and add `NotABillingCalendar` to the manual's unreachable list in
  place of the variant that is gone — see 3.5.2, which is the same list.

- **3.1.16 LOW — Two `pdf_text` behaviours worth a look.** `page_fragments`
  (`src/hydro_bill/pdf_text.rs:201-225`) resolves a
  CMap for every font in the page's resources, so a bill listing an unused font with no `ToUnicode`
  is refused whole; and `Q` (`:242`) restores the CTM but not the current font, where PDF's saved
  state includes both. Resolve a font's CMap on first use rather than up front, and push the
  current font onto the `q`/`Q` stack beside the CTM. **[reported]**

- **3.1.17 LOW — An excluded session never shows its `DuplicateId`.** `src/session/peak.rs:298-310`
  drops report-level anomalies on excluded sessions and `report.rs:480-484` lists only
  `s.anomalies`, so of two rows sharing an id where one is inconsistent, the kept one shows
  `DuplicateId` and the excluded one shows only `InconsistentDuration`, with nothing linking them.
  Carry report-level anomalies onto excluded sessions too, and render them in the Excluded table
  alongside `s.anomalies`. **[reported]**

### 3.2 Testing

- **3.2.1 MEDIUM — The doc-link check in `CLAUDE.md` misses private items.** With
  `--document-private-items`, the same command fails with ten unresolved links:
  `report_coverage` (`api/pure/reimbursement.rs:16`), `crate::time::excel`, `Kind`
  (`green_button/excel.rs:20`), `ev_load` (`session/common.rs:509`), `SessionRows::records`
  (`session/csv.rs:322`), `crate::peak_power`, `crate::AnomalyKind`, `Sessions::notes`,
  `civil::DateTime`, and a test name (`time/excel.rs:24`). Add the flag to the command the
  repository prescribes and to CI.

- **3.2.2 MEDIUM — `a_shared_instant_is_counted_once` cannot fail on what it is named for.**
  `src/session/segment_tiling_tests.rs:136-145` asserts `count > 1.0 && count < 5.0`. The exact
  count is 1 + 12/15 + 4/15 + 4/15 + 8/15 = 2.8667 (the golden prints `2.867`); the padded-end
  regression the test guards against gives 3.0, also inside the band. Assert the fraction.

- **3.2.3 LOW — `SessionReportNameError`'s variants are argued for and not tested apart.**
  `src/session/file_name.rs:41-44` says the reasons "are not interchangeable"; the refusal test
  (`:233-247`) asserts only `is_none()`. `Inverted` is constructed at `:123` and matched nowhere;
  the sibling `ChargesReportNameError::Inverted` is pinned (`charges_report.rs:1080`). Assert the
  variant for each refusal case, as the charges-side test does.

- **3.2.4 LOW — `state.rs` promises "decided here and tested here" and tests none of
  `ConversionSlot`, `checked_figure`, `WorkingDir::remember`.** The test module (`:1208-1956`)
  references none of them. Add a table test over the four inputs `checked_figure`'s own doc names,
  one test per `ConversionSlot` transition, and one asserting `WorkingDir::remember` keeps the
  directory of the path it is given. **[reported]**

- **3.2.5 LOW — `tests/docs_errors.rs` could pin the quoted anomaly text and does not.** Its doc
  (`:7-10`) says `Display` output "carries placeholders"; the eleven anomaly descriptions are
  placeholder-free static strings that `ERRORS.md` quotes verbatim. Extend `has_entry` to compare
  the block quote. 3.4.1 is exactly the drift this would have caught.

- **3.2.6 LOW — `pdf_text::page_fragments` has no test.** The operator interpreter
  (`Td`/`TD`/`T*`/`Tm`/`'`/`"`/`TJ`/`q`/`Q`/`cm`) and the `UndeclaredFont` error that closed fable
  C12 are exercised only by the ignored real-bill test. The branch's new `bill_pdf` tests stop one
  layer above this. Build an `lopdf::Document` in memory and test the interpreter directly: one
  case per operator, and one for `UndeclaredFont`.

- **3.2.7 LOW — Report shapes with no fixture.** `src/session/report_rendering_tests.rs:14-16`
  says the cases "cover every shape the renderer has"; not covered: the deserted-interval
  paragraph (`report.rs:370-379`), the singular "One session … was" branch (`:388-393`), a
  15-minute single-segment interval, and a two-file `Source:` line. Add a fixture for each, or
  narrow the claim to the shapes the cases actually reach. The two-file case is wanted anyway,
  since 3.1.7 changes how those reports render.

- **3.2.8 LOW — The holiday half of the 7-7 demand window has no external witness.**
  `src/green_button/peaks.rs:272` excludes holidays via `is_off_peak`; the only invoice-backed
  check (`invoice_tests.rs:75-93`, 23 May–23 June 2026) contains no holiday. Add an invoice-backed
  case over a period containing one; failing that, a unit test pinning that a holiday hour inside
  7-7 is excluded, so the rule is pinned by something. **[reported]**

- **3.2.9 MEDIUM — `full_feed.rs` blames missing data for every failure, including the one it
  exists to catch.** `tests/green_button/full_feed.rs:26-32` appends "The sample export is not in
  the repository: put … in place before running this." to every error from `read_gb_feed`. That
  function returns `GbReadError::Unreadable` when it cannot open the file and
  `GbReadError::Malformed` when the XML will not parse (`src/green_button/read_xml.rs:170-179`).
  A parse regression over the real 18 MB export is precisely what this slow-tier test is for, and
  it would be reported as absent data and waved away. The inline comment shows the intent, saying
  the suffix adds what the error "cannot know: that this particular file is expected to be absent
  from most checkouts", but it is appended unconditionally. Attach it to the unreadable case alone.

  Its sibling `tests/hydro_bill/all_bills.rs` is right on both counts and is the model: the
  `expect` on `read_dir` names the directory and the remedy, and a directory that exists but holds
  no PDFs is caught separately by `assert!(!paths.is_empty(), "no PDFs in {:?}", bills_dir())`.

- **3.2.10 LOW — `full_feed.rs`'s module doc does not state its precondition.** `:1-8` says the
  test is slow and gives the command, but not that it needs a file no checkout carries, nor where
  that file goes. Both facts sit further down, in `feed_path()` and the panic string. The sibling
  `all_bills.rs:1-4` states its precondition in the second sentence and is the pattern to copy.
  This matters because running these by name is what makes their panic defensible (see 2.2.4), and
  that argument assumes a reader can learn the precondition from the module doc.

### 3.3 Modularity and consistency

- **3.3.1 MEDIUM — `gb_peak_values` re-implements the overwrite guard the API already has.**
  `src/bin/gb_peak_values.rs:75-131`: `output_path` duplicates `is_xlsx`/`checked_workbook_path`
  (`src/api/io.rs:600-620`) and its message duplicates `ConversionError::OutputExists`
  (`src/error.rs:70-71`) in different words; the doc rationale at `gb_peak_values.rs:119-122` is a
  copy of `io.rs:596-599`. Have `gb_peak_values` call the API's conversion and delete `output_path`,
  as the sibling `ev_csv_to_xlsx` already does for exactly this reason.

- **3.3.2 MEDIUM — The two converters disagree on exit codes and error rendering.**
  `ev_csv_to_xlsx.rs:40-44` exits 1 when only the log failed; `gb_peak_values.rs:91-95` (and the
  app) exit 0 for the same event. `ev_csv_to_xlsx` prints errors bare (`:50-53`) where the other
  nine print `error: `, and prints usage to stdout with a failure exit (`:23-30`) where the others
  use stderr. Settle each of the three on the majority behaviour: a failed log alone exits 0,
  errors carry the `error: ` prefix, and usage printed on a failure path goes to stderr.

- **3.3.3 MEDIUM — `RSession` is documented as public and is `pub(crate)`.**
  `src/session/common.rs:139-142` "Public because it appears in public signatures … and naming
  the type a caller has to write is the point of an alias"; `src/session/mod.rs:60` is
  `pub(crate) use common::RSession`. `Sessions::sessions` is a public field typed with it, and
  `from_session_lists` (`common.rs:813-815`) says an outside caller "needs a way to make one" —
  that caller must spell `Vec<Vec<Rc<Session>>>`. The `unnameable_types` lint does not see
  aliases. Make the alias `pub use` — the doc comment already argues for it, and `Sessions::sessions`
  needs it — and fix the same comment's claim about `Anomaly` in the same edit; see 3.4.5.

- **3.3.4 LOW — Three copies of `export_row`, and one error slot drawn by two tabs.**
  `src/bin/ev_cost_recovery/surplus.rs:196`, `detail.rs:107` and `reimbursement.rs:251` define the
  same Copy/Save… row. The Cost recovery and Peak power detail tabs are two views of one
  `SurplusState`, so a save that fails on the Detail tab writes that state's single `error` field
  (`detail.rs:37`). The Detail tab does draw it, at `detail.rs:41`. The Cost recovery tab draws the
  same field at `surplus.rs:33-36`, directly under "Work out the surplus", where every other
  message in that slot means the run itself failed (`state.rs:691`, `:716`). The state already
  makes this distinction elsewhere: `SurplusState::log_failures` (`state.rs:286-292`) is a separate
  field precisely so that a failure arriving after the figures exist is not reported as the run's.
  Give a failed save its own field on the state, drawn only by the tab that wrote it, and lift the
  three `export_row` copies into one helper that takes that field. **[reported]**

- **3.3.5 LOW — `UndatedSessionReport` prints the file twice.** `src/api/pure/coverage.rs:66-70`
  writes `{path}: {cause}`; every `SessionReportNameError` arm (`file_name.rs:64-79`) already opens
  with the name: `data/June.csv: June is not a session report: …`. The app shows the cause alone
  (`state.rs:418-419`), so the same refusal reads differently from the two surfaces. Drop the
  `{path}: ` prefix and defer to the cause, which names its own file — the rule in `CLAUDE.md` about
  a wrapper that must not add a path to a cause that already carries one.

- **3.3.6 LOW — Helpers that only part of the traffic uses.** `src/session/peak.rs:166-171` writes
  out the body of `Sessions::countable()` (`src/session/common.rs:933-935`) instead of calling it;
  the other three sites do call it (`api/pure/energy.rs:201`, `recovery.rs:371`,
  `reimbursement.rs:220`). Call it here too.

  `src/session/csv.rs:412` writes `record: row - 2`. `Table::row_number` (`src/csv.rs:250-252`) is
  `index + 2`, so `- 2` is that function run backwards: it turns a CSV row number back into the
  record index. The value it recovers is one the caller still has. `session/csv.rs:210-212` holds
  the loop index `i`, computes `csv_row = Table::row_number(i)`, and passes only `csv_row` down, so
  the callee has to undo the addition to get `i` again. Passing `i` alongside `csv_row` would leave
  the `+ 2` written once, in the helper, rather than once forwards and once backwards.

- **3.3.7 LOW — Public items nothing calls, and one `pub` nothing can call.** A public item is
  reachable by definition, so "nothing calls it" is a claim about this repository, not about what
  the API permits. The test used below is: no call site in `src/`, `tests/` or `examples/`. What
  that costs is that no test exercises these and no build would notice if one broke.

  `EstimateSet::values` (`src/session/peak.rs:70-78`) has no call site. `EstimateSet` is `pub use`d
  (`src/session/mod.rs:51`) and its four fields are public, so a caller reads them directly.

  Three `From<…> for ApiError` impls have none either — `CostRecoverySurplusError`
  (`src/api/error.rs:184-191`), `PeakPowerError` (`:196-203`) and `EnergyError` (`:205-212`). Each
  builds `source: None`, and `io.rs` constructs those three variants by hand with `map_err`
  (`io.rs:111`, `:169`, `:203`, `:249`, `:361`) rather than letting `?` reach the impl.

  **These three stay.** Every one of those five sites computes a path first — `gb_source`,
  `bill_source` or `surplus_source` — so `?` could not be used there without discarding it. The
  impls are for a different caller, and `error.rs:193-194` says which: someone calling the public
  `pure::` functions directly, who holds no paths at all. That is a deliberate API shape, not an
  oversight, so the fact that this crate never takes that route is not an argument against them.
  **[reported]**

  `wall_clock_instant` (`src/time/excel.rs:60-64`) is the other kind, and it is called:
  `serial_of_civil` calls it at `src/time/excel.rs:36`. The defect is the `pub`. `mod excel` is
  private (`src/time/mod.rs:13`), and the `pub(crate) use` at `:37-39` lists its five `serial_of_*`
  siblings and not this one, so nothing outside that one file can name it. Its doc also gives a
  reason nothing acts on — "so that two of them can be subtracted"; nothing subtracts two, and the
  paragraph below it states the reason the function is actually there.

  Delete `EstimateSet::values`, which has no such justification: `EstimateSet`'s four fields are
  public and callers read them. Make `wall_clock_instant` private to its module and cut the
  subtraction sentence, leaving the paragraph that gives the real reason.

- **3.3.8 LOW — A library report names a GUI widget.** `src/api/pure/peak_power.rs:656-659`
  "The Peak power detail tab holds a section under each of those three names" is printed verbatim by
  `peak_power_cost_cli` and `cost_recovery_surplus_cli`, and pinned in the surplus golden. Say what
  the three names are for without naming a surface the reader may not be on, and regenerate the
  golden.

- **3.3.9 LOW — Two columns headed "TH blended rate" at different precision.** `energy.rs:365,474`
  `{:.5}` versus `peak_power.rs:555` `{:.4}`; the golden shows `0.15385` beside `10.0000`, under the
  same heading, in one report. Nothing in the code says the difference is deliberate.
  **It is, and the precisions stand.** The two rates
  are different orders of magnitude: a rate near `0.15` needs the fifth decimal to survive
  rounding, and one near `10` does not. Record that reason in a comment at each site, so the next
  reader does not unify them.

- **3.3.10 LOW — One ESPI join failure does not say which entry failed.**
  `src/green_button/espi.rs:186` raises "an IntervalBlock links to no MeterReading in this feed"
  when an `IntervalBlock` entry carries no `related` link matching any `MeterReading` collected in
  the pass before it. The message names no entry, so it does not say which block is the unlinked
  one.

  Its two neighbours do name theirs. `:166` raises "MeterReading {href} links to no ReadingType in
  this feed" and `:194` raises "ReadingTypes {seen} and {reading_type} both carry uom {uom}"; each
  interpolates the href it is complaining about.

  Scale is what makes the difference matter. The sample export holds 1,737 `IntervalBlock` entries
  (`docs/green_button/Toronto_Hydro_Object_Model.md:52`), so one bad link leaves a reader with 1,737
  candidates and nothing to narrow them by. The entry's own `self` href is in hand at that point:
  `link_href(*entry, "self")`, which `espi.rs:162-163` already calls for exactly this purpose one
  pass earlier. The fix is the same one-line `format!` the neighbours already use.

- **3.3.11 LOW — `hydro_bill_dump::with_advice`** (`src/bin/hydro_bill_dump.rs:65,80-84`) formats
  an error into a `String`
  and boxes it. It is the last hop before `eprintln!`, so nothing downstream loses structure, but
  it is the shape `CLAUDE.md` says nowhere is exempt from. Return the typed `BillError` and let the
  caller decide the advice from `is_layout()`, which is what it already inspects.

- **3.3.12 LOW — `resolve` returns a `Vec` its own contract says is always one element.**
  `src/session/csv.rs:374` returns `Vec<Row>`; the doc comment above it opens "Always one row"
  (`:368`) and the body ends `vec![self.row(…)]` (`:399`). The single production caller loops over
  it (`:212`), and the unit tests reach past the length with `rows[0]` or `.swap_remove(0)`
  (`:514`, `:667`, `:687`). Returning a `Row` would put the contract in the signature, where the
  compiler holds it, instead of in prose the caller has to trust.

### 3.4 Comments and doc comments

- **3.4.1 MEDIUM — Three places tell users a spike's average power is substituted; nothing
  substitutes one.** `src/session/common.rs:676-679` ("the estimating logic substitutes one") is
  rendered into every `.session.csv.read` log, the Convert tab and the session report
  (`tests/fixtures/sessions/Session_Report_Anomalies.report.md:82-85`); `docs/ERRORS.md:524-525`
  quotes it; `docs/session/README.md:170-172` gives an algorithm ("set `avg_kw` to 0 … otherwise to
  the constant `BREAKER_RATING_KW`") that the code does not run. What happens: a spike's energy is
  prorated over its reported span like any other session (`peak.rs:161-165`), and `avg_kw` is
  "left that way" (`common.rs:259-260`). The golden shows it: the `17:15` segment's `18.800` kW
  includes the `SPIKE` row's own 0.5 kWh. DSV4's D5 fixed the `spikes` field doc and missed these.
  `docs/session/README.md:146` also says "One of the five" anomaly kinds excludes; there are four.
  Rewrite `common.rs:676-679` to say a spike's energy is prorated over its reported span like any
  other session and its `avg_kw` is left as read; regenerate the anomalies fixture, and follow the
  same wording into `ERRORS.md` and `docs/session/README.md`, replacing the algorithm there. Fix
  the count in the same pass. 3.2.5 is the test that would hold `ERRORS.md` to it afterwards.

- **3.4.2 MEDIUM — Stale mechanisms described as current.**
  - `src/session/common.rs:505-509`: cause "(2) the session start/end adjustments cause an
    artificial overlap" — there are no adjustments. `src/session/peak.rs:553-557` justifies a
    test fraction by "Every adjusted end lands on the time grid … halves are not among the
    reachable fractions" — there is no grid, and a session `20:00:00–20:07:30` covers exactly half a
    quarter.
  - `src/session/common.rs:758-759`, `:990-991`, `src/session/csv.rs:67-68`: an excluded session
    may be one whose "reported wall time names no instant at all" — impossible on a fixed offset
    (`common.rs:115-116` says so), and no such `AnomalyKind` exists.
  - `src/api/pure/reimbursement.rs:520-522`: "energy is spread over its *adjusted* span, which pads
    the reported end out to the time grid" — the test's `0.05` tolerance rests on it; the true
    figure is exactly `4.0`. `src/api/pure/test_support.rs:121-125` says the same about "one-minute
    padding".
  - `src/api/io.rs:81-82` and five siblings document `session_csv1`/`session_csv2` parameters and
    "the two reports must cover"; the signatures take `session_csvs: &[&Path]` and
    `coverage.rs:126-127` says how many is not a rule. `docs/ERRORS.md:444` says "the two reports".
  - `src/session/csv.rs:675`, `common.rs:205-206`, `test_support.rs:56`,
    `src/session/consistency_band_tests.rs:165-167`: "three checks", "the other two checks",
    "check 1" of
    `duration_is_consistent`, whose own doc says one check (two on the branch).
  - `src/session/common.rs:777-811`, `src/log.rs:98-99,185-192`, `src/session/excel.rs:553-554`:
    "both readers", "read back from the workbook", `session.xlsx.read`, "adjusted UTC columns" —
    remnants of the deleted workbook reader that D1/D2 did not reach.
  - `src/session/common.rs:440`: segment sessions "in the order the report states them"; the
    golden prints `N1, N2, EXCESS, SPIKE` (rows 2, 3, 9, 6) because spikes are bucketed last.
  - `src/session/segment_tiling_tests.rs:10-11` cites `docs/session/segment-tiling.md`, deleted
    on `main` (see 2.1.2).

  Each describes a mechanism the crate no longer has, so each should be rewritten to the present
  rule or deleted. Two carry more than prose: `reimbursement.rs:520-522` justifies a `0.05`
  tolerance by the padding it describes, so tighten that test to the exact `4.0` when the comment
  goes; and the `session_csv1`/`session_csv2` parameter docs name parameters the signatures do not
  have, so those are doc-link breakage as well as stale prose.

- **3.4.3 MEDIUM — `reimbursement.rs` says two different things about where the month comes
  from.** Module doc `:15-16` "The month comes from the session report's file name, which is the only
  thing that states it"; `:36` and `:173-174` "from the Charges Report's own file name"; `io.rs:421`
  agrees with the latter. Correct the module doc to match the code, since the code is right.

- **3.4.4 MEDIUM — Comments that narrate earlier versions of the code.** The repository's own rules
  forbid this, and the count is high enough to be a pattern rather than a slip. The clearest:
  - `src/session/excel.rs:417-419`, written into every workbook: "The sheet used to carry a padded
    pair of columns beside these…" — user-visible, and the branch's new golden now pins it.
  - `src/session/peak.rs:439-441`, `:487-488`, `:602-603` ("`hi_crit` used to start at 0.0");
    `src/session/common.rs:175-177`, `:315-319`, `:735-737`, `:749-754`, `:975-976`;
    `src/session/csv.rs:316-317`; `src/session/consistency_band_tests.rs:12-14` (a commit hash),
    `:28`, `:44`; `src/csv.rs:10-11`; `src/error.rs:9-10`; `tests/session/mod.rs:9-11`.
  - `src/green_button/common.rs:3-4`, `:22-25`; `billing.rs:22-26`, `:59-61`; `espi.rs:237-239`,
    `:524-526`; `excel.rs:266-267`; `mod.rs:55-59`; `invoice_tests.rs:95-100`, `:138-145` (whose
    per-case `tolerance` column survives only to carry the history; all three are `0.001`);
    `src/time/excel.rs:7-13`; `tests/green_button/fixtures_golden.rs:28-31`, `:177-178`.
  - `src/hydro_bill/billing_period.rs:38-41` ("It was in three places before it was here"),
    `:270-272`; `src/charges_report.rs:733-736`.
  - `src/api/pure/peak_power.rs:744`; `recovery.rs:733-735`, `:1524-1526` (a test *named*
    `…_it_now_keeps`); `reimbursement.rs:678`; `test_support.rs:148,153`.
  - `src/bin/ev_cost_recovery/state.rs:822`, `:1815`; `theme.rs:89`, `:162`, `:225`; `main.rs:43`;
    `cost_recovery_cli.rs:49-50` and its sibling.
  - `docs/session/site-model-marcus.md` §9 and lines 7, 259, 275, 298 are a changelog against
    "the earlier typical-value document". This document is not exempt: remove the references to
    earlier versions, §9 included, and state the present figures and their derivation alone.
  Each should state the present rule; `git log` and `docs/archive/` hold the history.

- **3.4.5 LOW — False visibility claims.** Each of the four items below is `pub use`d out of a
  module `lib.rs` declares `pub mod`, so each is reachable from outside the crate by name.
  `src/api/error.rs:40` "the crate-private `green_button::GbReadError`" (`green_button/mod.rs:33`,
  out of `lib.rs:15`); `src/api/pure/mod.rs:18`, `src/green_button/excel.rs:288-289`,
  `src/hydro_bill/mod.rs:21-22` and `src/hydro_bill/bill.rs:63-65` "the crate-private
  `BillingPeriod`" (`hydro_bill/mod.rs:61`, out of `lib.rs:16`, and public because it types a
  public field); `src/hydro_bill/mod.rs:21-22` and `billing_period.rs:171-172` "the crate-private
  `billing_period_dates`" (`hydro_bill/mod.rs:52`, out of `lib.rs:16`, and called from the app);
  and `src/session/common.rs:142` "the crate-private `Anomaly`" (`session/mod.rs:48`, out of
  `lib.rs:22`), which sits in the doc comment 3.3.3 is about, so one edit settles both.
  `bill.rs:63-65` also says the period "runs from the 23rd of one month to the 23rd of the next",
  which `billing_period.rs:12-13` explicitly denies. Under the rule that these claims decide what
  may go private, each is the wrong kind of wrong. Drop "crate-private" from each site named above, and
  correct `bill.rs:63-65` to the midnight-starting-the-24th definition `billing_period.rs:12-13`
  gives.

- **3.4.6 LOW on its own, but do it with the branch — Two doc-comment twins of DSV4's C7.**
  `src/api/pure/energy.rs:87` reads "Total energy cost attributable to EV sessions, net of HST and
  OER" over `charges + hst - ontario_electricity_rebate` (`:314`). That is the same sentence over
  the same arithmetic as the delivery-side field DSV4 caught, and HST is added in both. DSV4 found
  only the one in `peak_power`. **The branch's fix to DSV4 C7 is what raises this from low to worth
  doing now:** with the delivery side corrected and this left alone, this becomes the only line in
  the crate still saying "net of HST and OER", beside a sibling field that spells out the opposite.
  The two then read as a deliberate distinction rather than as one uncorrected copy. See the C7 row
  in Part 1.

  Separately, `:58-63` documents three rate fields as "blended **nominal**" rates, where the
  function that derives them says the quotient "is what was actually charged per kWh, whatever the
  schedule said" (`:214-215`). Nominal is the opposite of actual, and a reader would take these for
  published tariff rates rather than figures divided out of the bill. Drop "nominal" and say the
  rates are derived from the bill, in the words `:214-215` already uses.

- **3.4.7 LOW — Miscellany.** `src/session/csv.rs:307-310` `#[allow(dead_code)]` on `energy_use`,
  which is read at `:387` and `:421` (the crate uses `#[expect]` elsewhere, which would have said
  so). `src/api/error.rs:105-106` and `io.rs:419-420` "before anything is opened" where four of
  seven readers open a file first. `src/api/io.rs:471-472` "where every reader of these files
  expects to find it" (nothing reads them). `src/green_button/common.rs:184` cites
  `csv::note_off_grid_rows`, which does not exist. `src/session/csv.rs:107` and `excel.rs:114-115`
  "zero-`Energy_Use` sessions" (the rule is on `Active_Charge_Time`). `src/session/common.rs:177,754`,
  `src/charges_report.rs:209,224,376` and `tests/charges_report/real_reports.rs:39` cite
  `Questions_for_Evolute.md` without its `docs/archive/` path. `tests/charges_report/real_reports.rs:122-123`
  names `charges_report::Charges::read` (no such item; the reader is `charges_report`);
  `src/charges_report.rs:665-666` says thousands separators "are handled by `number`" (it calls
  `parse_number`). `src/golden.rs:76-77` says `.gitattributes` "pins `*.report.md`"; the file says
  that rule failed and pins `tests/fixtures/**`. `src/session/report.rs:24-25` "the crate's single
  rendering module" (six other modules call `h2`/`table`); `:781-783` names fixtures this module
  does not use. `src/api/pure/peak_power.rs:9-11, 16, 277, 317-318, 414-415, 680-682`: paragraphs
  with orphaned fragments from an un-rewrapped link substitution. `src/api/pure/recovery.rs:771`: a
  `use` inside a test module between two functions. `src/bin/energy_cli.rs:60-62`,
  `peak_power_cli.rs:59-61`: "All three arguments are positional" and "All four arguments are
  positional", both for variadic usages.
  `src/session/consistency_band_tests.rs:63` "two sound ones" (there are three).

  Each is a one-line correction to the comment, except two that change code: swap the
  `#[allow(dead_code)]` for `#[expect]`, which then fails to compile and says the attribute is
  unnecessary; and move the `use` in `recovery.rs:771` to the top of its test module.

### 3.5 Documents

The maintenance manual is the largest concentration of error in the tree, and all of it is in the
first 380 lines — the part a maintainer reads first. Its second half is solid: I checked the four
fixture checksums against `sha256sum`, all eight named test functions, the six field-validation
messages, the 2026 holiday pin, the `debug_assert` at `peaks.rs:279` and the one-`print!` example,
and every one holds.

- **3.5.1 HIGH — `docs/maintenance-manual.md:375-376` names three anomaly kinds that do not
  exist.** "The three DST kinds on the session side exist because Evolute reports wall times."
  `AnomalyKind` (`src/session/common.rs:556-591`) has four variants and none is about DST, and
  `README.md:206` states the opposite premise — the session report is on standard time all year, so
  DST cannot reach it. A maintainer following this paragraph would go looking for a mechanism the
  crate does not have, and might build one. Delete the paragraph and re-derive the section from
  `AnomalyKind`'s four variants.

- **3.5.2 HIGH — `docs/maintenance-manual.md:256-258, 262` lists three error variants that do not
  exist.** `ReimbursementError::NotACalendarMonth`, `ChargesReportIsForAnotherMonth` and
  `NotOneSessionReport` appear nowhere in `src/`; `ReimbursementError`
  (`src/api/pure/reimbursement.rs:41-46`) has exactly one variant, `RatesNotYetInEffect`. The month
  check now goes through `pure::check_reports_cover` (`src/api/io.rs:421`), so the refusal is a
  `CoverageError`. This is the section that tells a maintainer which messages are deliberately
  absent from `docs/ERRORS.md`, so it has to be re-derived from the types as they are.
  (`GbReadError::BillEndDayOutOfRange`, named beside them, does exist.) Rewrite the list from the
  error enums as they stand, adding `GbReadError::NotABillingCalendar` (3.1.15) and keeping
  `BillEndDayOutOfRange`.

- **3.5.3 MEDIUM — `docs/maintenance-manual.md:299-300` names two methods that do not exist.**
  "`Session::adj_duration` and `SessionOverlap::duration` panic on the same inversion". Neither
  symbol is in the crate. The real second panic site is `Interval::from_start_end`
  (`src/time/base.rs`), which is where DSV4's A1 transcript shows the crash landing. Name that site
  instead of the two that do not exist.

- **3.5.4 MEDIUM — The manual's structure does not match its own contents, and it breaks its own
  citation rule.** The Contents lists two sections — "A generated workbook is not
  byte-reproducible" and "Adding an `AnomalyKind`" — that have no heading anywhere in the file, and
  "Adding an `Anomaly`" (`maintenance-manual.md:345-347`) then refers to the missing one as "the two
  sections". The body has `# Shared` and `# Green Button` and no `# Sessions`, so three session-only
  sections sit under Shared. And `:12` says "Cite sections by title, never by number", while `:99`
  and `:102` cite
  `§1` — which does not exist; the rule meant is "The rule the tests are written to" (`:179-181`).
  Rebuild the Contents from the headings that exist, add a `# Sessions` heading over the three
  session-only sections, and replace the two `§1` citations with that rule's title.

- **3.5.5 MEDIUM — The manual states the golden regeneration command four different ways.**
  `maintenance-manual.md:54-57` gives it with a `session::` filter, `:62` calls the same command
  "unfiltered", `:81`
  gives it with no filter, and `src/session/report_rendering_tests.rs:30` gives a fourth. The table
  also omits a golden: `tests/fixtures/api/EV_Cost_Recovery_Surplus.report.md`, pinned at
  `src/api/pure/recovery.rs:1521`. The two subsections below it repeat the table and the "read the
  diff" paragraph. One table, all three goldens, one command each.

- **3.5.6 MEDIUM — `README.md:242-243` gives a build command that fails on a fresh checkout.**
  `cargo build --release` panics in `build.rs:81` until `bash scripts/gen-notices.sh` has run. The
  prerequisite is stated only in the License section three screens earlier (`README.md:147`). Put it in the
  build block.

- **3.5.7 LOW — Four more errors in `docs/maintenance-manual.md`.** `:64-66` says `--test <file>`
  "no longer selects anything", two lines after using `--test integration`, and `tests/docs_errors.rs`
  is a separate binary. `maintenance-manual.md:324-325` says "README says so" of a byte-for-byte
  claim README does not make. `:454` cites the crate `ev-peak-contrib`, which was merged into this
  one (same sentence
  survives at `src/green_button/excel.rs:17`). `:508` and `:562` give the same export file in two
  directories. Four one-line corrections: name the binary and the module filter that actually
  select a test, drop the "README says so" clause or add the claim to README, name this crate in
  place of `ev-peak-contrib`, and give the export path once, under `data/green_button/`.

- **3.5.8 LOW — `README.md:236` describes the private test-only `golden` module as a mechanism for
  integration tests.** `src/golden.rs:1-9` says the reverse: it is for unit tests, and integration
  tests use `fixtures_dir_in` in `tests/common`. Nothing under `tests/` references `golden::`. The
  table reads as public API; `golden` and `markdown` are private. Correct the row to say unit tests,
  and mark both modules private in the table so it stops reading as the public surface.

- **3.5.9 LOW — `README.md:220` gives a per-vehicle figure that matches nothing.** "approximately
  6.7 kW and 6.8 kVA"; the model gives 6.589 kW / 6.656 kVA per vehicle, and 6.797 kW / 7.218 kVA
  at the transformer primary for one vehicle (`docs/session/site-load-report-marcus.txt:5,11`).
  Quote one pair and say where it is measured. `README.md:118` also reads "Each converted files".

- **3.5.10 LOW — `docs/site-specific-constants.md` duplicates three blocks of the manual
  verbatim** (`:15-29`, `:39-41`, `:97-102` against `maintenance-manual.md:129-143`, `:145-150`,
  `:190-197`), and says "The manual lists them in full" directly under its own copy of the table.
  Only the values column is unique to it. One home; the values belong here.

- **3.5.11 LOW — `CLAUDE.md:74-75` states a count that has moved.** "`with_extension("xlsx")` is
  written out in seven places"; there are now eleven. It is presented as a present-tense fact
  inside a rule. State the rule without the count: the count is what moved, and the rule did not.

- **3.5.12 MEDIUM — `docs/green_button/README.md:33` defines the billing period wrongly.** "spans
  00:00:00 EST (inclusive) on the 24th of a month to 00:00:00 EST (exclusive) on the 23rd of the
  following month" excludes the whole 23rd. The code (`billing_period.rs:91-94`), the golden
  (744 intervals for June, not 720) and `docs/time/README.md:64` all cut at midnight starting the
  24th of both months. Line 5 also omits the 7-7 kVA column the writer produces. Restate line 33 as
  midnight starting the 24th to midnight starting the 24th of the following month, and add the
  missing column to line 5.

- **3.5.13 MEDIUM — `docs/session/site-model-marcus.md:78` divides the wrong number.** "Per-unit
  winding resistance is total loss over rating, 1490 W / 75 000 VA = 0.0172 pu": 1490/75000 is
  0.0199. The figure 0.0172 is the *load* loss, 1293 W (the document's own line 73), over the
  rating, and it is what gives the code's `XFMR_REACTANCE_PU = 0.0383`. The result is right; the
  sentence would send a reader to the wrong datasheet line — and
  `docs/site-specific-constants.md:33` sends the next site to this document for "each derivation",
  where following it as written gives 0.0370. The error is inherited verbatim from
  `docs/session/Google_Transformer_characteristics.md:89-91`, whose own headline figure
  ("~0.040 to 0.041") also disagrees with the 0.0383 its calculation reaches. That file is a
  verbatim transcript, so a one-line note under it is the fix there; `site-model-marcus.md:78`
  should simply read 1293 W.

- **3.5.14 MEDIUM — `docs/time/README.md:81-82, 116-117` cite `time::SESSION_OFFSET` and
  `time::session_instant`.** Both are in `src/session/common.rs` (`:104`, `:117`);
  `src/time/format.rs:5`
  has it right. The second passage says `time` "knows nothing about sessions" and owns
  `session_instant` in the same sentence. `_todo/_todo.md` already marks this document for a full
  proofread; add these. Also `docs/time/README.md:17` "Excel serial dates, in both directions" —
  one direction exists.

- **3.5.15 MEDIUM — `tests/green_button/fixtures_golden.rs:9-10` says the DST fixtures are "745"
  and "671" intervals.** The goldens they check say `744` and `672` (`dst_fall.golden.txt:78`,
  `dst_spring.golden.txt:78`), and `billing.rs:40-41` states the rule. A maintainer reading this
  table would take a 745 as expected, which `docs/maintenance-manual.md:408-409` calls the drift
  signal. Correct the two counts in the test's doc to `744` and `672`, which is what the goldens
  and `billing.rs:40-41` both say.

- **3.5.16 LOW — `docs/Evolute_portal_alignment.md`.** `:5` "Every change below is on branch
  `evolute-portal-align`" — no such branch exists locally or on `origin`; the work is on `main`.
  `:36-38` "adjusted demand — loss factor and days/30 proration" conflates the two adjustments the
  bill makes (loss factor on kWh only; days/30 on demand only). `:179` "six API entry points" —
  there are seven. Say the work is on `main`, separate the two adjustments by which quantity each
  applies to, and drop the entry-point count rather than correcting it.

- **3.5.17 LOW — `docs/app-cheat-sheet.md:260`** "In each tab, pressing the **Save** button saves
  the displayed report": the Convert tab has none, the button is `Save…`, and the names offered
  (`EV_Cost_Recovery_Surplus_<date>.report.md` etc., `state.rs:722,727,929`) are not stated.
  Everything else in the sheet agrees with the app. Say which three tabs have the button, write it
  as `Save…`, and give the default file name each offers.

- **3.5.18 LOW — `docs/ERRORS.md` line citations were worse than DSV4 said.** Of 33 checked, 16 land
  on unrelated code and four on a *neighbouring* entry's arm (e.g. `bill_pdf.rs:195-197` for "the
  bill has no such figure" is the `UnknownCharge` arm; the message is at `:191-193`). Moot once the
  branch's symbol citations merge; noted because D8 sampled nine and reported drift of "one to three
  lines". Take the branch's change and cite by symbol throughout, which is what stops the drift
  returning; no separate work is needed here.

- **3.5.19 LOW — Stale paths and small errors outside D10's list.**
  `docs/green_button/Toronto_Hydro_Object_Model.md:21` "run `uv run explore_model.py`" (archived);
  `tests/green_button/full_feed.rs:34` "docs/Toronto_Hydro_Object_Model.md";
  `tests/green_button/fixtures_golden.rs:52` "`docs/reference/`" — correct all three paths. The
  Object Model document never
  describes the `rel="related"` link from `IntervalBlock` to `MeterReading` that `espi.rs:184-186`
  requires; add it, since that link is what 3.3.10 is about.
  `docs/session/README.md:173-174` lacks the blank line that stops "Not an anomaly…" from
  rendering as part of the `ExcessiveAvgKw` bullet; insert it.
  `docs/session/site-load-report-marcus.txt` is
  byte-identical to `tests/fixtures/sessions/site_load.report.txt` and pinned by nothing. It stays:
  it is end-user documentation, and an end user is not expected to read a fixture. Since the copy
  stays, pin it — assert the two files match in the test that already owns the fixture, so the
  document cannot drift from the report the code renders.

---

## Part 4 — Recommended order of work

**1. Land the branch.** In this order, on the branch: `cargo fmt`; move `parse_rates` to
`impl FromStr for CostRecoveryRates` with a typed error, and the four band/cent helpers into
`recovery.rs`; move `commas_group_thousands` out of `src/csv.rs`, which never calls it, into a
neutral leaf (2.2.3); restore the two ignored tests' panics and drop the "Run the data-gated tests
too" step from `.github/workflows/ci.yml`,
then give `full_feed.rs` the precondition sentence its module doc lacks (3.2.10) and stop it
blaming absent data for a parse failure (3.2.9), which is what running these by name depends on;
fix the two `docs/ERRORS.md` citation defects named in 2.1.3 and 2.3; cut the "used to carry"
sentence from the workbook comment and regenerate the two `.workbook.txt` goldens; drop the
branch's rewrite of `segment-tiling.md` and
keep `main`'s deletion, taking only the `segment_tiling_tests.rs` sketch correction and deleting
that module's citation of the document at `:10-11`; correct the energy-side twin of DSV4 C7 named
in 3.4.6, which the branch's own fix to DSV4 C7 is what makes misleading; the four small text items
in 2.3. Then merge.

**2. Correctness on `main`, smallest first.** 3.1.6 (`containing` assertion), 3.1.1
(`session_csv_to_xlsx` to `pub(crate)`), 3.1.4 (stale data paths, visible skips), 3.1.3 (coverage
message for a calendar month, and the `ERRORS.md` entry), 3.1.8
(`.csv` in the Charges Report name), 3.1.7 (which file a row number belongs to, and the note that
promises a lookup), 3.1.9 (`-0.00`). Then two one-liners
that cost nothing to carry but nothing to fix either: 3.1.5, tying the notices `rerun-if-changed`
to release builds so local iteration stops recompiling; and 3.1.2, which is
one comment: say on `metering_adj` that it is read because the bill states it, used by nothing, and
not to be used until someone establishes what the utility does with it.

**3. The maintenance manual's first half.** 3.5.1 and 3.5.2 first — the anomaly kinds and the error
variants that do not exist — then 3.5.3, 3.5.4 (structure and the `§1` citations), 3.5.5 (one golden
table), 3.5.7. This is the document a maintainer opens before touching anything, and it currently
names five symbols the crate does not have.

**4. Other prose that is false.** 3.4.1 (the `ZeroActiveChargeTime` sentence, `ERRORS.md`,
`docs/session/README.md`), 3.5.12 (billing period definition), 3.5.13 (1490 → 1293), 3.5.15
(745/671), 3.5.6 (the README build command), 3.4.3 (which file names the month), 3.4.2's `io.rs`
parameter docs.

**5. Checks that would have caught the above.** 3.2.1 (`--document-private-items` in the prescribed
command and in CI), 3.2.5 (pin the quoted anomaly text), 3.2.2 (the exact fraction), 3.2.3, 3.2.4.

**6. The comment sweep.** 3.4.4 (history), 3.4.2 (stale mechanisms), 3.4.5 (visibility claims),
3.4.7. One pass, file by file, against the rule already in `CLAUDE.md`. Most are a sentence each.

**7. Structure, when quiet.** 3.3.1–3.3.3 (the guard duplication, the converters' exit codes,
`RSession`), then the lows in 3.3 and 3.1.10–3.1.17, and the remaining document lows in 3.5.

---

## What is in good shape

Stated because a list of findings does not describe the crate. The money arithmetic is
careful and argued: every divisor goes through `HydroBill::divisor` or `blended_rate`,
`to_the_cent` rounds through the formatter so the surplus agrees with its own column by
construction, and a 200-schedule sweep pins that. The time-of-use schedules match the Ontario
Energy Board text quoted above them, the holiday calendar reproduces the ten 2026 dates including
the Christmas collision, and `tou_partition` is checked for coverage and contiguity across both
daylight-saving transitions. The ESPI join follows the link chain rather than a shared-token
shortcut and refuses a second reading type per unit, non-hourly data and an empty feed. The billing
period arithmetic is right at the month-end and leap-year edges, and the standard-time boundary is
both derived and documented. `load_over_panels` has the strongest test battery in the crate:
continuity at every panel boundary, monotonicity past aggregate capacity, and the idle-panel
standing block, none of them naming an electrical constant. The structured-error rule is followed
everywhere the reviewers looked, the `deny(private_interfaces, private_bounds, unnameable_types)`
at the crate root does the visibility policing the comments claim, and the invoice test reconciles
a real bill's kWh, both demand figures and all three time-of-use buckets. The defects above are
concentrated in prose that outlived the mechanism it described.
