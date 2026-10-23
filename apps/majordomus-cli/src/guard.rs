//! The edit guard (I2735): before a provider's tool writes a file of this checkout, is the
//! path claimed?
//!
//! A session reaches the mesh at its start (I2734), a task's scope is claimed there by
//! `start` (I2288), and a task completed off the mesh is refused (I2289). None of that stops
//! a worker from editing a file nobody claimed: the claim was a convention until the edit
//! itself asked for one. This is the asking. It answers one question — may this session
//! write this path now — from four facts, each read without building the index, because it
//! runs before every edit a provider makes:
//!
//! 1. the policy switch `session.edits_require_claim`, off unless a repository turns it on;
//! 2. whether the repository declares its mesh enabled — a repository without a mesh has no
//!    claims to be covered by;
//! 3. the active task's scope, which `start` has claimed on the mesh as `task-<id>`;
//! 4. the live claims the mesh holds for the session doing the writing.
//!
//! What it cannot decide it lets through and says why: an unreadable policy, no server to
//! ask, a mesh state that does not parse. Refusing an edit because a probe failed would make a
//! checkout nobody can serve a checkout nobody can edit, which is worse than the edit it
//! would have stopped; I2289 still refuses the completion of such work.
//!
//! One edit, from the path a provider names to the verdict it reads:
//!
//! ```
//! use std::path::Path;
//! use majordomus_cli::guard::{judge_facts, relative_to, ClaimSource, EditFacts};
//!
//! // a provider is about to write this file of the checkout at /work/repo
//! let root = Path::new("/work/repo");
//! let asked = "/work/repo/docs/MESH.md";
//! let rel = relative_to(root, asked);
//! assert_eq!(rel.as_deref(), Some("docs/MESH.md"));
//!
//! // the facts gathered for it: the policy asks for a claim, the mesh is on, a task claims lib
//! let scope = vec!["lib".to_string()];
//! let facts = EditFacts {
//!     required: Ok(true),
//!     mesh_enabled: true,
//!     task: Some(("t-1", &scope[..])),
//!     session: Some("claude-code-s1"),
//!     claims: ClaimSource::Claims(vec![]),
//! };
//! let verdict = judge_facts(asked, rel.as_deref(), facts);
//! assert!(!verdict.allowed, "neither the task nor a claim of the session covers docs");
//! assert!(verdict.remedy.unwrap().contains("majordomus start"));
//! ```

use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::integration::exclusive::Mesh;
use crate::mesh::state::{ClaimState, ClaimView};

/// The second question the provider's pre-tool hook asks, of a shell command rather than a
/// write: does it start a build on a declared cluster host outside the offload path (I3956)?
pub(crate) mod remote_build;

/// The answer to one edit: whether it may proceed, the one word it was decided on, the
/// sentence a person reads and, for a refusal, what would allow it. `guard.edit` returns it
/// as it is; `guard hook` turns it into an exit code.
///
/// ```
/// use majordomus_cli::guard::{judge_facts, ClaimSource, EditFacts, EditVerdict};
///
/// let facts = EditFacts {
///     required: Ok(false),
///     mesh_enabled: true,
///     task: None,
///     session: None,
///     claims: ClaimSource::Claims(vec![]),
/// };
/// let v: EditVerdict = judge_facts("a.md", Some("a.md"), facts);
/// assert!(v.allowed);
/// assert_eq!(v.ground, "off");
/// let json = serde_json::to_value(&v).unwrap();
/// assert!(json.get("remedy").is_none(), "an allowed edit carries no remedy");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EditVerdict {
    /// Whether the edit may proceed.
    pub allowed: bool,
    /// The path asked about, repository-relative when it is inside the checkout.
    pub path: String,
    /// Why, as one word: `off`, `outside`, `local`, `no_mesh`, `task`, `claim`,
    /// `undecided` or `unclaimed`. Only `unclaimed` refuses.
    pub ground: String,
    /// The sentence a person reads.
    pub detail: String,
    /// What makes the edit allowed, when it is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}

