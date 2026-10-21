//! How a download stops other than by being saved, and why one fails.

use std::fmt;
use std::io;
use std::path::PathBuf;

use nifty_runtime::NoHomeFolder;

use super::Abandoned;
use crate::connection::ConnectionError;
use crate::http::BadHeader;

/// How a download ended, other than being saved.
#[derive(Debug)]
pub(super) enum Stopped {
    Failed(DownloadError),
    Cancelled,
    Abandoned,
}

#[derive(Debug)]
pub(super) enum DownloadError {
    Connection(ConnectionError),
    Header(BadHeader),
    File(io::Error),
    NoHomeFolder(NoHomeFolder),
    /// The server sent a part of the file after the part already saved.
    PartTooFar {
        starts_at: u64,
        saved: u64,
    },
    /// The server sent a part of the file other than the one a connection asked for.
    PartMoved {
        starts_at: u64,
        asked: u64,
    },
    /// A connection's body ended before the end of its part.
    PartCutShort {
        saved: u64,
        end: u64,
    },
    /// The download stopped and its partly written file couldn't be removed.
    LeftBehind {
        reason: Box<Stopped>,
        path: PathBuf,
    },
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(error) => write!(f, "{error}"),
            Self::Header(error) => write!(f, "{error}"),
            Self::File(error) => write!(f, "{error}"),
            Self::NoHomeFolder(error) => write!(f, "{error}"),
            Self::PartTooFar { starts_at, saved } => write!(
                f,
                "the server sent the file from byte {starts_at}, but only {saved} bytes are saved"
            ),
            Self::PartMoved { starts_at, asked } => write!(
                f,
                "the server sent the file from byte {starts_at} when asked for byte {asked} on"
            ),
            Self::PartCutShort { saved, end } => write!(
                f,
                "the server stopped sending the file at byte {saved}, before byte {end}"
            ),
            Self::LeftBehind { reason, path } => {
                let reason = match &**reason {
                    Stopped::Failed(error) => error.to_string(),
                    Stopped::Cancelled => "cancelled".to_owned(),
                    Stopped::Abandoned => "the app stopped waiting for it".to_owned(),
                };
                write!(
                    f,
                    "{reason}; the partial file {} is left behind",
                    path.display()
                )
            }
        }
    }
}

impl From<io::Error> for Stopped {
    fn from(error: io::Error) -> Self {
        Self::Failed(DownloadError::File(error))
    }
}

impl From<NoHomeFolder> for Stopped {
    fn from(error: NoHomeFolder) -> Self {
        Self::Failed(DownloadError::NoHomeFolder(error))
    }
}

impl From<Abandoned> for Stopped {
    fn from(Abandoned: Abandoned) -> Self {
        Self::Abandoned
    }
}
