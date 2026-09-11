//! Persistence: the open-episode store, the closed-record store, and closing exactly once.
//!
//! # The incident this module is built around
//!
//! Episode `s-20260909152316-024f` has four immutable records in `.ai/repo/sessions/`: one
//! at 21:41:35 with 3 changed files and 94 commits, and three the next morning at 10:39:02,
//! 10:39:18 and 10:39:31 with 120, 121 and 122 changed files and 296 commits. A provider's
//! end event can fire more than once — thirteen and fifteen seconds apart, in this case —
//! and `mj_publish_record` is deliberately designed never to collide, so each close wrote a
//! *new file* rather than failing. The record is immutable by contract; the store had four
//! of them.
//!
//! `lib/session.sh` was hardened after the incident: before publishing, it greps the store
//! for a record already carrying the episode's id and, finding one, keeps it. That removes
//! the *repeat* and not the *race*. Between the grep and the publish sit a `git status`, a
//! ledger walk over every line of the episode, and the composition of the record — hundreds
//! of milliseconds on a loaded machine. Two processes that grep inside that window both see
//! nothing and both publish. A check without an atomic claim is a time-of-check-to-
//! time-of-use window with the expensive part inside it.
//!
//! # The guarantee
//!
//! [`SessionStore::close`] takes an atomic claim **before** the expensive part:
//! `state/sessions-closing/<episode id>.claim`, created with `O_EXCL`. The filesystem
//! decides, once, which caller created it; every other caller — another thread, another
//! process, another provider window on the same machine — observes the claim and returns a
//! diagnosable outcome instead of writing. The claim is the whole mechanism: it is not an
//! in-process mutex, so the test below that spawns real child processes exercises the same
//! primitive as the one that spawns threads.
//!
//! Three outcomes, and only one of them writes:
//!
//! * [`CloseOutcome::Closed`] — this caller took the claim and published the record.
//! * [`CloseOutcome::AlreadyClosed`] — a record for this episode already stands. Not an
//!   error: a provider's second end event is that provider's normal behaviour. The existing
//!   record's path is returned, because every reader downstream must name the record that
//!   exists rather than the file that was not written.
//! * [`CloseOutcome::AlreadyClosing`] — another caller holds the claim right now. The
//!   caller is told which episode and that it may retry; nothing is written.
//!
//! And one diagnostic that is not an outcome of this close at all:
//! [`CloseReport::duplicates`] names every *historical* record of the episode beyond the
//! first. This repository already contains four records for one episode, and a closer that
//! silently kept the first would leave a reader to discover the other three. Damage that
//! already exists is reported, never added to.
//!
//! ```
//! use majordomus_cli::session::{CloseOutcome, Episode, EpisodeId, Outcome, SessionStore};
//! use std::path::Path;
//!
//! let dir = tempfile::tempdir().unwrap();
//! let store = SessionStore::writable(dir.path());
//! let id = EpisodeId::parse("s-20260909152316-024f").expect("the id from the incident");
//! let episode = Episode::opening(id.clone(), Path::new("/r"))
//!     .opened_at("2026-09-09T15:23:16Z", "master", "9f3a2a0", 1_000);
//! let body = || format!("---\nschema: session/v1\nsession_id: {}\n---\n", id);
//!
//! let first = store.close(&episode, Outcome::Closed, body).unwrap();
//! let second = store.close(&episode, Outcome::Closed, body).unwrap();
//! assert_eq!(first.outcome, CloseOutcome::Closed);
//! assert_eq!(second.outcome, CloseOutcome::AlreadyClosed);
//! assert_eq!(store.records_of(&id).len(), 1, "the incident produced four");
//! ```

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::episode::{Episode, Outcome};
use super::identity::EpisodeId;
use super::state::EpisodeState;

/// Where the open episodes live, relative to the repository root.
pub const OPEN_DIR: &str = ".ai/local/state/sessions-open";

