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
//! * the same envelope — `ts`, `event`, `head`, `branch`, `by`, then `session` when this
//!   checkout has an open episode, then `pid` and `cwd` of the writing process and
//!   `mesh_session` when it names one (I4561), then the event's own payload, in that order;
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
/// The pointer to this checkout's open episode.
const SESSION_POINTER: &str = "session-current.yaml";

/// The envelope's keys, in the order a line composes them: `MJ_LEDGER_ENVELOPE_KEYS` in
/// `lib/common.sh`, which a payload may not carry in either writer.
pub(crate) const ENVELOPE: [&str; 9] = [
    "ts",
    "event",
    "head",
    "branch",
    "by",
    "session",
    "pid",
    "cwd",
    "mesh_session",
];

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
    /// The payload carries a key the envelope reserves, which every line would then carry
    /// twice, with readers disagreeing about which one it means.
    EnvelopeKey {
        /// The event.
        event: String,
        /// The reserved key the payload tried to carry.
        field: String,
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
            Self::EnvelopeKey { event, field } => write!(
                f,
                "event '{event}' carries the payload field '{field}', which is a ledger envelope key"
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
    append_as(
        root,
        vocabulary,
        git,
        now,
        event,
        payload,
        &Writer::of_this_process(),
    )
}

/// The process a line names as its writer (I4561): what it reports about itself, not an
/// authentication, and what makes an unexpected write to a checkout traceable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Writer {
    /// Its pid.
    pub(crate) pid: String,
    /// The directory it ran in, physical, as `pwd -P` spells it.
    pub(crate) cwd: String,
    /// The mesh session it works for, when its environment names one.
    pub(crate) mesh_session: Option<String>,
}

impl Writer {
    /// This process.
    pub(crate) fn of_this_process() -> Writer {
        Writer {
            pid: std::process::id().to_string(),
            cwd: std::env::current_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_default(),
            mesh_session: mesh_session(|k| std::env::var(k).ok()),
        }
    }
}

/// The mesh session an environment names, read as `mesh who` reads a process's owner:
/// `MAJORDOMUS_OWNER_SESSION`, else `CLAUDE_CODE_SESSION_ID` as `claude-code-<id>`, each
/// validated by [`crate::offload::owner::owner_value`]. `mj_ledger_mesh_session` in
/// `lib/common.sh` is the same rule.
pub(crate) fn mesh_session(env: impl Fn(&str) -> Option<String>) -> Option<String> {
    use crate::offload::owner::owner_value;
    let set = |k: &str| env(k).filter(|v| !v.trim().is_empty());
    if let Some(v) = set("MAJORDOMUS_OWNER_SESSION") {
        return owner_value(&v);
    }
    let id = set("CLAUDE_CODE_SESSION_ID")?;
    owner_value(&format!("claude-code-{}", id.trim()))
}

