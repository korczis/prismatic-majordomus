//! Which open episode is this process's: the one resolution, in the order the shell tool has
//! always used, so that a line the executable writes and a line the shell writes about the same
//! work name the same episode.
//!
//! The shell's resolution lives in `lib/common.sh` (`mj_session_here_file`,
//! `mj_open_session_id`), and it runs on every ledger append. The executable used to read a
//! different path — `state/session-<key>.yaml`, a layout the store left behind when an open
//! episode became one file per provider session — and never consulted the provider's own
//! session variable, so a line a capability wrote was stamped with the last-opened episode, or
//! with none, where the shell stamped the worker's own. That is two answers to "whose work is
//! this", which is the question the ledger exists to answer.
//!
//! In order:
//!
//! 1. `MJ_SESSION_KEY`, which a provider hook always passes. **Strict**: the episode that key
//!    names, or none. This is what keeps one provider session's end event from closing an
//!    episode another provider session opened.
//! 2. The provider session this process is running inside: `MAJORDOMUS_PROVIDER_SESSION`, then
//!    every variable a provider declares as `session_env` in `share/providers.yaml`. The first
//!    one whose episode is open here wins; one that names no open episode falls through, because
//!    a worker in a checkout whose episode was opened by hand still has that one.
//! 3. The checkout's pointer, `session-current.yaml`. Between two open episodes this is a guess,
//!    and [`ResolvedBy::Pointer`] says so rather than presenting it as a fact.
//!
//! A record whose `worktree` names another checkout is that checkout's episode, and resolves to
//! none here, as it does in the shell.
//!
//! ```
//! use majordomus_cli::session::resolver::{resolve, ResolvedBy};
//!
//! let dir = tempfile::tempdir().unwrap();
//! let open = dir.path().join(".ai/local/state/sessions-open");
//! std::fs::create_dir_all(&open).unwrap();
//! std::fs::write(open.join("01Bv2gJsf.yaml"), "session_id: s-20260910205542-e2a6\n").unwrap();
//!
//! // a hook names the provider session, and only that episode can answer
//! let env = |k: &str| (k == "MJ_SESSION_KEY").then(|| "01Bv2gJsf".to_string());
//! let found = resolve(dir.path(), &[], &env).expect("the episode that key names");
//! assert_eq!(found.episode.as_str(), "s-20260910205542-e2a6");
//! assert_eq!(found.by, ResolvedBy::Key);
//!
//! // a key that names no open episode is no episode, never the pointer
//! let env = |k: &str| (k == "MJ_SESSION_KEY").then(|| "gone".to_string());
//! assert!(resolve(dir.path(), &[], &env).is_none());
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::identity::{EpisodeId, ProviderSessionId};
use super::store::OPEN_DIR;
use crate::metadata::yaml;

/// The checkout's pointer to its open episode, relative to the repository root: a symlink into
/// [`OPEN_DIR`], which every reader follows without knowing it did.
pub const POINTER: &str = ".ai/local/state/session-current.yaml";

/// The variable a provider hook sets to the provider session it is handling.
pub const KEY_ENV: &str = "MJ_SESSION_KEY";

/// The provider-neutral variable a launcher may set to the session a worker runs inside.
pub const PROVIDER_SESSION_ENV: &str = "MAJORDOMUS_PROVIDER_SESSION";

/// How an episode was found, which is how much to trust the answer.
///
/// ```
/// use majordomus_cli::session::resolver::ResolvedBy;
/// assert!(!ResolvedBy::Pointer.is_certain());
/// assert!(ResolvedBy::Key.is_certain());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedBy {
    /// Named by the hook handling this provider session.
    Key,
    /// Named by the provider session this process runs inside.
    ProviderSession,
    /// The checkout's pointer: the episode opened last, which is a guess between two.
    Pointer,
}

impl ResolvedBy {
    /// Whether the resolution names this process's own episode rather than guessing it.
    ///
    /// ```
    /// use majordomus_cli::session::resolver::ResolvedBy;
    /// assert!(ResolvedBy::ProviderSession.is_certain());
    /// ```
    pub fn is_certain(self) -> bool {
        !matches!(self, ResolvedBy::Pointer)
    }
}

/// An open episode, and how it was found.
///
/// ```
/// use majordomus_cli::session::resolver::{Resolution, ResolvedBy};
/// use majordomus_cli::session::EpisodeId;
/// let r = Resolution {
///     episode: EpisodeId::parse("s-20260910205542-e2a6").unwrap(),
///     by: ResolvedBy::Pointer,
///     record: ".ai/local/state/session-current.yaml".into(),
/// };
/// assert!(!r.by.is_certain());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The episode's own identity, from the record.
    pub episode: EpisodeId,
    /// How it was found.
    pub by: ResolvedBy,
    /// The record it was read from.
    pub record: PathBuf,
}

