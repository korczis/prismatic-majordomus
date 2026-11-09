//! Whose record this is: the episode a durable record names, and the task it relates
//! (ADR 0164), resolved the way the shell resolves them.
//!
//! The shell's writers have resolved the caller's own episode since ADR 0164
//! (`mj_open_session_id`, `mj_task_opening_session` and `mj_record_attribution` in
//! `lib/common.sh`). The executable's writers did not: the ledger, the continuity publish and
//! the reasoning store each read `state/session-current.yaml`, the checkout's pointer, which
//! names whichever episode opened last. Two workers in one checkout then stamped each
//! other's session on a plan transition, the misattribution M002 exists to end, on the path
//! most workers use (the MCP tool and `majordomus run`). I4291.
//!
//! The resolution, in order:
//!
//! 1. an explicit key: the caller's own provider session, which a capability call knows from
//!    its connection's episode;
//! 2. `MJ_SESSION_KEY`;
//! 3. the provider's session environment: `MAJORDOMUS_PROVIDER_SESSION`, then each
//!    provider's own variable ([`PROVIDER_SESSION_VARS`]);
//! 4. with no key at all, the one episode open in this checkout, or none when there are
//!    several.
//!
//! A key names its own episode or none, and never the pointer: the last of several open
//! episodes is not a writer's identity. An episode whose record names another worktree is
//! that checkout's, and names nothing here.

use std::path::Path;

use crate::metadata::yaml;

use super::identity::ProviderSessionId;
use super::ledger::Ledger;
use super::store::OPEN_DIR;

/// The variables a provider exports its own session identity in, in the order the shell
/// reads them: column 13 of `MJ_CAPTURE_LIFECYCLE` in `lib/capture.sh`, the one declaration
/// of what a provider is. A test below holds this port to that table.
pub const PROVIDER_SESSION_VARS: &[&str] = &["CLAUDE_CODE_SESSION_ID"];

/// The provider session this process runs inside, when one is observable:
/// `MAJORDOMUS_PROVIDER_SESSION`, then each provider's own variable. `mj_provider_session_env`.
pub fn provider_session_env() -> Option<String> {
    std::iter::once("MAJORDOMUS_PROVIDER_SESSION")
        .chain(PROVIDER_SESSION_VARS.iter().copied())
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
}

/// The key of a capability call's caller: the provider session its connection's episode was
/// opened under. A connection that attached no episode has none, and the call resolves its
/// caller from the environment like a command line.
pub fn connection_key(
    episodes: &crate::episodes::EpisodeBoard,
    caller: Option<&crate::peers::PeerId>,
) -> Option<String> {
    caller
        .and_then(|peer| episodes.of_peer(peer))
        .map(|episode| episode.external_id)
}

/// The caller's key: `explicit` when given, else `MJ_SESSION_KEY`, else the provider's
/// session environment. `None` when nothing names the caller.
pub fn caller_key(explicit: Option<&str>) -> Option<String> {
    explicit
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .or_else(|| std::env::var("MJ_SESSION_KEY").ok().filter(|k| !k.is_empty()))
        .or_else(provider_session_env)
}

/// A provider session reduced to one path segment the way `mj_session_key` does: every byte
/// outside `A-Za-z0-9._-` becomes `-`, runs of `-` become one, the first 64 characters are
/// kept, and `-` is trimmed from both ends; nothing left is the hand-opened episode.
pub fn shell_key(value: &str) -> String {
    let mut squeezed = String::with_capacity(value.len());
    for c in value.chars() {
        let c = if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            c
        } else {
            '-'
        };
        if c == '-' && squeezed.ends_with('-') {
            continue;
        }
        squeezed.push(c);
    }
    let cut: String = squeezed.chars().take(64).collect();
    let trimmed = cut.trim_matches('-');
    if trimmed.is_empty() {
        "hand".to_string()
    } else {
        trimmed.to_string()
    }
}

/// One open record: its session id and the worktree it names, if any.
fn read_open(path: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    let s = |k: &str| map.get(k).and_then(yaml::scalar_string).unwrap_or_default();
    Some((s("session_id"), s("worktree")))
}

