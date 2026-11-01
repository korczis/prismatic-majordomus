//! The ledger: the append-only record of what happened to this checkout, and the one
//! writer of it in this executable.
//!
//! `lib/common.sh`'s `mj_ledger_append` has been the only writer since the record existed.
//! This is the second, and it exists for one reason: a capability of kind `command` must be
//! able to record what it did, and a capability cannot shell out to the tool that calls it.
//! Two writers of one record is the failure this repository exists to prevent, so the two
//! are held to the same contract rather than left to drift:
//!
//! * the same file — `.ai/local/state/ledger.jsonl`, never tracked, created on first write;
//! * the same envelope — `ts`, `event`, `head`, `branch`, `by`, then `session` when the
//!   caller has an open episode here (ADR 0164), then the event's own payload, in that order;
//!   `head` is `NONE` in an unborn repository and `branch` is `DETACHED` off a branch,
//!   because that is what the shell writes and a reader may not meet two spellings;
//! * the same vocabulary — `share/events.yaml` declares every name the ledger accepts and
//!   the payload keys each one must carry. An unregistered name is an error here exactly as
//!   it is there: before that file existed a mistyped name produced a durable line that
//!   every reader silently ignored.
//!
//! `test/cases/133_plan_transition.sh` holds the two writers to byte equality: it moves one
//! issue through each engine and compares the envelope and the key order of the lines they
//! append. Where they disagree the shell is the incumbent and is right; this file is the one
//! that changes.
//!
//! ```
//! use majordomus_cli::git::{GitInfo, GitState};
//! use majordomus_cli::ledger::{self, Vocabulary};
//! # let dir = std::env::temp_dir().join(format!("mj-ledger-doc-{}", std::process::id()));
//! # let _ = std::fs::remove_dir_all(&dir);
//! # std::fs::create_dir_all(&dir).unwrap();
//! # std::fs::write(dir.join("events.yaml"),
//! #   "version: 1\nevents:\n  - id: plan_start\n    requires: [issue]\n").unwrap();
//! let vocabulary = Vocabulary::load(&dir.join("events.yaml")).unwrap();
//! let git = GitState::Available(GitInfo {
//!     toplevel: dir.clone(), head: Some("a".repeat(40)),
//!     branch: Some("master".into()), working_tree: "clean".into(),
//! });
//!
//! // One line, appended; the envelope is composed in a fixed order and the payload follows.
//! let line = ledger::append(&dir, &vocabulary, &git, "2026-09-11T12:00:00Z",
//!                           "plan_start", &[("issue", "I0001".into())]).unwrap();
//! assert!(line.starts_with(r#"{"ts":"2026-09-11T12:00:00Z","event":"plan_start""#));
//!
//! // A name nothing declares never reaches the file, which is why the vocabulary exists.
//! assert!(ledger::append(&dir, &vocabulary, &git, "2026-09-11T12:00:00Z",
//!                        "plan_strat", &[("issue", "I0001".into())]).is_err());
//!
//! let (entries, skipped) = ledger::read(&dir);
//! assert_eq!((entries.len(), skipped), (1, 0));
//! # std::fs::remove_dir_all(&dir).unwrap();
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git::{GitInfo, GitState};
use crate::metadata::yaml;

/// Where the untracked half of the layer lives, relative to the repository root.
const STATE_DIR: &str = ".ai/local/state";
/// The record itself.
const LEDGER: &str = "ledger.jsonl";
/// The open episodes of this checkout: one record per provider session, under `STATE_DIR`.
const OPEN_DIR: &str = "sessions-open";
/// The checkout's active task, under `STATE_DIR`.
const CURRENT: &str = "current.yaml";
/// The variables a lifecycle adapter declares for the provider session a process runs
/// inside, read after `MAJORDOMUS_PROVIDER_SESSION` in this order: column 13 of
/// `MJ_CAPTURE_LIFECYCLE` in `lib/capture.sh`, which `mj_provider_session_env` reads and a
/// unit test here holds this list equal to. A provider that declares no variable gets
/// unattributed records when several episodes are open, never misattributed ones (ADR 0164).
pub const PROVIDER_SESSION_VARS: &[&str] = &["CLAUDE_CODE_SESSION_ID"];

/// Why a line was not written.
///
/// Every variant is a refusal to corrupt the record, never a partial write: the line is
/// composed and validated in full before the file is opened.
///
/// ```
/// use majordomus_cli::ledger::LedgerError;
/// let e = LedgerError::MissingField { event: "plan_evidence".into(), field: "covers".into() };
/// assert_eq!(e.to_string(), "event 'plan_evidence' is missing the required field 'covers'");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The event name is not declared in `share/events.yaml`.
    UnknownEvent {
        /// The name that was offered.
        event: String,
        /// Every name that is declared, in declaration order.
        known: Vec<String>,
    },
    /// The event is declared, but the payload omits a key the declaration requires.
    MissingField {
        /// The event.
        event: String,
        /// The key that is required and absent.
        field: String,
    },
    /// `share/events.yaml` could not be read or does not parse.
    Vocabulary {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        reason: String,
    },
    /// The line was composed and the file could not be appended to.
    Write {
        /// The ledger.
        path: PathBuf,
        /// What the filesystem said.
        reason: String,
    },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEvent { event, known } => write!(
                f,
                "unregistered event '{event}'; declare it in share/events.yaml or use one of: {}",
                known.join(" ")
            ),
            Self::MissingField { event, field } => {
                write!(f, "event '{event}' is missing the required field '{field}'")
            }
            Self::Vocabulary { path, reason } => write!(f, "{}: {reason}", path.display()),
            Self::Write { path, reason } => write!(f, "{}: {reason}", path.display()),
        }
    }
}

