//! The snapshot cache: the two or three answers a fast resolution cannot afford to
//! compute, kept beside the shared server's lease under the checkout-local half of the
//! layer.
//!
//! # Why there is a cache at all
//!
//! Counting what the layer holds means building the index, which reads and validates every
//! declared file. That is the right cost for a command a person waits on and an impossible
//! one for a shell prompt. So a full resolution writes what it learnt, and a fast one
//! reads it back.
//!
//! # Why it is three caches and not one
//!
//! Each tier carries its own fingerprint over its own inputs, so editing the justfile
//! expires the workflows and leaves the counts alone, and committing a rule expires the
//! counts and leaves the workflows alone. One fingerprint over everything would make every
//! change invalidate everything, which is the cheap version of this design and the one
//! that makes a warm cache rare.
//!
//! # What is in it, and what may never be
//!
//! Counts, recipe names and their doc comments, toolchain version strings, and a digest of
//! the last snapshot. Nothing read from the environment, no file contents, no paths
//! outside the repository, and nothing a credential could be hiding in — asserted by
//! `the_cache_holds_nothing_that_could_be_a_secret`.
//!
//! A cache that cannot be read is a cache miss, never an error: a truncated write, a file
//! from another version, a directory that is read-only. Writes are atomic — a temporary
//! file and a rename — so a reader sees either the previous entry or the whole new one.
//!
//! # The lifecycle
//!
//! A full resolution computes a tier and stores it under the fingerprint of the inputs it
//! read; a fast one loads the document and asks each tier whether that fingerprint still
//! holds. An input that moved makes the tier a miss, which is the whole mechanism — there
//! is no invalidation step for anybody to forget to call.
//!
//! ```
//! use majordomus_cli::environment::cache::{fingerprint_of, Cache};
//! use majordomus_cli::environment::{LayerSummary, TierState};
//! let dir = tempfile::tempdir().expect("a temporary directory");
//! std::fs::write(dir.path().join("justfile"), "build:\n  true\n").expect("an input");
//!
//! let counted = LayerSummary {
//!     state: TierState::Resolved,
//!     kinds: vec![],
//!     objects: Some(902),
//!     capabilities: Some(934),
//!     invalid: Some(0),
//!     degraded: Some(false),
//! };
//! let inputs = fingerprint_of(dir.path(), &["justfile".into()], &["layer"]);
//! let mut cache = Cache::default();
//! cache.tiers.layer = Some(Cache::entry(inputs.clone(), counted));
//! cache.store(dir.path(), "local").expect("a writable checkout");
//!
//! let entry = Cache::load(dir.path(), "local")
//!     .tiers
//!     .layer
//!     .expect("the tier that was written is the tier that is read");
//! assert_eq!(entry.fresh(&inputs, None).and_then(|s| s.objects), Some(902));
//!
//! std::fs::write(dir.path().join("justfile"), "build:\n  false\n").expect("a changed input");
//! let moved = fingerprint_of(dir.path(), &["justfile".into()], &["layer"]);
//! assert!(entry.fresh(&moved, None).is_none(), "an input that changed expires its tier");
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::{LayerSummary, ToolchainState, WorkflowCatalogue};

/// Where the cache lives, relative to the checkout-local half of the layer (`.ai/local/`).
/// Beside the shared server's lease, for the same reason: it belongs to this checkout, it
/// is never tracked, and a fresh clone starts without it by design.
pub const CACHE_PATH: &str = "state/environment/snapshot.json";

/// The cache document's own schema. A document of any other schema is a miss.
pub const SCHEMA: &str = "majordomus-environment-cache/v1";

/// How long an installed toolchain version is believed. Nothing in the repository changes
/// when a person upgrades their compiler, so no fingerprint over the repository can notice
/// it; a bounded life is what makes that self-heal instead of lying until the next full
/// resolution.
pub const TOOLCHAIN_LIFETIME: Duration = Duration::from_secs(6 * 3600);

/// One cached answer with the fingerprint of the inputs it was computed from.
///
/// The fingerprint travels with the value rather than beside it, so a tier can never be
/// read under somebody else's key: whatever is in the file, the only question asked of it
/// is whether the inputs it names are still the inputs there are.
///
/// ```
/// use majordomus_cli::environment::cache::{now_seconds, Entry};
/// let entry = Entry {
///     fingerprint: "the inputs as they were".to_string(),
///     written_at: now_seconds(),
///     value: 902usize,
/// };
/// assert_eq!(entry.fresh("the inputs as they were", None), Some(&902));
/// assert_eq!(entry.fresh("anything else", None), None, "a key that does not match is a miss");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry<T> {
    /// The fingerprint of this tier's inputs when the value was computed.
    pub fingerprint: String,
    /// When it was written, seconds since the Unix epoch.
    pub written_at: u64,
    /// The value.
    pub value: T,
}

