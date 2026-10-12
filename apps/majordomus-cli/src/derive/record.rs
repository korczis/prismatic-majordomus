//! What a derive remembers between runs, and where: one record of the last successful
//! stage B, and one marker while a stage B is in flight.
//!
//! # Where it lives
//!
//! Under the checkout's own git directory (`git rev-parse --git-path majordomus/derive`).
//! That makes it per worktree by construction, never tracked, outside `.ai/local/` (which
//! the stage itself writes into), and untouched by `git clean -x`. A sibling worktree's
//! record can never answer for this one.
//!
//! # What may be in it
//!
//! Only a success. The record is written after stage B exited 0 and nothing it could read
//! moved while it ran; a failed or killed run writes nothing, so there is no state a later
//! run could mistake for one. The marker is the other half of that: it is written before
//! stage B starts and removed when its result is recorded, so a marker found at the start of
//! a run means the run before it did not finish.
//!
//! Losing either file is always safe. No record is "never ran", and the stage runs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::key::{KEY_SCHEMA, NODE};
use super::{Mode, State};
use crate::bench::results::write_atomic;
use crate::git::read_only;

/// The schema of a [`Record`].
pub(crate) const RECORD_SCHEMA: &str = "majordomus/derive-record/v1";
/// The schema of a [`Pending`] marker.
pub(crate) const PENDING_SCHEMA: &str = "majordomus/derive-pending/v1";
/// The schema of a [`Divergence`] report.
pub(crate) const DIVERGENCE_SCHEMA: &str = "majordomus/derive-shadow-divergence/v1";

/// The last successful stage B of this checkout: the key it ran under and what it left.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    /// [`RECORD_SCHEMA`].
    pub(crate) schema: String,
    /// The [`KEY_SCHEMA`] the key was built under.
    pub(crate) key_schema: u32,
    /// The node, [`NODE`].
    pub(crate) node: String,
    /// The key of the tree the stage ran over.
    pub(crate) key: String,
    /// Each component of that key, so a later difference can be named.
    pub(crate) components: BTreeMap<String, String>,
    /// Every input with its state, so a moved input can be named.
    pub(crate) inputs: Vec<(String, String)>,
    /// Every output file the stage left, with its digest.
    pub(crate) outputs: Vec<(String, String)>,
    /// The generation of the executable that decided, for a reader of the file.
    pub(crate) generation: String,
    /// When the run was recorded, RFC 3339, UTC.
    pub(crate) recorded_at: String,
}

/// A stage B that was begun and whose result is not recorded yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pending {
    /// [`PENDING_SCHEMA`].
    pub(crate) schema: String,
    /// The mode the derive runs in.
    pub(crate) mode: Mode,
    /// What the incremental decision was when the stage began.
    pub(crate) expected: State,
    /// The settled digest of the tree at that moment.
    pub(crate) settled: String,
    /// The components at that moment, to name what moved if something did.
    pub(crate) components: BTreeMap<String, String>,
    /// Under shadow, when the decision was to reuse: the outputs that were verified, which
    /// the run that follows must leave exactly as they are.
    pub(crate) outputs: Option<Vec<(String, String)>>,
    /// When the stage began, RFC 3339, UTC.
    pub(crate) begun_at: String,
}

/// What a shadow run found when the stage it ran changed what the skip would have kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Divergence {
    /// [`DIVERGENCE_SCHEMA`].
    pub(crate) schema: String,
    /// The key under which the skip was decided.
    pub(crate) key: String,
    /// Every output that differs, with what the skip would have kept and what the run left.
    pub(crate) differing: Vec<DivergentOutput>,
    /// When it was observed, RFC 3339, UTC.
    pub(crate) observed_at: String,
}

/// One output a shadow run found different.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DivergentOutput {
    /// The output, repository-relative.
    pub(crate) path: String,
    /// Its digest before the run, or `None` when it was not there.
    pub(crate) kept: Option<String>,
    /// Its digest after the run, or `None` when the run removed it.
    pub(crate) written: Option<String>,
}

/// What reading the record found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Loaded {
    /// There is none.
    Absent,
    /// There is a file and it is not a record this executable reads; the sentence says why.
    Invalid(String),
    /// The record.
    Found(Box<Record>),
}

/// The directory a checkout keeps its derive state in.
#[derive(Debug, Clone)]
pub(crate) struct Store {
    dir: PathBuf,
}

impl Store {
    /// The store of the checkout at `root`, wherever its git directory is.
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        let out = read_only(root)
            .args(["rev-parse", "--git-path", "majordomus/derive"])
            .output()
            .map_err(|e| format!("git could not run: {e}"))?;
        let said = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || said.is_empty() {
            return Err(format!(
                "{} is not a git work tree: {}",
                root.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        // git answers relative to the directory it was asked in
        Ok(Store {
            dir: root.join(said),
        })
    }

    /// The record's file.
    pub(crate) fn record_path(&self) -> PathBuf {
        self.dir.join("site-data.json")
    }

    /// The marker's file.
    pub(crate) fn pending_path(&self) -> PathBuf {
        self.dir.join("site-data.pending.json")
    }

    /// The last divergence report's file.
    pub(crate) fn divergence_path(&self) -> PathBuf {
        self.dir.join("site-data.shadow-divergence.json")
    }

    /// Read the record. A file that does not parse, carries a field nothing reads, or was
    /// made under another schema is not a record: it is said to be invalid and never used.
    pub(crate) fn record(&self) -> Loaded {
        let path = self.record_path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::Absent,
            Err(e) => return Loaded::Invalid(format!("it cannot be read: {e}")),
        };
        let record: Record = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(e) => return Loaded::Invalid(format!("it does not parse: {e}")),
        };
        if record.schema != RECORD_SCHEMA || record.node != NODE {
            return Loaded::Invalid(format!(
                "it is a {} of {}, and this executable reads {RECORD_SCHEMA} of {NODE}",
                record.schema, record.node
            ));
        }
        if record.key_schema != KEY_SCHEMA {
            return Loaded::Invalid(format!(
                "its key was built under schema {}, and this executable builds schema {KEY_SCHEMA}",
                record.key_schema
            ));
        }
        Loaded::Found(Box::new(record))
    }

    /// Read the marker; one that does not parse still says a run was begun.
    pub(crate) fn pending(&self) -> Option<Result<Pending, String>> {
        match std::fs::read_to_string(self.pending_path()) {
            Ok(text) => Some(serde_json::from_str(&text).map_err(|e| e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => Some(Err(e.to_string())),
        }
    }

    /// Write one of the store's files whole, through a neighbour and a rename.
    pub(crate) fn write<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
        write_atomic(path, &(text + "\n")).map_err(|e| e.to_string())
    }

    /// Remove one of the store's files; one that is not there is already removed.
    pub(crate) fn remove(&self, path: &Path) -> Result<(), String> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
}
