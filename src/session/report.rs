//! Renders an [`IntervalEstimates`] as markdown that also reads as plain text.
//!
//! Both at once is the whole constraint, and it drives every choice here. Not every reader has a
//! markdown renderer, so the output has to survive being read raw:
//!
//! - **Setext headings** (`====`, `----`) rather than `#`, so a heading looks underlined instead of
//!   prefixed with punctuation. That allows two heading levels, which is why the sub-labels in the
//!   estimates section are sentences rather than a third level.
//! - **Every table cell padded** to its column's width, numerics right-aligned. A renderer ignores
//!   the padding; a plain reader depends on it entirely.
//! - **No four-space indentation anywhere**, since markdown would turn it into a code block. Wrapped
//!   list items indent by two.
//! - **No emphasis markers** inside a report. Labels are quoted — `"Energy-based"` — which reads
//!   identically either way. [`DEFINITIONS_POINTER`] is the one bold line, and it heads a document
//!   of reports rather than sitting inside one.
//! - **Session ids live in their own section**, not in a table cell, because a markdown table row is
//!   a single line and a segment holding twelve sessions cannot be wrapped inside one.
//!
//! **What the terms mean is said once per document.** Several interval reports are ordinarily read
//! together — one per demand a charge is priced on — and they share every term, so a report carries
//! figures and the document carries the definitions. A caller assembling them opens with
//! [`DEFINITIONS_POINTER`] and closes with [`definitions`].
//!
//! This is the crate's single rendering module. [`site_load_report`] lives here too: one rendering
//! rather than two that could drift.

use super::{
    Anomaly, AnomalyKind, Estimate, IntervalEstimates, RSession, Session, SessionNotes,
    report_coverage,
    site_model::{
        BREAKER_RATING_A, CONTINUOUS_DUTY_DERATE, PANEL_BREAKER_COUNT, PANEL_VOLTAGE_V,
        XFMR_RATING_KVA, ev_load, ev_pilot_current_a, loading_ratio, single_panel_load,
    },
};
use crate::{
    markdown::{Align, Left, Right, field, h1, h2, table, wrap},
    time::{Interval, time_zone, zoned_minute, zoned_span, zoned_span_end},
};
use jiff::{Timestamp, Zoned, civil::Date};
use std::{cmp::Ordering, collections::BTreeMap, path::PathBuf};

fn local(ts: Timestamp) -> Zoned {
    Zoned::new(ts, time_zone())
}

/// A segment's name: its start as a local clock time.
///
/// To the minute and no finer, and undated. Segments sit on the time grid, so their seconds
/// would be three zeroes in every row; and they all fall inside one interval of interest, whose
/// date the header states once. This is the name the Estimates table's `Segment` column and the
/// membership list both use, so the three sections join on it.
fn hm(ts: Timestamp) -> String {
    local(ts).strftime("%H:%M").to_string()
}

/// Whether an excluded session's reported span appears to meet the interval of interest, as a
/// report cell.
///
/// [`Session::lenient_intersects`] rather than [`Session::intersects`], and only here: an excluded
/// session may report an end before its start, which is a span the strict test treats as a broken
/// precondition and refuses. This listing exists to show exactly those records, so it answers for
/// them — as an appearance, which is all a contradictory record supports.
fn in_interval(session: &Session, ioi: &Interval) -> String {
    match session.lenient_intersects(ioi) {
        true => "yes".to_owned(),
        false => "no".to_owned(),
    }
}

/// An anomaly's cell: the bare kind, except where the kind is about a figure, in which case the
/// figure is written into it.
///
/// The value lives here rather than on [`AnomalyKind`], which stays a plain classification. That
/// keeps the workbook's `anomalies` column a list of bare variant names, matching
/// `AnomalyKind::as_str`, and keeps the glossary below the table explaining each kind once rather
/// than once per session.
fn anomaly_cell(kind: AnomalyKind, avg_kw: f64) -> String {
    match kind {
        AnomalyKind::ExcessiveAvgKw => format!("{}({avg_kw:.3})", kind.as_str()),
        _ => kind.as_str().to_owned(),
    }
}