impl std::error::Error for LedgerError {}

/// One event of the vocabulary, as `share/events.yaml` declares it.
///
/// Only the three fields a writer must honour are read. The rest of the declaration —
/// `title`, `summary`, `public`, `note` — is what documentation and `history` project, and
/// is deliberately not modelled here: a writer that grew opinions about prose would be a
/// second reason for that file to change.
#[derive(Debug, Clone, Deserialize)]
struct EventDecl {
    /// The name, as written into the record.
    id: String,
    /// Payload keys every line of this type must carry, beyond the envelope.
    #[serde(default)]
    requires: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EventsFile {
    #[serde(default)]
    events: Vec<EventDecl>,
}

/// The event vocabulary, read once and shared.
///
/// ```
/// use majordomus_cli::ledger::Vocabulary;
/// // The default is empty, and an empty vocabulary accepts nothing: a writer that could
/// // not read the declaration refuses every name rather than allowing any.
/// let v = Vocabulary::default();
/// assert!(v.ids().is_empty());
/// assert!(v.requires("plan_start").is_none());
/// ```
#[derive(Debug, Clone, Default)]
pub struct Vocabulary {
    events: BTreeMap<String, Vec<String>>,
    order: Vec<String>,
}

impl Vocabulary {
    /// Read `share/events.yaml`.
    ///
    /// The file ships with the tool, so a repository never carries its own: a name this
    /// executable does not know is a name nothing in the distribution writes.
    ///
    /// ```
    /// use majordomus_cli::ledger::Vocabulary;
    /// # let f = std::env::temp_dir().join(format!("mj-voc-{}.yaml", std::process::id()));
    /// # std::fs::write(&f, "version: 1\nevents:\n  - id: plan_done\n    requires: [issue]\n").unwrap();
    /// let v = Vocabulary::load(&f).unwrap();
    /// assert_eq!(v.ids(), ["plan_done"]);
    /// assert_eq!(v.requires("plan_done"), Some(&["issue".to_string()][..]));
    /// # std::fs::remove_file(&f).unwrap();
    ///
    /// // A file that is not there is an error, never an empty vocabulary that accepts all.
    /// assert!(Vocabulary::load(std::path::Path::new("/nonexistent/events.yaml")).is_err());
    /// ```
    pub fn load(events_yaml: &Path) -> Result<Self, LedgerError> {
        let text = fs::read_to_string(events_yaml).map_err(|e| LedgerError::Vocabulary {
            path: events_yaml.to_path_buf(),
            reason: e.to_string(),
        })?;
        let parsed: EventsFile =
            yaml::parse_into(&text).map_err(|reason| LedgerError::Vocabulary {
                path: events_yaml.to_path_buf(),
                reason,
            })?;
        let mut events = BTreeMap::new();
        let mut order = Vec::new();
        for e in parsed.events {
            order.push(e.id.clone());
            events.insert(e.id, e.requires);
        }
        Ok(Self { events, order })
    }

    /// Every declared name, in declaration order. What a refusal lists.
    pub fn ids(&self) -> &[String] {
        &self.order
    }

    /// The keys a line of this event must carry, or `None` when the name is not declared.
    ///
    /// `None` and `Some(&[])` are different answers: the first is a name nothing declares,
    /// the second a declared event whose envelope is all it needs.
    ///
    /// ```
    /// use majordomus_cli::ledger::Vocabulary;
    /// assert!(Vocabulary::default().requires("nothing").is_none());
    /// ```
    pub fn requires(&self, event: &str) -> Option<&[String]> {
        self.events.get(event).map(|v| v.as_slice())
    }
}

/// One payload key and its value, as it will be written.
///
/// A pair rather than a map because order is part of the contract: the shell appends the
/// caller's keys in the order the caller wrote them, and two writers that sort differently
/// produce two spellings of one event.
pub type Field<'a> = (&'a str, String);

