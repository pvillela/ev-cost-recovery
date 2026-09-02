//! Reading a session report CSV, as Evolute exports it.
//!
//! This is where the session report becomes [`Session`]s: the CSV is parsed, each record's local
//! wall times are resolved to UTC, the DST fold is settled or the record duplicated, and every
//! judgement call is recorded as an [`AnomalyKind`]. Nothing here knows about workbooks.
//!
//! The zone arithmetic itself is [`crate::time`]'s: this module holds only the policy — which
//! reading of an ambiguous wall time the record's own `Conn_Duration` supports, which anomaly to
//! raise, and what a record gets when no reading fits.
//!
//! Two ways out, sharing all of that:
//!
//! - [`csv_sessions`] buckets the sessions for the peak power contribution logic, straight from
//!   the CSV. No workbook is involved, which is why it is the route the API takes.
//! - `csv_session_rows` hands the rows over in report order, with the pass-through CSV fields
//!   still reachable, which is what
//!   [`session_csv_to_xlsx`](crate::session::session_csv_to_xlsx) needs to render them.
//!
//! Bucketing is lossy — it sorts sessions into three vectors and drops report order, the link back
//! to the CSV record, and the reported wall times — so the writer cannot be built on
//! [`csv_sessions`]. Both sit on `csv_session_rows` instead.
//!
//! Opening the file, finding the columns by name and refusing one that has no `Energy_Use` are
//! not this module's work: [`crate::csv`] does that for both of Evolute's reports, and what is
//! left here is what only a session report means.

use super::{
    Anomaly, AnomalyKind, BREAKER_MAX_NORMAL_KW, RSession, Session, Sessions, TIME_GRID_STEP,
    duration_is_consistent,
};
use crate::{
    csv::{CsvReadError, Document, Table},
    log::{RunLog, SourceLog},
    time::{
        UNPLACEABLE_END, UNPLACEABLE_START, falls_in_gap, is_on_grid, local_datetime,
        local_readings,
    },
};
use jiff::{Timestamp, civil};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

/// CSV columns that must be present for the file to mean anything.
const REQUIRED_HEADERS: &[&str] = &[
    "Charge_Session_ID",
    "Conn_DateTime_Start",
    "Conn_DateTime_End",
    "Conn_Duration",
    "Active_Charge_Time",
    "Energy_Use",
];

/// Why a session report CSV could not be read.
///
/// Everything a column-by-name reader can fail at is [`Self::Csv`], which is where the file, the
/// row and the column live and where the message is written. Only what is particular to *this*
/// document is a variant here. The counterparts are
/// [`GbReadError`](crate::green_button::GbReadError),
/// [`ChargesReportError`](crate::charges_report::ChargesReportError) and
/// [`BillError`](crate::hydro_bill::BillError).
///
/// Only whole-file failures are here. A per-row *judgement* call is not an error: it is carried on
/// [`Session::anomalies`] and summarised in the log, because the row still yields a session.
/// Resolving a wall time is not among them. A reported time that names no instant, or two, is a
/// judgement call and not a failure: it is flagged on the session and the row is still written.
#[derive(Debug)]
pub(crate) enum SessionCsvError {
    /// The file could not be opened, is not a readable CSV, is missing a column this reader needs,
    /// or holds a cell that will not parse.
    Csv(CsvReadError),
}

impl From<CsvReadError> for SessionCsvError {
    fn from(cause: CsvReadError) -> Self {
        Self::Csv(cause)
    }
}

impl fmt::Display for SessionCsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Already written in full, document and path included. Adding either here would print
            // it twice.
            Self::Csv(cause) => cause.fmt(f),
        }
    }
}

impl Error for SessionCsvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Csv(cause) => Some(cause),
        }
    }
}

/// Reads the session report CSV at `path` and returns the charging sessions it describes, ready
/// for the peak power contribution logic.
///
/// The counterpart of `excel::historic::xlsx_to_sessions`, and the way to reach the sessions
/// without a workbook in between — which is the route the API takes, the workbook reader being
/// behind the `historic` feature. The two agree on every figure — the workbook writer and this
/// function are the same parse — but they differ in what they can tell you afterwards. A workbook
/// has stored
/// derived columns that may have been edited, so reading one compares them against the recomputed
/// values and logs any disagreement. A CSV has nothing to compare against: it is the source.
///
/// The domain rules — the UTC conversion and its DST policy, the definitions of `adj_conn_end` and
/// `adj_conn_duration`, and the treatment of zero-`Energy_Use` sessions — are specified in
/// `docs/time/README.md` under "Time zone" and in `docs/session/README.md` under "Other".
///
/// The records are sorted into the three buckets of [`Sessions`] by
/// `Sessions::from_session_lists`, which carries the rules. Every session in the file reaches one
/// of them; none is dropped.
///
/// The anomalies found, and the off-grid warning if it applies, are returned on
/// [`Sessions::logs`] as a `session.csv.read` log — the same content [`super::excel::session_csv_to_xlsx`]
/// puts in its `session.convert` log, because the two run the same parse. Nothing is written here.
/// [`Sessions::write_logs`] is what puts it beside the input, and only a binary calls it.
///
/// # Errors
///
/// Returns `Err` only for conditions that invalidate the whole file: it cannot be read, a required
/// header from the private `REQUIRED_HEADERS` is missing, or a timestamp or duration does not
/// parse. Per-row judgement calls do not abort the read; they are carried on each
/// [`Session::anomalies`] and summarised in the log.
pub(crate) fn csv_sessions(path: &Path) -> Result<Sessions, SessionCsvError> {
    read_sessions(path)
}

