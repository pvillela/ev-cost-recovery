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
/// deliberately not written into the message. All four causes are structured types that name their
/// own file, each from a `path` field of its own —
/// [`GbReadError`](crate::green_button::GbReadError),
/// the private `session::csv::SessionCsvError`,
/// [`ChargesReportError`](crate::charges_report::ChargesReportError) and
/// [`BillError`](crate::hydro_bill::BillError). Writing it here as well produced
/// `data/x.XML: data/x.XML: ...`.
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
    /// Which of those it is, this does not say. `cause` is a
    /// [`BillError`](crate::hydro_bill::BillError), whose
    /// [`is_layout`](crate::hydro_bill::BillError::is_layout) tells the two apart — but nothing
    /// downcasts to it today, so treat that as a fact about the current implementation rather than
    /// as a contract. If telling them apart from outside is ever wanted, say so in the variant
    /// rather than leaving a caller to guess at the boxed type.
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

    /// The input could not be read, so there was nothing to convert.
    ///
    /// Distinct from [`Self::Write`], and carrying no `path` of its own: the readers raise typed
    /// errors that name the file from a field, so adding it here would print it twice. It is here
    /// rather than in [`ReadError`] because a conversion is a single operation to a caller — one
    /// call, one error type — and which half of it failed is what these variants are for.
    Input { cause: Box<dyn Error> },

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
            // Deferred to, exactly like `ReadError`'s: the readers raise typed errors that name
            // the file from a field of their own.
            Self::Input { cause } => cause.fmt(f),
            // The path is written in, unlike the arm above: the *writers* name no file of their
            // own, so without it a caller is told a workbook could not be written and left to
            // guess which.
            Self::Write { path, cause } => write!(f, "{}: {cause}", path.display()),
        }
    }
}

impl Error for ConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OutputWouldBeInput { .. } | Self::OutputExists { .. } => None,
            Self::Input { cause } | Self::Write { cause, .. } => Some(cause.as_ref()),
        }
    }
}
