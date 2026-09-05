//! Typed errors and the exit-code contract.
//!
//! The codes mirror `docs/CLI.md`: `0` ok, `2` usage, `10` contract unmet, `12` missing
//! artifact, `13` internal error. A malformed file inside the layer is not an `Error` but a
//! [`crate::Diagnostic`]; errors are for states where nothing can proceed.

use std::path::PathBuf;

/// Everything that stops a command.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No ancestor of the start directory carries `.ai/manifest.yaml`.
    #[error(
        "no Majordomus repository found from {start}: no ancestor directory carries .ai/manifest.yaml"
    )]
    RepositoryNotFound {
        /// Where the search began.
        start: PathBuf,
    },

    /// Project data sits under `.majordomus/` with no manifest: the pre-`.ai` layout.
    #[error(
        "project data lives under {root}/.majordomus (the pre-.ai layout), which nothing reads any more; run: majordomus migrate"
    )]
    LegacyLayout {
        /// The directory that holds the legacy layout.
        root: PathBuf,
    },

    /// The manifest does not parse, or a constraint of its schema fails.
    #[error("{path}: {reason}")]
    InvalidManifest {
        /// The manifest.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },

    /// The manifest declares a layer schema this executable does not read.
    #[error("{path}: unsupported schema '{found}'; this executable reads {supported}")]
    UnsupportedSchema {
        /// The manifest.
        path: PathBuf,
        /// The schema declared.
        found: String,
        /// The schema this executable reads.
        supported: String,
    },

    /// `sources.yaml` does not parse or contradicts its own rules.
    #[error("{path}: {reason}")]
    InvalidSources {
        /// The sources file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },

    /// A file carries keys nothing reads.
    #[error("{path}: unknown key(s): {}", keys.join(", "))]
    UnknownKeys {
        /// The file.
        path: PathBuf,
        /// The key paths, in document order.
        keys: Vec<String>,
    },

    /// A kinds file or a schema file cannot be used: malformed, contradictory, or redefining a distributed one.
    #[error("kind schema: {reason}")]
    KindSchema {
        /// What is wrong, with the file named.
        reason: String,
    },

    /// No directory tried holds `kinds.yaml`.
    #[error("no share directory holds kinds.yaml; tried {}; pass --share <dir> or set MAJORDOMUS_SHARE", tried.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))]
    ShareNotFound {
        /// Every directory looked at, in order.
        tried: Vec<PathBuf>,
    },

    /// `--strict` was given and the index carries error diagnostics.
    #[error("declarative state carries {count} error(s); refusing to serve under --strict (run: majordomus mcp --inspect)")]
    StrictDiagnostics {
        /// How many.
        count: usize,
    },

    /// `git` could not be run or answered with an error, where it was needed.
    #[error("git: {reason}")]
    Git {
        /// What `git` said.
        reason: String,
    },

    /// A file or directory could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The path.
        path: PathBuf,
        #[source]
        /// The underlying error.
        source: std::io::Error,
    },

    /// The stdio transport failed to read or write.
    #[error("transport: {0}")]
    Transport(#[source] std::io::Error),

    /// A protocol frame could not be encoded, or an internal answer could not be produced.
    #[error("protocol: {reason}")]
    Protocol {
        /// What is wrong.
        reason: String,
    },

    /// The capability registry does not build; every violation is listed.
    #[error("the capability registry does not build:\n{}", errors.iter().map(|e| format!("  {e}")).collect::<Vec<_>>().join("\n"))]
    Registry {
        /// The violations, in id order.
        errors: Vec<crate::capability::RegistryError>,
    },

    /// The HTTP projection failed: a port that cannot be bound, or an OpenAPI document that cannot be built.
    #[error("http: {reason}")]
    Http {
        /// What is wrong.
        reason: String,
    },

    /// No capability has this id.
    #[error("unknown capability: {id} (run: majordomus capabilities list)")]
    CapabilityNotFound {
        /// The id asked for.
        id: String,
    },

    /// `generate --check` found committed projections that differ from the registry, or are missing.
    #[error("generated artifact(s) stale: {} (run: majordomus generate)", files.join(", "))]
    Stale {
        /// Each stale file with `(differs)` or `(missing)`.
        files: Vec<String>,
    },
}

impl Error {
    /// The process exit code this error maps to, per the exit-code contract.
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::RepositoryNotFound { .. }
            | Error::LegacyLayout { .. }
            | Error::ShareNotFound { .. } => 12,
            Error::InvalidManifest { .. }
            | Error::UnsupportedSchema { .. }
            | Error::InvalidSources { .. }
            | Error::UnknownKeys { .. }
            | Error::StrictDiagnostics { .. }
            | Error::KindSchema { .. }
            | Error::Registry { .. }
            | Error::Stale { .. } => 10,
            Error::CapabilityNotFound { .. } => 12,
            Error::Git { .. }
            | Error::Io { .. }
            | Error::Transport(_)
            | Error::Http { .. }
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

/// A result whose error is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
