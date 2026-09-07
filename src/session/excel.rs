//! The `.xlsx` rendering of a session report.
//!
//! Writing only, and it does not parse a CSV: [`session_csv_to_xlsx`] takes the rows
//! [`super::csv`] has already resolved and lays them out as cells, formats, formulas and comments.
//! The API reaches sessions from the CSV instead, through
//! [`csv_sessions`](super::csv::csv_sessions).
//!
//! The `anomalies` column records the judgement calls made when the CSV was parsed — the ones that
//! remove a session from the estimates — so that a reader of the sheet can see why a row was left
//! out. Nothing reads the column back. See [`AnomalyKind`].

use super::{
    Anomaly, AnomalyKind,
    csv::{SessionRows, csv_session_rows},
};
use crate::{
    error::ConversionError,
    log::SourceLog,
    time::{serial_of_civil, serial_of_duration, serial_of_instant},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
use umya_spreadsheet::{Comment, HorizontalAlignmentValues, Workbook, Worksheet};

const DATETIME_FORMAT: &str = "yyyy-mm-dd hh:mm:ss ddd";
/// Elapsed-time format: unlike `hh:mm:ss` it does not wrap a 25-hour duration to `01:00:00`.
const DURATION_FORMAT: &str = "[h]:mm:ss";
const ENERGY_USE_FORMAT: &str = "0.000";
const AVG_KW_FORMAT: &str = "0.000";
const TOTAL_FEE_FORMAT: &str = "0.00";

/// Outcome of converting one CSV file. The reading direction returns a
/// [`Sessions`](super::Sessions) instead.
#[derive(Debug)]
pub struct SessionWriteReport {
    /// Where the workbook was written.
    pub output_path: PathBuf,
    /// Rows that needed a judgement call. Empty for a clean conversion.
    pub anomalies: Vec<Anomaly>,
    /// The run log, which says either that nothing was found or what was.
    ///
    /// Held rather than written, for the reason [`Sessions::logs`](super::Sessions::logs) gives.
    /// Write it with [`SourceLog::write`].
    pub log: SourceLog,
}

/// How an output column is populated.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Copied verbatim from the named CSV column.
    Text(&'static str),
    /// Parsed from the named CSV column and written as a number.
    Number(&'static str),
    /// Parsed from the named CSV column and written as an Excel duration.
    Duration(&'static str),
    /// The session id, verbatim from `Charge_Session_ID`.
    SessionId,
    ConnStartLocal,
    ConnStartUtc,
    ConnEndLocal,
    ConnEndUtc,
    /// Formula: `conn_end_utc - conn_start_utc`.
    ConnSpan,
    /// Formula: `Energy_Use / Active_Charge_Time`, in kW.
    AvgKw,
    /// Comma-separated [`AnomalyKind`] tokens for this row; empty when the row is clean.
    Anomalies,
}

/// The output sheet's columns, in order. Drives both the header row and every data row,
/// so layout changes need only happen here.
const COLUMNS: &[(&str, Source)] = &[
    ("UR_ID", Source::Text("UR_ID")),
    ("Location_Address", Source::Text("Location_Address")),
    ("Location_City", Source::Text("Location_City")),
    ("Location_Postal_Code", Source::Text("Location_Postal_Code")),
    ("Station_ID", Source::Text("Station_ID")),
    (
        "Station_Network_Provider",
        Source::Text("Station_Network_Provider"),
    ),
    ("Station_Make", Source::Text("Station_Make")),
    ("Station_Model", Source::Text("Station_Model")),
    ("Charge_Session_ID", Source::SessionId),
    ("User_ID", Source::Text("User_ID")),
    ("Conn_DateTime_Start", Source::ConnStartLocal),
    ("Conn_DateTime_End", Source::ConnEndLocal),
    ("Conn_Duration", Source::Duration("Conn_Duration")),
    ("Charge_Duration", Source::Duration("Charge_Duration")),
    ("Active_Charge_Time", Source::Duration("Active_Charge_Time")),
    ("Charging_Level", Source::Text("Charging_Level")),
    ("Energy_Use", Source::Number("Energy_Use")),
    ("Total_Fee", Source::Number("Total_Fee")),
    ("Vehicle_Make", Source::Text("Vehicle_Make")),
    ("Vehicle_Model", Source::Text("Vehicle_Model")),
    ("Vehicle_Year", Source::Number("Vehicle_Year")),
    // Everything from here on is this software's, not Evolute's. Grouped at the right end so the
    // left of the sheet is the session report as received and a reader can tell at a glance which
    // is which.
    ("conn_start_utc", Source::ConnStartUtc),
    ("conn_end_utc", Source::ConnEndUtc),
    ("conn_span", Source::ConnSpan),
    ("avg_kw", Source::AvgKw),
    ("anomalies", Source::Anomalies),
];

/// Reads the CSV file at `path`, which should have the same format as one on this project's `data`
/// directory, and transforms it into a `.xlsx` file saved to the same directory as the input file,
/// with the extension replaced.
///
/// The parse is `csv::csv_session_rows`; nothing about the session report is interpreted here. The
/// domain rules — the UTC conversion, the definition of `conn_span`, and the treatment of
/// zero-`Energy_Use` sessions — are specified in
/// `docs/time/README.md` under "Time zone" and in `docs/session/README.md` under "Excel workbook"
/// and "Anomalies". They are shared with the peak power contribution logic and are not restated
/// here.
///
/// What this function adds on top of those rules:
///
/// - Column order is given by the private `COLUMNS` table: the session report's own columns first,
///   in the order it states them, then everything this software derives. See
///   `docs/session/README.md`, "Excel workbook".
/// - Timestamp columns are Excel date/time numbers formatted `yyyy-mm-dd hh:mm:ss ddd`, left-
///   justified; duration columns are Excel durations formatted `[h]:mm:ss`, which does not wrap
///   past 24 hours, and are centered.
/// - `conn_span` and `avg_kw` are live formulas. `conn_span` subtracts the two
///   *UTC* columns rather than the local ones, so nothing about the zone can enter it; `avg_kw` is
///   `=Energy_Use/(Active_Charge_Time*24)`, in kW, displayed to 3 decimal
///   places, matching `Energy_Use`. The formula is written on every row, so a session with
///   zero `Active_Charge_Time` shows `#DIV/0!` rather than an empty cell:
///   it delivered energy in no time at all, and the sheet says so. `Total_Fee` is displayed to
///   2 decimal places.
/// - The last column, `anomalies`, carries the [`AnomalyKind`]s found for the row as a
///   comma-separated list of variant names. It is a record for whoever opens the sheet, saying
///   why a row was excluded from the estimates; nothing reads it back.
/// - The remaining columns are copied over with an explicit per-column type, so values that merely
///   look numeric — postal codes, station ids — keep their text form.
/// - The sheet is named by the private `sheet_name`.
///
/// Every session in the report is written to the workbook, anomalous ones included: the sheet is a
/// faithful rendering of the session report, and which sessions take part in an estimate is decided
/// on the reading side. A caller that wants the sessions and not the sheet can skip the workbook
/// entirely and call the private `csv::csv_sessions`, which runs the same parse.
///
/// # Errors
///
/// Returns `Err` only for conditions that invalidate the whole file: it cannot be read, a required
/// header is missing, a timestamp or duration does not parse, or the workbook cannot be written.
/// Per-row judgement calls do not abort the conversion; they are collected in
/// [`SessionWriteReport::anomalies`].
pub fn session_csv_to_xlsx(path: &Path) -> Result<SessionWriteReport, ConversionError> {
    // The two halves are kept apart. Reading the CSV yields a `SessionCsvError`, which names the
    // file from its own `path` field, so it goes into `Input` -- a variant that adds nothing and
    // would otherwise print the path twice. Only the writing half goes into `Write`, which does
    // add the path, because the workbook writers name no file of their own.
    let rows = csv_session_rows(path).map_err(|cause| ConversionError::Input {
        cause: Box::new(cause),
    })?;
    write_session_xlsx(path, rows).map_err(|cause| ConversionError::Write {
        path: path.with_extension("xlsx"),
        cause,
    })
}

fn write_session_xlsx(
    path: &Path,
    rows: SessionRows,
) -> Result<SessionWriteReport, Box<dyn Error>> {
    let output_path = path.with_extension("xlsx");
    let mut book = umya_spreadsheet::new_file();
    write_sheet(&mut book, &output_path, &rows)?;

    umya_spreadsheet::writer::xlsx::write(&book, &output_path)?;

    let log = SourceLog {
        // Beside the workbook rather than the CSV, because that is what this run produced.
        source: output_path.clone(),
        suffix: "session.convert",
        operation: "Converted Session Report",
        log: rows.log,
    };

    Ok(SessionWriteReport {
        output_path,
        anomalies: rows.anomalies,
        log,
    })
}

// ---------------------------------------------------------------------------
// Excel output
// ---------------------------------------------------------------------------

/// 1-based column index to its Excel letters (1 -> A, 27 -> AA).
///
/// The bridge between this module's index-based `COLUMNS` table and the three Excel APIs that
/// speak A1 notation instead: formula text, `column_dimension_mut`, and the auto-filter range.
///
/// `green_button::excel` has a copy, and the duplication is deliberate. Sharing it would mean a
/// crate-level module holding one function neither writer may own, and the two cannot drift: the
/// specification is Excel's bijective base-26, closed and external, and each copy is tested where
/// it sits. Should a second helper ever be worth sharing between the two writers, both move then.
fn column_letters(mut index: usize) -> String {
    let mut out = Vec::new();
    while index > 0 {
        let rem = (index - 1) % 26;
        out.push(b'A' + rem as u8);
        index = (index - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII")
}

fn column_index(source: Source) -> usize {
    COLUMNS
        .iter()
        .position(|(_, s)| *s == source)
        .expect("column present in COLUMNS")
        + 1
}

fn write_sheet(
    book: &mut Workbook,
    output_path: &Path,
    data: &SessionRows,
) -> Result<(), Box<dyn Error>> {
    let sheet = book.sheet_mut(0)?;
    sheet.set_name(sheet_name(output_path));

    for (i, (header, _)) in COLUMNS.iter().enumerate() {
        let col = i as u32 + 1;
        sheet.cell_mut((col, 1)).set_value_string(*header);
        sheet.style_mut((col, 1)).font_mut().set_bold(true);
    }

    let start_utc_col = column_letters(column_index(Source::ConnStartUtc));
    let end_utc_col = column_letters(column_index(Source::ConnEndUtc));
    let energy_col = column_letters(column_index(Source::Number("Energy_Use")));
    let active_col = column_letters(column_index(Source::Duration("Active_Charge_Time")));

    for (r, row) in data.rows.iter().enumerate() {
        let excel_row = r as u32 + 2;

        for (i, (_, source)) in COLUMNS.iter().enumerate() {
            let col = i as u32 + 1;
            match source {
                Source::Text(name) => {
                    let value = data.field(row, name);
                    if !value.is_empty() {
                        sheet.cell_mut((col, excel_row)).set_value_string(value);
                    }
                }
                Source::Number(name) => {
                    let value = data.field(row, name);
                    if !value.is_empty() {
                        match value.parse::<f64>() {
                            Ok(n) => {
                                sheet.cell_mut((col, excel_row)).set_value_number(n);
                                if let Some(code) = decimal_format(name) {
                                    set_format(sheet, col, excel_row, code);
                                }
                            }
                            // A non-numeric value in a numeric column is preserved rather than
                            // dropped; the workbook still shows what the report said.
                            Err(_) => {
                                sheet.cell_mut((col, excel_row)).set_value_string(value);
                            }
                        }
                    }
                }
                Source::Duration(name) => {
                    if let Some(d) = data.duration(row, name)? {
                        sheet
                            .cell_mut((col, excel_row))
                            .set_value_number(serial_of_duration(d));
                        set_duration_style(sheet, col, excel_row);
                    }
                }
                Source::SessionId => {
                    sheet
                        .cell_mut((col, excel_row))
                        .set_value_string(row.session.id.as_str());
                }
                Source::ConnStartLocal => {
                    write_datetime(sheet, col, excel_row, serial_of_civil(row.start_local));
                }
                Source::ConnEndLocal => {
                    write_datetime(sheet, col, excel_row, serial_of_civil(row.end_local));
                }
                Source::ConnStartUtc => {
                    write_datetime(
                        sheet,
                        col,
                        excel_row,
                        serial_of_instant(row.session.conn_start),
                    );
                }
                Source::ConnEndUtc => {
                    write_datetime(
                        sheet,
                        col,
                        excel_row,
                        serial_of_instant(row.session.conn_end),
                    );
                }
                Source::ConnSpan => {
                    // Subtracting the UTC columns, not the local ones, so no zone enters the
                    // arithmetic. The cell equals `Session::interval`'s width — the span the
                    // estimating logic places the session on, which is the point of showing it.
                    sheet.cell_mut((col, excel_row)).set_formula(format!(
                        "{end_utc_col}{excel_row}-{start_utc_col}{excel_row}"
                    ));
                    set_duration_style(sheet, col, excel_row);
                }
                Source::AvgKw => {
                    // Written unconditionally: with zero Active_Charge_Time
                    // this evaluates to #DIV/0!, which is the honest answer — energy delivered in
                    // no time at all has no finite average power.
                    sheet.cell_mut((col, excel_row)).set_formula(format!(
                        "{energy_col}{excel_row}/({active_col}{excel_row}*24)"
                    ));
                    set_format(sheet, col, excel_row, AVG_KW_FORMAT);
                }
                Source::Anomalies => {
                    if !row.session.anomalies.is_empty() {
                        let tokens: Vec<&str> = row
                            .session
                            .anomalies
                            .iter()
                            .map(AnomalyKind::as_str)
                            .collect();
                        sheet
                            .cell_mut((col, excel_row))
                            .set_value_string(tokens.join(","));
                    }
                }
            }
        }
    }

    add_comments(sheet);
    set_widths(sheet);
    let last_col = column_letters(COLUMNS.len());
    let last_row = data.rows.len() + 1;
    sheet.set_auto_filter(format!("A1:{last_col}{last_row}"));
    Ok(())
}

fn write_datetime(sheet: &mut Worksheet, col: u32, row: u32, serial: f64) {
    sheet.cell_mut((col, row)).set_value_number(serial);
    set_format(sheet, col, row, DATETIME_FORMAT);
    set_alignment(sheet, col, row, HorizontalAlignmentValues::Left);
}

fn set_duration_style(sheet: &mut Worksheet, col: u32, row: u32) {
    set_format(sheet, col, row, DURATION_FORMAT);
    set_alignment(sheet, col, row, HorizontalAlignmentValues::Center);
}

fn set_format(sheet: &mut Worksheet, col: u32, row: u32, code: &str) {
    sheet
        .style_mut((col, row))
        .number_format_mut()
        .set_format_code(code);
}

fn set_alignment(sheet: &mut Worksheet, col: u32, row: u32, horizontal: HorizontalAlignmentValues) {
    sheet
        .style_mut((col, row))
        .alignment_mut()
        .set_horizontal(horizontal);
}

/// Decimal precision for the `Source::Number` columns that need more than Excel's default display.
fn decimal_format(csv_column: &str) -> Option<&'static str> {
    match csv_column {
        "Energy_Use" => Some(ENERGY_USE_FORMAT),
        "Total_Fee" => Some(TOTAL_FEE_FORMAT),
        _ => None,
    }
}

/// Prefix carried by the session report exports. Stripped from the sheet name, which Excel caps
/// at 31 characters — long enough to lose the reporting period that follows it.
const SESSION_REPORT_PREFIX: &str = "Session_Report_";

/// The output file name, minus its `.xlsx` suffix and minus a leading [`SESSION_REPORT_PREFIX`],
/// so `Session_Report_June_1_2026-June_30_2026` names the sheet `June_1_2026-June_30_2026` rather
/// than being truncated to `Session_Report_June_1_2026-June`. Excel sheet names are capped at 31
/// characters and cannot contain `[]:*?/\`.
fn sheet_name(output_path: &Path) -> String {
    let stem = output_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Sessions".to_owned());
    // A name that is *only* the prefix keeps it: an empty sheet name is not a name.
    let stem = match stem.strip_prefix(SESSION_REPORT_PREFIX) {
        Some(rest) if !rest.is_empty() => rest.to_owned(),
        _ => stem,
    };
    let cleaned: String = stem
        .chars()
        .map(|c| if "[]:*?/\\".contains(c) { '_' } else { c })
        .collect();
    cleaned.chars().take(31).collect()
}

fn add_comments(sheet: &mut Worksheet) {
    let notes = [
        (
            Source::ConnSpan,
            "conn_end_utc - conn_start_utc: the span every estimate places this session on. \
             Computed from the UTC columns so that no time zone enters the arithmetic. It is the \
             reported start to the reported end, taken as written -- Evolute states both to the \
             second. The sheet used to carry a padded pair of columns beside these, because the \
             times were stated only to the minute and the true start and end were known only to \
             fall somewhere inside the minute named.",
        ),
        (
            Source::AvgKw,
            "Energy_Use / Active_Charge_Time, in kW. Active_Charge_Time is an Excel duration, i.e. \
             a fraction of a day, hence the *24 to convert it to hours. A zero Active_Charge_Time \
             yields #DIV/0!, which is the honest answer: there is no average power to state. Such \
             a row beside a non-zero Energy_Use is almost certainly a reporting fault -- the \
             session report's three duration fields track the same thing to within about a \
             second -- and is worth looking at individually.",
        ),
        (
            Source::Anomalies,
            "Comma-separated list of anomalies found for this row, named after the AnomalyKind \
             variants. Empty means the row needed no judgement call. This cell IS read back, and \
             InconsistentDuration is what removes a session from every estimate -- so editing it \
             changes the figures. The adjusted columns are not read back: they are recomputed, and \
             a disagreement is written to the .session.xlsx.read.log rather than obeyed.",
        ),
    ];
    for (source, text) in notes {
        let col = column_index(source) as u32;
        let mut comment = Comment::default();
        comment.new_comment((col, 1));
        comment.set_author("session_csv_to_xlsx");
        comment.set_text_string(text);
        sheet.add_comments(comment);
    }
}

/// The date/time format needs real width or Excel renders the cell as `####`.
///
/// Every column holding a date and time takes that width, whichever it is. The header's own length
/// decides nothing here: `Conn_DateTime_Start` is a long name and a value longer still, and sizing
/// to the name left the value hidden.
fn set_widths(sheet: &mut Worksheet) {
    for (i, (header, source)) in COLUMNS.iter().enumerate() {
        let letters = column_letters(i + 1);
        let width = match source {
            Source::ConnStartLocal
            | Source::ConnStartUtc
            | Source::ConnEndLocal
            | Source::ConnEndUtc => 24.0,
            Source::Duration(_) | Source::ConnSpan => 13.0,
            // Room for a couple of variant names side by side.
            Source::Anomalies => 40.0,
            _ => (header.len() as f64 + 2.0).max(10.0),
        };
        sheet.column_dimension_mut(&letters).set_width(width);
    }
}

#[cfg(test)]
// cargo test --lib -- session::excel::test --nocapture
mod test {
    use super::*;
    use crate::session::test_support::{timing_anomalies, timing_anomalies_in_cell};
    use std::{env, fs, process};

    /// A scratch directory of its own per test, since these run in parallel within one process.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("ev_peak_excel_{}_{tag}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sheet_name_strips_the_report_prefix() {
        let name = |s: &str| sheet_name(Path::new(s));
        assert_eq!(
            name("Session_Report_June_1_2026-June_30_2026.xlsx"),
            "June_1_2026-June_30_2026"
        );
        // No prefix: the stem is used as it stands.
        assert_eq!(name("July_data.xlsx"), "July_data");
        // Stripping would leave nothing, so the prefix stays.
        assert_eq!(name("Session_Report_.xlsx"), "Session_Report_");
        // Excel's 31-character cap still applies, and it applies after stripping.
        assert_eq!(
            name("Session_Report_a_very_long_reporting_period_name.xlsx"),
            "a_very_long_reporting_period_na"
        );
        assert_eq!(name("bad[name]:here.xlsx"), "bad_name__here");
    }

    #[test]
    fn column_letters_span_past_z() {
        assert_eq!(column_letters(1), "A");
        assert_eq!(column_letters(26), "Z");
        assert_eq!(column_letters(27), "AA");
        assert_eq!(column_letters(COLUMNS.len()), "Z");
    }

    const FIXTURE: &str = "\
UR_ID,Location_Address,Location_City,Location_Postal_Code,Station_ID,Station_Network_Provider,Station_Make,Station_Model,Charge_Session_ID,User_ID,Conn_DateTime_Start,Conn_DateTime_End,Conn_Duration,Charge_Duration,Active_Charge_Time,Charging_Level,Energy_Use,Total_Fee,Vehicle_Make,Vehicle_Model,Vehicle_Year
CKT-7,,Toronto,,Station-7,Evolute Inc.,FLO,G5,S69865,,2026-06-01 16:22:00,2026-06-01 21:29:53,5:07:53,5:07:53,5:07:52,Level 2,30.6,5.63,VinFast,Vf8,2024
CKT-7,,Toronto,,Station-7,Evolute Inc.,FLO,G5,S13577,,2026-06-02 08:00:00,2026-06-02 08:00:11,0:00:11,0:00:11,0:00:10,Level 2,0,0,VinFast,Vf8,2024
";

    #[test]
    fn round_trip_produces_the_expected_workbook() {
        let dir = temp_dir("round_trip");
        let csv_path = dir.join("Session_Report_Test.csv");
        fs::write(&csv_path, FIXTURE).unwrap();

        let report = session_csv_to_xlsx(&csv_path).unwrap();
        assert_eq!(report.output_path, dir.join("Session_Report_Test.xlsx"));
        assert!(
            timing_anomalies(&report.anomalies.iter().map(|a| a.kind).collect::<Vec<_>>())
                .is_empty(),
            "{:?}",
            report.anomalies
        );

        let book = umya_spreadsheet::reader::xlsx::read(&report.output_path).unwrap();
        let sheet = book.sheet(0).unwrap();

        // Header row, in the agreed order.
        let expected: Vec<&str> = COLUMNS.iter().map(|(h, _)| *h).collect();
        for (i, header) in expected.iter().enumerate() {
            assert_eq!(&sheet.value((i as u32 + 1, 1)), header);
        }

        let col = |s: Source| column_index(s) as u32;

        // conn_end_utc on the first row: 21:29:53 at the session offset is 02:29:53Z on the 2nd.
        let end_utc: f64 = sheet.value((col(Source::ConnEndUtc), 2)).parse().unwrap();
        let expected = crate::time::serial_of_instant(
            "2026-06-02T02:29:53Z"
                .parse()
                .expect("an RFC 3339 timestamp"),
        );
        assert!((end_utc - expected).abs() < 1e-9, "{end_utc} vs {expected}");

        // Formulas, not cached values. Both operands are the *adjusted* UTC columns, so the cell
        // equals `Session::interval`'s width rather than a span starting at the reported start.
        let expect_formula = format!(
            "{}2-{}2",
            column_letters(column_index(Source::ConnEndUtc)),
            column_letters(column_index(Source::ConnStartUtc))
        );
        assert_eq!(
            sheet.cell((col(Source::ConnSpan), 2)).unwrap().formula(),
            expect_formula
        );
        let avg_kw_formula = |r: u32| {
            format!(
                "{}{r}/({}{r}*24)",
                column_letters(column_index(Source::Number("Energy_Use"))),
                column_letters(column_index(Source::Duration("Active_Charge_Time")))
            )
        };
        assert_eq!(
            sheet.cell((col(Source::AvgKw), 2)).unwrap().formula(),
            avg_kw_formula(2)
        );

        // Sheet name is the output file's name, minus the .xlsx suffix and the report prefix.
        assert_eq!(sheet.name(), "Test");

        // Number formats.
        assert_eq!(
            sheet
                .style((col(Source::ConnStartLocal), 2))
                .number_format()
                .unwrap()
                .format_code(),
            DATETIME_FORMAT
        );
        assert_eq!(
            sheet
                .style((col(Source::Number("Energy_Use")), 2))
                .number_format()
                .unwrap()
                .format_code(),
            ENERGY_USE_FORMAT
        );
        assert_eq!(
            sheet
                .style((col(Source::AvgKw), 2))
                .number_format()
                .unwrap()
                .format_code(),
            AVG_KW_FORMAT
        );
        assert_eq!(
            sheet
                .style((col(Source::Number("Total_Fee")), 2))
                .number_format()
                .unwrap()
                .format_code(),
            TOTAL_FEE_FORMAT
        );

        // Date/time values are left-justified, duration values are centered.
        assert_eq!(
            *sheet
                .style((col(Source::ConnStartLocal), 2))
                .alignment()
                .unwrap()
                .horizontal(),
            HorizontalAlignmentValues::Left
        );
        assert_eq!(
            *sheet
                .style((col(Source::Duration("Conn_Duration")), 2))
                .alignment()
                .unwrap()
                .horizontal(),
            HorizontalAlignmentValues::Center
        );
        assert_eq!(
            *sheet
                .style((col(Source::ConnSpan), 2))
                .alignment()
                .unwrap()
                .horizontal(),
            HorizontalAlignmentValues::Center
        );
        assert_eq!(
            sheet
                .style((col(Source::Duration("Conn_Duration")), 2))
                .number_format()
                .unwrap()
                .format_code(),
            DURATION_FORMAT
        );

        // Explicit typing: Vehicle_Year is numeric, Station_ID stays text.
        assert_eq!(
            sheet.value((col(Source::Number("Vehicle_Year")), 2)),
            "2024"
        );
        assert_eq!(
            sheet.value((col(Source::Text("Station_ID")), 2)),
            "Station-7"
        );

        // The zero-energy session is present, not filtered out here.
        assert_eq!(sheet.value((col(Source::SessionId), 3)), "S13577");

        // avg_kw is written on every row, the zero-energy one included, so a row that would
        // divide by zero shows #DIV/0! rather than nothing at all.
        assert_eq!(
            sheet.cell((col(Source::AvgKw), 3)).unwrap().formula(),
            avg_kw_formula(3)
        );

        // Neither fixture row has anything wrong with its times, so the Anomalies column carries
        // no timing kind.
        for row in [2, 3] {
            assert!(
                timing_anomalies_in_cell(&sheet.value((col(Source::Anomalies), row))).is_empty()
            );
        }

        fs::remove_dir_all(&dir).ok();
    }

    /// The conversion report's anomalies carry rows of the file they were read from, which for a
    /// conversion is the CSV — header on row 1, so the first record is row 2.
    #[test]
    fn conversion_report_anomalies_carry_source_rows() {
        const CSV: &str = "\
Charge_Session_ID,Conn_DateTime_Start,Conn_DateTime_End,Conn_Duration,Active_Charge_Time,Energy_Use
S1,2026-11-01 01:10,2026-11-01 01:40,0:30:00,0:29:00,2.9
S2,2026-11-02 08:00,2026-11-02 08:00,0:00:00,0:00:00,4.2
S3,2026-11-03 09:00,2026-11-03 09:30,9:00:00,0:29:00,2.9
";
        let dir = temp_dir("excel_rows");
        let csv_path = dir.join("Session_Report_Test.csv");
        fs::write(&csv_path, CSV).unwrap();
        let report = session_csv_to_xlsx(&csv_path).unwrap();

        let items: Vec<_> = report
            .anomalies
            .iter()
            .filter(|a| a.kind != AnomalyKind::ExcessiveAvgKw)
            .map(|a| (a.session.row, a.session.id.as_str(), a.kind))
            .collect();
        assert_eq!(
            items,
            [
                (3, "S2", AnomalyKind::ZeroActiveChargeTime),
                (4, "S3", AnomalyKind::InconsistentDuration),
            ]
        );

        fs::remove_dir_all(&dir).ok();
    }
}