/// Whether a record's `worktree` is this checkout: absent, or the same directory under any
/// spelling.
fn is_here(worktree: &str, root: &Path) -> bool {
    if worktree.is_empty() || Path::new(worktree) == root {
        return true;
    }
    match (std::fs::canonicalize(worktree), std::fs::canonicalize(root)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The episode a record written here by the caller names (`mj_open_session_id`).
///
/// `explicit` is the caller's own provider session when the caller knows it (a capability
/// call, from its connection's episode); otherwise [`caller_key`] reads the environment.
/// A key names the open record filed under it — the shell's file name, or the executable's
/// ([`ProviderSessionId::store_key`]) — or nothing. With no key, the one episode open in
/// this checkout, or nothing when there are several. Never the pointer.
pub fn open_session_id(root: &Path, explicit: Option<&str>) -> Option<String> {
    let dir = root.join(OPEN_DIR);
    let (session, worktree) = match caller_key(explicit) {
        Some(key) => {
            let names = [
                shell_key(&key),
                ProviderSessionId::new(key.clone()).store_key(),
            ];
            names
                .iter()
                .find_map(|n| read_open(&dir.join(format!("{n}.yaml"))))?
        }
        None => {
            let mut here = std::fs::read_dir(&dir)
                .ok()?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
                .filter_map(|p| read_open(&p))
                .filter(|(_, w)| is_here(w, root));
            let one = here.next()?;
            if here.next().is_some() {
                return None;
            }
            one
        }
    };
    (is_here(&worktree, root) && !session.is_empty()).then_some(session)
}

/// The episode that opened task `id`: the session of its one `task.started` event. Missing
/// or ambiguous history supplies no identity (`mj_task_opening_session`).
pub fn task_opening_session(root: &Path, id: &str) -> Option<String> {
    let read = Ledger::of(root).read();
    let mut opened = read.events.iter().filter_map(|line| {
        let obj = line.as_object()?;
        let event = obj.get("event")?.as_str()?;
        let task = obj.get("task_id")?.as_str()?;
        (event == "task.started" && task == id).then(|| {
            obj.get("session")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string()
        })
    });
    let first = opened.next()?;
    if opened.next().is_some() || first.is_empty() {
        return None;
    }
    Some(first)
}

/// Whether `session` is still open in this checkout: an open record of this worktree names
/// it (`mj_session_is_open`).
pub fn session_is_open(root: &Path, session: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(root.join(OPEN_DIR)) else {
        return false;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .filter_map(|p| read_open(&p))
        .any(|(s, w)| s == session && is_here(&w, root))
}

/// What a durable record written by the caller names: its episode, and the active task when
/// the caller may relate it (`mj_record_attribution`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attribution {
    /// The caller's episode, or `None` when nothing names one.
    pub session: Option<String>,
    /// The active task, unless another episode that is still open opened it.
    pub task: Option<String>,
}

/// The attribution of a record the caller writes in `root`.
///
/// The active task is this checkout's `current.yaml`, related unless another episode that is
/// still open opened it: that is the one case a record could be filed under somebody else's
/// work. A task whose opener has closed, or whose opening names no episode, has no live
/// owner, and the caller continuing it carries it on.
pub fn record_attribution(root: &Path, explicit: Option<&str>) -> Attribution {
    let session = open_session_id(root, explicit);
    let task = active_task(root).filter(|id| {
        match task_opening_session(root, id) {
            Some(opened) => {
                Some(&opened) == session.as_ref() || !session_is_open(root, &opened)
            }
            None => true,
        }
    });
    Attribution { session, task }
}

/// The task open in this checkout, from `state/current.yaml`, when its record is this
/// checkout's.
fn active_task(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(".ai/local/state/current.yaml")).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    let s = |k: &str| map.get(k).and_then(yaml::scalar_string).unwrap_or_default();
    if !is_here(&s("worktree"), root) {
        return None;
    }
    Some(s("id")).filter(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The environment is the process's: tests that set it run one at a time.
    static ENV: Mutex<()> = Mutex::new(());

    fn clear_env() {
        for v in ["MJ_SESSION_KEY", "MAJORDOMUS_PROVIDER_SESSION"]
            .iter()
            .chain(PROVIDER_SESSION_VARS)
        {
            std::env::remove_var(v);
        }
    }

    fn open(root: &Path, file: &str, session: &str) {
        let dir = root.join(OPEN_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{file}.yaml")),
            format!("session_id: {session}\nworktree: {}\n", root.display()),
        )
        .unwrap();
    }

    fn started(root: &Path, task: &str, session: &str) {
        let state = root.join(".ai/local/state");
        std::fs::create_dir_all(&state).unwrap();
        let line = format!(
            "{{\"ts\":\"2026-11-07T12:00:00Z\",\"event\":\"task.started\",\"task_id\":\"{task}\",\"session\":\"{session}\"}}\n"
        );
        let path = state.join("ledger.jsonl");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text.push_str(&line);
        std::fs::write(path, text).unwrap();
        std::fs::write(
            state.join("current.yaml"),
            format!("id: {task}\nworktree: {}\n", root.display()),
        )
        .unwrap();
    }

    /// A key is squeezed into one path segment exactly as `mj_session_key` squeezes it.
    #[test]
    fn a_key_is_squeezed_as_the_shell_squeezes_it() {
        assert_eq!(shell_key("01Bv2gJsf"), "01Bv2gJsf");
        assert_eq!(shell_key("a//b c"), "a-b-c");
        assert_eq!(shell_key("/x/"), "x");
        assert_eq!(shell_key(""), "hand");
        assert_eq!(shell_key(&"z".repeat(80)).len(), 64);
    }

    /// The port of the provider table is the table: column 13 of `MJ_CAPTURE_LIFECYCLE`.
    #[test]
    fn the_provider_variables_are_the_shells_table() {
        let capture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/capture.sh");
        let source = std::fs::read_to_string(capture).unwrap();
        // the single-quoted value, one row per line however many rows it grows to
        let start = source
            .find("\nMJ_CAPTURE_LIFECYCLE='")
            .expect("lib/capture.sh declares MJ_CAPTURE_LIFECYCLE")
            + "\nMJ_CAPTURE_LIFECYCLE='".len();
        let body = &source[start..start + source[start..].find('\'').unwrap()];
        let table: Vec<&str> = body
            .lines()
            .filter_map(|row| row.split_whitespace().nth(12))
            .filter(|v| *v != "-")
            .collect();
        assert_eq!(table, PROVIDER_SESSION_VARS, "the port and lib/capture.sh disagree");
    }

    /// I4291: two episodes in one checkout. Each caller names its own episode, the pointer
    /// is never read, and the task the other (still open) episode opened is not related.
    #[test]
    fn two_episodes_each_name_their_own_and_neither_takes_the_others_task() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        open(root, "worker-a", "s-a");
        open(root, "worker-b", "s-b");
        // the pointer names B, as it would after B opened last; nobody may read it
        std::fs::write(
            root.join(".ai/local/state/session-current.yaml"),
            "session_id: s-b\n",
        )
        .unwrap();
        started(root, "t-1", "s-a");

        let a = record_attribution(root, Some("worker-a"));
        assert_eq!(a.session.as_deref(), Some("s-a"));
        assert_eq!(a.task.as_deref(), Some("t-1"), "A opened the task and relates it");

        let b = record_attribution(root, Some("worker-b"));
        assert_eq!(b.session.as_deref(), Some("s-b"));
        assert_eq!(b.task, None, "B may not relate the task A, still open, opened");

        // a key that names no open episode names nothing, never the pointer's
        assert_eq!(open_session_id(root, Some("worker-gone")), None);
        // and with no key, two open episodes are ambiguous: none
        assert_eq!(open_session_id(root, None), None);

        // the environment is the key when the caller passes none
        std::env::set_var("MAJORDOMUS_PROVIDER_SESSION", "worker-b");
        assert_eq!(open_session_id(root, None).as_deref(), Some("s-b"));
        std::env::set_var("MJ_SESSION_KEY", "worker-a");
        assert_eq!(
            open_session_id(root, None).as_deref(),
            Some("s-a"),
            "MJ_SESSION_KEY outranks the provider environment"
        );
        assert_eq!(
            open_session_id(root, Some("worker-b")).as_deref(),
            Some("s-b"),
            "an explicit key outranks the environment"
        );
        clear_env();
    }

    /// One episode open is unambiguous; a task whose opener closed is carried on by whoever
    /// continues it; an episode of another worktree names nothing here.
    #[test]
    fn one_episode_is_the_writer_and_a_closed_openers_task_is_carried_on() {
        let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        open(root, "worker-b", "s-b");
        started(root, "t-2", "s-a"); // A opened it and has closed (no open record)
        let b = record_attribution(root, None);
        assert_eq!(b.session.as_deref(), Some("s-b"));
        assert_eq!(b.task.as_deref(), Some("t-2"));

        let elsewhere = root.join(OPEN_DIR).join("worker-c.yaml");
        std::fs::write(&elsewhere, "session_id: s-c\nworktree: /somewhere/else\n").unwrap();
        assert_eq!(open_session_id(root, Some("worker-c")), None);
        assert_eq!(
            open_session_id(root, None).as_deref(),
            Some("s-b"),
            "another checkout's episode does not make this one ambiguous"
        );
        clear_env();
    }

    /// A task started twice, or by nobody, supplies no identity.
    #[test]
    fn ambiguous_or_anonymous_openings_name_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        started(root, "t-3", "s-a");
        started(root, "t-3", "s-b");
        assert_eq!(task_opening_session(root, "t-3"), None);
        started(root, "t-4", "");
        assert_eq!(task_opening_session(root, "t-4"), None);
        assert_eq!(task_opening_session(root, "t-none"), None);
    }
}