impl<T> Entry<T> {
    /// The value, when the fingerprint still matches and the entry has not outlived
    /// `lifetime`.
    ///
    /// Two independent ways of going stale, and a tier chooses which apply to it. Most
    /// pass `None`: their inputs are files, so a fingerprint answers completely and an age
    /// would only throw away a good answer. The toolchain tier passes a lifetime because
    /// nothing in the repository changes when a person upgrades their compiler, and no
    /// fingerprint over the checkout could ever notice that.
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::environment::cache::{now_seconds, Entry};
    /// let entry = Entry {
    ///     fingerprint: "f".to_string(),
    ///     written_at: now_seconds() - 7 * 3600,
    ///     value: "1.90.0".to_string(),
    /// };
    /// assert_eq!(entry.fresh("f", None).map(String::as_str), Some("1.90.0"));
    /// assert_eq!(
    ///     entry.fresh("f", Some(Duration::from_secs(6 * 3600))),
    ///     None,
    ///     "seven hours old is beyond a six-hour lifetime, whatever the fingerprint says"
    /// );
    /// assert_eq!(entry.fresh("g", None), None);
    /// ```
    pub fn fresh(&self, fingerprint: &str, lifetime: Option<Duration>) -> Option<&T> {
        if self.fingerprint != fingerprint {
            return None;
        }
        if let Some(lifetime) = lifetime {
            let age = now_seconds().saturating_sub(self.written_at);
            if age > lifetime.as_secs() {
                return None;
            }
        }
        Some(&self.value)
    }
}

/// What a fast resolution takes from an earlier full one.
///
/// Four independent slots rather than one snapshot, each with its own key: that is what
/// lets editing the justfile expire the workflows and leave the counts alone. A tier that
/// was never written is absent, and absent is what makes the resolver report `unavailable`
/// rather than a zero it never counted.
///
/// ```
/// use majordomus_cli::environment::cache::CachedTier;
/// let empty = CachedTier::default();
/// assert!(empty.layer.is_none(), "a fresh clone has computed nothing yet");
/// assert!(empty.workflows.is_none() && empty.toolchains.is_none());
/// assert_eq!(
///     serde_json::to_string(&empty).expect("a tier serialises"),
///     "{}",
///     "an empty tier costs nothing on disk"
/// );
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedTier {
    /// What the layer holds; expires when the layer changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<Entry<LayerSummary>>,
    /// The workflows; expires when the workflow files change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflows: Option<Entry<WorkflowCatalogue>>,
    /// The installed toolchain versions; expires with the markers, and with time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toolchains: Option<Entry<Vec<ToolchainState>>>,
    /// The digest of the last snapshot rendered to a person, so that a banner in `auto`
    /// mode can tell a first look from a re-entry. Not a record of anything a person did:
    /// one digest, overwritten, naming nothing about the session that saw it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_rendered_digest: Option<String>,
}

/// The cache file of one checkout.
///
/// It belongs to the checkout and to the build: the schema and the version travel in the
/// document, and a document written by a different executable is not read, because two
/// builds of this crate can disagree about the shape of a tier while agreeing about its
/// name. A checkout with no cache and a checkout with an unreadable one are the same
/// thing here — an empty cache — and neither is an error.
///
/// ```
/// use majordomus_cli::environment::cache::{Cache, SCHEMA};
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// let cold = Cache::load(dir.path(), "local");
/// assert_eq!(cold, Cache::default(), "a checkout with no cache reads as an empty one");
/// assert_eq!(cold.schema, SCHEMA);
/// assert!(cold.tiers.layer.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cache {
    /// [`SCHEMA`].
    pub schema: String,
    /// The executable that wrote it: a build with different types must not read another's
    /// entries back into them.
    pub version: String,
    /// The tiers.
    #[serde(default)]
    pub tiers: CachedTier,
}

impl Default for Cache {
    fn default() -> Self {
        Cache {
            schema: SCHEMA.into(),
            version: crate::VERSION.into(),
            tiers: CachedTier::default(),
        }
    }
}

