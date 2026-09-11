//! The session domain: one typed account of what an episode is, what identifies it, how it
//! moves, and how it is closed exactly once.
//!
//! # Why this module exists
//!
//! This repository runs two programs over one subsystem. The shell tool (`lib/session.sh`,
//! `lib/capture.sh`, `lib/checkpoint.sh`, `lib/handover.sh`) owns the writes, the lifecycle
//! and the state machine; this executable owns the typed read model, the server, the HTTP
//! API, MCP and the Cockpit. [`crate::capability::builtin::continuity`] opens by declaring
//! the split — *"It is read, never written. The lifecycle is the shell tool's."*
//!
//! ADR 0041 found what the split costs. For six days this repository wrote no checkpoint
//! and no handover while `doctor` and `watch` reported health, because the invariant that
//! would have caught it — *lifecycle events are arriving, therefore derived state must
//! advance* — spans both halves and is owned by neither. The reader faithfully reported the
//! newest record it could find; the writer faithfully refused to write one. ADR 0041 names
//! the target: **one canonical session/continuity domain service in this crate**, with
//! `.envrc`, the provider hook shims, the CLI, MCP, HTTP and the Cockpit as adapters over
//! it. This module is that service's domain.
//!
//! # What this change is, and what it is not
//!
//! It is **additive**. The shell keeps every write it has. Nothing here changes the
//! behaviour of the running lifecycle: the projections are read-only queries, and the write
//! path ([`SessionStore::close`]) exists, is tested, and is reachable only through
//! [`SessionStore::writable`] — a constructor no surface calls. ADR 0047 records the cutover
//! and its order. Four workers are editing `lib/*.sh` as this is written, and a cutover in
//! the same change would be a merge whose conflicts are semantic rather than textual, in the
//! one subsystem where a wrong merge is invisible until somebody resumes from a record that
//! is not theirs.
//!
//! # The four things it settles
//!
//! **[Identity](identity).** Six things the stores spell as three words become six types
//! with no conversion between them. [`RepositoryId`] and [`CheckoutId`] wrap the digests
//! this crate already computes for its lease, index and worktree topology, rather than
//! inventing a seventh notion of "which repository".
//!
//! **[The aggregates](episode).** An [`Episode`] carries `Option<TaskId>` and nothing else
//! of a task. ADR 0041's defect — a task's state deciding whether an episode's artefact is
//! written — is unrepresentable, because [`EpisodeState::may_move_to`] takes no task.
//!
//! **[The machine](state).** Four states, six transitions, one terminal state, declared
//! once and projected as data by `session.machine` so that no surface draws its own.
//!
//! **[Persistence](store).** Closing takes an `O_EXCL` claim before the expensive part, so
//! four repeated closes and eight simultaneous ones yield exactly one record. The episode
//! that has four records in this repository is the reason.
//!
//! **[Events](ledger).** Typed, over the ledger that already exists, against the vocabulary
//! `share/events.yaml` already declares. No second store: locked append-only JSONL meets
//! concurrency, corruption detection and recovery, and ADR 0047 records why a database was
//! refused rather than merely not chosen.
//!
//! # Freshness is consumed, not restated
//!
//! `fresh | aging | stale | unknown | invalid`, `Thresholds::judge` and `epoch_seconds`
//! arrive with ADR 0041 in `continuity.rs`. This domain does not define a second vocabulary
//! for age: it takes Unix seconds and answers the single predicate the machine needs,
//! [`Episode::stranded_after`].
//!
//! ```
//! use majordomus_cli::session::{EpisodeState, Machine, open_episodes};
//!
//! // the machine is derived from the types and has one terminal state
//! let machine = Machine::describe();
//! assert_eq!(machine.states.len(), EpisodeState::ALL.len());
//! assert_eq!(machine.states.iter().filter(|s| s.terminal).count(), 1);
//!
//! // and a checkout that has never run the lifecycle reports absence, not a fault
//! let dir = tempfile::tempdir().unwrap();
//! assert!(open_episodes(dir.path()).is_empty());
//! ```

pub mod episode;
pub mod identity;
pub mod ledger;
pub mod state;
pub mod store;

pub use episode::{Episode, Outcome};
pub use identity::{
    CheckoutId, EpisodeId, IdentityFacet, ProviderSessionId, RecordedSpelling, RepositoryId,
    Spelling, TaskId, WorkerId,
};
pub use ledger::{
    Corruption, DeclaredEvent, Envelope, Ledger, LedgerError, LedgerEvent, LedgerRead, Vocabulary,
};
pub use state::{EpisodeState, Machine, StateView, Transition, TransitionView};
pub use store::{CloseOutcome, CloseReport, SessionStore, StoreError};