/// Source files ordered by the first date their names say they cover.
///
/// Every list of files in a report goes through this, so a reader meets them running forward in
/// time however the run was given them. The order they were read in is not that order: the app's
/// two pickers are filled in whichever order suits the user, and the command line takes its reports
/// as they were typed.
///
/// The *name* is what states the dates — see [`report_coverage`], and the module docs there for why
/// the contents cannot be asked. A name that states none sorts last rather than first: it is the
/// odd one out, and it reads as such at the foot of a list that is otherwise in order.
///
/// The sort is stable and the date is the whole key, so files the name cannot date — and two files
/// covering the same dates — stay in the order they were given. Sorting those on the path instead
/// would order the one population that has no order of its own by something a reader cannot see.
fn chronological(sources: &[PathBuf]) -> Vec<&PathBuf> {
    let mut dated: Vec<(Option<Date>, &PathBuf)> = sources
        .iter()
        .map(|path| (report_coverage(path).map(|c| c.from), path))
        .collect();
    dated.sort_by(|(a, _), (b, _)| match (a, b) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    dated.into_iter().map(|(_, path)| path).collect()
}

/// One glossary entry per kind present, in first-appearance order.
///
/// The prose comes from each kind's [`Display`](std::fmt::Display), so there is one wording to
/// maintain rather than a second copy here that could drift from it.
fn glossary(kinds: impl IntoIterator<Item = AnomalyKind>, out: &mut Vec<String>) {
    let mut seen: Vec<AnomalyKind> = Vec::new();
    for kind in kinds {
        if !seen.contains(&kind) {
            seen.push(kind);
            out.push(wrap(&format!("- {} - {}.", kind.as_str(), kind), "  "));
        }
    }
}

impl SessionNotes {
    /// Renders what a figure was drawn from as markdown that also reads as plain text.
    ///
    /// Three parts, and each is omitted when it has nothing to say — except the sources, which are
    /// always named. A period's figures rest on two monthly reports, and which two is the first
    /// thing a reader checking a number wants to know.
    ///
    /// Grouped by source file throughout. A row number means nothing without the file it is a row
    /// of, and a reader who has spotted something goes to one file to look it up.
    ///
    /// Written here rather than beside each result type, because every one of them would render it
    /// the same way and the sections are about sessions rather than about money.
    pub fn to_markdown(&self) -> String {
        // Nothing at all, not even a heading. A sub-report inside a surplus gives its notes up to
        // the hoisted section at the top, and an empty section under an empty heading would read
        // as a claim that there was nothing to say.
        if self.sources.is_empty() && self.is_clean() {
            return String::new();
        }

        let mut out: Vec<String> = Vec::new();

        out.push(h2("Session data"));
        out.push(String::new());
        for source in chronological(&self.sources) {
            out.push(format!("- {}", source.display()));
        }
        out.push(String::new());
        if self.sources.len() > 1 {
            // Says why a period needs covering, and not how many files that takes. Any number is
            // wrong somewhere: one export spanning the whole period is a single file, a period can
            // be covered by three, and a file named twice puts two entries above with one report
            // behind them.
            out.push(wrap(
                "A billing period runs from the 24th of one month to the 23rd of the next, so it \
                 takes as many session reports as it takes to reach across those dates. They are \
                 listed above in the order their names say they begin.",
                "",
            ));
            out.push(String::new());
        }

        self.push_excluded(&mut out);
        self.push_anomalies(&mut out);
        out.join("\n")
    }

    /// The sessions left out of the figures entirely, listed in full.
    ///
    /// Counted would not do. Such a record cannot be placed on a timeline, so the only way to judge
    /// what happened is to read the row -- and its absence moves every figure drawn from these
    /// sessions without appearing in any of them.
    fn push_excluded(&self, out: &mut Vec<String>) {
        if self.excluded.is_empty() {
            return;
        }
        out.push(h2("Sessions left out"));
        out.push(String::new());
        out.push(wrap(
            "These records cannot be placed on a timeline, so they take no part in any figure \
             above: their reported start, end and duration contradict each other. Every one of \
             them is energy the chargers may have drawn and none of the figures counts.",
            "",
        ));
        out.push(String::new());
        out.push(by_source_table(
            self.excluded.iter().map(|s| (s.clone(), None)),
        ));
        out.push(String::new());
    }

    /// What needed a judgement call, filtered to what bears on the figure. See [`Sessions::notes`].
    fn push_anomalies(&self, out: &mut Vec<String>) {
        if self.anomalies.is_empty() {
            return;
        }
        out.push(h2("Sessions needing a look"));
        out.push(String::new());
        out.push(wrap(
            "These sessions count towards the figures above, and something about them needed a \
             judgement call. Only what bears on the above figures is listed.",
            "",
        ));
        out.push(String::new());
        out.push(by_source_table(
            self.anomalies
                .iter()
                .map(|a| (a.session.clone(), Some(a.kind))),
        ));
        out.push(String::new());
        glossary(self.anomalies.iter().map(|a| a.kind), out);
        out.push(String::new());
    }
}

/// Rows grouped under the file they came from, as one table with a `File` column.
///
/// One table rather than one per file. The lists are ordinarily short -- a period with nothing
/// wrong in it renders neither section at all -- and a single table lines its columns up across
/// files, which several tables of two rows each would not.
///
/// The groups run in [`chronological`] order, which is the order the section above lists the same
/// files in. Within a group the rows keep report order, so a `Row` column reads downwards.
fn by_source_table(rows: impl IntoIterator<Item = (RSession, Option<AnomalyKind>)>) -> String {
    // Keyed by the whole path as the reader was given it, not by the file's name: two reports of
    // the same name in different directories are two files, and grouping them together would put
    // rows from both under one heading with nothing to tell them apart. The `File` column still
    // shows the name alone -- see `file_name`.
    //
    // `paths` records first appearance, which is the order the files were read in. It is what
    // `chronological` falls back on for a name that states no dates, and a map's own order -- by
    // path, or by hash -- would be no order to a reader.
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut by_file: BTreeMap<PathBuf, Vec<Vec<String>>> = BTreeMap::new();
    for (session, kind) in rows {
        let flags = match kind {
            Some(kind) => kind.as_str().to_owned(),
            // No kind given means the whole row is the point: list what it carries.
            None => session
                .anomalies
                .iter()
                .map(AnomalyKind::as_str)
                .collect::<Vec<_>>()
                .join(", "),
        };
        let path = session.path.as_ref().clone();
        if !by_file.contains_key(&path) {
            paths.push(path.clone());
        }
        by_file.entry(path).or_default().push(vec![
            file_name(&session),
            session.row.to_string(),
            session.id.clone(),
            flags,
        ]);
    }
    let rows: Vec<Vec<String>> = chronological(&paths)
        .into_iter()
        .filter_map(|path| by_file.remove(path))
        .flatten()
        .collect();
    table(
        &["File", "Row", "Session", "Anomaly"],
        &rows,
        &[Left, Right, Left, Left],
    )
}

/// The source file's name alone, without its directory.
///
/// The full paths are listed once at the head of the section; repeating a directory on every row
/// would push the columns that matter off the width a plain-text reader has.
fn file_name(session: &Session) -> String {
    session.path.file_name().map_or_else(
        || session.path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

const ESTIMATE_HEADERS: [&str; 4] = ["Estimate", "Unit", "All-in power", "Segment"];
const ESTIMATE_ALIGN: [Align; 4] = [Left, Left, Right, Left];

impl IntervalEstimates {
    /// Renders the report as markdown that is also readable as plain text. See the module docs for
    /// what that constraint rules out.
    ///
    /// `peak` names the building peak the interval was chosen for — `"kVA"`, `"kW"`, `"kW 7-7"` —
    /// and titles the report. `share` is the estimate that is EV charging's share of that peak,
    /// and is marked in the Estimates table. Both are the caller's to say: an interval is only an
    /// interval, and which peak it was the one for is decided where it was chosen.
    ///
    /// No `Display`, for the same reason. There is no rendering of an interval that does not know
    /// what it was chosen for.
    ///
    /// The terms the report uses are not defined in it; see the module docs.
    pub fn to_markdown(&self, peak: &str, share: Estimate) -> String {
        let mut out: Vec<String> = Vec::new();

        out.push(h1(&format!("EV Peak {peak} Contribution")));
        out.push(String::new());
        out.push(field(
            "Source",
            &chronological(&self.sources)
                .into_iter()
                .map(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.to_string_lossy().into_owned())
                })
                .collect::<Vec<_>>()
                .join(", "),
        ));
        out.push(field("Interval", &interval_line(self.interval)));
        out.push(String::new());
        out.push(String::new());

        self.push_estimates(&mut out, peak, share);
        self.push_segments(&mut out);
        self.push_membership(&mut out);
        self.push_excluded(&mut out);
        self.push_anomalies(&mut out);

        let mut s = out.join("\n");
        s.push('\n');
        s
    }

    /// Whether no session reached any segment of the interval.
    fn is_deserted(&self) -> bool {
        self.seg_estimates
            .iter()
            .all(|(seg, _)| seg.sessions.is_empty())
    }

    /// The figures for the maximal segment on each derivation, with `share` marked.
    fn push_estimates(&self, out: &mut Vec<String>, peak: &str, share: Estimate) {
        out.push(h2("Estimates"));
        out.push(String::new());

        // The mark is part of the cell, so it lines up in plain text as well as in a renderer: the
        // column is right-aligned, and the marked figure's digits stay under the others'.
        let rows: Vec<Vec<String>> = Estimate::ALL
            .into_iter()
            .map(|estimate| {
                let figure = self.figure(estimate);
                vec![
                    estimate.derivation().to_owned(),
                    estimate.unit().to_owned(),
                    if estimate == share {
                        format!("* {figure:.3}")
                    } else {
                        format!("{figure:.3}")
                    },
                    hm(self.segment_for(estimate).0.start()),
                ]
            })
            .collect();
        out.push(table(&ESTIMATE_HEADERS, &rows, &ESTIMATE_ALIGN));
        out.push(String::new());
        out.push(wrap(
            &format!("\"*\" - Portion of building's peak {peak} attributed to EV charging."),
            "",
        ));

        if self.is_deserted() {
            out.push(String::new());
            out.push(wrap(
                "No session intersected the interval of interest, so no vehicle charged in it. \
                 The figures above are not zero all the same: the charging infrastructure draws a \
                 standing block whenever the transformer is energised, and that block is part of \
                 the building's demand whether or not a car is plugged in.",
                "",
            ));
        }

        if !self.excluded_sessions.is_empty() {
            out.push(String::new());
            let n = self.excluded_sessions.len();
            out.push(wrap(
                &format!(
                    "{} in the source report {} excluded from every figure above, having \
                     reported times that cannot be placed on a timeline. They are listed under Excluded sessions.",
                    if n == 1 {
                        "One session".to_owned()
                    } else {
                        format!("{n} sessions")
                    },
                    if n == 1 { "was" } else { "were" },
                ),
                "",
            ));
        }

        out.push(String::new());
        out.push(String::new());
    }

    /// Every segment of the interval, with the two aggregates every estimate is derived from.
    ///
    /// `agg_count` and `agg_kw` and nothing else: the four estimates are functions of these two,
    /// so a table of the estimates per segment would repeat the Estimates section four times over
    /// while saying no more than this does.
    fn push_segments(&self, out: &mut Vec<String>) {
        out.push(h2("Segments"));
        out.push(String::new());

        let rows: Vec<Vec<String>> = self
            .seg_estimates
            .iter()
            .map(|(seg, _)| {
                vec![
                    hm(seg.start()),
                    format!("{:.3}", seg.agg_count()),
                    format!("{:.3}", seg.agg_kw()),
                ]
            })
            .collect();
        // Named for what the sessions did in the segment, as distinct from the Estimates table's
        // "All-in power", which is what the model makes of these two once the transformer's own
        // load is added. `definitions` says how each is worked out.
        out.push(table(
            &["Segment", "Session count", "Session kW"],
            &rows,
            &[Left, Right, Right],
        ));
        out.push(String::new());
        out.push(String::new());
    }

    /// Which sessions are in which segment.
    fn push_membership(&self, out: &mut Vec<String>) {
        out.push(h2("Sessions by segment"));
        out.push(String::new());

        for (seg, _) in &self.seg_estimates {
            let ids: Vec<String> = seg.sessions.iter().map(|s| s.id.clone()).collect();
            let body = if ids.is_empty() {
                "none".to_owned()
            } else {
                ids.join(", ")
            };
            // Wrapped so a long segment can break across lines.
            out.push(wrap(&format!("- {} - {body}", hm(seg.start())), "  "));
        }

        out.push(String::new());
        out.push(String::new());
    }

    /// Every session excluded from the estimates, whether or not it appears to touch the interval.
    ///
    /// Listed in full rather than filtered, because the filter would be applied to exactly the
    /// timestamps that are in doubt. A session whose fields contradict each other may belong in
    /// this interval and still test as falling outside it, so "In interval" is reported as what it
    /// is — a reading of the same unreliable times — and no row is dropped on its say-so.
    fn push_excluded(&self, out: &mut Vec<String>) {
        if self.excluded_sessions.is_empty() {
            return;
        }
        out.push(h2("Excluded sessions"));
        out.push(String::new());

        let rows: Vec<Vec<String>> = self
            .excluded_sessions
            .iter()
            .map(|s| {
                vec![
                    s.row.to_string(),
                    s.id.clone(),
                    zoned_minute(s.conn_start),
                    zoned_span_end(s.conn_start, s.conn_end),
                    in_interval(s, &self.interval),
                    // An excluded session is in no segment, but the report holds the session
                    // itself here, so its figure needs no lookup.
                    s.anomalies
                        .iter()
                        .map(|k| anomaly_cell(*k, s.avg_kw()))
                        .collect::<Vec<_>>()
                        .join(", "),
                ]
            })
            .collect();
        out.push(table(
            &["Row", "Session", "From", "To", "In interval", "Anomaly"],
            &rows,
            &[Right, Left, Left, Left, Left, Left],
        ));
        out.push(String::new());
        out.push(wrap(
            "These sessions take no part in any estimate. Times are local and name the zone they \
             are read in, which through the summer is an hour later than the session report states \
             them; the report is on standard time all year. The list covers the whole source \
             report rather than the interval estimated, so \"From\" carries its date and \"To\" \
             carries one only when the session crosses midnight. \
             \"In interval\" \
             is whether the session appears to fall in the interval - appears only, because a \
             record whose own fields contradict each other cannot be trusted to say where it \
             belongs. It reads the same doubtful times, so no row was dropped on its say-so.",
            "",
        ));
        out.push(String::new());

        glossary(
            self.excluded_sessions
                .iter()
                .flat_map(|s| s.anomalies.iter().copied()),
            out,
        );
        out.push(String::new());
        out.push(String::new());
    }

    fn push_anomalies(&self, out: &mut Vec<String>) {
        out.push(h2("Anomalies"));
        out.push(String::new());

        if self.session_anomalies.is_empty() {
            out.push(wrap(
                "None. Every session considered for this interval was well formed.",
                "",
            ));
            out.push(String::new());
            return;
        }

        // No "In interval" column here, unlike the Excluded sessions table. Every session listed
        // reaches the interval of interest — that is the condition on which the anomaly was
        // collected at all — so the column would read yes on every row of every report, and a
        // column with one possible value tells a reader nothing while inviting them to look for a
        // distinction that is not there. The scoping is stated in the note below instead.
        let rows: Vec<Vec<String>> = self
            .session_anomalies
            .iter()
            .map(|a: &Anomaly| {
                vec![
                    a.session.row.to_string(),
                    a.session.id.clone(),
                    // The session's own figure, not the sheet's `#DIV/0!` — the number that fed the
                    // totals is the one worth seeing beside the flag.
                    anomaly_cell(a.kind, a.session.avg_kw()),
                ]
            })
            .collect();
        out.push(table(
            &["Row", "Session", "Anomaly"],
            &rows,
            &[Right, Left, Left],
        ));
        out.push(String::new());
        let mut note = "Row numbers are rows of the source data file named above, so each one can \
                        be looked up directly. Only sessions reaching the interval of interest are \
                        listed here"
            .to_owned();
        if self.excluded_sessions.is_empty() {
            note.push_str(
                "; a session anomalous elsewhere in the source report is not this interval's \
                 concern.",
            );
        } else {
            note.push_str(
                ". The Excluded sessions table above is scoped differently - it covers the whole \
                 source report, and carries an \"In interval\" column for that reason.",
            );
        }
        out.push(wrap(&note, ""));
        out.push(String::new());

        glossary(self.session_anomalies.iter().map(|a| a.kind), out);
        out.push(String::new());
    }
}

/// The header's interval line: the span, then how long it is.
///
/// The span comes from [`zoned_span`], which names the offset in force at each end — see its docs
/// for why that is not decoration.
fn interval_line(interval: Interval) -> String {
    let (lo, hi) = (interval.start, interval.end());
    format!("{}  ({})", zoned_span(lo, hi), interval_length(lo, hi))
}

/// "1 hour" / "15 minutes", for the header.
fn interval_length(lo: Timestamp, hi: Timestamp) -> String {
    let plural = |n: i64, unit: &str| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    let secs = hi.duration_since(lo).as_secs();
    match secs {
        s if s % 3600 == 0 => plural(s / 3600, "hour"),
        s if s % 60 == 0 => plural(s / 60, "minute"),
        s => plural(s, "second"),
    }
}

/// The line that opens a document of interval reports, pointing to [`definitions`] at its end.
pub const DEFINITIONS_POINTER: &str =
    "**See definitions and conventions at the end of this report.**";

/// The section that closes a document of interval reports: what its terms mean, and how to read
/// its times.
///
/// One section for the whole document, because every interval report in it uses the same terms.
pub fn definitions() -> String {
    let paragraph = |text: &str| wrap(text, "");
    // A list's items sit on consecutive lines, each wrapped under its own dash.
    let list = |items: &[&str]| {
        items
            .iter()
            .map(|item| wrap(&format!("- {item}"), "  "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let blocks = [
        paragraph(
            "\"Interval\" is a metering interval during which a particular power metric peaks. An \
             interval is half-open, i.e., it runs from its start time up to but not including its \
             end time.",
        ),
        paragraph(
            "\"Segment\" designates the 15-minute segment where a particular value peaks. \
             Segments are half-open: each runs from its own start up to but not including the \
             next one's, so no instant falls in two of them and they tile the interval exactly.",
        ),
        paragraph(
            "A peak value is always a 15-minute average, whatever the length of the interval, \
             because that is the basis the demand charge is billed on. Metering data for a \
             one-hour interval reports the highest of its four segments, not an average over the \
             whole hour.",
        ),
        paragraph(
            "\"Session count\" is the number of active sessions during the segment. It is how much \
             of a segment was occupied - each session contributes the fraction of the segment \
             that it covers, so one session connected throughout adds 1 and one connected for \
             half of it adds 0.5, which makes the session count fractional.",
        ),
        paragraph(
            "\"Session kW\" is the average power over a segment: each session's reported energy is \
             spread evenly over its own connection span, the part of it falling in the segment is \
             taken, and the sum is divided by the segment's duration in hours (i.e., 0.25).",
        ),
        // One block, the list directly under the sentence that introduces it: every definition here
        // is one block, and this one is the sentence and its list together.
        format!(
            "{}\n{}",
            paragraph(
                "The \"All-in power\" values are calculated using the built-in electrotechnical \
                 model, as follows:",
            ),
            list(&[
                "Energy-based kW and kVA -- The model takes the \"Session kW\" value and computes \
                 the resulting \"all-in\" kW and kVA values, which include the load from the \
                 transformer.",
                // The derate is the model's own, so the definition follows it if it is changed.
                &format!(
                    "Count-based kW and kVA -- A charger's nominal current is the breaker's \
                     amperage rating derated to {:.0}% for continuous duty; multiplied by the standard \
                     voltage, that gives the nominal kVA of one charger, and multiplied by the \
                     typical power factor for a charger, its nominal kW. The model scales these \
                     by the \"Session count\" and adds in the load from the transformer, as for \
                     the energy-based values.",
                    CONTINUOUS_DUTY_DERATE * PERCENT
                ),
            ]),
        ),
        paragraph(
            "Cost estimates are always derived from \"Energy-based\" values. \"Count-based\" values \
             are shown for comparison purposes only. Also, the peak \"Energy-based\" and \
             \"Count-based\" segments may differ.",
        ),
        paragraph(
            "Times are local, on the zone the interval names. During DST, that is an hour later \
             than the session report states the same instants, which are on standard time all \
             year. Each 15-minute segment is named by the minute it starts on.",
        ),
    ];
    let mut out = vec![h1("Definitions and Conventions")];
    for block in blocks {
        out.push(String::new());
        out.push(block);
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

// ---------------------------------------------------------------------------
// Site load
// ---------------------------------------------------------------------------

/// Percentage scaling, so the two places that need it read as one intent rather than as a bare
/// `100.0`.
const PERCENT: f64 = 100.0;

/// The site load model tabulated for every vehicle count the panel can hold.
///
/// Fixed-width plain text rather than markdown: this is a table of the model's own constants, read
/// beside `docs/session/site-model-marcus.md`, not a document anyone renders.
pub fn site_load_report() -> String {
    let mut out = String::new();
    let per_ev = ev_load();

    out.push_str("Level 2 EV charging site - load at transformer primary\n\n");
    out.push_str(&format!(
        "  Panel            {:.0} V, {} x {:.0} A breakers\n",
        PANEL_VOLTAGE_V, PANEL_BREAKER_COUNT, BREAKER_RATING_A
    ));
    out.push_str(&format!(
        "  Pilot current    {:.1} A per vehicle ({:.0}% continuous derate)\n",
        ev_pilot_current_a(),
        CONTINUOUS_DUTY_DERATE * PERCENT
    ));
    out.push_str(&format!(
        "  Per vehicle      {:.3} kVA = {:.3} kW + {:.3} kvar + {:.3} kvar distortion\n",
        per_ev.apparent_kva(),
        per_ev.real_kw,
        per_ev.reactive_kvar,
        per_ev.distortion_kvar
    ));
    out.push_str(&format!(
        "  Transformer      {:.0} kVA\n\n",
        XFMR_RATING_KVA
    ));

    out.push_str(&format!(
        "{:>4}  {:>9}  {:>9}  {:>11}  {:>9}  {:>7}  {:>8}\n",
        "EVs", "kW", "kvar", "kvar (dis)", "kVA", "PF", "% rated"
    ));
    out.push_str(&format!("{}\n", "-".repeat(69)));

    for ev_count in 0..=PANEL_BREAKER_COUNT {
        let load = single_panel_load(ev_count as f64);
        let percent = loading_ratio(load) * PERCENT;
        let flag = if percent > PERCENT {
            "  <- over nameplate"
        } else {
            ""
        };

        out.push_str(&format!(
            "{:>4}  {:>9.3}  {:>9.3}  {:>11.3}  {:>9.3}  {:>7.3}  {:>7.1}%{}\n",
            ev_count,
            load.real_kw,
            load.reactive_kvar,
            load.distortion_kvar,
            load.apparent_kva(),
            load.true_power_factor(),
            percent,
            flag
        ));
    }

    let full = single_panel_load(PANEL_BREAKER_COUNT as f64);
    out.push_str(&format!(
        "\nAt full occupancy: {:.3} kW, {:.3} kVA, {:.1}% of nameplate.\n",
        full.real_kw,
        full.apparent_kva(),
        loading_ratio(full) * PERCENT
    ));

    out
}

#[cfg(test)]
mod test {
    use super::*;

    fn ordered(names: &[&str]) -> Vec<String> {
        let paths: Vec<PathBuf> = names.iter().map(PathBuf::from).collect();
        chronological(&paths)
            .into_iter()
            .map(|p| p.display().to_string())
            .collect()
    }

    /// Every list of files a report prints is in this order, and the order is the dates the names
    /// state rather than the order the run was given them. A user fills the app's two pickers in
    /// whichever order suits them.
    ///
    /// A unit test on the ordering alone: the rendering fixtures name their files `May.csv` and
    /// `June.csv`, which state no dates at all, so nothing in them could pin this.
    #[test]
    fn source_files_are_listed_by_the_date_their_names_begin() {
        assert_eq!(
            ordered(&[
                "/data/Session_Report_June_1_2026-June_30_2026.csv",
                "/data/Session_Report_May_1_2026-May_31_2026.csv",
            ]),
            [
                "/data/Session_Report_May_1_2026-May_31_2026.csv",
                "/data/Session_Report_June_1_2026-June_30_2026.csv",
            ]
        );

        // The first date, not the last: a report reaching further back comes first however far
        // forward the other one runs.
        assert_eq!(
            ordered(&[
                "/data/Session_Report_June_1_2026-June_30_2026.csv",
                "/data/Session_Report_May_1_2026-June_30_2026.csv",
            ]),
            [
                "/data/Session_Report_May_1_2026-June_30_2026.csv",
                "/data/Session_Report_June_1_2026-June_30_2026.csv",
            ]
        );
    }

    /// A file whose name states no dates has no place in the order, so it keeps the one it came in
    /// with and goes at the end, where it reads as the exception it is.
    #[test]
    fn a_file_the_name_cannot_date_sorts_last_and_keeps_its_place() {
        assert_eq!(
            ordered(&[
                "/data/sessions.csv",
                "/data/Session_Report_June_1_2026-June_30_2026.csv",
                "/data/anything.csv",
                "/data/Session_Report_May_1_2026-May_31_2026.csv",
            ]),
            [
                "/data/Session_Report_May_1_2026-May_31_2026.csv",
                "/data/Session_Report_June_1_2026-June_30_2026.csv",
                "/data/sessions.csv",
                "/data/anything.csv",
            ]
        );
    }

    /// The section that explains the model is pinned, like the reports around it.
    ///
    /// It is prose a user reads to understand a figure, and it is the one rendering with no
    /// fixture of its own: `peak_power_cli` prints it under every report and the app's Peak power
    /// detail tab renders and saves it, so a change to it is user-visible text. A golden rather
    /// than assertions about substrings, for the reason [`crate::golden`] gives — what matters
    /// here is the wording and the wrapping, and only a diff shows either.
    #[test]
    fn the_definitions_section_matches_its_golden() {
        crate::golden::check("sessions/definitions.txt", &definitions());
    }
}