/// [`append`], naming `writer` as the process that wrote the line.
pub(crate) fn append_as(
    root: &Path,
    vocabulary: &Vocabulary,
    git: &GitState,
    now: &str,
    event: &str,
    payload: &[Field<'_>],
    writer: &Writer,
) -> Result<String, LedgerError> {
    if let Some((k, _)) = payload.iter().find(|(k, _)| ENVELOPE.contains(k)) {
        return Err(LedgerError::EnvelopeKey {
            event: event.to_string(),
            field: (*k).to_string(),
        });
    }
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
    if let Some(session) = open_session_id(root) {
        line.push(',');
        push_pair(&mut line, "session", &session);
    }
    line.push(',');
    push_pair(&mut line, "pid", &writer.pid);
    line.push(',');
    push_pair(&mut line, "cwd", &writer.cwd);
    if let Some(mesh) = &writer.mesh_session {
        line.push(',');
        push_pair(&mut line, "mesh_session", mesh);
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

/// This checkout's open episode, when it has one and it is this checkout's own.
///
/// The shell resolves the worker's own episode first, from `MJ_SESSION_KEY` or the
/// provider's own session environment, and falls back to this pointer. Here the pointer is
/// read directly and `MJ_SESSION_KEY` is honoured when it is set, which is what a capability
/// running inside a provider's session is given. A record that names another worktree is
/// another checkout's episode and is not this line's session — the same refusal
/// `mj_open_session_id` makes.
fn open_session_id(root: &Path) -> Option<String> {
    let dir = root.join(STATE_DIR);
    let file = match std::env::var("MJ_SESSION_KEY") {
        Ok(k) if !k.is_empty() => dir.join(format!("session-{k}.yaml")),
        _ => dir.join(SESSION_POINTER),
    };
    let text = fs::read_to_string(file).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    if let Some(w) = map.get("worktree").and_then(yaml::scalar_string) {
        if !w.is_empty() && Path::new(&w) != root {
            return None;
        }
    }
    map.get("session_id")
        .and_then(yaml::scalar_string)
        .filter(|s| !s.is_empty())
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
    /// The pid of the process that wrote it; absent on lines written before I4561.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<String>,
    /// The directory that process ran in; absent on lines written before I4561.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// The mesh session it worked for, when it named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_session: Option<String>,
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
        let line = append_as(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "i-1".into())],
            &writer(Some("claude-code-s1")),
        )
        .expect("the event is declared and its payload complete");
        assert_eq!(
            line,
            format!(
                r#"{{"ts":"2026-09-11T12:00:00Z","event":"plan_start","head":"{}","branch":"master","by":"majordomus/{}","pid":"4242","cwd":"/w/d","mesh_session":"claude-code-s1","issue":"i-1"}}"#,
                "a".repeat(40),
                crate::VERSION
            )
        );
    }

    fn writer(mesh: Option<&str>) -> Writer {
        Writer {
            pid: "4242".into(),
            cwd: "/w/d".into(),
            mesh_session: mesh.map(str::to_string),
        }
    }

    /// The keys a line composes, in order, are the envelope the shell declares, and a
    /// process that names no mesh session writes no `mesh_session` at all.
    #[test]
    fn a_line_names_its_writer_in_the_declared_order() {
        let dir = tempdir();
        let line = append_as(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "i-1".into())],
            &writer(None),
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        let mut want: Vec<&str> = ENVELOPE
            .iter()
            .copied()
            .filter(|k| *k != "session" && *k != "mesh_session")
            .collect();
        want.push("issue");
        let mut got = keys.clone();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want, "{line}");
        assert!(
            line.contains(r#""by":"majordomus/"#)
                && line.contains(r#","pid":"4242","cwd":"/w/d","issue""#),
            "{line}"
        );
        let shell = include_str!("../../../lib/common.sh");
        assert!(
            shell.contains(&format!("MJ_LEDGER_ENVELOPE_KEYS='{}'", ENVELOPE.join(" "))),
            "lib/common.sh declares another envelope than ledger::ENVELOPE"
        );
    }

    /// `append` names this process: its pid and the directory it runs in.
    #[test]
    fn append_names_this_process() {
        let dir = tempdir();
        let line = append(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            &[("issue", "i-1".into())],
        )
        .unwrap();
        let (entries, skipped) = read(dir.path());
        assert_eq!((entries.len(), skipped), (1, 0), "{line}");
        let me = Writer::of_this_process();
        assert_eq!(entries[0].pid.as_deref(), Some(me.pid.as_str()));
        assert_eq!(entries[0].cwd.as_deref(), Some(me.cwd.as_str()));
        assert!(
            !entries[0].payload.contains_key("pid"),
            "the reader typed pid"
        );
        assert_eq!(entries[0].mesh_session, me.mesh_session);
    }

    /// A payload that names an envelope key is refused and writes nothing, in this writer
    /// as in the shell's.
    #[test]
    fn a_payload_naming_an_envelope_key_is_refused() {
        for key in ["pid", "cwd", "mesh_session", "session", "by"] {
            let dir = tempdir();
            let err = append_as(
                dir.path(),
                &vocabulary(),
                &git(),
                "2026-09-11T12:00:00Z",
                "plan_start",
                &[("issue", "i-1".into()), (key, "x".into())],
                &writer(None),
            )
            .unwrap_err();
            assert!(
                matches!(err, LedgerError::EnvelopeKey { .. }),
                "{key}: {err}"
            );
            assert_eq!(
                err.to_string(),
                format!("event 'plan_start' carries the payload field '{key}', which is a ledger envelope key")
            );
            assert!(!dir.path().join(STATE_DIR).join(LEDGER).exists(), "{key}");
        }
    }

    /// The mesh session is read as `mesh who` reads an owner, and a value the owner rule
    /// refuses is no session rather than a guess.
    #[test]
    fn the_mesh_session_is_read_as_mesh_who_reads_it() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(mesh_session(env(&[])), None);
        assert_eq!(
            mesh_session(env(&[("CLAUDE_CODE_SESSION_ID", " f9c3 ")])).as_deref(),
            Some("claude-code-f9c3")
        );
        assert_eq!(
            mesh_session(env(&[
                ("MAJORDOMUS_OWNER_SESSION", "lundra-7"),
                ("CLAUDE_CODE_SESSION_ID", "f9c3")
            ]))
            .as_deref(),
            Some("lundra-7"),
            "a stated owner wins"
        );
        assert_eq!(
            mesh_session(env(&[("MAJORDOMUS_OWNER_SESSION", "a b")])),
            None
        );
        assert_eq!(mesh_session(env(&[("CLAUDE_CODE_SESSION_ID", "  ")])), None);
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