use std::path::Path;

use crate::metadata::yaml;

/// Every event name this domain writes.
///
/// It is a list of *names*, not of declarations: `share/events.yaml` is the vocabulary and
/// this is the domain's claim on part of it. A test in [`ledger`] asserts that every name
/// here is declared there, which is what fails when somebody adds an event and forgets the
/// vocabulary — rather than a durable line every reader silently ignores, which is what
/// `mj_ledger_append` produced before the vocabulary existed.
///
/// ```
/// use majordomus_cli::session::WRITTEN_EVENTS;
/// assert!(WRITTEN_EVENTS.contains(&"session.closed"));
/// ```
pub const WRITTEN_EVENTS: &[&str] = &["session.started", "session.closed"];

/// The open episodes of one checkout, as the store holds them.
///
/// Read from `.ai/local/state/sessions-open/`, which is a file per provider session rather
/// than a singleton: an episode used to be one `session-current.yaml` per checkout, and a
/// second provider session opening one was folded into the first. Measured in this
/// repository on 2026-09-09: seven concurrent sessions, one record between them.
///
/// `last_sign_of_life` is the later of the episode's `started_at` and the newest ledger
/// line it stamped, which is what makes "nobody has been here for a day" a measurement
/// rather than an inference from the open record's own age.
///
/// ```
/// use majordomus_cli::session::open_episodes;
///
/// let dir = tempfile::tempdir().unwrap();
/// let open = dir.path().join(".ai/local/state/sessions-open");
/// std::fs::create_dir_all(&open).unwrap();
/// std::fs::write(
///     open.join("01Bv2gJsf.yaml"),
///     "session_id: s-20260910205542-e2a6\nstarted_at: 2026-09-10T20:55:42Z\nowner: \"korczis\"\n\
///      provider: \"claude-code\"\nprovider_session: \"01Bv2gJsf\"\nbranch: master\nstart_head: 9f3a2a0\n",
/// ).unwrap();
///
/// let episodes = open_episodes(dir.path());
/// assert_eq!(episodes.len(), 1);
/// assert_eq!(episodes[0].id.as_str(), "s-20260910205542-e2a6");
/// assert!(episodes[0].task.is_none(), "an episode without a task is ordinary");
/// assert_eq!(episodes[0].provider_session.as_ref().map(|p| p.as_str()), Some("01Bv2gJsf"));
/// assert_eq!(episodes[0].last_sign_of_life, Some(1_789_073_742));
/// ```
pub fn open_episodes(root: &Path) -> Vec<Episode> {
    let dir = root.join(store::OPEN_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let stamps = ledger_stamps(root);
    let mut out: Vec<Episode> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(map) = yaml::parse_mapping(&text) else {
            continue;
        };
        let s = |k: &str| map.get(k).and_then(yaml::scalar_string).unwrap_or_default();
        let Some(id) = EpisodeId::parse(&s("session_id")) else {
            continue;
        };
        let started_at = s("started_at");
        let provider_session = {
            let raw = s("provider_session");
            (!raw.is_empty()).then(|| ProviderSessionId::new(raw))
        };
        let opened = crate::peers::epoch_seconds(&started_at);
        let newest = stamps
            .iter()
            .filter(|(episode, _)| episode == id.as_str())
            .filter_map(|(_, at)| *at)
            .max();
        let last = match (opened, newest) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let mut episode = Episode {
            id,
            state: EpisodeState::Open,
            repository: RepositoryId::of(root),
            checkout: CheckoutId::of(root),
            provider: s("provider"),
            provider_session,
            worker: WorkerId::supplied(&s("worker")),
            task: None,
            started_at,
            branch: s("branch"),
            start_head: s("start_head"),
            last_sign_of_life: last,
        };
        // The task, when the checkout has one open. A relation the episode carries; its
        // absence is ordinary and is never a reason to skip anything (ADR 0041).
        if let Some(task) = active_task(root) {
            episode.task = Some(task);
        }
        out.push(episode);
    }
    crate::order::canonical(&mut out);
    out
}

