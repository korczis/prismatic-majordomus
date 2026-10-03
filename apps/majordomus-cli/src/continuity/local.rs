//! The machine's side: what a publication reads from this checkout, what a resume writes
//! into it, and the one small file of checkout-local state the continuity commands keep.
//!
//! Reading: the newest handover record, the task it was written under, the decisions that
//! task recorded, the open episode, and the source state from git. Every one of those is
//! written by the shell tool; this module reads them and converts them into the portable
//! [`Record`](super::record::Record), dropping every value that names this machine.
//!
//! Writing, on resume, is the two places where a continued handover has to land for the
//! rest of the lifecycle to see it without knowing it came from elsewhere: the handovers
//! directory, through the same writer the mesh uses, and the decision log, appended in its
//! own format. Then `continuity.json`, which records which record this checkout continues
//! on each branch — the parent of its next publication — and what the last sync saw, so
//! that the entry banner can say a resumable handover is waiting without asking a remote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::record::{
    CarriedDecision, EpisodeAtPublish, SourceState, TaskRef, WorkingTree, MAX_CHANGED,
    MAX_DECISIONS, MAX_DECISION_BYTES,
};
use crate::capability::builtin::continuity::{document, read_task};

/// The local state directory, relative to the checkout.
pub const STATE_DIR: &str = ".ai/local/state";

/// The file this module keeps, inside [`STATE_DIR`].
pub const LOCAL_FILE: &str = "continuity.json";

/// The schema of [`LOCAL_FILE`].
pub const LOCAL_SCHEMA: &str = "majordomus-continuity-local/v1";

/// The key a detached HEAD is recorded under.
pub const DETACHED: &str = "DETACHED";

/// How this checkout came to stand on a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// This checkout published it.
    Published,
    /// This checkout resumed from it.
    Resumed,
}

/// The record this checkout continues on one branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Position {
    /// The line.
    pub line: String,
    /// The record: the parent of this checkout's next publication on the branch.
    pub record: String,
    /// Published here or resumed here.
    pub via: Via,
    /// When, by this machine's clock.
    pub at: String,
}

/// A handover another device published that this checkout could resume, as the last sync
/// or publication computed it — the cache the entry banner reads instead of the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Offer {
    /// The record.
    pub record: String,
    /// The publishing device's label.
    pub device: String,
    /// The branch it was written on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// When it was published, by the publisher's clock.
    pub published_at: String,
    /// The task's title, when it carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The issue, when it carried one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

/// What the last sync did, so that a later status can report it without the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncNote {
    /// The remote.
    pub remote: String,
    /// When, by this machine's clock.
    pub at: String,
    /// `ok`, or `unreachable` when the remote could not be asked.
    pub outcome: String,
    /// What git said when it was not `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// The contents of [`LOCAL_FILE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LocalState {
    /// [`LOCAL_SCHEMA`].
    pub schema: String,
    /// Branch → the record this checkout continues there.
    #[serde(default)]
    pub branches: BTreeMap<String, Position>,
    /// Resumable handovers from other devices, newest computation.
    #[serde(default)]
    pub offers: Vec<Offer>,
    /// The last sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<SyncNote>,
}

impl Default for LocalState {
    fn default() -> Self {
        LocalState {
            schema: LOCAL_SCHEMA.into(),
            branches: BTreeMap::new(),
            offers: Vec::new(),
            last_sync: None,
        }
    }
}

/// The path of [`LOCAL_FILE`] in `root`.
pub fn local_path(root: &Path) -> PathBuf {
    root.join(STATE_DIR).join(LOCAL_FILE)
}

/// Read [`LOCAL_FILE`]. Absent is the default; unreadable is an error, never silently
/// replaced, because it holds the parent of the next publication.
pub fn load(root: &Path) -> Result<LocalState, String> {
    let path = local_path(root);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let state: LocalState = serde_json::from_str(&text)
                .map_err(|e| format!("{}: {e}", relative(root, &path)))?;
            if state.schema != LOCAL_SCHEMA {
                return Err(format!(
                    "{}: schema {} is not {LOCAL_SCHEMA}",
                    relative(root, &path),
                    state.schema
                ));
            }
            Ok(state)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LocalState::default()),
        Err(e) => Err(format!("{}: {e}", relative(root, &path))),
    }
}