fn read_sessions(path: &Path) -> Result<Sessions, SessionCsvError> {
    let rows = csv_session_rows(path)?;
    let log = SourceLog {
        source: path.to_path_buf(),
        suffix: "session.csv.read",
        operation: "Read Session Report",
        log: rows.log,
    };
    // One list, and it still goes through the merge: a file can state a record twice as readily as
    // two files can, and the merge is also where a shared `Charge_Session_ID` is noticed.
    let sessions: Vec<RSession> = rows.rows.into_iter().map(|row| row.session).collect();
    Ok(Sessions::from_session_lists(
        vec![sessions],
        vec![path.to_path_buf()],
        vec![log],
    ))
}

/// Every row one session report CSV yields, in the order the report states them.
///
/// The unbucketed form of [`csv_sessions`], for a caller that has to render the report rather than
/// estimate from it. It keeps what bucketing discards: report order, the pass-through CSV fields,
/// and the reported wall times, which differ from a re-derivation inside the DST gap.
///
/// The `table` is held rather than copied out because the pass-through columns are not part of
/// a [`Session`] and should not become part of one — a `Session` is what the arithmetic needs, not
/// a copy of the source row. Reach them through [`SessionRows::field`] and
/// [`SessionRows::duration`], which keep the column lookup and the parsing on this side of the
/// boundary.
pub(super) struct SessionRows {
    /// The file as read. Held for the pass-through columns, and for its path:
    /// [`SessionRows::duration`] parses a cell on demand and its error has to name the report,
    /// which the rest of the parsing did while it still had the path in hand.
    table: Table,
    /// One per output row, in report order. A record duplicated to resolve a DST fold yields two.
    pub rows: Vec<Row>,
    /// Every judgement call made, numbered by output row rather than by CSV record.
    pub anomalies: Vec<Anomaly>,
    /// Unwritten. The caller writes it beside its own output file, under its own suffix.
    pub log: RunLog,
}

impl SessionRows {
    /// The named CSV column for `row`, trimmed. Empty when the column is absent or blank.
    pub fn field(&self, row: &Row, name: &str) -> &str {
        self.table.cell(row.record, name)
    }

    /// The named CSV column for `row` parsed as an elapsed time, or `None` when the cell is blank.
    pub fn duration(
        &self,
        row: &Row,
        name: &'static str,
    ) -> Result<Option<Duration>, SessionCsvError> {
        let raw = self.field(row, name);
        if raw.is_empty() {
            return Ok(None);
        }
        parse_duration(raw, self.table.path(), Table::row_number(row.record), name)
            .map(Some)
            .map_err(SessionCsvError::from)
    }
}

/// Parses `path` and resolves every record, without writing anything.
///
/// Shared by [`csv_sessions`] and [`session_csv_to_xlsx`](crate::session::session_csv_to_xlsx),
/// which is what makes the two agree by construction rather than by inspection.
pub(super) fn csv_session_rows(path: &Path) -> Result<SessionRows, SessionCsvError> {
    let table = Table::read(path, Document::SessionReport, REQUIRED_HEADERS)?;
    // One allocation for the file, shared by every session read from it.
    let source = Rc::new(path.to_path_buf());

    let mut anomalies = Vec::new();
    let mut rows: Vec<Row> = Vec::new();
    for i in 0..table.record_count() {
        // The CSV row, counting the header, and the number every session parsed from this record
        // carries. A record duplicated to resolve a DST fold yields two sessions and they share it,
        // because they share the row they came from — see `Session::row`. The workbook row is a
        // different number, and it belongs to the workbook: `super::excel` derives it from a
        // session's position in `rows` when it writes one.
        let csv_row = Table::row_number(i);
        let session = CsvSession::parse(&table, i, csv_row)?;
        for row in session.resolve(&source, csv_row) {
            anomalies.extend(row.session.anomalies.iter().map(|&kind| Anomaly {
                session: row.session.clone(),
                kind,
            }));
            rows.push(row);
        }
    }

    // Anomalies only: there is nothing to compare against on this side, since this is what
    // produces the values in the first place. See `crate::log` for why discrepancies are a
    // separate channel.
    let mut log = RunLog::new();
    note_off_grid_rows(&rows, &mut log);
    for anomaly in &anomalies {
        // `OffGridTimes` is summarised above instead. A report that has switched resolution has
        // switched it throughout, so listing it per row would bury every other finding under a few
        // hundred identical lines.
        if anomaly.kind != AnomalyKind::OffGridTimes {
            log.note(anomaly.to_string());
        }
    }

    Ok(SessionRows {
        table,
        rows,
        anomalies,
        log,
    })
}

/// Warns once per file when reported boundaries do not land on [`TIME_GRID_STEP`].
///
/// Every allowance this software makes for the reporting's truncation assumes the reported times
/// are truncated to that step. If Evolute starts reporting seconds, they no longer are, and the
/// allowances become too wide rather than wrong — sessions get a padded end they do not need, and
/// the consistency window admits records it should reject. Nothing crashes and no figure looks
/// odd, which is exactly why it needs saying out loud.
///
/// Once per file with a count and the first three rows, not once per row: on a report that has
/// switched resolution every row qualifies, and a log with 238 identical lines is a log nobody
/// reads.
fn note_off_grid_rows(rows: &[Row], log: &mut RunLog) {
    // Reads the flag rather than repeating the test, so the summary and the anomalies cannot
    // disagree about which rows qualify.
    let offenders: Vec<&Row> = rows
        .iter()
        .filter(|r| r.session.anomalies.contains(&AnomalyKind::OffGridTimes))
        .collect();
    if offenders.is_empty() {
        return;
    }
    let examples: Vec<String> = offenders
        .iter()
        .take(3)
        .map(|r| format!("row {} ({})", r.session.row, r.session.id))
        .collect();
    log.note(format!(
        "{} of {} rows report a start or end that is not a whole multiple of {:?}: {}. The \
         session report's resolution has become finer than this software's time grid. Nothing is \
         wrong with these rows, but the padding and the consistency window are now wider than the \
         data needs — see docs/maintenance-manual.md, \"Boundaries and the time grid\".",
        offenders.len(),
        rows.len(),
        TIME_GRID_STEP,
        examples.join(", ")
    ));
}

