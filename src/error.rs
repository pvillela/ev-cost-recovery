use std::{error::Error, fmt, path::PathBuf};

/// A source file could not be read.
///
/// One of the two error kinds this module raises on its own. Everything else it returns comes from
/// a pure function it delegated to.
///
/// `path` is held for a caller that wants to act on which file failed rather than print it. It is
/// deliberately *not* written into the message: both readers name the file they concern, so adding
/// it here produced `data/x.XML: data/x.XML: ...`.
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
