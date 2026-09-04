//! Typed errors and the exit-code contract.
//!
//! The codes mirror `docs/CLI.md`: `0` ok, `2` usage, `10` contract unmet, `12` missing
//! artifact, `13` internal error. A malformed file inside the layer is not an `Error` but a
//! [`crate::Diagnostic`]; errors are for states where nothing can proceed.

use std::path::PathBuf;

/// Everything that stops a command.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no Majordomus repository found from {start}: no ancestor directory carries .ai/manifest.yaml"
    )]
    RepositoryNotFound { start: PathBuf },

    #[error(
        "project data lives under {root}/.majordomus (the pre-.ai layout), which nothing reads any more; run: majordomus migrate"
    )]
    LegacyLayout { root: PathBuf },

    #[error("{path}: {reason}")]
    InvalidManifest { path: PathBuf, reason: String },

    #[error("{path}: unsupported schema '{found}'; this executable reads {supported}")]
    UnsupportedSchema {
        path: PathBuf,
        found: String,
        supported: String,
    },

    #[error("{path}: {reason}")]
    InvalidSources { path: PathBuf, reason: String },

    #[error("{path}: unknown key(s): {}", keys.join(", "))]
    UnknownKeys { path: PathBuf, keys: Vec<String> },

    #[error("embedded kind schema is invalid: {reason}")]
    KindSchema { reason: String },

    #[error("declarative state carries {count} error(s); refusing to serve under --strict (run: majordomus mcp --inspect)")]
    StrictDiagnostics { count: usize },

    #[error("git: {reason}")]
    Git { reason: String },

    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("transport: {0}")]
    Transport(#[source] std::io::Error),

    #[error("protocol: {reason}")]
    Protocol { reason: String },
}

impl Error {
    /// The process exit code this error maps to, per the exit-code contract.
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::RepositoryNotFound { .. } | Error::LegacyLayout { .. } => 12,
            Error::InvalidManifest { .. }
            | Error::UnsupportedSchema { .. }
            | Error::InvalidSources { .. }
            | Error::UnknownKeys { .. }
            | Error::StrictDiagnostics { .. } => 10,
            Error::KindSchema { .. }
            | Error::Git { .. }
            | Error::Io { .. }
            | Error::Transport(_)
            | Error::Protocol { .. } => 13,
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