// ---------------------------------------------------------------------------
// CSV input
// ---------------------------------------------------------------------------

/// A cell of this report that will not parse.
///
/// The document is pinned here, once, rather than at each of the parsers below.
fn bad_value(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
    cause: impl fmt::Display,
) -> CsvReadError {
    CsvReadError::bad_value(Document::SessionReport, path, row, column, s, cause)
}

/// Local time as `YYYY-MM-DD HH:MM`, or with seconds appended so a finer-grained report still
/// parses.
fn parse_local(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<civil::DateTime, CsvReadError> {
    // 1. Try parsing with seconds first
    if let Ok(dt) = civil::DateTime::strptime("%Y-%m-%d %H:%M:%S", s) {
        return Ok(dt);
    }

    // 2. Fall back to parsing without seconds (seconds default to 00)
    civil::DateTime::strptime("%Y-%m-%d %H:%M", s).map_err(|e| bad_value(s, path, row, column, e))
}

/// `H:MM:SS`, with hours unbounded so a session longer than a day still parses.
///
/// Returns an unsigned [`Duration`], matching [`Session`]'s own fields. The sign is rejected here
/// rather than carried: `Conn_Duration` and `Active_Charge_Time` are elapsed times, and a negative
/// one is a malformed cell, not a value to propagate. Only the DST-fold comparison in
/// [`CsvSession::reproduces_reported_end`] genuinely needs a sign, and it makes its own.
fn parse_duration(
    s: &str,
    path: &Path,
    row: usize,
    column: &'static str,
) -> Result<Duration, CsvReadError> {
    let bad = || bad_value(s, path, row, column, "expected an elapsed time as H:MM:SS");
    let mut parts = s.split(':');
    let (Some(h), Some(m), Some(sec), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad());
    };
    let h: u64 = h.trim().parse().map_err(|_| bad())?;
    let m: u64 = m.trim().parse().map_err(|_| bad())?;
    let sec: u64 = sec.trim().parse().map_err(|_| bad())?;
    if !(0..60).contains(&m) || !(0..60).contains(&sec) {
        return Err(bad());
    }
    Ok(Duration::from_secs(h * 3600 + m * 60 + sec))
}

/// The parsed fields of one CSV record that participate in the time calculations. Named apart
/// from [`Session`], which is the finished, UTC-resolved article this module hands to the peak
/// power contribution logic.
struct CsvSession {
    id: String,
    start_local: civil::DateTime,
    end_local: civil::DateTime,
    conn_duration: Duration,
    active_charge_time: Duration,
    /// Kept for its parse: a non-numeric `Energy_Use` invalidates the row, and that is caught in
    /// [`CsvSession::parse`]. The value itself is consumed on the reading side.
    #[allow(dead_code)]
    energy_use: f64,
}

/// One output row. A session normally yields one; an unresolvable DST fold yields two.
///
/// Carries a whole [`Session`] rather than loose timestamps, so that every derived column is
/// computed by the same methods the estimating logic uses. When they were separate fields the
/// write path had its own `adj_conn_end`, and it was wrong.
///
/// The pass-through CSV columns are not part of a `Session` and never should be, so the row keeps
/// an index back into the records instead — see [`SessionRows::field`].
pub(super) struct Row {
    /// Index into [`SessionRows::records`], for the pass-through columns.
    record: usize,
    pub session: RSession,
    /// The two reported wall times, kept as written. `Session` holds instants, and the local
    /// columns must show what the report said rather than a re-derivation of it — those differ in
    /// the DST gap, where the reported wall time never occurred.
    pub start_local: civil::DateTime,
    pub end_local: civil::DateTime,
}

impl Row {
    pub fn adj_start_local(&self) -> civil::DateTime {
        local_datetime(self.session.adj_conn_start())
    }

    pub fn adj_end_local(&self) -> civil::DateTime {
        local_datetime(self.session.adj_conn_end())
    }
}

impl CsvSession {
    fn parse(table: &Table, index: usize, row: usize) -> Result<Self, CsvReadError> {
        let path = table.path();
        let energy_raw = table.cell(index, "Energy_Use");
        Ok(Self {
            id: table.cell(index, "Charge_Session_ID").to_owned(),
            start_local: parse_local(
                table.cell(index, "Conn_DateTime_Start"),
                path,
                row,
                "Conn_DateTime_Start",
            )?,
            end_local: parse_local(
                table.cell(index, "Conn_DateTime_End"),
                path,
                row,
                "Conn_DateTime_End",
            )?,
            conn_duration: parse_duration(
                table.cell(index, "Conn_Duration"),
                path,
                row,
                "Conn_Duration",
            )?,
            active_charge_time: parse_duration(
                table.cell(index, "Active_Charge_Time"),
                path,
                row,
                "Active_Charge_Time",
            )?,
            energy_use: energy_raw.parse().map_err(|e: std::num::ParseFloatError| {
                bad_value(energy_raw, path, row, "Energy_Use", e)
            })?,
        })
    }