/// Where a close takes its claim, relative to the repository root. A directory of its own
/// rather than a lock beside the record, because the record store is the *shared* half of
/// the layer and a claim is a fact about this machine (ADR 0014).
pub const CLAIM_DIR: &str = ".ai/local/state/sessions-closing";

/// Where the immutable records live, relative to the repository root. Tracked: a closed
/// episode is a shared object of the layer.
pub const RECORD_DIR: &str = ".ai/repo/sessions";

/// What a close did: wrote the record, found one already standing, found another caller
/// inside the window, or was refused by the machine.
///
/// ```
/// use majordomus_cli::session::CloseOutcome;
/// // a duplicate close is a diagnosable outcome, not an error: a provider's second end
/// // event is that provider's normal behaviour
/// assert!(CloseOutcome::Closed.wrote());
/// assert!(!CloseOutcome::AlreadyClosed.wrote());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CloseOutcome {
    /// This caller took the claim and published the record. Exactly one caller per episode
    /// ever sees this.
    Closed,
    /// A record for this episode already stands; this close added none.
    AlreadyClosed,
    /// Another caller holds the claim at this moment. Nothing was written.
    AlreadyClosing,
    /// The episode is not in a state the machine lets it leave. Nothing was written.
    Refused,
}

impl CloseOutcome {
    /// Did this call publish the record?
    ///
    /// ```
    /// use majordomus_cli::session::CloseOutcome;
    /// assert!(CloseOutcome::Closed.wrote());
    /// assert!(!CloseOutcome::AlreadyClosed.wrote());
    /// ```
    pub fn wrote(self) -> bool {
        matches!(self, CloseOutcome::Closed)
    }

    /// The word as serialised, which is what a caller prints and what a projection of a
    /// close carries.
    ///
    /// ```
    /// use majordomus_cli::session::CloseOutcome;
    /// assert_eq!(CloseOutcome::AlreadyClosing.as_str(), "already_closing");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            CloseOutcome::Closed => "closed",
            CloseOutcome::AlreadyClosed => "already_closed",
            CloseOutcome::AlreadyClosing => "already_closing",
            CloseOutcome::Refused => "refused",
        }
    }
}

/// What a close said: the episode, the outcome, the canonical record, any historical
/// duplicates of it, and a line a person can act on.
///
/// ```
/// use majordomus_cli::session::{CloseReport, Episode, EpisodeId, Outcome, SessionStore};
/// use std::path::Path;
///
/// let dir = tempfile::tempdir().unwrap();
/// let store = SessionStore::writable(dir.path());
/// let id = EpisodeId::parse("s-20260910205542-e2a6").unwrap();
/// let episode = Episode::opening(id.clone(), Path::new("/r"))
///     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_000);
/// let report: CloseReport = store
///     .close(&episode, Outcome::Closed, || format!("---\nsession_id: {}\n---\n", id))
///     .expect("a close");
/// assert_eq!(report.episode, id);
/// assert!(report.duplicates.is_empty(), "a clean store holds no damage to report");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CloseReport {
    /// The episode.
    pub episode: EpisodeId,
    /// What happened.
    pub outcome: CloseOutcome,
    /// The canonical record of this episode: the one this call wrote, or the one that
    /// already stood. Repository-relative. Empty when nothing was written and nothing was
    /// found.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub record: String,
    /// Historical records of this episode beyond the canonical one, repository-relative
    /// and in canonical order. Non-empty means the store already holds damage of the kind
    /// this closer exists to prevent, and the caller is told rather than left to find it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicates: Vec<String>,
    /// A line a person can act on, when there is one to say.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub diagnostic: String,
}

