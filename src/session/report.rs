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
//! - **No emphasis markers.** Labels are quoted — `"Energy-based"` — which reads identically either
//!   way.
//! - **Session ids live in their own section**, not in a table cell, because a markdown table row is
//!   a single line and a segment holding twelve sessions cannot be wrapped inside one.
//!
//! This is the crate's single rendering module. [`site_load_report`] lives here too, for the same
//! reason [`fmt::Display`] delegates to [`IntervalEstimates::to_markdown`]: one rendering rather
//! than two that could drift.

use super::{
    Anomaly, AnomalyKind, Bracket, IntervalEstimates, RSession, Segment, Session, SessionNotes,
    site_load::{
        BREAKER_RATING_A, CONTINUOUS_DUTY_DERATE, PANEL_BREAKER_COUNT, PANEL_VOLTAGE_V,
        XFMR_RATING_KVA, ev_load, ev_pilot_current_a, loading_ratio, site_load,
    },
};
use crate::{
    markdown::{Align, Left, Right, h1, h2, table, wrap},
    time::{Interval, time_zone},
};
use jiff::{Timestamp, Zoned};
use std::{collections::BTreeMap, fmt};

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

/// Dated, and to the minute. The excluded list covers the whole source report, so its dates cannot
/// be left implicit the way a segment's can.
fn ymd_hm(ts: Timestamp) -> String {
    local(ts).strftime("%Y-%m-%d %H:%M").to_string()
}

/// The far end of a span whose near end is already printed: the date only when it differs.
///
/// The same convention the header's interval line follows, and it is what keeps the excluded-
/// sessions table inside the width the report has to read at. Nearly every session begins and ends
/// on one day, so repeating the date would spend sixteen columns to say what the previous cell
/// already said; a session that does cross midnight still says so.
fn ymd_hm_to(from: Timestamp, to: Timestamp) -> String {
    let (from, to) = (local(from), local(to));
    match from.date() == to.date() {
        true => to.strftime("%H:%M").to_string(),
        false => to.strftime("%Y-%m-%d %H:%M").to_string(),
    }
}

