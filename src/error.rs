//! Errors a library module raises about a file it was asked to produce.
//!
//! One type, for now. The test for whether something belongs here is who *raises* it, not who
//! renders it: [`ConversionError`] is returned by
//! [`session::session_csv_to_xlsx`](crate::session::session_csv_to_xlsx) and
//! [`write_gb_workbook`](crate::green_button::write_gb_workbook), neither of which depends on the
//! API, so it cannot live in `api::error` without pointing that arrow the wrong way.
//!
//! [`ReadError`](crate::io::ReadError) was here too, briefly. It failed the same test in the other
//! direction — nothing outside `api::io` ever built one — and is declared with the API again.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use umya_spreadsheet::XlsxError;

/// A workbook could not be produced from the file it was to be converted from.
///
/// Here rather than in `api::error` because the modules that *raise* it are here: both
/// [`session::session_csv_to_xlsx`](crate::session::session_csv_to_xlsx) and
/// [`write_gb_workbook`](crate::green_button::write_gb_workbook) return it, and neither depends on
/// the API. `ReadError`, which travelled with it once, went back to `api::error`: nothing outside
/// `api::io` ever raised that one.
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
    /// rather than in [`ReadError`](crate::io::ReadError) because a conversion is a single
    /// operation to a caller — one
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