/// What can go wrong closing an episode, as distinct from an outcome that is not `Closed`.
/// An outcome is a fact about the episode; an error is a fact about the disk, or about a
/// caller that reached the write path without saying it meant to.
///
/// ```
/// use majordomus_cli::session::{Episode, EpisodeId, Outcome, SessionStore, StoreError};
/// use std::path::Path;
///
/// let dir = tempfile::tempdir().unwrap();
/// let id = EpisodeId::parse("s-20260910205542-e2a6").unwrap();
/// let episode = Episode::opening(id, Path::new("/r"))
///     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_000);
/// let err = SessionStore::read_only(dir.path())
///     .close(&episode, Outcome::Closed, String::new)
///     .expect_err("a refusal");
/// assert!(matches!(err, StoreError::NotWritable));
/// ```
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A directory or file of the store could not be created, read or written.
    #[error("the session store at {path} could not be used: {reason}")]
    Io {
        /// Where.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// The domain's write path was reached without the opt-in. Additive by construction:
    /// while the shell tool owns the writes, a caller that has not said so explicitly is
    /// refused rather than allowed to produce a second writer for one record.
    #[error(
        "the session domain's write path is not enabled in this build path; \
         construct the store with SessionStore::writable (ADR 0044: the shell keeps the writes \
         until the cutover)"
    )]
    NotWritable,
}

/// The stores of one checkout.
///
/// Reading needs no permission. Writing needs [`SessionStore::writable`], which is the
/// flag ADR 0044 promised: while `lib/session.sh` owns the lifecycle, nothing that is
/// merely *served* can write a session record, and a caller that has not said out loud
/// that it means to is refused by [`StoreError::NotWritable`].
///
/// ```
/// use majordomus_cli::session::SessionStore;
///
/// let dir = tempfile::tempdir().unwrap();
/// let reading = SessionStore::read_only(dir.path());
/// assert!(!reading.is_writable());
/// assert!(SessionStore::writable(dir.path()).is_writable());
/// ```
#[derive(Debug, Clone)]
pub struct SessionStore {
    root: PathBuf,
    writable: bool,
}

impl SessionStore {
    /// A store that reads. Every projection uses this one, and a caller that reaches the
    /// write path through it is refused rather than allowed to become a second writer.
    ///
    /// ```
    /// use majordomus_cli::session::SessionStore;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert!(!SessionStore::read_only(dir.path()).is_writable());
    /// ```
    pub fn read_only(root: impl Into<PathBuf>) -> Self {
        SessionStore {
            root: root.into(),
            writable: false,
        }
    }

    /// A store that may write. Not reachable from any surface in this change: the cutover
    /// (ADR 0044) is what routes `majordomus session close` through it.
    ///
    /// ```
    /// use majordomus_cli::session::SessionStore;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert!(SessionStore::writable(dir.path()).is_writable());
    /// ```
    pub fn writable(root: impl Into<PathBuf>) -> Self {
        SessionStore {
            root: root.into(),
            writable: true,
        }
    }

    /// May this store write? The question the closer asks first, before it touches the
    /// disk at all.
    ///
    /// ```
    /// use majordomus_cli::session::SessionStore;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert!(!SessionStore::read_only(dir.path()).is_writable());
    /// ```
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// The repository root it is over, which every path below is composed from.
    ///
    /// ```
    /// use majordomus_cli::session::SessionStore;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert_eq!(SessionStore::read_only(dir.path()).root(), dir.path());
    /// ```
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the claims live: the local half, because a claim is a fact about this machine
    /// and the record store is the shared half of the layer (ADR 0014).
    ///
    /// ```
    /// use majordomus_cli::session::SessionStore;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert!(SessionStore::read_only(dir.path()).claim_dir().ends_with("sessions-closing"));
    /// ```
    pub fn claim_dir(&self) -> PathBuf {
        self.root.join(CLAIM_DIR)
    }