    /// Resolves this session's reported wall times to UTC instants.
    ///
    /// Returns one row normally, or two when both reported times fall in the DST fold and the
    /// record cannot say which of the repeated hours it belongs to — see docs/time/README.md,
    /// "Time zone", for why duplication is the policy and why the copies get distinct ids.
    ///
    /// # Not the same problem as [`crate::time::map_local`]
    ///
    /// Both resolve an ambiguous local time, and the two must **not** be merged. They share the
    /// probe that enumerates the readings, [`crate::time::local_readings`], and differ in what they
    /// do with more than one of them:
    ///
    /// - `map_local` is asked *"what could this wall time mean?"* by a user picking an interval of
    ///   interest. It has nothing but the wall time, so it returns every reading and makes the
    ///   caller choose, or say `EST`/`EDT`.
    /// - This is asked *"which reading was this session actually at?"* and has evidence the other
    ///   lacks: `Conn_Duration`, untruncated elapsed time. Every start-end combination is tested
    ///   with [`duration_is_consistent`], and duplication is the fallback for when more than one
    ///   survives.
    ///
    /// Giving this one `map_local`'s behaviour would throw away the duration evidence; giving
    /// `map_local` this one's would have it invent evidence it does not have.
    fn resolve(&self, source: &Rc<PathBuf>, row: usize) -> Vec<Row> {
        // Kinds known before the DST branch runs. They describe the record itself, so on
        // duplication both copies inherit them.
        let mut common = Vec::new();

        // avg_kw is a division by Active_Charge_Time. The sheet shows it as #DIV/0!; it is
        // reported here so it is not left to be noticed by eye. Zero energy is no exception: 0/0
        // is just as undefined, and the session becomes a spike either way.
        if self.active_charge_time.is_zero() {
            common.push(AnomalyKind::ZeroActiveChargeTime);
        } else {
            // The bound is the breaker rating at the top of the normal voltage band, not the
            // rating itself: a draw inside that band is the installation working. Above it, the
            // record says something is wrong with `Energy_Use` or `Active_Charge_Time` — but not
            // which, which is why this only reports and never excludes.
            let avg_kw = self.energy_use / (self.active_charge_time.as_secs_f64() / 3600.0);
            if avg_kw > BREAKER_MAX_NORMAL_KW {
                common.push(AnomalyKind::ExcessiveAvgKw);
            }
        }

        // The gap is settled before anything else, and settles the record on its own. A wall time
        // the clocks jumped over never occurred, so there is no instant to assign and nothing that
        // reads one may run: both `duration_is_consistent` and `is_on_grid` would be reporting the
        // sentinels rather than the record.
        if falls_in_gap(self.start_local) || falls_in_gap(self.end_local) {
            common.push(AnomalyKind::FellInDstGap);
            return vec![self.row(
                source,
                row,
                UNPLACEABLE_START,
                UNPLACEABLE_END,
                None,
                common,
            )];
        }

        // One reading at each end, or two where the hour repeats. Every combination of the two is
        // tested, so a start and an end read at different offsets is rejected by the same test that
        // accepts a matched pair: it implies a duration a whole hour out.
        let starts = local_readings(self.start_local);
        let ends = local_readings(self.end_local);
        let ambiguous = starts.len() > 1 || ends.len() > 1;
        let fits: Vec<(&'static str, Timestamp, Timestamp)> = starts
            .iter()
            .flat_map(|&(name, start)| ends.iter().map(move |&(_, end)| (name, start, end)))
            .filter(|&(_, start, end)| duration_is_consistent(start, end, self.conn_duration))
            .collect();

        match fits.as_slice() {
            // The record says which reading it was at.
            [(_, start, end)] => vec![self.row(source, row, *start, *end, None, common)],

            // Two survivors are reachable only from the fold at both ends, where the hour cancels
            // on each side and the two matched combinations pass or fail together. The record
            // cannot say which, so both are kept and told apart by the id.
            [_, _, ..] => {
                common.push(AnomalyKind::DstAmbiguousDuplicated);
                fits.iter()
                    .map(|&(name, start, end)| {
                        self.row(source, row, start, end, Some(name), common.clone())
                    })
                    .collect()
            }

            // Nothing fits. Where the reading was ambiguous, no instant can be assigned at all and
            // the record gets the sentinels, exactly as a gap does; `DstUnresolvable` says the
            // ambiguity is why. Where it was not, the single reading stands and only the
            // consistency of the record is in question.
            [] if ambiguous => {
                common.push(AnomalyKind::DstUnresolvable);
                common.push(AnomalyKind::InconsistentDuration);
                vec![self.row(
                    source,
                    row,
                    UNPLACEABLE_START,
                    UNPLACEABLE_END,
                    None,
                    common,
                )]
            }
            [] => {
                common.push(AnomalyKind::InconsistentDuration);
                vec![self.row(source, row, starts[0].1, ends[0].1, None, common)]
            }
        }
    }

    /// Builds one output row from a pair of resolved instants.
    ///
    /// `designator` is `Some` only for the two halves of a duplicated fold, which share a CSV row
    /// and are told apart by the `-EDT`/`-EST` suffix it puts on the id.
    ///
    /// [`AnomalyKind::OffGridTimes`] is decided here because it is the last kind that reads the
    /// instants, and a record given the sentinels has none to read.
    fn row(
        &self,
        source: &Rc<PathBuf>,
        row: usize,
        conn_start: Timestamp,
        conn_end: Timestamp,
        designator: Option<&str>,
        mut anomalies: Vec<AnomalyKind>,
    ) -> Row {
        // Checked on the resolved instants rather than the reported wall times: the two differ only
        // by a whole-hour offset in this zone, so either answers the question, and these are the
        // values every later allowance is applied to. Skipped for a record holding the sentinels,
        // which sit off the grid for reasons that say nothing about Evolute's reporting.
        let placeable = conn_start <= conn_end;
        if placeable
            && (!is_on_grid(conn_start, TIME_GRID_STEP) || !is_on_grid(conn_end, TIME_GRID_STEP))
        {
            anomalies.push(AnomalyKind::OffGridTimes);
        }

        Row {
            record: row - 2,
            session: Rc::new(Session {
                path: source.clone(),
                // The CSV row this record occupies. Both halves of a duplicated fold carry it,
                // since both were read from that one row.
                row,
                id: match designator {
                    Some(s) => format!("{}-{s}", self.id),
                    None => self.id.clone(),
                },
                conn_start,
                conn_end,
                conn_duration: self.conn_duration,
                charge_time: self.active_charge_time,
                energy_use: self.energy_use,
                anomalies,
            }),
            start_local: self.start_local,
            end_local: self.end_local,
        }
    }
}

#[cfg(test)]
// cargo test --lib -- session::csv::test --nocapture
mod test {
    use super::*;
    use crate::{session::test_support::timing_anomalies, time::serial_of_civil};
    use jiff::{SignedDuration, tz::TimeZone};
    use std::{env, fs, path::PathBuf, process};