/// Write [`LOCAL_FILE`] atomically.
pub fn save(root: &Path, state: &LocalState) -> Result<(), String> {
    let path = local_path(root);
    let dir = root.join(STATE_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", relative(root, &dir)))?;
    let mut text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    text.push('\n');
    let tmp = dir.join(format!(".{LOCAL_FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", relative(root, &tmp)))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", relative(root, &path)))
}

/// `path` relative to `root`, for a message: a message names the repository's paths, not
/// the machine's.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        })
}

fn git_out(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// The branch and HEAD of `root`.
pub fn branch_and_head(root: &Path) -> (Option<String>, Option<String>) {
    let text = |args: &[&str]| {
        git_out(root, args)
            .map(|b| String::from_utf8_lossy(&b).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    (
        text(&["symbolic-ref", "--quiet", "--short", "HEAD"]),
        text(&["rev-parse", "--verify", "--quiet", "HEAD"]),
    )
}

/// The source state of `root` now: branch, HEAD, and — when the tree is dirty — the paths
/// that differ and a fingerprint of the difference. The fingerprint is a digest, never the
/// content: whether two machines hold the same uncommitted work can be compared, and the
/// work itself is not published.
pub fn source_state(root: &Path) -> SourceState {
    let (branch, head) = branch_and_head(root);
    let status = git_out(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .unwrap_or_default();
    let mut changed = Vec::new();
    let mut untracked = Vec::new();
    let mut entries = status.split(|b| *b == 0).filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let code = &entry[..2];
        let path = String::from_utf8_lossy(&entry[3..]).to_string();
        // a rename or copy is followed by its source path, which is not a second change
        if code[0] == b'R' || code[0] == b'C' {
            entries.next();
        }
        if code == b"??" {
            untracked.push(path.clone());
        }
        changed.push(path);
    }
    crate::order::canonical_strings(&mut changed);
    changed.dedup();
    let total = changed.len();
    let working_tree = if total == 0 {
        WorkingTree::Clean
    } else {
        WorkingTree::Dirty
    };
    let fingerprint = (total > 0).then(|| {
        let mut hasher = Sha256::new();
        if head.is_some() {
            hasher.update(git_out(root, &["diff", "--binary", "HEAD"]).unwrap_or_default());
        }
        crate::order::canonical_strings(&mut untracked);
        for path in &untracked {
            hasher.update(b"\0untracked\0");
            hasher.update(path.as_bytes());
            hasher.update(std::fs::read(root.join(path)).unwrap_or_default());
        }
        hasher
            .finalize()
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect()
    });
    changed.truncate(MAX_CHANGED);
    SourceState {
        branch,
        head,
        working_tree,
        changed,
        changed_total: total,
        fingerprint,
    }
}

/// The episode open in this checkout, when its record is this checkout's.
pub fn open_episode(root: &Path) -> Option<String> {
    let f = document(&root.join(STATE_DIR).join("session-current.yaml"))?;
    let worktree = f.get("worktree").cloned().unwrap_or_default();
    if !worktree.is_empty() && Path::new(&worktree) != root {
        let same = std::fs::canonicalize(&worktree).ok() == std::fs::canonicalize(root).ok();
        if !same {
            return None;
        }
    }
    f.get("session_id").cloned().filter(|s| !s.is_empty())
}

/// Whether an episode is open here.
pub fn episode_at_publish(root: &Path) -> (Option<String>, EpisodeAtPublish) {
    match open_episode(root) {
        Some(id) => (Some(id), EpisodeAtPublish::Open),
        None => (None, EpisodeAtPublish::None),
    }
}

/// The task `id` names: the active record when it is that task, else its archived record.
pub fn task(root: &Path, id: &str) -> Option<TaskRef> {
    let dir = root.join(STATE_DIR);
    let found = read_task(&dir.join("current.yaml"))
        .filter(|t| t.id == id)
        .or_else(|| {
            // the archive is named by task id; an id is never a path
            let safe = id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            safe.then(|| read_task(&dir.join("archive").join(format!("{id}.yaml"))))
                .flatten()
        })?;
    Some(TaskRef {
        id: found.id,
        title: found.task,
        profile: Some(found.profile).filter(|p| !p.is_empty() && p != "none"),
        scope: found.scope,
        outcome: Some(found.outcome).filter(|o| !o.is_empty()),
    })
}

/// The decisions the log holds for task `id`, oldest first, as written.
pub fn decisions(root: &Path, id: &str) -> Vec<CarriedDecision> {
    let Ok(text) = std::fs::read_to_string(root.join(STATE_DIR).join("decisions.md")) else {
        return Vec::new();
    };
    parse_decisions(&text)
        .into_iter()
        .filter(|d| {
            d.text
                .lines()
                .any(|l| l.strip_prefix("Task: ").map(str::trim) == Some(id))
        })
        .take(MAX_DECISIONS)
        .collect()
}

/// Every entry of a decision log: `## <heading>` and the lines below it. The template's
/// commented example is not an entry.
///
/// ```
/// use majordomus_cli::continuity::local::parse_decisions;
/// let log = "# Decisions\n<!--\n## YYYY — example\n-->\n\n## 2026-10-03 — Use a ref\nTask: t-1\nWhy: x\n";
/// let d = parse_decisions(log);
/// assert_eq!(d.len(), 1);
/// assert_eq!(d[0].title, "2026-10-03 — Use a ref");
/// assert!(d[0].text.contains("Task: t-1"));
/// ```
pub fn parse_decisions(text: &str) -> Vec<CarriedDecision> {
    let mut out: Vec<CarriedDecision> = Vec::new();
    let mut comment = false;
    for line in text.lines() {
        if line.contains("<!--") {
            comment = true;
        }
        if comment {
            if line.contains("-->") {
                comment = false;
            }
            continue;
        }
        if let Some(title) = line.strip_prefix("## ") {
            out.push(CarriedDecision {
                title: title.trim().to_string(),
                text: String::new(),
            });
        } else if let Some(d) = out.last_mut() {
            if d.text.len() + line.len() < MAX_DECISION_BYTES {
                d.text.push_str(line);
                d.text.push('\n');
            }
        }
    }
    for d in &mut out {
        d.text = d.text.trim_end().to_string();
        d.text.push('\n');
    }
    out
}

/// Append carried decisions to this checkout's log, each once: an entry whose heading the
/// log already holds is not written again. Each appended entry names where it came from.
/// Returns how many were appended.
pub fn carry_decisions(
    root: &Path,
    carried: &[CarriedDecision],
    record: &str,
    device: &str,
) -> Result<usize, String> {
    if carried.is_empty() {
        return Ok(0);
    }
    let path = root.join(STATE_DIR).join("decisions.md");
    let mut text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            "# Decisions\n\nAppend-only. One dated entry per decision. Newest at the bottom.\n"
                .to_string()
        }
        Err(e) => return Err(format!("{}: {e}", relative(root, &path))),
    };
    let held: std::collections::BTreeSet<String> = parse_decisions(&text)
        .into_iter()
        .map(|d| d.title)
        .collect();
    let mut appended = 0usize;
    for d in carried {
        let title: String = d.title.chars().filter(|c| !c.is_control()).collect();
        if held.contains(&title) {
            continue;
        }
        // a carried entry cannot open a second heading inside itself
        let body: String = d
            .text
            .lines()
            .filter(|l| !l.starts_with("## ") && !l.contains("<!--"))
            .map(|l| format!("{l}\n"))
            .collect();
        let device: String = device.chars().filter(|c| !c.is_control()).collect();
        text.push_str(&format!(
            "\n## {title}\n{body}Carried: continuity {} from {device}\n",
            &record[..16.min(record.len())]
        ));
        appended += 1;
    }
    if appended > 0 {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", relative(root, dir)))?;
        }
        let tmp = path.with_extension("md.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", relative(root, &tmp)))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", relative(root, &path)))?;
    }
    Ok(appended)
}