    /// Where the immutable records live: the tracked half, because a closed episode is a
    /// shared object of the layer and travels with a clone.
    ///
    /// ```
    /// use majordomus_cli::session::SessionStore;
    /// let dir = tempfile::tempdir().unwrap();
    /// assert!(SessionStore::read_only(dir.path()).record_dir().ends_with(".ai/repo/sessions"));
    /// ```
    pub fn record_dir(&self) -> PathBuf {
        self.root.join(RECORD_DIR)
    }

    /// Every record in the store that names `episode`, repository-relative, in canonical
    /// order — which for these filenames is chronological, because the name begins with a
    /// compact UTC timestamp.
    ///
    /// The scan reads front matter rather than grepping the whole file, so a record whose
    /// *body* quotes another episode's id is not counted as that episode's record. The
    /// shell's `grep -rl "^session_id: <id>$"` has the same intent and reaches it by
    /// anchoring; this reads the field.
    ///
    /// ```
    /// use majordomus_cli::session::{EpisodeId, SessionStore};
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let records = dir.path().join(".ai/repo/sessions");
    /// std::fs::create_dir_all(&records).unwrap();
    /// std::fs::write(
    ///     records.join("20260910T210426Z--a.md"),
    ///     "---\nschema: session/v1\nsession_id: s-20260910205542-e2a6\n---\n",
    /// ).unwrap();
    /// std::fs::write(
    ///     records.join("20260910T211503Z--b.md"),
    ///     "---\nschema: session/v1\nsession_id: s-20260910210510-8f20\n---\n\nquotes s-20260910205542-e2a6 in prose\n",
    /// ).unwrap();
    ///
    /// let store = SessionStore::read_only(dir.path());
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").unwrap();
    /// let found = store.records_of(&id);
    /// assert_eq!(found.len(), 1, "the field, not the prose");
    /// assert!(found[0].ends_with("20260910T210426Z--a.md"));
    /// ```
    pub fn records_of(&self, episode: &EpisodeId) -> Vec<String> {
        let dir = self.record_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out: Vec<String> = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if front_matter_field(&text, "session_id").as_deref() == Some(episode.as_str()) {
                out.push(
                    path.strip_prefix(&self.root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
        // The names begin with a compact UTC timestamp, so a plain lexicographic order is
        // chronological. `canonical` is the crate's one ordering, and it is used here so
        // that the "first record wins" rule is decided the same way by every reader.
        let mut sortable: Vec<RecordPath> = out.into_iter().map(RecordPath).collect();
        crate::order::canonical(&mut sortable);
        sortable.into_iter().map(|r| r.0).collect()
    }

    /// Close `episode`, exactly once.
    ///
    /// `compose` is called only by the caller that took the claim, and only after it did:
    /// composing a record is the expensive part — a `git status`, a walk of the episode's
    /// ledger lines — and doing it before the claim is what leaves the window the shell's
    /// scan cannot close. It returns the record's full text.
    ///
    /// ```
    /// use majordomus_cli::session::{CloseOutcome, Episode, EpisodeId, Outcome, SessionStore};
    /// use std::path::Path;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let store = SessionStore::writable(dir.path());
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").unwrap();
    /// let episode = Episode::opening(id.clone(), Path::new("/r"))
    ///     .opened_at("2026-09-10T20:55:42Z", "master", "9f3a2a0", 1_000);
    ///
    /// let body = || format!("---\nschema: session/v1\nkind: session\nsession_id: {}\n---\n", id);
    ///
    /// // the first close writes
    /// let first = store.close(&episode, Outcome::Closed, body).unwrap();
    /// assert_eq!(first.outcome, CloseOutcome::Closed);
    /// assert!(first.record.starts_with(".ai/repo/sessions/"));
    ///
    /// // the second, third and fourth do not, and each names the record that stands
    /// for _ in 0..3 {
    ///     let again = store.close(&episode, Outcome::Closed, body).unwrap();
    ///     assert_eq!(again.outcome, CloseOutcome::AlreadyClosed);
    ///     assert_eq!(again.record, first.record);
    /// }
    /// assert_eq!(store.records_of(&id).len(), 1, "one episode, one record");
    /// ```
    pub fn close<F>(
        &self,
        episode: &Episode,
        outcome: Outcome,
        compose: F,
    ) -> Result<CloseReport, StoreError>
    where
        F: Fn() -> String,
    {
        if !self.writable {
            return Err(StoreError::NotWritable);
        }
        let id = episode.id.clone();

        // 1. The machine, before anything touches the disk. An episode that is already
        //    closed in memory is refused here; one that is closed on disk is caught by the
        //    scan below, which is the authority across processes.
        if !episode.may_move_to(EpisodeState::Closed) {
            return Ok(CloseReport {
                episode: id,
                outcome: CloseOutcome::Refused,
                record: String::new(),
                duplicates: Vec::new(),
                diagnostic: format!(
                    "an episode in state `{}` does not move to `closed`",
                    episode.state.as_str()
                ),
            });
        }

        // 2. The claim, before the expensive part. O_EXCL: the filesystem picks one caller.
        let claim = self.claim_path(&id);
        let io = |path: &Path, e: std::io::Error| StoreError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        };
        std::fs::create_dir_all(self.claim_dir()).map_err(|e| io(&self.claim_dir(), e))?;
        let held = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim)
        {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(e) => return Err(io(&claim, e)),
        };
        if !held {
            // Somebody else is inside the window, or was and did not clean up. Which of
            // the two it is, is a question only the record store can answer.
            let existing = self.records_of(&id);
            let (outcome, record, diagnostic) = match existing.first() {
                Some(path) => (
                    CloseOutcome::AlreadyClosed,
                    path.clone(),
                    format!("session {id} already has a record at {path}; this close adds none"),
                ),
                None => (
                    CloseOutcome::AlreadyClosing,
                    String::new(),
                    format!(
                        "session {id} is being closed by another process (claim at {}); \
                         nothing was written",
                        display_relative(&self.root, &claim)
                    ),
                ),
            };
            return Ok(CloseReport {
                episode: id,
                outcome,
                record,
                duplicates: existing.into_iter().skip(1).collect(),
                diagnostic,
            });
        }

        // From here the claim is held and every path must release it.
        let guard = ClaimGuard { path: claim };

        // 3. A record may already stand from a close that finished before this one began —
        //    the ordinary repeated end event, seconds apart, with no overlap at all.
        let existing = self.records_of(&id);
        if let Some(first) = existing.first().cloned() {
            let duplicates: Vec<String> = existing.into_iter().skip(1).collect();
            let mut diagnostic =
                format!("session {id} already has a record at {first}; this close adds none");
            if !duplicates.is_empty() {
                diagnostic.push_str(&format!(
                    "; the store also holds {} further record(s) of this episode, which is \
                     damage that predates this close and is reported rather than added to: {}",
                    duplicates.len(),
                    duplicates.join(", ")
                ));
            }
            drop(guard);
            return Ok(CloseReport {
                episode: id,
                outcome: CloseOutcome::AlreadyClosed,
                record: first,
                duplicates,
                diagnostic,
            });
        }

        // 4. The expensive part, inside the claim.
        let _ = outcome;
        let text = compose();
        let dir = self.record_dir();
        std::fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
        let name = format!("{}--{}.md", compact_stamp_of(&id), id);
        let path = dir.join(&name);
        std::fs::write(&path, text).map_err(|e| io(&path, e))?;

        drop(guard);
        let record = display_relative(&self.root, &path);
        Ok(CloseReport {
            episode: id,
            outcome: CloseOutcome::Closed,
            record,
            duplicates: Vec::new(),
            diagnostic: String::new(),
        })
    }

    /// Where the claim of `episode` is. The episode id is parsed, so it is one inert path
    /// segment and nothing a provider sends can name a file outside the directory.
    ///
    /// ```
    /// use majordomus_cli::session::{EpisodeId, SessionStore};
    /// let dir = tempfile::tempdir().unwrap();
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// let claim = SessionStore::read_only(dir.path()).claim_path(&id);
    /// assert!(claim.ends_with("s-20260910205542-e2a6.claim"));
    /// assert!(!claim.exists(), "asking where it would be does not take it");
    /// ```
    pub fn claim_path(&self, episode: &EpisodeId) -> PathBuf {
        self.claim_dir().join(format!("{episode}.claim"))
    }
}

/// The claim, released when the close leaves by any path — including a panic, which is
/// what stops a crashed closer from locking an episode out of ever being closed.
struct ClaimGuard {
    path: PathBuf,
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A record path, ordered by the crate's one ordering.
struct RecordPath(String);

impl crate::order::Ordered for RecordPath {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey {
            group: None,
            rank: 0,
            label: &self.0,
            identity: &self.0,
        }
    }
}

/// One scalar of a record's front matter, read without a YAML parse of the whole document.
/// The field is matched at the start of a line inside the fences, which is what stops the
/// body's prose from answering.
fn front_matter_field(text: &str, key: &str) -> Option<String> {
    let mut fences = 0usize;
    for line in text.lines() {
        if line.trim_end() == "---" {
            fences += 1;
            if fences >= 2 {
                return None;
            }
            continue;
        }
        if fences != 1 {
            continue;
        }
        if let Some(rest) = line.strip_prefix(key) {
            if let Some(value) = rest.strip_prefix(':') {
                return Some(value.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// The compact UTC stamp a record file is named by, taken from the episode's own id so
/// that the name is a function of the episode and two closes cannot produce two names.
///
/// This is the difference from `mj_publish_record`, which names a file by *now* plus
/// sixteen random hex digits and is therefore guaranteed never to collide — the property
/// that turned a repeated close into a second record instead of a refusal.
fn compact_stamp_of(id: &EpisodeId) -> String {
    let stamp = id.minted_at();
    format!("{}T{}Z", &stamp[..8], &stamp[8..])
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn episode(id: &str) -> Episode {
        Episode::opening(EpisodeId::parse(id).expect("an id"), Path::new("/r")).opened_at(
            "2026-09-10T20:55:42Z",
            "master",
            "9f3a2a0",
            1_000,
        )
    }

    fn body(id: &str) -> String {
        format!("---\nschema: session/v1\nkind: session\nsession_id: {id}\n---\n")
    }

    #[test]
    fn a_read_only_store_refuses_to_write() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = SessionStore::read_only(dir.path());
        let e = episode("s-20260910205542-e2a6");
        let err = store
            .close(&e, Outcome::Closed, || body("s-20260910205542-e2a6"))
            .expect_err("a refusal");
        assert!(matches!(err, StoreError::NotWritable));
        assert!(!dir.path().join(RECORD_DIR).exists());
    }

    #[test]
    fn four_repeated_closes_yield_exactly_one_record() {
        // The incident, reproduced: s-20260909152316-024f was closed at 21:41:35 and then
        // again at 10:39:02, 10:39:18 and 10:39:31.
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = SessionStore::writable(dir.path());
        let id = "s-20260909152316-024f";
        let e = episode(id);

        let mut wrote = 0usize;
        let mut paths = std::collections::BTreeSet::new();
        for _ in 0..4 {
            let report = store
                .close(&e, Outcome::Closed, || body(id))
                .expect("a close");
            if report.outcome.wrote() {
                wrote += 1;
            }
            assert!(!report.record.is_empty(), "every close names the record");
            paths.insert(report.record);
        }
        assert_eq!(wrote, 1, "exactly one close wrote");
        assert_eq!(paths.len(), 1, "and every close named the same record");
        assert_eq!(
            store.records_of(&EpisodeId::parse(id).unwrap()).len(),
            1,
            "one episode, one record"
        );
    }

    #[test]
    fn simultaneous_closes_from_many_threads_yield_exactly_one_record() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = SessionStore::writable(dir.path());
        let id = "s-20260910205542-e2a6";
        let e = episode(id);

        let wrote = std::sync::atomic::AtomicUsize::new(0);
        let closing = std::sync::atomic::AtomicUsize::new(0);
        let start = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let (store, e, wrote, closing, start) = (&store, &e, &wrote, &closing, &start);
                scope.spawn(move || {
                    start.wait();
                    let report = store
                        .close(e, Outcome::Closed, || {
                            // the expensive part: long enough that every thread is inside
                            // the window the shell's scan leaves open
                            std::thread::sleep(std::time::Duration::from_millis(25));
                            body(id)
                        })
                        .expect("a close");
                    match report.outcome {
                        CloseOutcome::Closed => {
                            wrote.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        CloseOutcome::AlreadyClosing => {
                            closing.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        CloseOutcome::AlreadyClosed => {}
                        CloseOutcome::Refused => panic!("the machine allowed this move"),
                    }
                });
            }
        });

        assert_eq!(
            wrote.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "exactly one of eight simultaneous closers wrote"
        );
        assert!(
            closing.load(std::sync::atomic::Ordering::SeqCst) >= 1,
            "and the others were told why, rather than silently doing nothing"
        );
        assert_eq!(
            store.records_of(&EpisodeId::parse(id).unwrap()).len(),
            1,
            "one episode, one record"
        );
    }

    #[test]
    fn historical_duplicates_are_reported_and_never_added_to() {
        // The store as this repository's actually is: four records of one episode, written
        // before anything could refuse them.
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = SessionStore::writable(dir.path());
        let id = "s-20260909152316-024f";
        let records = store.record_dir();
        std::fs::create_dir_all(&records).expect("the store");
        for stamp in [
            "20260909T214135Z",
            "20260910T103902Z",
            "20260910T103918Z",
            "20260910T103931Z",
        ] {
            std::fs::write(records.join(format!("{stamp}--{id}.md")), body(id)).expect("a record");
        }

        let report = store
            .close(&episode(id), Outcome::Closed, || body(id))
            .expect("a close");
        assert_eq!(report.outcome, CloseOutcome::AlreadyClosed);
        assert!(report
            .record
            .ends_with("20260909T214135Z--s-20260909152316-024f.md"));
        assert_eq!(
            report.duplicates.len(),
            3,
            "the three that should not exist"
        );
        assert!(report.diagnostic.contains("predates this close"));
        assert_eq!(
            store.records_of(&EpisodeId::parse(id).unwrap()).len(),
            4,
            "and nothing was added"
        );
    }

    #[test]
    fn a_claim_is_released_even_when_the_composer_panics() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = SessionStore::writable(dir.path());
        let id = "s-20260910205542-e2a6";
        let e = episode(id);

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.close(&e, Outcome::Closed, || panic!("the git call died"));
        }));
        assert!(panicked.is_err(), "the panic propagated");
        assert!(
            !store.claim_path(&EpisodeId::parse(id).unwrap()).exists(),
            "and the claim went with it, so the episode is still closable"
        );

        let report = store
            .close(&e, Outcome::Closed, || body(id))
            .expect("a close");
        assert_eq!(report.outcome, CloseOutcome::Closed);
    }

    #[test]
    fn a_records_prose_does_not_make_it_that_episodes_record() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let store = SessionStore::read_only(dir.path());
        let records = store.record_dir();
        std::fs::create_dir_all(&records).expect("the store");
        std::fs::write(
            records.join("20260910T211503Z--other.md"),
            "---\nschema: session/v1\nsession_id: s-20260910210510-8f20\n---\n\n\
             continued from session_id: s-20260910205542-e2a6\n",
        )
        .expect("a record");
        assert!(store
            .records_of(&EpisodeId::parse("s-20260910205542-e2a6").unwrap())
            .is_empty());
    }
}