    /// Both forms the reader itself accepts, in the same order — see `parse_local`. A helper
    /// that took only whole minutes could not express a report that has moved to seconds, which is
    /// the case [`AnomalyKind::OffGridTimes`] exists for.
    fn dt(s: &str) -> civil::DateTime {
        civil::DateTime::strptime("%Y-%m-%d %H:%M:%S", s)
            .or_else(|_| civil::DateTime::strptime("%Y-%m-%d %H:%M", s))
            .unwrap()
    }

    /// A stand-in source file for tests that call [`CsvSession::resolve`] or the cell parsers
    /// directly. Nothing here reads it; it is only what the sessions record as where they came
    /// from, and what an error raised by a parser would name.
    fn test_source() -> Rc<PathBuf> {
        Rc::new(PathBuf::from("Session_Report_Test.csv"))
    }

    fn session(start: &str, end: &str, conn: &str) -> CsvSession {
        let src = test_source();
        let active_charge_time = parse_duration(conn, &src, 1, "Active_Charge_Time").unwrap();
        CsvSession {
            id: "S1".to_owned(),
            start_local: dt(start),
            end_local: dt(end),
            conn_duration: parse_duration(conn, &src, 1, "Conn_Duration").unwrap(),
            active_charge_time,
            // 6 kW, under the breaker rating, so a record built here carries only the anomaly the
            // test that built it is about. A flat energy figure would draw far above the rating on
            // the shorter durations and pick up `ExcessiveAvgKw` throughout.
            energy_use: 6.0 * active_charge_time.as_secs_f64() / 3600.0,
        }
    }

    fn local_of(ts: Timestamp) -> civil::DateTime {
        local_datetime(ts)
    }

    /// A scratch directory of its own per test, since these run in parallel within one process.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("ev_peak_csv_{}_{tag}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn durations_parse_including_over_24_hours() {
        let src = test_source();
        assert_eq!(
            parse_duration("5:07:53", &src, 1, "d").unwrap(),
            Duration::from_secs(5 * 3600 + 7 * 60 + 53)
        );
        assert_eq!(
            parse_duration("30:00:00", &src, 1, "d").unwrap(),
            Duration::from_secs(30 * 3600)
        );
        assert!(parse_duration("5:70:00", &src, 1, "d").is_err());
        assert!(parse_duration("5:07", &src, 1, "d").is_err());

        // A rejected cell names the file, the row, the column and the value it could not read.
        let err = parse_duration("5:07", &src, 42, "Conn_Duration")
            .unwrap_err()
            .to_string();
        for expected in ["Session_Report_Test.csv", "row 42", "Conn_Duration", "5:07"] {
            assert!(err.contains(expected), "{expected:?} missing from {err:?}");
        }
    }

    /// `adj_conn_end` is the reported end padded past the end of its minute — the exclusive end of
    /// the window the true end lies in, so `21:29` pads to `21:30:00` and not `21:29:59`. Both rows
    /// are real sample rows, and they straddle the case the old `min(...)` rule treated specially:
    /// the second has `start + duration` (23:40:29) *before* the reported end.
    #[test]
    fn adj_conn_end_pads_the_reported_end() {
        let rows =
            session("2026-06-01 16:22", "2026-06-01 21:29", "5:07:53").resolve(&test_source(), 2);
        assert_eq!(
            local_of(rows[0].session.adj_conn_end()),
            civil::date(2026, 6, 1).at(21, 30, 0, 0)
        );
        assert!(timing_anomalies(&rows[0].session.anomalies).is_empty());

        let rows =
            session("2026-06-07 16:42", "2026-06-07 23:41", "6:58:29").resolve(&test_source(), 2);
        assert_eq!(
            local_of(rows[0].session.adj_conn_end()),
            civil::date(2026, 6, 7).at(23, 42, 0, 0)
        );
        assert!(timing_anomalies(&rows[0].session.anomalies).is_empty());
    }

    /// Both invariants the rule exists to guarantee, on the whole-minute durations that are the
    /// awkward case: the adjusted end never precedes the reported end, and the adjusted duration
    /// is never shorter than the reported one.
    #[test]
    fn adjustment_invariants_hold_on_whole_minute_durations() {
        let cases = [
            ("2026-06-06 14:59", "2026-06-06 16:36", "1:37:00"),
            ("2026-06-07 05:46", "2026-06-07 06:19", "0:33:00"),
            ("2026-06-15 01:45", "2026-06-15 01:55", "0:10:00"),
        ];
        for (start, end, conn) in cases {
            let s = session(start, end, conn);
            let rows = s.resolve(&test_source(), 2);
            let row = &rows[0];
            assert!(
                row.session.adj_conn_end() >= row.session.conn_end,
                "{start}: adjusted end precedes reported end"
            );
            assert!(
                row.session
                    .adj_conn_end()
                    .duration_since(row.session.conn_start)
                    .unsigned_abs()
                    >= s.conn_duration,
                "{start}: adjusted duration shorter than Conn_Duration"
            );
            assert!(
                timing_anomalies(&row.session.anomalies).is_empty(),
                "{start}: unexpected {:?}",
                row.session.anomalies
            );
        }
    }