impl EditVerdict {
    fn allow(path: &str, ground: &str, detail: impl Into<String>) -> Self {
        EditVerdict {
            allowed: true,
            path: path.to_string(),
            ground: ground.to_string(),
            detail: detail.into(),
            remedy: None,
        }
    }
}

/// Where the guard reads the mesh's live claims: the running server's own state, or that
/// server asked over HTTP, or nothing when no server answers.
///
/// ```
/// use majordomus_cli::guard::{judge_facts, ClaimSource, EditFacts};
///
/// let facts = |claims| EditFacts {
///     required: Ok(true),
///     mesh_enabled: true,
///     task: None,
///     session: Some("claude-code-s1"),
///     claims,
/// };
/// // the claims were read and none covers the path: refused
/// assert!(!judge_facts("a", Some("a"), facts(ClaimSource::Claims(vec![]))).allowed);
/// // nothing could be read: let through, and said
/// let v = judge_facts("a", Some("a"), facts(ClaimSource::Unavailable("no server".into())));
/// assert!(v.allowed);
/// assert!(v.detail.contains("no server"));
/// ```
pub enum ClaimSource {
    /// The claims, already read.
    Claims(Vec<ClaimView>),
    /// No server answers, or its answer could not be read: why.
    Unavailable(String),
}

/// Whether a claim on `claim` covers a write to `path`: the same path, an ancestor of it,
/// or the whole repository (`.`).
///
/// ```
/// use majordomus_cli::guard::covers;
///
/// assert!(covers("lib", "lib/capture.sh"));
/// assert!(covers("lib/capture.sh", "lib/capture.sh"));
/// assert!(covers(".", "docs/MESH.md"), "a claim on the root covers everything");
/// assert!(covers("lib/", "lib/a.sh"), "a trailing slash is the same directory");
/// assert!(!covers("lib/capture.sh", "lib"), "a file does not cover its directory");
/// assert!(!covers("li", "lib/a.sh"), "a prefix of a name is not an ancestor");
/// assert!(!covers("", "lib/a.sh"));
/// ```
pub fn covers(claim: &str, path: &str) -> bool {
    let claim = claim.trim().trim_end_matches('/');
    let path = path.trim().trim_end_matches('/');
    if claim == "." {
        return !path.is_empty();
    }
    !claim.is_empty() && (path == claim || path.starts_with(&format!("{claim}/")))
}

/// `path` relative to `root`, when it is inside it: absolute or relative to `root`, with `.`
/// and `..` resolved lexically, so a file that does not exist yet — the commonest thing a
/// tool writes — is placed as surely as one that does. `None` outside the checkout.
///
/// ```
/// use std::path::Path;
/// use majordomus_cli::guard::relative_to;
///
/// let root = Path::new("/work/repo");
/// assert_eq!(relative_to(root, "/work/repo/lib/a.sh").as_deref(), Some("lib/a.sh"));
/// assert_eq!(relative_to(root, "lib/new.sh").as_deref(), Some("lib/new.sh"));
/// assert_eq!(relative_to(root, "/work/repo/docs/../lib/a.sh").as_deref(), Some("lib/a.sh"));
/// assert_eq!(relative_to(root, "/work/other/a.sh"), None);
/// assert_eq!(relative_to(root, "/work/repo/../repo-wt/a.sh"), None);
/// ```
pub fn relative_to(root: &Path, path: &str) -> Option<String> {
    let given = Path::new(path);
    let joined = if given.is_absolute() {
        given.to_path_buf()
    } else {
        root.join(given)
    };
    let normal = lexical(&joined);
    let root = lexical(root);
    let rel = normal.strip_prefix(&root).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    (!rel.is_empty()).then_some(rel)
}

fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The facts one edit is judged from, gathered by the caller so that this function reads
/// nothing and can be proved on every combination.
///
/// ```
/// use majordomus_cli::guard::{judge_facts, ClaimSource, EditFacts};
///
/// let scope = vec!["lib".to_string()];
/// let facts = EditFacts {
///     required: Ok(true),
///     mesh_enabled: true,
///     task: Some(("t-1", &scope[..])),
///     session: None,
///     claims: ClaimSource::Unavailable("not asked".into()),
/// };
/// let v = judge_facts("lib/a.sh", Some("lib/a.sh"), facts);
/// assert_eq!((v.allowed, v.ground.as_str()), (true, "task"), "the task's scope decides first");
/// ```
pub struct EditFacts<'a> {
    /// `session.edits_require_claim`, or why the policy could not be read.
    pub required: Result<bool, String>,
    /// Whether the repository declares its mesh enabled.
    pub mesh_enabled: bool,
    /// The active task's id and scope, when one is active in this checkout.
    pub task: Option<(&'a str, &'a [String])>,
    /// The writing session's id on the mesh, when the caller knows it.
    pub session: Option<&'a str>,
    /// The mesh's live claims.
    pub claims: ClaimSource,
}

/// Judge one edit of `rel` (repository-relative, or `None` when outside the checkout).
///
/// ```
/// use majordomus_cli::guard::{judge_facts, ClaimSource, EditFacts};
///
/// let scope = vec!["lib".to_string()];
/// let facts = |task, claims| EditFacts {
///     required: Ok(true), mesh_enabled: true, task, session: Some("claude-code-s1"), claims,
/// };
/// // the active task's scope covers it
/// let v = judge_facts("lib/a.sh", Some("lib/a.sh"), facts(Some(("t-1", &scope[..])), ClaimSource::Claims(vec![])));
/// assert!(v.allowed);
/// assert_eq!(v.ground, "task");
/// // nothing covers it
/// let v = judge_facts("docs/x.md", Some("docs/x.md"), facts(Some(("t-1", &scope[..])), ClaimSource::Claims(vec![])));
/// assert!(!v.allowed);
/// assert_eq!(v.ground, "unclaimed");
/// assert!(v.remedy.unwrap().contains("--session claude-code-s1"));
/// // no server to ask: let through, and said
/// let v = judge_facts("docs/x.md", Some("docs/x.md"), facts(None, ClaimSource::Unavailable("no server".into())));
/// assert!(v.allowed);
/// assert_eq!(v.ground, "undecided");
/// ```
pub fn judge_facts(asked: &str, rel: Option<&str>, facts: EditFacts<'_>) -> EditVerdict {
    let Some(rel) = rel else {
        return EditVerdict::allow(asked, "outside", "the path is outside this checkout");
    };
    match facts.required {
        Ok(false) => {
            return EditVerdict::allow(
                rel,
                "off",
                "session.edits_require_claim is off in this repository's policy",
            )
        }
        Err(e) => {
            return EditVerdict::allow(
                rel,
                "undecided",
                format!("the policy could not be read, so nothing is enforced: {e}"),
            )
        }
        Ok(true) => {}
    }
    if rel == ".ai/local"
        || rel.starts_with(".ai/local/")
        || rel == ".git"
        || rel.starts_with(".git/")
    {
        return EditVerdict::allow(rel, "local", "the path is this checkout's local state");
    }
    if !facts.mesh_enabled {
        return EditVerdict::allow(rel, "no_mesh", "the repository declares no enabled mesh");
    }
    if let Some((id, scope)) = facts.task {
        if let Some(s) = scope.iter().find(|s| covers(s, rel)) {
            return EditVerdict::allow(rel, "task", format!("the active task {id} claims {s}"));
        }
    }
    let claims = match facts.claims {
        ClaimSource::Claims(c) => c,
        ClaimSource::Unavailable(why) => {
            return EditVerdict::allow(
                rel,
                "undecided",
                format!("the mesh could not be asked, so nothing is enforced: {why}"),
            )
        }
    };
    if let Some(session) = facts.session {
        let own = format!("/{session}");
        if let Some(c) = claims.iter().find(|c| {
            matches!(c.state, ClaimState::Held | ClaimState::Conflicted(_))
                && c.session.ends_with(&own)
                && c.scope.iter().any(|s| covers(s, rel))
        }) {
            return EditVerdict::allow(
                rel,
                "claim",
                format!("the session's claim {} covers it", c.key),
            );
        }
    }
    let by_session = facts
        .session
        .map(|s| format!(", or claim it for this session: majordomus mesh claim {rel} --session {s} --advisory"))
        .unwrap_or_default();
    EditVerdict {
        allowed: false,
        path: rel.to_string(),
        ground: "unclaimed".into(),
        detail: format!(
            "no claim covers {rel}: neither the active task's scope{} nor a live mesh claim of this session",
            facts.task.map(|(id, _)| format!(" ({id})")).unwrap_or_default()
        ),
        remedy: Some(format!(
            "start a task that claims it: majordomus start \"<task>\" --scope {rel}{by_session}"
        )),
    }
}