/// Resolve this process's open episode in `root`.
///
/// `session_vars` is every provider's declared `session_env`, in declaration order; `env` reads
/// one variable, injected so that a test sets nothing in its own process.
///
/// ```
/// use majordomus_cli::session::resolver::{resolve, ResolvedBy};
///
/// let dir = tempfile::tempdir().unwrap();
/// let open = dir.path().join(".ai/local/state/sessions-open");
/// std::fs::create_dir_all(&open).unwrap();
/// std::fs::write(open.join("abc.yaml"), "session_id: s-20260910205542-e2a6\n").unwrap();
///
/// // a process inside the provider session `abc` finds its own episode by the declared variable
/// let vars = vec!["CLAUDE_CODE_SESSION_ID".to_string()];
/// let env = |k: &str| (k == "CLAUDE_CODE_SESSION_ID").then(|| "abc".to_string());
/// assert_eq!(resolve(dir.path(), &vars, &env).unwrap().by, ResolvedBy::ProviderSession);
/// ```
pub fn resolve(
    root: &Path,
    session_vars: &[String],
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<Resolution> {
    let open_file = |value: &str| {
        root.join(OPEN_DIR).join(format!(
            "{}.yaml",
            ProviderSessionId::new(value).store_key()
        ))
    };
    let set = |name: &str| env(name).filter(|v| !v.is_empty());

    if let Some(key) = set(KEY_ENV) {
        return read(root, open_file(&key), ResolvedBy::Key);
    }
    let candidates =
        std::iter::once(PROVIDER_SESSION_ENV.to_string()).chain(session_vars.iter().cloned());
    for name in candidates {
        if let Some(value) = set(&name) {
            let f = open_file(&value);
            if f.is_file() {
                return read(root, f, ResolvedBy::ProviderSession);
            }
        }
    }
    read(root, root.join(POINTER), ResolvedBy::Pointer)
}

/// Every `session_env` the distribution's providers declare, in declaration order. Empty when
/// the share cannot be located or does not parse: resolution by key and by pointer still works,
/// and a ledger append must never fail because a declaration is unreadable.
///
/// ```
/// let nowhere = tempfile::tempdir().unwrap();
/// // a directory holding no distribution declares nothing, which is not an error
/// let _ = majordomus_cli::session::resolver::declared_session_vars(nowhere.path());
/// ```
pub fn declared_session_vars(root: &Path) -> Vec<String> {
    let Ok(share) = crate::share::Share::locate(None, root) else {
        return Vec::new();
    };
    let Ok(decls) = share.providers() else {
        return Vec::new();
    };
    decls
        .providers
        .into_iter()
        .filter_map(|p| p.session_env)
        .collect()
}

fn read(root: &Path, record: PathBuf, by: ResolvedBy) -> Option<Resolution> {
    let text = std::fs::read_to_string(&record).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    if let Some(w) = map.get("worktree").and_then(yaml::scalar_string) {
        if !w.is_empty() && Path::new(&w) != root {
            return None;
        }
    }
    let episode = map
        .get("session_id")
        .and_then(yaml::scalar_string)
        .and_then(|s| EpisodeId::parse(&s))?;
    Some(Resolution {
        episode,
        by,
        record,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(records: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let open = dir.path().join(OPEN_DIR);
        std::fs::create_dir_all(&open).unwrap();
        for (key, body) in records {
            std::fs::write(open.join(format!("{key}.yaml")), body).unwrap();
        }
        dir
    }

    const A: &str = "session_id: s-20260910205542-e2a6\n";
    const B: &str = "session_id: s-20260911101010-b0b0\n";

    fn pointer_at(dir: &Path, key: &str) {
        std::os::unix::fs::symlink(
            dir.join(OPEN_DIR).join(format!("{key}.yaml")),
            dir.join(POINTER),
        )
        .unwrap();
    }

    #[test]
    fn a_key_is_strict_and_never_falls_back_to_the_pointer() {
        let dir = store(&[("a", A), ("b", B)]);
        pointer_at(dir.path(), "b");
        let env = |k: &str| (k == KEY_ENV).then(|| "missing".to_string());
        assert!(resolve(dir.path(), &[], &env).is_none());
    }

    #[test]
    fn a_declared_provider_variable_finds_its_own_episode_over_the_pointer() {
        let dir = store(&[("a", A), ("b", B)]);
        pointer_at(dir.path(), "b");
        let vars = vec!["SOME_PROVIDER_SESSION".to_string()];
        let env = |k: &str| (k == "SOME_PROVIDER_SESSION").then(|| "a".to_string());
        let r = resolve(dir.path(), &vars, &env).unwrap();
        assert_eq!(
            (r.episode.as_str(), r.by),
            ("s-20260910205542-e2a6", ResolvedBy::ProviderSession)
        );
    }

    #[test]
    fn an_undeclared_variable_is_not_consulted() {
        let dir = store(&[("a", A), ("b", B)]);
        pointer_at(dir.path(), "b");
        // the variable holds a real key, but no provider declares it: the pointer answers
        let env = |k: &str| (k == "SOME_PROVIDER_SESSION").then(|| "a".to_string());
        let r = resolve(dir.path(), &[], &env).unwrap();
        assert_eq!(
            (r.episode.as_str(), r.by),
            ("s-20260911101010-b0b0", ResolvedBy::Pointer)
        );
    }

    #[test]
    fn a_provider_variable_naming_no_open_episode_falls_through_to_the_pointer() {
        let dir = store(&[("b", B)]);
        pointer_at(dir.path(), "b");
        let env = |k: &str| (k == PROVIDER_SESSION_ENV).then(|| "gone".to_string());
        assert_eq!(
            resolve(dir.path(), &[], &env).unwrap().by,
            ResolvedBy::Pointer
        );
    }

    #[test]
    fn another_checkouts_episode_is_not_this_ones() {
        let dir = store(&[(
            "a",
            "session_id: s-20260910205542-e2a6\nworktree: /somewhere/else\n",
        )]);
        let env = |k: &str| (k == KEY_ENV).then(|| "a".to_string());
        assert!(resolve(dir.path(), &[], &env).is_none());
    }

    #[test]
    fn the_key_is_spelled_as_the_store_spells_it() {
        // `a b/c` is stored as a-b-c by the shell; the resolver must look there
        let dir = store(&[("a-b-c", A)]);
        let env = |k: &str| (k == KEY_ENV).then(|| "a b/c".to_string());
        assert!(resolve(dir.path(), &[], &env).is_some());
    }
}