/// The task this checkout has open, when it has one. Read from `state/current.yaml`, the
/// active task record — which `lib/start.sh` writes and check, checkpoint, handover and
/// finish update.
///
/// It reads the id and nothing else. An episode relates to a task; it does not hold one,
/// and a copy of the task's outcome on this side would be the second account that ADR 0041
/// spent six days of silence proving is a defect.
fn active_task(root: &Path) -> Option<TaskId> {
    let text = std::fs::read_to_string(root.join(".ai/local/state/current.yaml")).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    TaskId::parse(&map.get("id").and_then(yaml::scalar_string)?)
}

/// The URL of `origin`, which is what `mj_repository_id` writes into a shared record.
///
/// A subprocess, and deliberately not a read of `.git/config`: a linked worktree's config
/// is the common one and an `includeIf` can put the remote somewhere else, so asking git is
/// the only answer that is right in every checkout this tool runs in. One call per reading
/// of a capability whose cache is two seconds wide.
fn remote_url(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// `(episode id, its timestamp in Unix seconds)` for every ledger line that carries one.
///
/// Selection is by the session id the writer stamped on each line, never by a time range.
/// The ledger is one file per repository; a window of "everything after this episode
/// opened" collects another worker's events, because two sessions write to one repository
/// at once and nothing in a timestamp can tell them apart.
fn ledger_stamps(root: &Path) -> Vec<(String, Option<i64>)> {
    Ledger::of(root)
        .read()
        .events
        .iter()
        .filter_map(|line| {
            let obj = line.as_object()?;
            let session = obj.get("session")?.as_str()?.to_string();
            let ts = obj
                .get("ts")
                .and_then(|v| v.as_str())
                .and_then(crate::peers::epoch_seconds);
            Some((session, ts))
        })
        .collect()
}

/// How this checkout spells each of the identities the audit found conflated, beside the
/// canonical value this executable computes.
///
/// The point is not to choose a winner. It is to stop pretending the spellings are one
/// value: a reader that knows a string is the *local* spelling can compare it with other
/// local spellings and refuse to compare it with a shared one, which is exactly the
/// comparison that made two records of one repository, written four minutes apart, look
/// like records of two repositories.
///
/// ```
/// use majordomus_cli::session::identities;
///
/// let dir = tempfile::tempdir().unwrap();
/// let facets = identities(dir.path());
/// let subjects: Vec<&str> = facets.iter().map(|f| f.subject.as_str()).collect();
/// assert_eq!(subjects, ["repository", "checkout", "episode", "provider_session"]);
///
/// // a plain directory belongs to no git repository, and that is reported as absence
/// assert!(facets[0].canonical.is_empty());
/// // a checkout always has an identity: the question has to be answerable for a record
/// // written on a disk that has since been unmounted
/// assert_eq!(facets[1].canonical.len(), 32);
/// ```
pub fn identities(root: &Path) -> Vec<IdentityFacet> {
    let git = crate::repository::git_identity(root);
    let mut repository_spellings = vec![
        RecordedSpelling {
            spelling: Spelling::Local,
            writer: "mj_git_repo_id (lib/common.sh)".to_string(),
            value: git
                .as_ref()
                .map(|g| g.common_dir.display().to_string())
                .unwrap_or_default(),
        },
        RecordedSpelling {
            spelling: Spelling::Shared,
            writer: "mj_repository_id (lib/common.sh)".to_string(),
            value: remote_url(root).unwrap_or_default(),
        },
    ];
    crate::order::canonical(&mut repository_spellings);

    let mut checkout_spellings = vec![
        RecordedSpelling {
            spelling: Spelling::Local,
            writer: "mj_record_front_matter `worktree:` (lib/common.sh)".to_string(),
            value: root.display().to_string(),
        },
        RecordedSpelling {
            spelling: Spelling::Shared,
            writer: "mj_worktree_id `worktree_id:` (lib/common.sh)".to_string(),
            value: String::new(),
        },
    ];
    crate::order::canonical(&mut checkout_spellings);

    let open = open_episodes(root);
    let episode_spellings = open
        .first()
        .map(|e| {
            vec![RecordedSpelling {
                spelling: Spelling::Local,
                writer: "session_id (state/sessions-open/<key>.yaml)".to_string(),
                value: e.id.as_str().to_string(),
            }]
        })
        .unwrap_or_default();
    let provider_spellings = open
        .first()
        .and_then(|e| e.provider_session.clone())
        .map(|p| {
            vec![RecordedSpelling {
                spelling: Spelling::Local,
                writer: "provider_session (state/sessions-open/<key>.yaml)".to_string(),
                value: p.as_str().to_string(),
            }]
        })
        .unwrap_or_default();

    vec![
        IdentityFacet {
            subject: "repository".to_string(),
            canonical: git.map(|g| g.id).unwrap_or_default(),
            spellings: repository_spellings,
            note: "one value for every checkout of one repository. The local half writes a \
                   path and the shared half writes a remote URL; neither can equal the other, \
                   so a reader that compares them concludes two records of one repository are \
                   about two repositories."
                .to_string(),
        },
        IdentityFacet {
            subject: "checkout".to_string(),
            canonical: CheckoutId::of(root).as_str().to_string(),
            spellings: checkout_spellings,
            note: "one worktree. Two checkouts of one repository share a repository identity \
                   and differ here, which is the first tier of every record resolution in the \
                   tool: a record from another worktree is never silently offered as your \
                   context."
                .to_string(),
        },
        IdentityFacet {
            subject: "episode".to_string(),
            canonical: open
                .first()
                .map(|e| e.id.as_str().to_string())
                .unwrap_or_default(),
            spellings: episode_spellings,
            note: "the canonical session identity and the only one. Minted at open, stamped \
                   on every ledger line of the episode, and what `session_id:` names in the \
                   closed record."
                .to_string(),
        },
        IdentityFacet {
            subject: "provider_session".to_string(),
            canonical: String::new(),
            spellings: provider_spellings,
            note: "an external correlation id, never the session identity. It is how a \
                   provider's hook finds the episode it opened. An episode opened by hand has \
                   none; two providers may name their sessions alike; and a worker that \
                   reconnects keeps its work while the value changes under it."
                .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_open_episodes_in_one_checkout_are_two_episodes() {
        // The singleton defect, as a test: before `state/sessions-open/` existed, a second
        // provider session opening an episode in one checkout was folded into the first,
        // and either window's end event closed the episode for both.
        let dir = tempfile::tempdir().expect("a temp dir");
        let open = dir.path().join(store::OPEN_DIR);
        std::fs::create_dir_all(&open).expect("the store");
        for (key, id) in [
            ("first", "s-20260910205542-e2a6"),
            ("second", "s-20260910210510-8f20"),
        ] {
            std::fs::write(
                open.join(format!("{key}.yaml")),
                format!(
                    "session_id: {id}\nstarted_at: 2026-09-10T20:55:42Z\nprovider_session: \"{key}\"\n"
                ),
            )
            .expect("an open record");
        }
        let episodes = open_episodes(dir.path());
        assert_eq!(episodes.len(), 2);
        let ids: std::collections::BTreeSet<&str> =
            episodes.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), 2, "two identities, not one");
    }

    #[test]
    fn the_newest_ledger_line_of_an_episode_is_its_sign_of_life() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let open = dir.path().join(store::OPEN_DIR);
        std::fs::create_dir_all(&open).expect("the store");
        std::fs::write(
            open.join("k.yaml"),
            "session_id: s-20260910205542-e2a6\nstarted_at: 2026-09-10T20:55:42Z\n",
        )
        .expect("an open record");
        let ledger = dir.path().join(ledger::LEDGER_PATH);
        std::fs::create_dir_all(ledger.parent().expect("a parent")).expect("the state dir");
        std::fs::write(
            &ledger,
            "{\"ts\":\"2026-09-10T22:00:00Z\",\"event\":\"task.started\",\"session\":\"s-20260910205542-e2a6\"}\n\
             {\"ts\":\"2026-09-11T09:00:00Z\",\"event\":\"task.started\",\"session\":\"s-20260910210510-8f20\"}\n",
        )
        .expect("a ledger");

        let episodes = open_episodes(dir.path());
        assert_eq!(episodes.len(), 1);
        let opened = crate::peers::epoch_seconds("2026-09-10T20:55:42Z").expect("an instant");
        let stamped = crate::peers::epoch_seconds("2026-09-10T22:00:00Z").expect("an instant");
        assert_eq!(
            episodes[0].last_sign_of_life,
            Some(stamped),
            "the later of the two, and never another episode's line"
        );
        assert!(stamped > opened);
    }

    #[test]
    fn a_checkout_that_has_never_run_the_lifecycle_reports_absence_and_not_a_fault() {
        let dir = tempfile::tempdir().expect("a temp dir");
        assert!(open_episodes(dir.path()).is_empty());
        assert!(Ledger::of(dir.path()).read().events.is_empty());
    }
}