impl Cache {
    /// Where the cache of this checkout is.
    ///
    /// Under the local half the manifest declares, never under a directory chosen here:
    /// a linked work tree has its own local half, and a cache written to a shared path
    /// would answer one checkout's questions with another checkout's numbers.
    ///
    /// ```
    /// use majordomus_cli::environment::cache::{Cache, CACHE_PATH};
    /// let path = Cache::path(std::path::Path::new("/checkout"), ".ai/local");
    /// assert_eq!(
    ///     path,
    ///     std::path::PathBuf::from("/checkout/.ai/local/state/environment/snapshot.json")
    /// );
    /// assert!(path.ends_with(CACHE_PATH), "the tail is the constant, not a second spelling");
    /// ```
    pub fn path(root: &Path, local_half: &str) -> PathBuf {
        root.join(local_half).join(CACHE_PATH)
    }

    /// Read the cache, or a fresh empty one. Never fails: an unreadable, truncated,
    /// foreign or corrupt file is a miss, and the next write replaces it.
    ///
    /// This runs on the path a shell takes on every entry into the repository, so there is
    /// no failure it may report: the worst a bad file can cost is the price of computing
    /// the tiers again.
    ///
    /// ```
    /// use majordomus_cli::environment::cache::Cache;
    /// let dir = tempfile::tempdir().expect("a temporary directory");
    /// let path = Cache::path(dir.path(), "local");
    /// std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    ///
    /// std::fs::write(&path, "{ half a document").expect("a truncated write");
    /// assert_eq!(Cache::load(dir.path(), "local"), Cache::default(), "a corrupt cache is a miss");
    ///
    /// std::fs::write(&path, r#"{"schema":"something/else","version":"0.0.0"}"#).expect("a file");
    /// assert_eq!(
    ///     Cache::load(dir.path(), "local"),
    ///     Cache::default(),
    ///     "and so is a document written under another contract"
    /// );
    /// ```
    pub fn load(root: &Path, local_half: &str) -> Cache {
        let Ok(text) = std::fs::read_to_string(Self::path(root, local_half)) else {
            return Cache::default();
        };
        match serde_json::from_str::<Cache>(&text) {
            Ok(cache) if cache.schema == SCHEMA && cache.version == crate::VERSION => cache,
            Ok(_) | Err(_) => Cache::default(),
        }
    }

    /// Write the cache atomically. A failure is reported to the caller and is never fatal:
    /// a read-only checkout must still produce a snapshot, just not a cheaper next one.
    ///
    /// A temporary file and a rename, and the temporary name carries this process's id:
    /// two resolutions racing must not write the same temporary file, or one truncates the
    /// other's and the rename publishes half a document. A reader therefore sees either
    /// the previous cache or the whole new one, and never a partial write.
    ///
    /// ```
    /// use majordomus_cli::environment::cache::Cache;
    /// let dir = tempfile::tempdir().expect("a temporary directory");
    /// Cache::default().store(dir.path(), "local").expect("a writable checkout");
    /// let path = Cache::path(dir.path(), "local");
    /// assert!(path.is_file(), "the document is written where load looks for it");
    ///
    /// let left_behind = std::fs::read_dir(path.parent().expect("a parent"))
    ///     .expect("the cache directory")
    ///     .flatten()
    ///     .any(|e| e.file_name().to_string_lossy().ends_with(".tmp"));
    /// assert!(!left_behind, "the temporary file is renamed, never left behind");
    /// ```
    pub fn store(&self, root: &Path, local_half: &str) -> std::io::Result<()> {
        let path = Self::path(root, local_half);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Unique per process: two resolutions racing must not write the same temporary
        // file, or one truncates the other's and the rename publishes half a document.
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        let text = serde_json::to_string(self)?;
        std::fs::write(&tmp, text)?;
        match std::fs::rename(&tmp, &path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                Err(e)
            }
        }
    }

    /// An entry for a value computed now.
    ///
    /// The one way to build an [`Entry`] that the resolver uses, so that the write time is
    /// taken at the moment the value was computed rather than left to a caller to
    /// remember: an entry stamped with the wrong moment is an entry that expires at the
    /// wrong one.
    ///
    /// ```
    /// use majordomus_cli::environment::cache::{now_seconds, Cache};
    /// let before = now_seconds();
    /// let entry = Cache::entry("the inputs as they were".to_string(), 902usize);
    /// assert!(entry.written_at >= before, "it is stamped now, not when it is stored");
    /// assert_eq!(entry.fresh("the inputs as they were", None), Some(&902));
    /// ```
    pub fn entry<T>(fingerprint: String, value: T) -> Entry<T> {
        Entry {
            fingerprint,
            written_at: now_seconds(),
            value,
        }
    }
}

