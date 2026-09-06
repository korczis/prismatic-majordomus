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

    /// `scope.yaml` does not parse or contradicts its own rules.
    #[error("{path}: {reason}")]
    InvalidScope {
        /// The scope file.
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

    /// The shared server's lease could not be acquired or joined.
    #[error("lease: {reason}")]
    Lease {
        /// What is wrong.
        reason: String,
    },

    /// A deployment was named that the layer does not declare.
    #[error("no deployment '{id}' in this layer (the deployments are the objects under the layer's deployments section)")]
    DeploymentNotFound {
        /// The id asked for.
        id: String,
    },

    /// A deployment object exists and cannot be read as one.
    #[error("{reason}")]
    InvalidDeployment {
        /// What is wrong, with the file, the key, the value and the correction.
        reason: String,
    },

    /// No capability has this id.
    #[error("unknown capability: {id} (run: majordomus capabilities list)")]
    CapabilityNotFound {
        /// The id asked for.
        id: String,
    },

    /// A caller named something this repository does not hold. Not the caller's fault in
    /// the sense an internal error is: the request was well formed and the thing is
    /// absent, which is the missing-artifact code and not the internal one.
    #[error("{reason}")]
    NotFound {
        /// What was asked for and where the caller can see what exists.
        reason: String,
    },

    /// The policy file does not parse, or does not carry what the projections need.
    #[error("policy {path} is invalid: {reason}")]
    InvalidPolicy {
        /// The policy file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// A declared provider projection cannot be produced.
    #[error("projection {target}: {reason}")]
    InvalidProjection {
        /// The declared target.
        target: String,
        /// What is wrong.
        reason: String,
    },
    /// A web surface's declaration cannot be read, or the topology it would join is not valid.
    #[error("web surface {surface}: {reason}")]
    InvalidSurface {
        /// The surface's id, or the declaration's path when the id is what is wrong.
        surface: String,
        /// What is wrong, and what to do about it.
        reason: String,
    },
    /// The distribution model cannot be read, or it breaks an invariant of its own contract.
    #[error("distribution model {path}: {reason}")]
    InvalidDistribution {
        /// Where the model was read from.
        path: String,
        /// What is wrong, and what to do about it.
        reason: String,
    },
    /// A release record cannot be read, or it disagrees with the distribution model.
    #[error("release record {path}: {reason}")]
    InvalidRelease {
        /// Where the record was read from.
        path: String,
        /// What is wrong, and what to do about it.
        reason: String,
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
            | Error::InvalidScope { .. }
            | Error::UnknownKeys { .. }
            | Error::StrictDiagnostics { .. }
            | Error::KindSchema { .. }
            | Error::Registry { .. }
            | Error::InvalidPolicy { .. }
            | Error::InvalidProjection { .. }
            | Error::InvalidSurface { .. }
            | Error::InvalidDeployment { .. }
            | Error::InvalidDistribution { .. }
            | Error::InvalidRelease { .. }
            | Error::Stale { .. } => 10,
            Error::CapabilityNotFound { .. }
            | Error::NotFound { .. }
            | Error::DeploymentNotFound { .. } => 12,
            Error::Git { .. }
            | Error::Io { .. }
            | Error::Transport(_)
            | Error::Http { .. }
            | Error::Lease { .. }
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