/// Append one line to the ledger of `root`.
///
/// The line is composed in full, validated against the vocabulary, and only then appended.
/// `now` is passed in rather than read here so that a test can write a line whose timestamp
/// it knows; every caller in the executable passes [`now`].
///
/// ```
/// use majordomus_cli::git::GitState;
/// use majordomus_cli::ledger::{self, Vocabulary};
/// # let dir = std::env::temp_dir().join(format!("mj-append-{}", std::process::id()));
/// # let _ = std::fs::remove_dir_all(&dir);
/// # std::fs::create_dir_all(&dir).unwrap();
/// # std::fs::write(dir.join("events.yaml"),
/// #   "version: 1\nevents:\n  - id: plan_done\n    requires: [issue]\n").unwrap();
/// let vocabulary = Vocabulary::load(&dir.join("events.yaml")).unwrap();
/// let git = GitState::Unavailable { reason: "not a work tree".into() };
///
/// let line = ledger::append(&dir, &vocabulary, &git, "2026-09-11T12:00:00Z",
///                           "plan_done", &[("issue", "I0001".into())]).unwrap();
/// // Where git cannot answer, the two fields are spelled with the literals the shell uses,
/// // so a reader never meets a second spelling of "unknown".
/// assert!(line.contains(r#""head":"NONE""#), "{line}");
/// assert!(line.contains(r#""branch":"DETACHED""#), "{line}");
///
/// // A declared event whose payload omits a required key writes nothing at all.
/// let err = ledger::append(&dir, &vocabulary, &git, "2026-09-11T12:00:00Z",
///                          "plan_done", &[]).unwrap_err();
/// assert_eq!(err.to_string(), "event 'plan_done' is missing the required field 'issue'");
/// assert_eq!(ledger::read(&dir).0.len(), 1, "the refusal appended nothing");
/// # std::fs::remove_dir_all(&dir).unwrap();
/// ```
pub fn append(
    root: &Path,
    vocabulary: &Vocabulary,
    git: &GitState,
    now: &str,
    event: &str,
    payload: &[Field<'_>],
) -> Result<String, LedgerError> {
    append_as(root, vocabulary, git, now, event, payload, None)
}

/// [`append`], for a caller whose episode is known in-process rather than from its
/// environment: `session_key` is the provider session (an MCP connection's `external_id`)
/// whose open record names the episode, resolved exactly as `MJ_SESSION_KEY` is
/// ([`open_session_id_with`]). `None` resolves from the environment, which is what the
/// command line and a plain HTTP request are.
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_as(
    root: &Path,
    vocabulary: &Vocabulary,
    git: &GitState,
    now: &str,
    event: &str,
    payload: &[Field<'_>],
    session_key: Option<&str>,
) -> Result<String, LedgerError> {
    let requires = vocabulary
        .requires(event)
        .ok_or_else(|| LedgerError::UnknownEvent {
            event: event.to_string(),
            known: vocabulary.ids().to_vec(),
        })?;
    for key in requires {
        if !payload.iter().any(|(k, _)| k == key) {
            return Err(LedgerError::MissingField {
                event: event.to_string(),
                field: key.clone(),
            });
        }
    }

    let (head, branch) = match git {
        GitState::Available(GitInfo { head, branch, .. }) => (
            head.clone().unwrap_or_else(|| "NONE".into()),
            branch.clone().unwrap_or_else(|| "DETACHED".into()),
        ),
        // What the shell writes when git cannot answer: `mj_git_head` falls back to NONE
        // and `mj_git_branch` to DETACHED, each independently of why git was silent.
        GitState::Unavailable { .. } => ("NONE".into(), "DETACHED".into()),
    };

    let mut line = String::from("{");
    push_pair(&mut line, "ts", now);
    line.push(',');
    push_pair(&mut line, "event", event);
    line.push(',');
    push_pair(&mut line, "head", &head);
    line.push(',');
    push_pair(&mut line, "branch", &branch);
    line.push(',');
    push_pair(&mut line, "by", &format!("majordomus/{}", crate::VERSION));
    if let Some(session) = open_session_id(root, session_key) {
        line.push(',');
        push_pair(&mut line, "session", &session);
    }
    for (k, v) in payload {
        line.push(',');
        push_pair(&mut line, k, v);
    }
    line.push('}');

    let dir = root.join(STATE_DIR);
    fs::create_dir_all(&dir).map_err(|e| LedgerError::Write {
        path: dir.clone(),
        reason: e.to_string(),
    })?;
    let path = dir.join(LEDGER);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| LedgerError::Write {
            path: path.clone(),
            reason: e.to_string(),
        })?;
    writeln!(f, "{line}").map_err(|e| LedgerError::Write {
        path,
        reason: e.to_string(),
    })?;
    Ok(line)
}

/// The envelope's timestamp, in the one spelling both writers use: `date -u
/// +%Y-%m-%dT%H:%M:%SZ`, to the second, always UTC.
///
/// [`crate::peers::rfc3339`] already renders that spelling for the peer board; a second
/// clock in this file would be a second answer to what time it is.
///
/// ```
/// let t = majordomus_cli::ledger::now();
/// assert_eq!(t.len(), 20, "{t}");
/// assert!(t.ends_with('Z') && t.contains('T'), "{t}");
/// ```
pub fn now() -> String {
    crate::peers::rfc3339(std::time::SystemTime::now())
}

/// `"key":"value"`, with the value escaped as JSON.
///
/// Every field the ledger carries is a string, in both writers. A payload that wanted a
/// number would be a change to the record's shape, not something a caller may decide.
fn push_pair(out: &mut String, key: &str, value: &str) {
    out.push('"');
    out.push_str(key);
    out.push_str("\":");
    escape_json(out, value);
}

/// Append `value` as a JSON string literal, quotes included.
fn escape_json(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// What a durable record says about who wrote it (ADR 0164): the caller's own episode, when
/// it has one open here, and the active task, unless another episode that is still open
/// opened it. Prepared once before a record is published, so that its ledger line and its
/// body say the same thing — the Rust reading of `mj_record_attribution`.
///
/// ```
/// use majordomus_cli::ledger::{attribution_with, Attribution};
/// # let dir = std::env::temp_dir().join(format!("mj-attr-doc-{}", std::process::id()));
/// # let _ = std::fs::remove_dir_all(&dir);
/// # std::fs::create_dir_all(dir.join(".ai/local/state/sessions-open")).unwrap();
/// let state = dir.join(".ai/local/state");
/// std::fs::write(state.join("sessions-open/a.yaml"), "session_id: s-a\n").unwrap();
/// std::fs::write(state.join("current.yaml"), "id: t-1\n").unwrap();
/// std::fs::write(state.join("ledger.jsonl"),
///     r#"{"ts":"1","event":"task.started","head":"h","branch":"b","by":"x","session":"s-a","task_id":"t-1"}"#).unwrap();
/// // The one open episode is the caller's, and its task is its own.
/// let a = attribution_with(&dir, None, |_| None);
/// assert_eq!(a, Attribution { session: Some("s-a".into()), task: Some("t-1".into()) });
/// // A second episode: B is B, and A's task is not related to B's record.
/// std::fs::write(state.join("sessions-open/b.yaml"), "session_id: s-b\n").unwrap();
/// let b = attribution_with(&dir, Some("b"), |_| None);
/// assert_eq!(b, Attribution { session: Some("s-b".into()), task: None });
/// # std::fs::remove_dir_all(&dir).unwrap();
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attribution {
    /// The episode the caller runs inside, when it has one open in this checkout.
    pub session: Option<String>,
    /// The active task, when it is the caller's to relate.
    pub task: Option<String>,
}

/// [`attribution_with`], reading the caller's identity from this process's environment.
pub(crate) fn attribution(root: &Path, key: Option<&str>) -> Attribution {
    attribution_with(root, key, env_var)
}

/// The attribution of a record written now by the caller `key` names, or by whoever the
/// environment `env` says is running (see [`open_session_id_with`]).
///
/// The task is related unless the session in its unique `task.started` line is a different
/// episode that is still open here: the one case a record could be filed under somebody
/// else's work. A task whose opener has closed, or whose opening names no episode, has no
/// live owner, and the caller continuing it carries it on. With no episode at all the
/// workers cannot be told apart and the task is the caller's (ADR 0164).
///
/// ```
/// use majordomus_cli::ledger::{attribution_with, Attribution};
/// # let dir = std::env::temp_dir().join(format!("mj-attr-with-doc-{}", std::process::id()));
/// # let _ = std::fs::remove_dir_all(&dir);
/// # std::fs::create_dir_all(dir.join(".ai/local/state")).unwrap();
/// // Nothing open, no task: a record written here belongs to nobody and relates nothing.
/// assert_eq!(attribution_with(&dir, None, |_| None), Attribution::default());
/// // A task and no episode at all: the workers cannot be told apart, so it is the caller's.
/// std::fs::write(dir.join(".ai/local/state/current.yaml"), "id: t-1\n").unwrap();
/// assert_eq!(attribution_with(&dir, None, |_| None).task.as_deref(), Some("t-1"));
/// # std::fs::remove_dir_all(&dir).unwrap();
/// ```
pub fn attribution_with(
    root: &Path,
    key: Option<&str>,
    env: impl Fn(&str) -> Option<String>,
) -> Attribution {
    let session = open_session_id_with(root, key, &env);
    let task = current_task_here(root).filter(|id| match task_opening_session(root, id) {
        Some(opened) => {
            Some(opened.as_str()) == session.as_deref() || !session_is_open(root, &opened)
        }
        None => true,
    });
    Attribution { session, task }
}

/// [`open_session_id_with`], reading the caller's identity from this process's environment.
pub(crate) fn open_session_id(root: &Path, key: Option<&str>) -> Option<String> {
    open_session_id_with(root, key, env_var)
}

/// The caller's open episode in this checkout, or none — `mj_open_session_id`.
///
/// The caller's own identity first: `key` when the caller knows it in-process (an MCP
/// connection's episode), else `MJ_SESSION_KEY`, else the provider session the environment
/// names (`MAJORDOMUS_PROVIDER_SESSION`, then [`PROVIDER_SESSION_VARS`]). An explicit
/// identity names its own open record under `sessions-open/` or none, never the checkout's
/// pointer. With no identity, exactly one open episode of this checkout is unambiguous and
/// is the answer; several are nobody's, because the last of several open episodes is not a
/// writer's identity. A record that names another worktree is another checkout's episode.
///
/// ```
/// use majordomus_cli::ledger::open_session_id_with;
/// # let dir = std::env::temp_dir().join(format!("mj-osid-doc-{}", std::process::id()));
/// # let _ = std::fs::remove_dir_all(&dir);
/// # std::fs::create_dir_all(dir.join(".ai/local/state/sessions-open")).unwrap();
/// let open = dir.join(".ai/local/state/sessions-open");
/// std::fs::write(open.join("a.yaml"), "session_id: s-a\n").unwrap();
/// std::fs::write(open.join("b.yaml"), "session_id: s-b\n").unwrap();
/// let env = |k: &str| (k == "CLAUDE_CODE_SESSION_ID").then(|| "b".to_string());
/// assert_eq!(open_session_id_with(&dir, Some("a"), env).as_deref(), Some("s-a"));
/// assert_eq!(open_session_id_with(&dir, None, env).as_deref(), Some("s-b"));
/// // two open episodes and no identity: nobody's, not the last one's
/// assert_eq!(open_session_id_with(&dir, None, |_| None), None);
/// // an identity with no record here is no episode, whatever else is open
/// assert_eq!(open_session_id_with(&dir, Some("missing"), env), None);
/// # std::fs::remove_dir_all(&dir).unwrap();
/// ```
pub fn open_session_id_with(
    root: &Path,
    key: Option<&str>,
    env: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let supplied = key
        .map(str::to_string)
        .filter(|k| !k.is_empty())
        .or_else(|| env("MJ_SESSION_KEY"))
        .or_else(|| env("MAJORDOMUS_PROVIDER_SESSION"))
        .or_else(|| PROVIDER_SESSION_VARS.iter().find_map(|v| env(v)));
    match supplied {
        Some(k) => {
            let file = root
                .join(STATE_DIR)
                .join(OPEN_DIR)
                .join(format!("{}.yaml", session_key(&k)));
            open_record_here(root, &file)
        }
        None => {
            let mut here = open_records_here(root);
            if here.len() != 1 {
                return None;
            }
            here.pop()
        }
    }
}

/// The episode that opened task `id`: the session of its unique `task.started` line.
/// Missing or ambiguous history supplies no identity — `mj_task_opening_session`.
fn task_opening_session(root: &Path, id: &str) -> Option<String> {
    let (entries, _) = read(root);
    let mut starts = entries.iter().filter(|e| {
        e.event == "task.started" && e.payload.get("task_id").and_then(|v| v.as_str()) == Some(id)
    });
    let first = starts.next()?;
    if starts.next().is_some() {
        return None;
    }
    first.session.clone().filter(|s| !s.is_empty())
}

/// Whether episode `session` is still open in this checkout — `mj_session_is_open`.
fn session_is_open(root: &Path, session: &str) -> bool {
    open_records_here(root).iter().any(|s| s == session)
}

/// A provider session reduced to the one path segment its open record is named by, exactly
/// as `mj_session_key` spells it: every byte outside `A-Za-z0-9._-` becomes `-`, runs of
/// `-` are squeezed, the result is cut to 64 and stripped of leading and trailing `-`;
/// an empty or unnameable value is the hand-opened episode.
///
/// ```
/// use majordomus_cli::ledger::session_key;
/// assert_eq!(session_key("01Bv2gJsf"), "01Bv2gJsf");
/// assert_eq!(session_key("a/b c"), "a-b-c");
/// assert_eq!(session_key("--x--"), "x");
/// assert_eq!(session_key(""), "hand");
/// assert_eq!(session_key("///"), "hand");
/// ```
pub fn session_key(value: &str) -> String {
    let mut mapped = String::new();
    let mut last_dash = false;
    for c in value.chars() {
        let c = if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            c
        } else {
            '-'
        };
        if c == '-' {
            if last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }
        mapped.push(c);
    }
    let cut: String = mapped.chars().take(64).collect();
    let key = cut.trim_matches('-');
    if key.is_empty() {
        "hand".to_string()
    } else {
        key.to_string()
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The session ids of every open record that is this checkout's. Unordered: the callers
/// count them and take the one, or ask whether a given id is among them.
fn open_records_here(root: &Path) -> Vec<String> {
    let dir = root.join(STATE_DIR).join(OPEN_DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .filter_map(|f| open_record_here(root, &f))
        .collect()
}

/// The session id of one open record, when the record is this checkout's.
fn open_record_here(root: &Path, file: &Path) -> Option<String> {
    let text = fs::read_to_string(file).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    let worktree = map
        .get("worktree")
        .and_then(yaml::scalar_string)
        .unwrap_or_default();
    if !same_checkout(root, &worktree) {
        return None;
    }
    map.get("session_id")
        .and_then(yaml::scalar_string)
        .filter(|s| !s.is_empty())
}

/// The active task of this checkout, when its record is this checkout's.
fn current_task_here(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join(STATE_DIR).join(CURRENT)).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    let worktree = map
        .get("worktree")
        .and_then(yaml::scalar_string)
        .unwrap_or_default();
    if !same_checkout(root, &worktree) {
        return None;
    }
    map.get("id")
        .and_then(yaml::scalar_string)
        .filter(|s| !s.is_empty())
}

/// Whether a record's `worktree` is this checkout: absent, or the same directory under any
/// spelling. Another worktree's record is another checkout's episode or task.
fn same_checkout(root: &Path, worktree: &str) -> bool {
    if worktree.is_empty() || Path::new(worktree) == root {
        return true;
    }
    match (fs::canonicalize(worktree), fs::canonicalize(root)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// One line of the record, as a reader sees it.
///
/// The envelope is typed and the payload is left as it was written: a reader of one event
/// knows its own keys, and this type may not grow a field every time an event does.
///
/// ```
/// use majordomus_cli::ledger::Entry;
/// let e: Entry = serde_json::from_str(
///     r#"{"ts":"2026-09-11T12:00:00Z","event":"plan_done","head":"abc","branch":"master",
///         "by":"majordomus/0.5.0","issue":"I0001"}"#).unwrap();
/// assert_eq!(e.event, "plan_done");
/// assert!(e.session.is_none(), "no episode was open when this was written");
/// // Everything the envelope does not name stays in the payload, whatever the event is.
/// assert_eq!(e.payload["issue"], "I0001");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// When it happened.
    pub ts: String,
    /// The event name, from the vocabulary.
    pub event: String,
    /// The commit the checkout was at.
    pub head: String,
    /// The branch it was on.
    pub branch: String,
    /// What wrote it: `majordomus/<version>`.
    pub by: String,
    /// The episode it belongs to, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Everything else the line carried.
    #[serde(flatten)]
    pub payload: BTreeMap<String, serde_json::Value>,
}

/// Read the record of `root`, oldest first.
///
/// A line that does not parse is skipped rather than fatal: the ledger is append-only and
/// survives every version of every writer that ever appended to it, so a reader that
/// refused the whole file over one malformed line would lose the history it exists to show.
/// The count of skipped lines is returned beside the entries so the loss is never silent.
///
/// ```
/// // A checkout that has never written one has no record, which is an empty answer rather
/// // than an error: absence is a state the lifecycle has, not a failure to read.
/// let (entries, skipped) = majordomus_cli::ledger::read(std::path::Path::new("/nonexistent"));
/// assert!(entries.is_empty() && skipped == 0);
/// ```
pub fn read(root: &Path) -> (Vec<Entry>, usize) {
    let path = root.join(STATE_DIR).join(LEDGER);
    let Ok(text) = fs::read_to_string(path) else {
        return (Vec::new(), 0);
    };
    let mut entries = Vec::new();
    let mut skipped = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Entry>(line) {
            Ok(e) => entries.push(e),
            Err(_) => skipped += 1,
        }
    }
    (entries, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary() -> Vocabulary {
        let mut events = BTreeMap::new();
        events.insert("plan_start".to_string(), vec!["issue".to_string()]);
        events.insert(
            "plan_evidence".to_string(),
            vec!["issue".to_string(), "covers".to_string()],
        );
        Vocabulary {
            events,
            order: vec!["plan_start".into(), "plan_evidence".into()],
        }
    }

    fn git() -> GitState {
        GitState::Available(GitInfo {
            toplevel: PathBuf::from("/r"),
            head: Some("a".repeat(40)),
            branch: Some("master".into()),
            working_tree: "clean".into(),
        })
    }

    /// The envelope, in the order and spelling `mj_ledger_append` writes it. This is the
    /// assertion that fails when the two writers drift.
    #[test]
    fn the_envelope_is_the_shells_envelope() {
        let dir = tempdir();
        let line = append(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "i-1".into())],
        )
        .expect("the event is declared and its payload complete");
        assert_eq!(
            line,
            format!(
                r#"{{"ts":"2026-09-11T12:00:00Z","event":"plan_start","head":"{}","branch":"master","by":"majordomus/{}","issue":"i-1"}}"#,
                "a".repeat(40),
                crate::VERSION
            )
        );
    }

    /// An unborn repository and a detached head are the two states the shell spells with
    /// literals rather than leaving empty; a reader may not meet a third spelling.
    #[test]
    fn an_unavailable_git_is_none_and_detached() {
        let dir = tempdir();
        let line = append(
            dir.path(),
            &vocabulary(),
            &GitState::Unavailable {
                reason: "not a work tree".into(),
            },
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "i-1".into())],
        )
        .unwrap();
        assert!(line.contains(r#""head":"NONE""#), "{line}");
        assert!(line.contains(r#""branch":"DETACHED""#), "{line}");
    }

    /// The refusal that is the whole reason `share/events.yaml` exists: a name nothing
    /// declares never reaches the file.
    #[test]
    fn an_unregistered_event_is_refused_and_writes_nothing() {
        let dir = tempdir();
        let err = append(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_strat",
            &[("issue", "i-1".into())],
        )
        .unwrap_err();
        assert!(matches!(err, LedgerError::UnknownEvent { .. }));
        assert!(
            !dir.path().join(STATE_DIR).join(LEDGER).exists(),
            "a refused event created the ledger"
        );
    }

    /// A declared event whose payload omits a required key is refused the same way: the
    /// declaration is a contract on the line, not documentation beside it.
    #[test]
    fn a_missing_required_field_is_refused() {
        let dir = tempdir();
        let err = append(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_evidence",
            &[("issue", "i-1".into())],
        )
        .unwrap_err();
        assert_eq!(
            err,
            LedgerError::MissingField {
                event: "plan_evidence".into(),
                field: "covers".into()
            }
        );
    }

    /// A value carrying a quote, a backslash or a newline must not be able to end the
    /// string it sits in. An issue title is repository content and reaches this function
    /// unmodified.
    #[test]
    fn a_payload_value_cannot_escape_its_string() {
        let dir = tempdir();
        let line = append(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "a\"b\\c\nd\te".into())],
        )
        .unwrap();
        assert!(line.contains(r#""issue":"a\"b\\c\nd\te""#), "{line}");
        let parsed: serde_json::Value =
            serde_json::from_str(&line).expect("the line is valid JSON whatever the value held");
        assert_eq!(parsed["issue"], "a\"b\\c\nd\te");
    }

    /// Append-only, and the lines come back in the order they were written.
    #[test]
    fn the_record_is_append_only_and_reads_back_in_order() {
        let dir = tempdir();
        for id in ["i-1", "i-2", "i-3"] {
            append(
                dir.path(),
                &vocabulary(),
                &git(),
                "2026-09-11T12:00:00Z",
                "plan_start",
                &[("issue", id.into())],
            )
            .unwrap();
        }
        let (entries, skipped) = read(dir.path());
        assert_eq!(skipped, 0);
        let ids: Vec<&str> = entries
            .iter()
            .map(|e| e.payload["issue"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["i-1", "i-2", "i-3"]);
    }

    /// A line no reader can parse costs that line and is counted, never the file.
    #[test]
    fn an_unreadable_line_is_skipped_and_counted() {
        let dir = tempdir();
        let state = dir.path().join(STATE_DIR);
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join(LEDGER), "not json\n{\"ts\":\"t\",\"event\":\"plan_start\",\"head\":\"h\",\"branch\":\"b\",\"by\":\"x\",\"issue\":\"i-1\"}\n").unwrap();
        let (entries, skipped) = read(dir.path());
        assert_eq!(skipped, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event, "plan_start");
    }

    // ------------------------------------------------------------ who wrote it (ADR 0164)

    /// The fixture of case 3049 in miniature: two episodes open in one checkout, A's task
    /// active. Every writer that uses the resolver gets the same answers the shell's
    /// `mj_record_attribution` gives, and none of them reads the pointer.
    fn two_episodes(dir: &TempDir) -> PathBuf {
        // the directory outlives this call: the caller holds it
        let state = dir.path().join(STATE_DIR);
        fs::create_dir_all(state.join(OPEN_DIR)).unwrap();
        let here = dir.path().display().to_string();
        fs::write(
            state.join(OPEN_DIR).join("worker-a.yaml"),
            format!("session_id: s-a\nworktree: {here}\n"),
        )
        .unwrap();
        fs::write(
            state.join(OPEN_DIR).join("worker-b.yaml"),
            format!("session_id: s-b\nworktree: {here}\n"),
        )
        .unwrap();
        // the pointer names B, who opened last; it is a status answer, never an identity
        fs::write(
            state.join("session-current.yaml"),
            format!("session_id: s-b\nworktree: {here}\n"),
        )
        .unwrap();
        fs::write(state.join(CURRENT), format!("id: t-1\nworktree: {here}\n")).unwrap();
        fs::write(
            state.join(LEDGER),
            concat!(
                r#"{"ts":"1","event":"task.started","head":"h","branch":"b","by":"x","session":"s-a","task_id":"t-1"}"#,
                "\n"
            ),
        )
        .unwrap();
        state
    }

    #[test]
    fn each_caller_is_its_own_episode_and_only_the_opener_relates_the_task() {
        let held = tempdir();
        let dir = two_episodes(&held);
        let root = held.path();
        let none = |_: &str| None;
        // A, by the key its connection holds: its episode, its task.
        assert_eq!(
            attribution_with(root, Some("worker-a"), none),
            Attribution {
                session: Some("s-a".into()),
                task: Some("t-1".into())
            }
        );
        // B, by its provider's environment: its episode, and not A's task.
        let env_b = |k: &str| (k == "CLAUDE_CODE_SESSION_ID").then(|| "worker-b".to_string());
        assert_eq!(
            attribution_with(root, None, env_b),
            Attribution {
                session: Some("s-b".into()),
                task: None
            }
        );
        // MAJORDOMUS_PROVIDER_SESSION and MJ_SESSION_KEY are read before the provider's own.
        let env_both = |k: &str| match k {
            "MJ_SESSION_KEY" => Some("worker-a".to_string()),
            "CLAUDE_CODE_SESSION_ID" => Some("worker-b".to_string()),
            _ => None,
        };
        assert_eq!(
            open_session_id_with(root, None, env_both).as_deref(),
            Some("s-a")
        );
        // An anonymous caller among two episodes is nobody, and A's live task is not its
        // to relate — the pointer that names B is not consulted.
        assert_eq!(attribution_with(root, None, none), Attribution::default());
        // An explicit identity with no record here resolves to none, whatever is open.
        assert_eq!(
            attribution_with(root, Some("missing"), none),
            Attribution::default()
        );
        // The key is the shell's: a value a path segment may not hold names the same file.
        fs::rename(
            dir.join(OPEN_DIR).join("worker-a.yaml"),
            dir.join(OPEN_DIR).join("worker-a-x.yaml"),
        )
        .unwrap();
        assert_eq!(
            open_session_id_with(root, Some("worker a/x"), none).as_deref(),
            Some("s-a")
        );
        // A closes: its task has no live owner, and B continuing it carries it on.
        fs::remove_file(dir.join(OPEN_DIR).join("worker-a-x.yaml")).unwrap();
        assert_eq!(
            attribution_with(root, Some("worker-b"), none),
            Attribution {
                session: Some("s-b".into()),
                task: Some("t-1".into())
            }
        );
        // B alone and anonymous is the one open episode, which is unambiguous.
        assert_eq!(
            attribution_with(root, None, none),
            Attribution {
                session: Some("s-b".into()),
                task: Some("t-1".into())
            }
        );
    }

    /// A task opened twice, or with no session, has no identifiable opener.
    #[test]
    fn an_ambiguous_or_anonymous_opening_names_no_opener() {
        let held = tempdir();
        let dir = two_episodes(&held);
        let root = held.path();
        assert_eq!(task_opening_session(root, "t-1").as_deref(), Some("s-a"));
        assert_eq!(task_opening_session(root, "t-9"), None);
        let mut ledger = OpenOptions::new()
            .append(true)
            .open(dir.join(LEDGER))
            .unwrap();
        writeln!(
            ledger,
            r#"{{"ts":"2","event":"task.started","head":"h","branch":"b","by":"x","session":"s-b","task_id":"t-1"}}"#
        )
        .unwrap();
        assert_eq!(
            task_opening_session(root, "t-1"),
            None,
            "two openings name nobody"
        );
        writeln!(
            ledger,
            r#"{{"ts":"3","event":"task.started","head":"h","branch":"b","by":"x","task_id":"t-2"}}"#
        )
        .unwrap();
        assert_eq!(
            task_opening_session(root, "t-2"),
            None,
            "no session is no opener"
        );
        // and with no identifiable opener the task is every caller's
        fs::write(dir.join(CURRENT), "id: t-2\n").unwrap();
        assert_eq!(
            attribution_with(root, Some("worker-b"), |_| None)
                .task
                .as_deref(),
            Some("t-2")
        );
    }

    /// A record of another worktree is another checkout's, for episodes and for the task.
    #[test]
    fn another_worktrees_records_are_another_checkouts() {
        let held = tempdir();
        let dir = two_episodes(&held);
        let root = held.path();
        fs::write(
            dir.join(OPEN_DIR).join("worker-a.yaml"),
            "session_id: s-a\nworktree: /srv/elsewhere\n",
        )
        .unwrap();
        assert_eq!(open_session_id_with(root, Some("worker-a"), |_| None), None);
        assert!(!session_is_open(root, "s-a"));
        fs::write(dir.join(CURRENT), "id: t-1\nworktree: /srv/elsewhere\n").unwrap();
        assert_eq!(
            attribution_with(root, Some("worker-b"), |_| None).task,
            None
        );
    }

    /// The envelope's `session` is the caller's, through `append_as`, and absent for a
    /// caller that has none — never the pointer's.
    #[test]
    fn the_envelope_names_the_callers_episode_or_nobody() {
        let held = tempdir();
        two_episodes(&held);
        let root = held.path();
        let line = append_as(
            root,
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "i-1".into())],
            Some("worker-a"),
        )
        .unwrap();
        assert!(
            line.contains(r#""by":"majordomus/"#)
                && line.contains(r#","session":"s-a","issue":"i-1"}"#),
            "{line}"
        );
        // The environment of this very test process may name a provider session; the
        // anonymous case is therefore asserted on the resolver, which takes its environment.
        assert_eq!(open_session_id_with(root, None, |_| None), None);
    }

    /// The provider variables this writer reads are the lifecycle adapters' own column, as
    /// `mj_provider_session_env` reads them: one table, in `lib/capture.sh`.
    #[test]
    fn the_provider_session_variables_are_the_lifecycle_tables() {
        let table =
            fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../lib/capture.sh"))
                .expect("lib/capture.sh is in this tree");
        let start = table
            .find("MJ_CAPTURE_LIFECYCLE='")
            .expect("the lifecycle table is declared");
        let rows = &table[start + "MJ_CAPTURE_LIFECYCLE='".len()..];
        let rows = &rows[..rows.find('\'').expect("the table closes")];
        let declared: Vec<&str> = rows
            .lines()
            .filter_map(|row| row.split_whitespace().nth(12))
            .filter(|v| !v.is_empty() && *v != "-")
            .collect();
        assert_eq!(
            declared, PROVIDER_SESSION_VARS,
            "column 13 of MJ_CAPTURE_LIFECYCLE"
        );
    }

    /// The wrappers that read this process's environment answer exactly as their explicit
    /// forms do: a named caller is resolved from the records alone, and an anonymous one
    /// from whatever the environment names, or from nobody when two episodes are open.
    #[test]
    fn the_environment_reading_wrappers_answer_as_their_explicit_forms() {
        let held = tempdir();
        two_episodes(&held);
        let root = held.path();
        let named = attribution(root, Some("worker-a"));
        assert_eq!(named.session.as_deref(), Some("s-a"));
        assert_eq!(named.task.as_deref(), Some("t-1"));
        assert_eq!(
            open_session_id(root, None),
            open_session_id_with(root, None, env_var)
        );
        assert_eq!(
            attribution(root, None),
            attribution_with(root, None, env_var)
        );
        // two episodes and nobody named: whatever the environment says, nobody is the opener
        assert_eq!(attribution(root, None).task, None);
    }

    /// The key mapping is the shell's `mj_session_key`: a run of unsafe characters is one
    /// dash, the ends are trimmed, and a key with nothing left is the hand's.
    #[test]
    fn a_session_key_squeezes_dashes_and_an_empty_one_is_the_hands() {
        assert_eq!(session_key("a--b//c"), "a-b-c");
        assert_eq!(session_key("--x--"), "x");
        assert_eq!(session_key("///"), "hand");
        assert_eq!(session_key(""), "hand");
        assert_eq!(session_key(&"y".repeat(70)), "y".repeat(64));
    }

    /// A current task whose record names no worktree is not this checkout's.
    #[test]
    fn a_current_task_without_a_worktree_is_nobodys() {
        let held = tempdir();
        let state = held.path().join(STATE_DIR);
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join(CURRENT), "id: t-9\n").unwrap();
        assert_eq!(current_task_here(held.path()), None);
        fs::write(
            state.join(CURRENT),
            format!("id: t-9\nworktree: {}\n", held.path().display()),
        )
        .unwrap();
        assert_eq!(current_task_here(held.path()).as_deref(), Some("t-9"));
    }

    fn tempdir() -> TempDir {
        TempDir::new()
    }

    /// A directory that removes itself. The crate has no dev-dependency for this and one
    /// test module is not a reason to add one.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "mj-ledger-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
