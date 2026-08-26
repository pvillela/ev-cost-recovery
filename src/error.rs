use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use umya_spreadsheet::XlsxError;

/// A source file could not be read.
///
/// One of the two kinds `api::io` raises about a file rather than passing on from a pure function.
/// It is declared here, not there, because the modules that *raise* it are the readers themselves.
///
/// `path` is held for a caller that wants to act on which file failed rather than print it, and is
/// deliberately not written into the message. All four causes name their own file, each from a
/// `path` field of its own — see [`GbReadError`](crate::green_button::GbReadError),
/// [`ChargesReportError`](crate::charges_report::ChargesReportError) and
/// [`BillError`](crate::hydro_bill::BillError). Writing it here as well produced
/// `data/x.XML: data/x.XML: ...`.
///
/// The one cause that is not yet a structured type is the session reader's: `csv_sessions` returns
/// `Box<dyn Error>` with the path formatted into the string. That is the last place in the crate
/// where a path reaches a message without being a field, and it is why this doc says "all four
/// causes" rather than "all four error types".
#[derive(Debug)]
pub enum ReadError {
    /// The Green Button export could not be read, could not be parsed, or carries no reading in
    /// the billing period asked for.
    GreenButton {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// A session report could not be read.
    SessionReport {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// Evolute's Charges Report could not be read, or is not one.
    ChargesReport {
        path: PathBuf,
        cause: Box<dyn Error>,
    },

    /// A Toronto Hydro bill PDF could not be read, or is not laid out the way one is read.
    ///
    /// [`BillError::is_layout`](crate::hydro_bill::BillError::is_layout) tells those two apart,
    /// and `cause` downcasts to [`BillError`](crate::hydro_bill::BillError) for a caller that
    /// wants to ask.
    Bill {
        path: PathBuf,
        cause: Box<dyn Error>,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GreenButton { cause, .. }
            | Self::SessionReport { cause, .. }
            | Self::ChargesReport { cause, .. }
            | Self::Bill { cause, .. } => cause.fmt(f),
        }
    }
}

impl Error for ReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GreenButton { cause, .. }
            | Self::SessionReport { cause, .. }
            | Self::ChargesReport { cause, .. }
            | Self::Bill { cause, .. } => Some(cause.as_ref()),
        }
    }
}

/// A workbook could not be produced from the file it was to be converted from.
///
/// Raised only by the two conversions. What the input could not be *read* as is
/// [`ReadError`]; this is about the output.
#[derive(Debug)]
pub enum ConversionError {
    /// The workbook's name would be the input's own, so the conversion would read and write one
    /// file. Reached by handing in something already named `.xlsx`.
    OutputWouldBeInput { path: PathBuf },

    /// A file already stands where the workbook would go, and the caller asked to be refused.
    ///
    /// Refusing is the default rather than a courtesy: the figures in these workbooks get
    /// reconciled against real invoices by hand, and a silent overwrite is how that work is lost.
    /// Move the existing file, delete it, or call again with
    /// [`OnExistingWorkbook::Replace`](crate::io::OnExistingWorkbook::Replace).
    OutputExists { path: PathBuf },

    /// The workbook could not be built or could not be written.
    Write {
        path: PathBuf,
        cause: Box<dyn Error>,
    },
}

impl ConversionError {
    pub fn from_xlsx_error(xlsx_error: XlsxError, path: &Path) -> Self {
        Self::Write {
            path: path.to_path_buf(),
            cause: xlsx_error.into(),
        }
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutputWouldBeInput { path } => write!(
                f,
                "{} is already an .xlsx file, so converting it would read and write the same file",
                path.display()
            ),
            Self::OutputExists { path } => write!(
                f,
                "{} already exists. Move or delete it first, or ask for it to be replaced -- a \
                 conversion never overwrites its output unless told to.",
                path.display()
            ),
            // The path is written in, unlike `ReadError`'s: the writers name no file of their own,
            // so without it a caller is told a workbook could not be written and left to guess
            // which.
            Self::Write { path, cause } => write!(f, "{}: {cause}", path.display()),
        }
    }
}

impl Error for ConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OutputWouldBeInput { .. } | Self::OutputExists { .. } => None,
            Self::Write { cause, .. } => Some(cause.as_ref()),
        }
    }
}