/// Whether the repository at `root` declares its mesh enabled: a declaration under
/// `.ai/repo/mesh/` whose top-level `enabled` is `true`. Read from the files, not the index:
/// this runs before every edit.
///
/// ```
/// use majordomus_cli::guard::mesh_declared_enabled;
///
/// let dir = tempfile::tempdir().unwrap();
/// assert!(!mesh_declared_enabled(dir.path()), "no declaration");
/// let mesh = dir.path().join(".ai/repo/mesh");
/// std::fs::create_dir_all(&mesh).unwrap();
/// std::fs::write(mesh.join("mesh.yaml"), "enabled: false\n").unwrap();
/// assert!(!mesh_declared_enabled(dir.path()));
/// std::fs::write(mesh.join("mesh.yaml"), "enabled: true\n").unwrap();
/// assert!(mesh_declared_enabled(dir.path()));
/// ```
pub fn mesh_declared_enabled(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root.join(".ai/repo/mesh")) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        matches!(p.extension().and_then(|x| x.to_str()), Some("yaml" | "yml"))
            && std::fs::read_to_string(&p)
                .ok()
                .and_then(|t| crate::metadata::yaml::parse_mapping(&t).ok())
                .and_then(|m| m.get("enabled").and_then(|v| v.as_bool()))
                == Some(true)
    })
}

/// `session.edits_require_claim` of the repository at `root`, or why its policy could not
/// be read — which [`judge_facts`] answers by enforcing nothing.
///
/// ```
/// use majordomus_cli::guard::edits_require_claim;
///
/// let dir = tempfile::tempdir().unwrap();
/// assert!(edits_require_claim(dir.path()).is_err(), "no manifest, so no policy");
/// ```
pub fn edits_require_claim(root: &Path) -> Result<bool, String> {
    let repository = crate::repository::Repository::open(root).map_err(|e| e.to_string())?;
    crate::policy::LoadedPolicy::load(&repository)
        .map(|p| p.policy.session.edits_require_claim)
        .map_err(|e| e.to_string())
}

/// The active task of the checkout at `root`: its id and scope, while its outcome is active.
///
/// ```
/// use majordomus_cli::guard::active_task;
///
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(active_task(dir.path()), None, "no task record");
/// let state = dir.path().join(".ai/local/state");
/// std::fs::create_dir_all(&state).unwrap();
/// let record = "id: t-1\noutcome: active\nscope:\n  - lib\n";
/// std::fs::write(state.join("current.yaml"), record).unwrap();
/// assert_eq!(active_task(dir.path()), Some(("t-1".to_string(), vec!["lib".to_string()])));
/// let finished = record.replace("active", "completed");
/// std::fs::write(state.join("current.yaml"), finished).unwrap();
/// assert_eq!(active_task(dir.path()), None, "a finished task claims nothing");
/// ```
pub fn active_task(root: &Path) -> Option<(String, Vec<String>)> {
    let task = crate::capability::builtin::continuity::read_task(
        &root.join(".ai/local/state/current.yaml"),
    )?;
    (task.outcome == "active").then_some((task.id, task.scope))
}