    #[test]
    fn utc_conversion_uses_edt_in_june() {
        let rows =
            session("2026-06-01 16:22", "2026-06-01 21:29", "5:07:53").resolve(&test_source(), 2);
        assert_eq!(
            rows[0]
                .session
                .conn_start
                .to_zoned(TimeZone::UTC)
                .datetime(),
            civil::date(2026, 6, 1).at(20, 22, 0, 0)
        );
    }

    /// A long session starting inside the Nov 1 fold: the reported end rules out one offset.
    #[test]
    fn dst_fold_resolved_by_reported_end() {
        // 01:30 EDT + 3h elapsed = 03:30 EST. Starting at 01:30 EST would end at 04:30.
        let rows =
            session("2026-11-01 01:30", "2026-11-01 03:30", "3:00:00").resolve(&test_source(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0]
                .session
                .conn_start
                .to_zoned(TimeZone::UTC)
                .datetime(),
            civil::date(2026, 11, 1).at(5, 30, 0, 0), // EDT is UTC-4
        );
        assert!(timing_anomalies(&rows[0].session.anomalies).is_empty());
    }

    /// The mirror of the test above, and the case a one-sided `start + duration <= adj_conn_end`
    /// test would get wrong: here EST is correct, and the EDT candidate lands a full hour *early*.
    /// Only a two-sided comparison rejects it; accepting it would duplicate a session that is not
    /// ambiguous at all, double-counting its power.
    #[test]
    fn dst_fold_resolved_to_est_rejects_the_hour_early_candidate() {
        // 01:30 EST + 3h elapsed = 04:30 EST. Starting at 01:30 EDT would end at 03:30.
        let rows =
            session("2026-11-01 01:30", "2026-11-01 04:30", "3:00:00").resolve(&test_source(), 2);
        assert_eq!(
            rows.len(),
            1,
            "should not duplicate: {:?}",
            rows[0].session.id
        );
        assert_eq!(
            rows[0]
                .session
                .conn_start
                .to_zoned(TimeZone::UTC)
                .datetime(),
            civil::date(2026, 11, 1).at(6, 30, 0, 0), // EST is UTC-5
        );
        assert!(timing_anomalies(&rows[0].session.anomalies).is_empty());
    }

    /// A reported boundary that does not land on a whole minute is flagged on the record itself,
    /// since it is a fact about the times the report states.
    ///
    /// Not a fault: every figure is computed the same way either way. It says the report's
    /// resolution has outgrown this software's time grid, which makes the truncation allowances
    /// wider than the data needs.
    #[test]
    fn a_boundary_off_the_time_grid_is_flagged() {
        // Ordinary minute boundaries, so nothing is flagged.
        let rows =
            session("2026-06-10 02:00", "2026-06-10 03:00", "1:00:00").resolve(&test_source(), 2);
        assert!(
            !rows[0]
                .session
                .anomalies
                .contains(&AnomalyKind::OffGridTimes),
            "{:?}",
            rows[0].session.anomalies
        );

        // A start carrying seconds: the report has moved to a finer resolution than the grid.
        let rows = session("2026-06-10 02:00:30", "2026-06-10 03:00", "0:59:30")
            .resolve(&test_source(), 2);
        assert!(
            rows[0]
                .session
                .anomalies
                .contains(&AnomalyKind::OffGridTimes),
            "{:?}",
            rows[0].session.anomalies
        );
        // And it is only that: the record is otherwise sound and still counts.
        assert!(
            !rows[0]
                .session
                .anomalies
                .contains(&AnomalyKind::InconsistentDuration),
            "{:?}",
            rows[0].session.anomalies
        );
    }

    /// Reported times are truncated to the minute while `Conn_Duration` carries seconds, so a
    /// consistent record's `start + duration` lands up to a minute *either side* of the reported
    /// end. Requiring equal minutes rejected roughly half of all real records; on a fold start that
    /// meant a spurious `DstUnresolvable` and UTC timestamps an hour early.
    #[test]
    fn fold_resolves_when_start_plus_duration_falls_short_of_the_reported_minute() {
        // 01:30 EDT + 2:59:31 = 03:29:31 local, which truncates to 03:29, not the reported 03:30.
        // The EST candidate lands at 04:29:31, an hour out, so only EDT is consistent — but the old
        // equal-minutes test rejected *both* and called the record unresolvable.
        let rows =
            session("2026-11-01 01:30", "2026-11-01 03:30", "2:59:31").resolve(&test_source(), 2);
        assert_eq!(rows.len(), 1);
        assert!(
            !rows[0]
                .session
                .anomalies
                .contains(&AnomalyKind::DstUnresolvable),
            "spurious DstUnresolvable: {:?}",
            rows[0].session.anomalies
        );
        assert_eq!(
            rows[0]
                .session
                .conn_start
                .to_zoned(TimeZone::UTC)
                .datetime(),
            civil::date(2026, 11, 1).at(5, 30, 0, 0), // EDT is UTC-4
        );
    }