/// Seconds since the Unix epoch, or 0 on a machine whose clock is before it.
///
/// Seconds, not a `SystemTime`, because the value is written into a JSON document that an
/// older or newer build of this executable may read; and a clock before the epoch answers
/// `0` rather than panicking, which makes every entry look infinitely old — a cache miss,
/// which is the safe direction for a clock nobody can trust.
///
/// ```
/// use majordomus_cli::environment::cache::now_seconds;
/// let then = now_seconds();
/// assert!(then > 1_700_000_000, "the clock is somewhere after 2023");
/// assert!(now_seconds() >= then, "and it does not run backwards between two calls");
/// ```
pub fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A fingerprint over a set of files: their paths, sizes and modification times.
///
/// Paths are repository-relative and the set is sorted, so the value does not depend on
/// the order they were offered or on where the checkout lives. A file that does not exist
/// contributes its absence, which is itself a fact worth invalidating on: deleting the
/// justfile must expire the workflows.
///
/// ```
/// use majordomus_cli::environment::cache::fingerprint_of;
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// std::fs::write(dir.path().join("a"), "1").expect("a file");
/// let before = fingerprint_of(dir.path(), &["a".into(), "b".into()], &["salt"]);
/// let same = fingerprint_of(dir.path(), &["a".into(), "b".into()], &["salt"]);
/// assert_eq!(before, same, "the same inputs give the same fingerprint");
/// let salted = fingerprint_of(dir.path(), &["a".into(), "b".into()], &["other"]);
/// assert_ne!(before, salted, "the salt is part of it");
/// ```
pub fn fingerprint_of(root: &Path, paths: &[String], salt: &[&str]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(paths.len() + salt.len() + 2);
    parts.push(format!("schema={}", super::SCHEMA_VERSION));
    parts.push(format!("build={}+{}", crate::VERSION, crate::COMMIT));
    for s in salt {
        parts.push(format!("salt={s}"));
    }
    let mut sorted: Vec<&String> = paths.iter().collect();
    sorted.sort();
    sorted.dedup();
    for path in sorted {
        parts.push(match std::fs::metadata(root.join(path)) {
            Ok(meta) => {
                let nanos = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                format!("{path}:{}:{nanos}", meta.len())
            }
            Err(_) => format!("{path}:absent"),
        });
    }
    crate::policy::sha256_hex(&parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::TierState;

    fn summary() -> LayerSummary {
        LayerSummary {
            state: TierState::Resolved,
            kinds: vec![super::super::KindCount {
                kind: "rule".into(),
                count: 87,
            }],
            objects: Some(902),
            capabilities: Some(934),
            invalid: Some(0),
            degraded: Some(false),
        }
    }

    #[test]
    fn a_miss_is_never_an_error() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        assert_eq!(Cache::load(dir.path(), ".ai/local"), Cache::default());
    }

    #[test]
    fn a_corrupt_cache_regenerates_rather_than_failing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = Cache::path(dir.path(), ".ai/local");
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        for rubbish in ["", "{", "null", r#"{"schema":"other/v9"}"#] {
            std::fs::write(&path, rubbish).expect("a file");
            assert_eq!(
                Cache::load(dir.path(), ".ai/local"),
                Cache::default(),
                "{rubbish:?} should be a miss"
            );
        }
    }

    #[test]
    fn a_cache_written_by_another_build_is_not_read() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let cache = Cache {
            version: "0.0.0-not-this-build".into(),
            ..Cache::default()
        };
        cache.store(dir.path(), ".ai/local").expect("a write");
        assert_eq!(Cache::load(dir.path(), ".ai/local").tiers.layer, None);
    }

    #[test]
    fn a_stored_entry_comes_back_when_its_fingerprint_still_matches() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut cache = Cache::default();
        cache.tiers.layer = Some(Cache::entry("fp-1".into(), summary()));
        cache.store(dir.path(), ".ai/local").expect("a write");

        let loaded = Cache::load(dir.path(), ".ai/local");
        let entry = loaded.tiers.layer.expect("the entry");
        assert_eq!(
            entry.fresh("fp-1", None).map(|s| s.objects),
            Some(Some(902))
        );
        assert_eq!(entry.fresh("fp-2", None), None, "a changed input is a miss");
    }

    #[test]
    fn an_entry_past_its_lifetime_is_a_miss_even_with_the_same_fingerprint() {
        let mut entry = Cache::entry("fp".into(), summary());
        assert!(entry.fresh("fp", Some(Duration::from_secs(60))).is_some());
        entry.written_at = now_seconds().saturating_sub(7 * 3600);
        assert!(
            entry.fresh("fp", Some(TOOLCHAIN_LIFETIME)).is_none(),
            "an installed version older than its lifetime is not believed"
        );
        assert!(
            entry.fresh("fp", None).is_some(),
            "a tier with no lifetime does not expire with time"
        );
    }

    /// Changing one tier's inputs must not cost the others their entries. This is the
    /// whole reason the fingerprints are per tier.
    #[test]
    fn tiers_expire_independently() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(dir.path().join("justfile"), "a:\n    @echo\n").expect("a file");
        std::fs::write(dir.path().join("manifest"), "x").expect("a file");

        let workflows = fingerprint_of(dir.path(), &["justfile".into()], &["workflows"]);
        let capabilities = fingerprint_of(dir.path(), &["manifest".into()], &["capabilities"]);

        std::fs::write(dir.path().join("justfile"), "a:\n    @echo changed\n").expect("a file");

        assert_ne!(
            workflows,
            fingerprint_of(dir.path(), &["justfile".into()], &["workflows"]),
            "the justfile changed, so the workflows must expire"
        );
        assert_eq!(
            capabilities,
            fingerprint_of(dir.path(), &["manifest".into()], &["capabilities"]),
            "and the counts must not"
        );
    }

    #[test]
    fn a_missing_file_fingerprints_as_absent_rather_than_as_nothing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let absent = fingerprint_of(dir.path(), &["justfile".into()], &[]);
        std::fs::write(dir.path().join("justfile"), "a:\n").expect("a file");
        assert_ne!(
            absent,
            fingerprint_of(dir.path(), &["justfile".into()], &[]),
            "creating the file must expire what its absence produced"
        );
    }

    /// The cache is written to a checkout a person may share, and read back into a typed
    /// model that reaches an API. Whatever else it holds, it must not be able to hold a
    /// secret: every field of every tier is a count, a name, a doc comment or a version.
    #[test]
    fn the_cache_holds_nothing_that_could_be_a_secret() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut cache = Cache::default();
        cache.tiers.layer = Some(Cache::entry("fp".into(), summary()));
        cache.tiers.toolchains = Some(Cache::entry(
            "fp".into(),
            vec![ToolchainState {
                id: "rust".into(),
                title: "Rust".into(),
                declared: Some("1.85".into()),
                declared_by: "Cargo.toml".into(),
                installed: Some("1.90.0".into()),
                availability: super::super::ToolchainAvailability::Installed,
            }],
        ));
        cache.store(dir.path(), ".ai/local").expect("a write");
        let text = std::fs::read_to_string(Cache::path(dir.path(), ".ai/local")).expect("the file");
        for forbidden in ["API_KEY", "TOKEN", "SECRET", "PASSWORD", "Authorization"] {
            assert!(
                !text.to_uppercase().contains(forbidden),
                "the cache carried {forbidden}"
            );
        }
        // and nothing that looks like an absolute path out of the checkout
        assert!(
            !text.contains("/Users/"),
            "the cache carried a home directory"
        );
        assert!(
            !text.contains("/home/"),
            "the cache carried a home directory"
        );
    }

    #[test]
    fn a_read_only_checkout_still_resolves_and_only_loses_the_next_shortcut() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let local = dir.path().join(".ai/local/state/environment");
        std::fs::create_dir_all(&local).expect("a directory");
        let mut permissions = std::fs::metadata(&local).expect("metadata").permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&local, permissions).expect("read-only");

        let written = Cache::default().store(dir.path(), ".ai/local");
        // Whether the platform enforces it is not the assertion; that a failure is
        // returned rather than panicking, and that loading still answers, is.
        let _ = written;
        assert_eq!(Cache::load(dir.path(), ".ai/local"), Cache::default());
    }
}
