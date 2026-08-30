//! Plain-text run logs written beside each output file.
//!
//! One per operation, overwritten each run: `<stem>.session.convert.log` for CSV to Excel,
//! `<stem>.session.csv.read.log` for CSV straight to sessions, and
//! `<stem>.session.xlsx.read.log` for Excel back to sessions. A log always says one of two
//! things — that everything was fine, or what was found — so an empty-looking run is
//! distinguishable from one that was never made.
//!
//! # What a suffix is made of
//!
//! The kind of document, then the format read, then the operation. The kind comes first because a
//! log names its subject the way an error message does: the reader has several kinds of file in
//! one folder, and `June.csv` alone does not say which of them this log is about. The format has
//! to stay because the three logs of one session report share a stem — the CSV and the workbook it
//! converts to — so dropping it would have each run overwrite the last.
//!
//! # Why discrepancies are not anomalies
//!
//! [`AnomalyKind`](crate::session::AnomalyKind) describes the *session*: something about the reported data
//! needs a judgement call, and one kind, `InconsistentDuration`, removes the session from every
//! estimate. A discrepancy describes the *workbook*: a stored column disagrees with what the
//! `Session` methods recompute, which means the sheet is stale or was edited, and says nothing
//! about the session itself.
//!
//! Keeping them apart is the point. If a stale cell could raise an anomaly, editing a workbook
//! could silently change which sessions feed an estimate — and the estimate would still look
//! clean. The recomputed value always wins; the disagreement is logged and nothing else changes.

use std::{
    error::Error,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

/// What a log records: either it is empty, or every line is something that needs a reader.
#[derive(Debug, Default, Clone)]
pub struct RunLog {
    /// Free-text lines, in the order they were found.
    entries: Vec<String>,
}

impl RunLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn note(&mut self, line: impl Into<String>) {
        self.entries.push(line.into());
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The log's text, ending in a newline.
    ///
    /// `operation` names what was run and `subject` the file it ran on; both appear in the header
    /// so a log read on its own still says what it is about.
    pub fn render(&self, operation: &str, subject: &Path) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{operation}: {}", subject.display());
        if self.entries.is_empty() {
            let _ = writeln!(
                out,
                "\nNothing to report. No errors, warnings or anomalies."
            );
            return out;
        }
        let _ = writeln!(
            out,
            "\n{} item(s) to review, in the order found:\n",
            self.entries.len()
        );
        for entry in &self.entries {
            let _ = writeln!(out, "  {entry}");
        }
        out
    }

    /// Writes the log beside `output`, replacing any extension with `<suffix>.log`.
    ///
    /// Returns where it went, so a caller can tell the user. A failure to write the log is
    /// returned rather than swallowed: a log the user believes exists and does not is worse than
    /// no log at all.
    pub fn write_beside(
        &self,
        output: &Path,
        suffix: &str,
        operation: &str,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let path = log_path(output, suffix);
        fs::write(&path, self.render(operation, output))?;
        Ok(path)
    }
}

/// One source file's run log, held rather than written.
///
/// A reader produces this and hands it back; whether it reaches a file is the caller's to decide.
/// That is the rule this crate follows for everything short of a halt: a function that can return
/// what it found returns it, and only a function that cannot — a binary's `main`, which has
/// nowhere left to return to — writes it out.
///
/// It carries `suffix` and `operation` because the reader knows them and the caller does not. A
/// binary writing the logs it was handed should not have to know that a CSV read is called
/// `session.csv.read` while a read-back from a workbook is called `session.xlsx.read`.
#[derive(Debug, Clone)]
pub struct SourceLog {
    /// The file the log is about, and the file it is written beside.
    pub source: PathBuf,
    /// The log file's own suffix: `<stem>.<suffix>.log`.
    pub suffix: &'static str,
    /// What was run, for the log's header.
    pub operation: &'static str,
    /// What was found.
    pub log: RunLog,
}

impl SourceLog {
    /// Writes the log beside its source, returning where it went.
    ///
    /// # Errors
    ///
    /// Whatever the write failed with. Returned rather than swallowed, for the reason
    /// [`RunLog::write_beside`] gives.
    pub fn write(&self) -> Result<PathBuf, Box<dyn Error>> {
        self.log
            .write_beside(&self.source, self.suffix, self.operation)
    }

    /// Where [`Self::write`] would put it.
    pub fn path(&self) -> PathBuf {
        log_path(&self.source, self.suffix)
    }

    /// The log's text, as a file would hold it.
    pub fn render(&self) -> String {
        self.log.render(self.operation, &self.source)
    }
}

/// `<stem>.<suffix>.log` beside `output`.
fn log_path(output: &Path, suffix: &str) -> PathBuf {
    let stem = output
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "session_report".to_owned());
    output.with_file_name(format!("{stem}.{suffix}.log"))
}

// cargo test --lib -- log::test --nocapture
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn an_empty_log_says_so_rather_than_being_blank() {
        let text = RunLog::new().render("Converted", Path::new("/tmp/June.xlsx"));
        assert!(text.contains("Nothing to report"), "{text}");
        assert!(text.contains("June.xlsx"), "{text}");
    }

    #[test]
    fn a_log_lists_what_it_found_in_order() {
        let mut log = RunLog::new();
        log.note("row 2: first");
        log.note("row 9: second");
        let text = log.render("Read", Path::new("/tmp/June.xlsx"));
        assert!(text.contains("2 item(s)"), "{text}");
        assert!(
            text.find("first").unwrap() < text.find("second").unwrap(),
            "{text}"
        );
        assert!(!text.contains("Nothing to report"), "{text}");
    }

    /// The log sits beside the workbook and replaces its extension, so a `.xlsx` and its log share
    /// a stem and sort together in a directory listing.
    #[test]
    fn the_log_path_sits_beside_its_output() {
        assert_eq!(
            log_path(
                Path::new("/data/Session_Report_June.xlsx"),
                "session.convert"
            ),
            Path::new("/data/Session_Report_June.session.convert.log")
        );
        assert_eq!(
            log_path(
                Path::new("/data/Session_Report_June.xlsx"),
                "session.xlsx.read"
            ),
            Path::new("/data/Session_Report_June.session.xlsx.read.log")
        );
        // Beside the CSV rather than the workbook, and under its own suffix, so the three logs of
        // one session report cannot overwrite each other.
        assert_eq!(
            log_path(
                Path::new("/data/Session_Report_June.csv"),
                "session.csv.read"
            ),
            Path::new("/data/Session_Report_June.session.csv.read.log")
        );
    }
}
