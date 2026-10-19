//! Errors of the command line tool.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use mtg_import::scryfall::ScryfallError;
use thiserror::Error;

/// A request or download that failed.
#[derive(Debug, Error)]
pub enum FetchError {
    /// The connection, the request or the body failed.
    #[error("{url}: {source}")]
    Transport {
        /// The address that was requested.
        url: String,
        /// What went wrong.
        source: reqwest::Error,
    },
    /// The server refused with an HTTP status.
    #[error("{url}: the server answered {status}")]
    Status {
        /// The address that was requested.
        url: String,
        /// The HTTP status code.
        status: u16,
        /// How long the server asked us to wait, when it said.
        retry_after: Option<Duration>,
    },
    /// The server has no such file.
    #[error("{url}: not found")]
    NotFound {
        /// The address that was requested.
        url: String,
    },
    /// What arrived does not add up; the partial file was discarded.
    #[error("{url}: {reason}")]
    Corrupt {
        /// The address that was requested.
        url: String,
        /// What does not add up.
        reason: String,
    },
    /// The answer is not what this source is documented to send.
    #[error("{url}: unexpected answer: {reason}")]
    Invalid {
        /// The address that was requested.
        url: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A cache file could not be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The operating system's complaint.
        source: io::Error,
    },
    /// The HTTP client could not be set up.
    #[error("cannot set up the HTTP client: {0}")]
    Client(reqwest::Error),
}

impl FetchError {
    /// Whether asking again can help: dropped connections, timeouts, overload and discarded partial files.
    pub fn is_retryable(&self) -> bool {
        match self {
            FetchError::Transport { source, .. } => !source.is_builder() && !source.is_redirect(),
            FetchError::Status { status, .. } => matches!(status, 408 | 425 | 429 | 500..=599),
            FetchError::Corrupt { .. } => true,
            FetchError::NotFound { .. }
            | FetchError::Invalid { .. }
            | FetchError::Io { .. }
            | FetchError::Client(_) => false,
        }
    }

    /// The wait the server asked for, if any.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            FetchError::Status { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    /// An I/O failure on `path`.
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        FetchError::Io {
            path: path.into(),
            source,
        }
    }
}

/// Why a command failed.
#[derive(Debug, Error)]
pub enum CliError {
    /// A download failed.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// The database failed.
    #[error("database: {0}")]
    Db(#[from] mtg_db::DbError),
    /// An import run failed.
    #[error(transparent)]
    Import(#[from] mtg_db::ImportError),
    /// The settings cannot be used.
    #[error("{0}")]
    Config(String),
    /// A file could not be read or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The operating system's complaint.
        source: io::Error,
    },
    /// A cached file or the manifest is unusable.
    #[error("{}: {reason}", path.display())]
    Cache {
        /// The file.
        path: PathBuf,
        /// What is wrong with it.
        reason: String,
    },
    /// A bulk file is broken in a way that ends reading it.
    #[error("{what}: {source}")]
    Bulk {
        /// The file that was read.
        what: String,
        /// The framing failure.
        source: ScryfallError,
    },
}

impl CliError {
    /// An I/O failure on `path`.
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        CliError::Io {
            path: path.into(),
            source,
        }
    }
}