/// The values of this process's secret environment variables: every variable whose name
/// says it holds a credential, with a value long enough to be one. A publication refuses a
/// record carrying any of them, whatever shape they have.
pub fn secret_environment() -> Vec<String> {
    const MARKS: &[&str] = &[
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "APIKEY",
        "PRIVATE_KEY",
        "CREDENTIAL",
        "ACCESS_KEY",
        "AUTH",
    ];
    std::env::vars()
        .filter(|(k, v)| {
            let k = k.to_ascii_uppercase();
            v.len() >= 8 && MARKS.iter().any(|m| k.contains(m))
        })
        .map(|(_, v)| v)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .status()
                .unwrap()
                .success());
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
        git(&["add", "a.txt"]);
        git(&["commit", "-q", "-m", "a"]);
        dir
    }

    #[test]
    fn a_dirty_tree_lists_relative_paths_and_a_fingerprint_not_its_content() {
        let dir = repo();
        let clean = source_state(dir.path());
        assert_eq!(clean.working_tree, WorkingTree::Clean);
        assert_eq!(clean.branch.as_deref(), Some("main"));
        assert!(clean.fingerprint.is_none());

        std::fs::write(dir.path().join("a.txt"), "secret work in progress\n").unwrap();
        std::fs::write(dir.path().join("new.txt"), "n\n").unwrap();
        let dirty = source_state(dir.path());
        assert_eq!(dirty.working_tree, WorkingTree::Dirty);
        assert_eq!(
            dirty.changed,
            vec!["a.txt".to_string(), "new.txt".to_string()]
        );
        assert_eq!(dirty.changed_total, 2);
        let fp = dirty.fingerprint.clone().unwrap();
        assert_eq!(fp.len(), 32);
        assert!(!serde_json::to_string(&dirty)
            .unwrap()
            .contains("in progress"));

        std::fs::write(dir.path().join("a.txt"), "other work\n").unwrap();
        assert_ne!(source_state(dir.path()).fingerprint.unwrap(), fp);
    }

    #[test]
    fn decisions_are_carried_once_and_name_their_origin() {
        let dir = repo();
        let carried = parse_decisions("## 2026-10-03 — Use a ref\nTask: t-1\nWhy: no PR noise\n");
        assert_eq!(
            carry_decisions(dir.path(), &carried, &"a".repeat(32), "mac").unwrap(),
            1
        );
        assert_eq!(
            carry_decisions(dir.path(), &carried, &"a".repeat(32), "mac").unwrap(),
            0
        );
        let log = std::fs::read_to_string(dir.path().join(STATE_DIR).join("decisions.md")).unwrap();
        assert_eq!(log.matches("## 2026-10-03 — Use a ref").count(), 1);
        assert!(log.contains("Carried: continuity aaaaaaaaaaaaaaaa from mac"));
        assert_eq!(decisions(dir.path(), "t-1").len(), 1);
    }

    #[test]
    fn the_local_state_round_trips_and_a_corrupt_one_is_an_error() {
        let dir = repo();
        assert_eq!(load(dir.path()).unwrap(), LocalState::default());
        let mut s = LocalState::default();
        s.branches.insert(
            "main".into(),
            Position {
                line: "a".repeat(32),
                record: "b".repeat(32),
                via: Via::Resumed,
                at: "2026-10-03T12:00:00Z".into(),
            },
        );
        save(dir.path(), &s).unwrap();
        assert_eq!(load(dir.path()).unwrap(), s);
        std::fs::write(local_path(dir.path()), "{").unwrap();
        assert!(load(dir.path()).is_err());
    }
}