/// A bracket as one cell, `min-max`. Three decimals, matching every other figure in the report.
///
/// Always both ends, never a midpoint: the two numbers are what the reported times actually
/// support, and collapsing them would state a precision the minute-resolution source does not have.
/// An exact bracket still prints both, so a column of them stays a column of the same shape.
fn bracket_cell(b: Bracket<f64>) -> String {
    format!("{:.3}-{:.3}", b.min, b.max)
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
/// keeps the workbook's `anomalies` column a list of bare variant names that
/// [`AnomalyKind::from_token`] can read back, and keeps the glossary below the table explaining
/// each kind once rather than once per session.
fn anomaly_cell(kind: AnomalyKind, avg_kw: f64) -> String {
    match kind {
        AnomalyKind::ExcessiveAvgKw => format!("{}({avg_kw:.3})", kind.as_str()),
        _ => kind.as_str().to_owned(),
    }
}

/// One glossary entry per kind present, in first-appearance order.
///
/// The prose comes from each kind's [`fmt::Display`], so there is one wording to maintain rather
/// than a second copy here that could drift from it.
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
        for source in &self.sources {
            out.push(format!("- {}", source.display()));
        }
        out.push(String::new());
        if self.sources.len() > 1 {
            out.push(wrap(
                "A billing period straddles two calendar months and a session report covers one, \
                 so two are read.",
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
    /// Counted would not do. Such a record's start, end and duration contradict each other, so the
    /// only way to judge what happened is to read the row -- and its absence moves every figure
    /// drawn from these sessions without appearing in any of them.
    fn push_excluded(&self, out: &mut Vec<String>) {
        if self.excluded.is_empty() {
            return;
        }
        out.push(h2("Sessions left out"));
        out.push(String::new());
        out.push(wrap(
            "These records' reported start, end and duration contradict each other, so they \
             cannot be placed on a timeline and take no part in any figure above. Every one of \
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
             judgement call. Only what bears on these figures is listed.",
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
fn by_source_table(rows: impl IntoIterator<Item = (RSession, Option<AnomalyKind>)>) -> String {
    let mut by_file: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
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
        by_file.entry(file_name(&session)).or_default().push(vec![
            file_name(&session),
            session.row.to_string(),
            session.id.clone(),
            flags,
        ]);
    }
    let rows: Vec<Vec<String>> = by_file.into_values().flatten().collect();
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

const ESTIMATE_HEADERS: [&str; 5] = ["Estimate", "Unit", "Min", "Max", "Segment"];
const ESTIMATE_ALIGN: [Align; 5] = [Left, Left, Right, Right, Left];

impl IntervalEstimates {
    /// Renders the report as markdown that is also readable as plain text. See the module docs for
    /// what that constraint rules out.
    ///
    /// [`fmt::Display`] delegates here, so there is one rendering rather than two that could drift.
    pub fn to_markdown(&self) -> String {
        let mut out: Vec<String> = Vec::new();

        out.push(h1("EV Peak Power Contribution"));
        out.push(String::new());
        out.push(format!(
            "Source     {}",
            self.sources
                .iter()
                .map(|p| p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string_lossy().into_owned()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push(format!("Interval   {}", interval_line(self.interval)));
        out.push(String::new());
        out.push(String::new());

        self.push_estimates(&mut out);
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

    /// The figures for the maximal segment on each derivation, and the prose that reads them.
    fn push_estimates(&self, out: &mut Vec<String>) {
        out.push(h2("Estimates"));
        out.push(String::new());

        let (energy_seg, energy_est) = &self.energy_based_seg_estimate;
        let (count_seg, count_est) = &self.count_based_seg_estimate;
        let row = |label: &str, unit: &str, b: Bracket<f64>, seg: &Segment| {
            vec![
                label.to_owned(),
                unit.to_owned(),
                format!("{:.3}", b.min),
                format!("{:.3}", b.max),
                hm(seg.start()),
            ]
        };
        let rows = vec![
            row("Energy-based", "kW", energy_est.energy_based_kw, energy_seg),
            row(
                "Energy-based",
                "kVA",
                energy_est.energy_based_kva,
                energy_seg,
            ),
            row("Count-based", "kW", count_est.count_based_kw, count_seg),
            row("Count-based", "kVA", count_est.count_based_kva, count_seg),
        ];
        out.push(table(&ESTIMATE_HEADERS, &rows, &ESTIMATE_ALIGN));
        out.push(String::new());

        out.push(wrap(
            "Every figure is a bracket: the reported session times are stated only to the minute, \
             so each estimate runs from what those times least support to what they most support. \
             \"Energy-based\" is derived from the sessions' own consumption, \"Count-based\" from \
             how many of them were charging and the per-EV rating of the infrastructure. \
             \"Segment\" names the 15-minute segment the figure was drawn from - the one where \
             that derivation peaks, which the two need not agree on.",
            "",
        ));
        out.push(String::new());

        out.push(wrap(
            "The peak is always a 15-minute average, whatever the length of the interval asked \
             for, because that is the basis the demand charge is billed on. An hour is reported as \
             the highest of its four segments, not as an average over the whole hour.",
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
                     reported times that contradict each other. They are listed under Excluded sessions.",
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
                    bracket_cell(seg.agg_count()),
                    bracket_cell(seg.agg_kw()),
                ]
            })
            .collect();
        out.push(table(
            &["Segment", "Count", "kW"],
            &rows,
            &[Left, Right, Right],
        ));
        out.push(String::new());
        out.push(wrap(
            "Times are local (ET), and each segment is 15 minutes long, named by the minute it \
             starts on. Segments are half-open: each runs from its own start up to but not \
             including the next one's, so no instant falls in two of them and they tile the \
             interval exactly. \"Count\" is a session count weighted by how much of the segment \
             each session covered, so it is fractional; \"kW\" weights each session's average \
             power the same way.",
            "",
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
            // Wrapped rather than put in a table cell: a markdown row is one line, so a segment of
            // twelve sessions could not be broken across lines inside one.
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
                    ymd_hm(s.adj_conn_start()),
                    ymd_hm_to(s.adj_conn_start(), s.adj_conn_end()),
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
            "These sessions take no part in any estimate. Times are local (ET), and the list \
             covers the whole source report rather than the interval estimated, so \"From\" \
             carries its date and \"To\" carries one only when the session crosses midnight. \
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

/// The header's interval line, naming the UTC offset in force at each end.
///
/// Naming it is not decoration. On the night DST ends an hour of wall time occurs twice, so an
/// interval can begin at `01:30` and end at `01:30` — the same clock reading an hour apart. Written
/// as bare local times that reads as a window of no duration; written with the offsets it reads as
/// what it is. When both ends share an offset, which is every interval but two a year, it is stated
/// once at the end.
fn interval_line(interval: Interval) -> String {
    let (lo, hi) = (interval.start, interval.end());
    let (lo_z, hi_z) = (local(lo), local(hi));
    let (lo_off, hi_off) = (
        lo_z.strftime("%Z").to_string(),
        hi_z.strftime("%Z").to_string(),
    );
    let length = interval_length(lo, hi);
    if lo_off == hi_off {
        format!(
            "{} - {} {lo_off}  ({length})",
            lo_z.strftime("%Y-%m-%d %H:%M"),
            hi_z.strftime("%H:%M"),
        )
    } else {
        format!(
            "{} {lo_off} - {} {hi_off}  ({length})",
            lo_z.strftime("%Y-%m-%d %H:%M"),
            hi_z.strftime("%H:%M"),
        )
    }
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

impl fmt::Display for IntervalEstimates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_markdown())
    }
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
/// beside `docs/ev-charger-power-factor-and-kva-allocation.md`, not a document anyone renders.
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
        "  Per vehicle      {:.2} kVA = {:.2} kW + {:.2} kvar + {:.2} kvar distortion\n",
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
        let load = site_load(ev_count as f64);
        let percent = loading_ratio(load) * PERCENT;
        let flag = if percent > PERCENT {
            "  <- over nameplate"
        } else {
            ""
        };

        out.push_str(&format!(
            "{:>4}  {:>9.2}  {:>9.2}  {:>11.2}  {:>9.2}  {:>7.3}  {:>7.1}%{}\n",
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

    let full = site_load(PANEL_BREAKER_COUNT as f64);
    out.push_str(&format!(
        "\nAt full occupancy: {:.2} kW, {:.2} kVA, {:.1}% of nameplate.\n",
        full.real_kw,
        full.apparent_kva(),
        loading_ratio(full) * PERCENT
    ));

    out
}