/// The live claims of the checkout's shared server, asked over HTTP from outside it.
///
/// ```
/// use majordomus_cli::guard::{server_claims, ClaimSource};
///
/// let dir = tempfile::tempdir().unwrap();
/// let ClaimSource::Unavailable(why) = server_claims(dir.path()) else {
///     panic!("a checkout no server serves has no claims to read");
/// };
/// assert!(why.contains("no shared server"));
/// ```
pub fn server_claims(root: &Path) -> ClaimSource {
    match crate::integration::exclusive::server_of(root) {
        Some(server) => claims_of(&server),
        None => ClaimSource::Unavailable("no shared server answers for this checkout".into()),
    }
}

/// The live claims `mesh` answers with, or why they could not be read. A server that does
/// not run the mesh answers 200 with `active: false` and its reason, and that is said as
/// such rather than as a state that does not parse.
pub(crate) fn claims_of(mesh: &dyn Mesh) -> ClaimSource {
    match mesh.ask("GET", "/api/v1/mesh/state?view=full&claims=live", None) {
        Ok((200, v)) if v["active"] == false => {
            let why = v["reason"].as_str().unwrap_or("no reason given");
            ClaimSource::Unavailable(format!("the server does not run the mesh: {why}"))
        }
        Ok((200, v)) => {
            match serde_json::from_value::<Vec<ClaimView>>(v["state"]["claims"].clone()) {
                Ok(c) => ClaimSource::Claims(c),
                Err(e) => ClaimSource::Unavailable(format!("the mesh state does not parse: {e}")),
            }
        }
        Ok((status, v)) => {
            let m = v["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            ClaimSource::Unavailable(format!("the server answered {status}: {m}"))
        }
        Err(e) => ClaimSource::Unavailable(e),
    }
}

/// Judge an edit of `path` by `session` in the checkout at `root`, reading every fact.
/// `claims` is asked only when the cheaper facts have not already decided.
///
/// ```
/// use majordomus_cli::guard::judge;
///
/// let dir = tempfile::tempdir().unwrap();
/// // no manifest, so no policy: nothing is enforced, and the mesh is never asked
/// let v = judge(dir.path(), "a.md", None, || unreachable!("the claims are not needed"));
/// assert!(v.allowed);
/// assert_eq!((v.path.as_str(), v.ground.as_str()), ("a.md", "undecided"));
/// ```
pub fn judge(
    root: &Path,
    path: &str,
    session: Option<&str>,
    claims: impl FnOnce() -> ClaimSource,
) -> EditVerdict {
    let rel = relative_to(root, path);
    let required = edits_require_claim(root);
    let mesh_enabled = mesh_declared_enabled(root);
    let task = active_task(root);
    let needs_claims = rel.is_some()
        && required == Ok(true)
        && mesh_enabled
        && !task
            .as_ref()
            .zip(rel.as_deref())
            .is_some_and(|((_, scope), r)| scope.iter().any(|s| covers(s, r)));
    let claims = if needs_claims {
        claims()
    } else {
        ClaimSource::Claims(Vec::new())
    };
    judge_facts(
        path,
        rel.as_deref(),
        EditFacts {
            required,
            mesh_enabled,
            task: task
                .as_ref()
                .map(|(id, scope)| (id.as_str(), scope.as_slice())),
            session,
            claims,
        },
    )
}

/// The path a provider's pre-tool payload names, and the provider's own session id: the
/// fields Claude Code sends (`tool_input.file_path`, `tool_input.notebook_path`,
/// `session_id`). `None` when the payload names no path — a tool that writes no file.
///
/// ```
/// use majordomus_cli::guard::from_payload;
///
/// let p = r#"{"session_id":"s1","tool_name":"Edit","tool_input":{"file_path":"/r/lib/a.sh"}}"#;
/// assert_eq!(from_payload(p), Some(("/r/lib/a.sh".to_string(), Some("s1".to_string()))));
/// let n = r#"{"session_id":"s1","tool_input":{"notebook_path":"/r/n.ipynb"}}"#;
/// assert_eq!(from_payload(n).unwrap().0, "/r/n.ipynb");
/// assert_eq!(from_payload(r#"{"tool_input":{"command":"ls"}}"#), None);
/// assert_eq!(from_payload("not json"), None);
/// ```
pub fn from_payload(payload: &str) -> Option<(String, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let input = &v["tool_input"];
    let path = input["file_path"]
        .as_str()
        .or_else(|| input["notebook_path"].as_str())?
        .to_string();
    let session = v["session_id"].as_str().map(str::to_string);
    Some((path, session))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(session: &str, scope: &[&str], state: ClaimState) -> ClaimView {
        let stream = "641bdb94dae9f62268abf5ca73c11e15-94bb82f89e960c38-daeb650615a562b1";
        serde_json::from_value(serde_json::json!({
            "key": format!("{stream}/c-1"),
            "stream": stream,
            "runtime": "641bdb94dae9f62268abf5ca73c11e15-94bb82f89e960c38",
            "session": format!("{stream}/{session}"),
            "scope": scope,
            "mode": "advisory",
            "acquired_lamport": 1,
            "acquired_seq": 1,
            "state": state,
        }))
        .expect("a claim view")
    }

    fn facts<'a>(task: Option<(&'a str, &'a [String])>, claims: ClaimSource) -> EditFacts<'a> {
        EditFacts {
            required: Ok(true),
            mesh_enabled: true,
            task,
            session: Some("claude-code-s1"),
            claims,
        }
    }

    #[test]
    fn a_claim_covers_its_path_its_descendants_and_the_root_covers_all() {
        assert!(covers(".", "docs/a.md"));
        assert!(!covers(".", ""));
        assert!(covers("docs/", "docs/a.md"));
        assert!(!covers("doc", "docs/a.md"));
    }

    #[test]
    fn a_path_is_placed_lexically_inside_or_outside_the_checkout() {
        let root = Path::new("/work/repo");
        assert_eq!(
            relative_to(root, "./docs/../lib/a.sh").as_deref(),
            Some("lib/a.sh")
        );
        assert_eq!(relative_to(root, "/work/other/a.sh"), None);
        assert_eq!(
            relative_to(root, "/work/repo"),
            None,
            "the root itself is no file"
        );
        let relative = Path::new("./repo");
        assert_eq!(relative_to(relative, "a.sh").as_deref(), Some("a.sh"));
    }

    #[test]
    fn a_task_that_claims_elsewhere_is_named_in_the_refusal() {
        let scope = ["lib".to_string()];
        let v = judge_facts(
            "docs/a.md",
            Some("docs/a.md"),
            facts(Some(("t-1", &scope[..])), ClaimSource::Claims(vec![])),
        );
        assert!(!v.allowed);
        assert!(v.detail.contains("(t-1)"), "{}", v.detail);
    }

    #[test]
    fn without_a_session_only_a_task_is_offered_as_the_remedy() {
        let f = EditFacts {
            session: None,
            ..facts(None, ClaimSource::Claims(vec![]))
        };
        let v = judge_facts("docs/a.md", Some("docs/a.md"), f);
        assert!(!v.allowed);
        let remedy = v.remedy.unwrap();
        assert!(remedy.contains("majordomus start"), "{remedy}");
        assert!(!remedy.contains("--session"), "{remedy}");
    }

    #[test]
    fn the_sessions_live_claim_covers_the_edit() {
        let c = vec![claim("claude-code-s1", &["docs"], ClaimState::Held)];
        let v = judge_facts(
            "docs/a.md",
            Some("docs/a.md"),
            facts(None, ClaimSource::Claims(c)),
        );
        assert!(v.allowed, "{v:?}");
        assert_eq!(v.ground, "claim");
    }

    #[test]
    fn another_sessions_claim_does_not() {
        let c = vec![claim("claude-code-s2", &["docs"], ClaimState::Held)];
        let v = judge_facts(
            "docs/a.md",
            Some("docs/a.md"),
            facts(None, ClaimSource::Claims(c)),
        );
        assert!(!v.allowed);
        assert_eq!(v.ground, "unclaimed");
    }

    #[test]
    fn a_released_claim_does_not() {
        let c = vec![claim("claude-code-s1", &["docs"], ClaimState::Released)];
        let v = judge_facts(
            "docs/a.md",
            Some("docs/a.md"),
            facts(None, ClaimSource::Claims(c)),
        );
        assert!(!v.allowed);
    }

    #[test]
    fn off_no_mesh_local_and_outside_are_let_through() {
        let off = EditFacts {
            required: Ok(false),
            ..facts(None, ClaimSource::Claims(vec![]))
        };
        assert_eq!(judge_facts("a", Some("a"), off).ground, "off");
        let no_mesh = EditFacts {
            mesh_enabled: false,
            ..facts(None, ClaimSource::Claims(vec![]))
        };
        assert_eq!(judge_facts("a", Some("a"), no_mesh).ground, "no_mesh");
        let local = judge_facts(
            ".ai/local/state/x",
            Some(".ai/local/state/x"),
            facts(None, ClaimSource::Claims(vec![])),
        );
        assert_eq!(local.ground, "local");
        assert_eq!(
            judge_facts("/elsewhere", None, facts(None, ClaimSource::Claims(vec![]))).ground,
            "outside"
        );
    }

    #[test]
    fn an_unreadable_policy_enforces_nothing() {
        let f = EditFacts {
            required: Err("no manifest".into()),
            ..facts(None, ClaimSource::Claims(vec![]))
        };
        let v = judge_facts("a", Some("a"), f);
        assert!(v.allowed);
        assert_eq!(v.ground, "undecided");
    }

    /// A repository made of the shipped skeleton's manifest and policy, with the policy
    /// switch and the mesh declaration as asked.
    fn repository(required: bool, mesh: bool) -> tempfile::TempDir {
        let skeleton = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/skeleton");
        let dir = tempfile::tempdir().unwrap();
        let ai = dir.path().join(".ai");
        std::fs::create_dir_all(ai.join("repo/mesh")).unwrap();
        std::fs::copy(skeleton.join("ai/manifest.yaml"), ai.join("manifest.yaml")).unwrap();
        let policy = std::fs::read_to_string(skeleton.join("policy.yaml")).unwrap();
        let line = "edits_require_claim: false";
        assert!(policy.contains(line), "the skeleton declares the switch");
        let policy = policy.replace(line, &format!("edits_require_claim: {required}"));
        std::fs::write(ai.join("repo/policy.yaml"), policy).unwrap();
        std::fs::write(ai.join("repo/mesh/mesh.yaml"), format!("enabled: {mesh}\n")).unwrap();
        dir
    }

    #[test]
    fn the_policy_switch_is_read_from_the_repository() {
        assert!(
            !crate::policy::SessionPolicy::default().edits_require_claim,
            "off unless a repository turns it on"
        );
        assert_eq!(edits_require_claim(repository(true, true).path()), Ok(true));
        assert_eq!(
            edits_require_claim(repository(false, true).path()),
            Ok(false)
        );
        let bare = tempfile::tempdir().unwrap();
        assert!(edits_require_claim(bare.path()).is_err());
        let broken = repository(true, true);
        std::fs::write(broken.path().join(".ai/repo/policy.yaml"), "session: [\n").unwrap();
        assert!(
            edits_require_claim(broken.path()).is_err(),
            "a policy that does not parse"
        );
    }

    #[test]
    fn the_mesh_declaration_is_read_from_its_yaml_files() {
        assert!(mesh_declared_enabled(repository(true, true).path()));
        let bare = tempfile::tempdir().unwrap();
        assert!(!mesh_declared_enabled(bare.path()), "no mesh directory");
        let off = repository(true, false);
        assert!(!mesh_declared_enabled(off.path()));
        let mesh = off.path().join(".ai/repo/mesh");
        std::fs::write(mesh.join("README.md"), "enabled: true\n").unwrap();
        assert!(
            !mesh_declared_enabled(off.path()),
            "only a YAML file declares"
        );
        std::fs::write(mesh.join("other.yml"), "enabled: true\n").unwrap();
        assert!(mesh_declared_enabled(off.path()));
    }

    #[test]
    fn an_unclaimed_edit_of_a_repository_on_the_mesh_is_refused() {
        let dir = repository(true, true);
        let asked = std::cell::Cell::new(false);
        let v = judge(dir.path(), "docs/a.md", Some("claude-code-s1"), || {
            asked.set(true);
            ClaimSource::Claims(vec![])
        });
        assert!(asked.get(), "only the claims could decide");
        assert!(!v.allowed);
        assert_eq!(v.ground, "unclaimed");
    }

    #[test]
    fn the_active_tasks_scope_lets_the_edit_through_without_asking_the_mesh() {
        let dir = repository(true, true);
        let state = dir.path().join(".ai/local/state");
        std::fs::create_dir_all(&state).unwrap();
        let record = "id: t-1\noutcome: active\nscope:\n  - docs\n";
        std::fs::write(state.join("current.yaml"), record).unwrap();
        let v = judge(dir.path(), "docs/a.md", None, || {
            unreachable!("the task decides")
        });
        assert_eq!((v.allowed, v.ground.as_str()), (true, "task"));
    }

    #[test]
    fn a_checkout_no_server_serves_has_no_claims_to_read() {
        let dir = repository(true, true);
        assert!(matches!(
            server_claims(dir.path()),
            ClaimSource::Unavailable(_)
        ));
    }

    /// A server that answers every question the same way.
    struct Answering(Result<(u16, serde_json::Value), String>);

    impl Mesh for Answering {
        fn ask(
            &self,
            _: &str,
            _: &str,
            _: Option<serde_json::Value>,
        ) -> Result<(u16, serde_json::Value), String> {
            self.0.clone()
        }
    }

    #[test]
    fn the_servers_answer_is_read_as_claims_or_as_why_not() {
        let held = claim("claude-code-s1", &["docs"], ClaimState::Held);
        let ok = serde_json::json!({ "state": { "claims": [held] } });
        let ClaimSource::Claims(c) = claims_of(&Answering(Ok((200, ok)))) else {
            panic!("a well-formed answer is claims");
        };
        assert_eq!(c.len(), 1);
        let inactive = serde_json::json!({ "active": false, "reason": "not declared" });
        assert!(matches!(
            claims_of(&Answering(Ok((200, inactive)))),
            ClaimSource::Unavailable(w) if w == "the server does not run the mesh: not declared"
        ));
        let silent = serde_json::json!({ "active": false });
        assert!(matches!(
            claims_of(&Answering(Ok((200, silent)))),
            ClaimSource::Unavailable(w) if w.ends_with("no reason given")
        ));
        let malformed = serde_json::json!({ "state": { "claims": "none" } });
        assert!(matches!(
            claims_of(&Answering(Ok((200, malformed)))),
            ClaimSource::Unavailable(w) if w.contains("does not parse")
        ));
        let refused = serde_json::json!({ "error": { "message": "mesh off" } });
        assert!(matches!(
            claims_of(&Answering(Ok((503, refused)))),
            ClaimSource::Unavailable(w) if w.contains("503: mesh off")
        ));
        assert!(matches!(
            claims_of(&Answering(Err("connection refused".into()))),
            ClaimSource::Unavailable(w) if w == "connection refused"
        ));
    }
}