    /// A short session wholly inside the repeated hour: neither offset can be ruled out, so the
    /// record is duplicated with distinct ids.
    #[test]
    fn dst_fold_ambiguous_duplicates_the_record() {
        let rows =
            session("2026-11-01 01:10", "2026-11-01 01:40", "0:30:00").resolve(&test_source(), 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session.id, "S1-EDT");
        assert_eq!(rows[1].session.id, "S1-EST");
        // The copies are an hour apart in real time, which is the whole point.
        assert_eq!(
            rows[1]
                .session
                .conn_start
                .duration_since(rows[0].session.conn_start),
            SignedDuration::from_hours(1)
        );
        // Both copies carry the flag, so each output row says why it is there.
        for row in &rows {
            assert_eq!(
                timing_anomalies(&row.session.anomalies),
                vec![AnomalyKind::DstAmbiguousDuplicated]
            );
        }
    }

    /// A start that never occurred, on the March 8 spring-forward. No instant is assigned: the
    /// record gets the sentinels, and shifting it to either side of the gap would be a guess.
    #[test]
    fn a_start_in_the_dst_gap_gets_no_instant() {
        let rows =
            session("2026-03-08 02:30", "2026-03-08 04:00", "0:30:00").resolve(&test_source(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session.conn_start, UNPLACEABLE_START);
        assert_eq!(rows[0].session.conn_end, UNPLACEABLE_END);
        assert!(!rows[0].session.is_placeable());
        assert_eq!(
            timing_anomalies(&rows[0].session.anomalies),
            vec![AnomalyKind::FellInDstGap]
        );
    }

    /// The same fault on the *end* column poisons the whole span, not just that end. A half-real
    /// span would invite a reader to take the good half for a reading.
    #[test]
    fn an_end_in_the_dst_gap_gets_no_instant() {
        let rows =
            session("2026-03-08 01:30", "2026-03-08 02:30", "1:00:00").resolve(&test_source(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session.conn_start, UNPLACEABLE_START);
        assert_eq!(rows[0].session.conn_end, UNPLACEABLE_END);
        assert_eq!(
            timing_anomalies(&rows[0].session.anomalies),
            vec![AnomalyKind::FellInDstGap]
        );
    }

    /// One flag for a session whose reported start and end both fall in the gap, not two.
    #[test]
    fn a_gap_at_both_ends_is_flagged_once() {
        let rows =
            session("2026-03-08 02:10", "2026-03-08 02:40", "0:30:00").resolve(&test_source(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            timing_anomalies(&rows[0].session.anomalies),
            vec![AnomalyKind::FellInDstGap]
        );
    }

    /// A gap settles the record on its own. Nothing that reads the instants runs, so neither the
    /// consistency test nor the grid test may add a kind of its own — both would be reporting the
    /// sentinels rather than the record. `0:30:00` here contradicts the reported hour between
    /// 02:10 and 02:40 read either side of the gap, and the seconds put both times off the grid.
    #[test]
    fn a_gap_suppresses_every_test_that_reads_the_instants() {
        let rows = session("2026-03-08 02:10:17", "2026-03-08 02:40:44", "9:00:00")
            .resolve(&test_source(), 2);
        assert_eq!(
            rows[0].session.anomalies,
            vec![AnomalyKind::FellInDstGap],
            "only the gap should be reported"
        );
    }

    /// The reported wall times survive on the row even though no instant does. They are what the
    /// workbook's local columns show, and re-deriving them from the sentinels would give nonsense.
    #[test]
    fn a_gap_row_keeps_the_wall_times_the_report_stated() {
        let rows =
            session("2026-03-08 02:30", "2026-03-08 04:00", "0:30:00").resolve(&test_source(), 2);
        assert_eq!(rows[0].start_local, dt("2026-03-08 02:30"));
        assert_eq!(rows[0].end_local, dt("2026-03-08 04:00"));
    }

    /// The case local arithmetic gets wrong: a session spanning the fold. Wall clock says 2 hours,
    /// elapsed is 3.
    #[test]
    fn fold_spanning_session_has_true_elapsed_duration() {
        let rows =
            session("2026-11-01 00:30", "2026-11-01 02:30", "3:00:00").resolve(&test_source(), 2);
        let row = &rows[0];
        let elapsed = row
            .session
            .adj_conn_end()
            .duration_since(row.session.conn_start);
        assert!(
            elapsed >= SignedDuration::from_hours(3),
            "elapsed {elapsed:?} lost the repeated hour"
        );
        // The same subtraction done on local wall times loses the repeated hour.
        let wall_secs = serial_of_civil(row.adj_end_local()) - serial_of_civil(row.start_local);
        assert!(
            wall_secs * 86_400.0 < elapsed.as_secs() as f64,
            "local subtraction should undercount here"
        );
    }

    /// Zero `Active_Charge_Time` is flagged whatever the energy: the `avg_kw` cell shows `#DIV/0!`
    /// either way, and the session becomes a spike either way.
    #[test]
    fn zero_active_charge_time_is_reported() {
        for energy in [5.0, 0.0] {
            let mut s = session("2026-06-01 10:00", "2026-06-01 10:00", "0:00:00");
            s.energy_use = energy;
            let rows = s.resolve(&test_source(), 7);
            assert_eq!(
                timing_anomalies(&rows[0].session.anomalies),
                vec![AnomalyKind::ZeroActiveChargeTime],
                "energy {energy}"
            );
        }
    }

    /// The three checks of [`duration_is_consistent`], each pinned at the boundary it draws.
    ///
    /// Both bounds are exclusive and both are pinned to the second, because getting either off by
    /// one silently reclassifies real records — the sample data reaches to within 3 seconds of the
    /// early edge. With the reported times at 10:00 and 10:30 the sound durations are exactly
    /// `[0:29:01, 0:31:00]`.
    ///
    /// The window is asymmetric: one second wider late than early. That second is not slack, it is
    /// the reporting's uncertainty about whether the last second of the end minute is included.
    /// See `docs/session/time-reporting-uncertainty.md`.
    #[test]
    fn inconsistent_duration_is_reported() {
        let kinds = |start, end, conn| {
            let all = session(start, end, conn)
                .resolve(&test_source(), 2)
                .swap_remove(0)
                .session
                .anomalies
                .clone();
            timing_anomalies(&all)
        };
        let bad = vec![AnomalyKind::InconsistentDuration];

        // Overshoot: 10:00 + 2h = 12:00, well past the 10:31:01 upper bound.
        assert_eq!(
            kinds("2026-06-01 10:00", "2026-06-01 10:30", "2:00:00"),
            bad
        );

        // Check 1, doing work no other check does. A one-minute inversion with a zero duration
        // satisfies both of the others -- 10:01 + 0 = 10:01 is under the 10:01:01 upper bound and
        // over the 09:59:00 lower one -- so nothing but the start-before-end test rejects it. It
        // is also the smallest inversion the reporting can express, since both reported times are
        // whole minutes; that in turn forces the duration to zero, hence the extra anomaly here.
        // Letting this row through panics `Session::intersects` downstream.
        assert_eq!(
            kinds("2026-06-01 10:01", "2026-06-01 10:00", "0:00:00"),
            vec![
                AnomalyKind::ZeroActiveChargeTime,
                AnomalyKind::InconsistentDuration
            ]
        );
        // The same fault at a scale the overshoot check would also have caught.
        assert_eq!(
            kinds("2026-06-01 10:00", "2026-06-01 09:00", "0:10:00"),
            bad
        );

        // One second outside each bound. 10:31:01 is the first instant check 2 rejects, 10:29:00
        // the last one check 3 does.
        assert_eq!(
            kinds("2026-06-01 10:00", "2026-06-01 10:30", "0:31:01"),
            bad
        );
        assert_eq!(
            kinds("2026-06-01 10:00", "2026-06-01 10:30", "0:29:00"),
            bad
        );

        // Exactly on each bound, and both sound. `0:31:00` is the case the old predicate rejected
        // and the document accepts.
        assert!(kinds("2026-06-01 10:00", "2026-06-01 10:30", "0:31:00").is_empty());
        assert!(kinds("2026-06-01 10:00", "2026-06-01 10:30", "0:29:01").is_empty());
    }

    /// The sessions reach the peak power contribution logic straight from the CSV, bucketed the
    /// same way `excel::historic::xlsx_to_sessions` buckets them out of a workbook — one row per
    /// bucket here, so all three rules are exercised.
    #[test]
    fn csv_sessions_buckets_straight_from_the_csv() {
        const CSV: &str = "\
Charge_Session_ID,Conn_DateTime_Start,Conn_DateTime_End,Conn_Duration,Active_Charge_Time,Energy_Use
S1,2026-06-01 16:22,2026-06-01 21:29,5:07:53,5:07:52,30.6
S2,2026-06-02 10:00,2026-06-02 09:00,0:10:00,0:09:00,1.5
S3,2026-06-03 09:00,2026-06-03 09:00,0:00:00,0:00:00,4.2
";
        let dir = temp_dir("csv_sessions");
        let csv_path = dir.join("Session_Report_Test.csv");
        fs::write(&csv_path, CSV).unwrap();

        let report = csv_sessions(&csv_path).unwrap();

        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].id, "S1");
        assert_eq!(report.excluded.len(), 1);
        assert_eq!(report.excluded[0].id, "S2");
        assert_eq!(report.spikes.len(), 1);
        assert_eq!(report.spikes[0].id, "S3");

        // Beside the CSV, under its own suffix, so it cannot collide with the workbook's logs. The
        // suffix leads with the kind of document, as the error messages do.
        assert_eq!(
            report.logs[0].path(),
            dir.join("Session_Report_Test.session.csv.read.log")
        );
        // The log's text, not a file: the reader no longer writes one. See `Sessions::logs`.
        let log = report.logs[0].render();
        assert!(
            log.contains("InconsistentDuration") || log.contains("contradict"),
            "{log}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    /// A missing required column invalidates the whole file rather than one row: the sessions it
    /// would describe cannot be trusted.
    /// The message opens with the kind of document expected, ahead of the path. Picking a Charges
    /// Report for this slot fails on a missing column, and so does picking a session report for
    /// the Charges Report slot; the path alone leaves the two indistinguishable. The paired test
    /// is `charges_report::test::a_message_opens_with_the_kind_of_document_expected`.
    ///
    /// Through `csv_sessions`, not over a hand-built error: what is at stake is the [`Document`]
    /// this reader hands the shared reader, and only the real call passes it.
    #[test]
    fn a_message_opens_with_the_kind_of_document_expected() {
        const CSV: &str = "\
Start_Date,End_Date,Bill_Status,kWh,Cost
01-Jun-26,30-Jun-26,Issued,10,$1.00
";
        let dir = temp_dir("document_kind");
        // A Charges Report, named as one, offered to this slot.
        let csv_path = dir.join("XX-XX_charges_2026-06-01.csv");
        fs::write(&csv_path, CSV).unwrap();

        let err = csv_sessions(&csv_path).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "Session Report {}: missing required column `Charge_Session_ID`",
                csv_path.display()
            )
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_required_column_is_rejected() {
        const CSV: &str = "\
Charge_Session_ID,Conn_DateTime_Start,Conn_DateTime_End,Conn_Duration,Active_Charge_Time
S1,2026-06-01 16:22,2026-06-01 21:29,5:07:53,5:07:52
";
        let dir = temp_dir("missing_column");
        let csv_path = dir.join("Session_Report_Test.csv");
        fs::write(&csv_path, CSV).unwrap();

        let err = csv_sessions(&csv_path).unwrap_err().to_string();
        assert!(err.contains("Energy_Use"), "{err}");

        fs::remove_dir_all(&dir).ok();
    }
}
