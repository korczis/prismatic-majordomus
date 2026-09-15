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
//!   checkout has an open episode, then the event's own payload, in that order;
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
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git::{GitInfo, GitState};
use crate::metadata::yaml;

/// Where the untracked half of the layer lives, relative to the repository root.
const STATE_DIR: &str = ".ai/local/state";
/// The record itself.
const LEDGER: &str = "ledger.jsonl";

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
    /// The payload carries a key the envelope owns. Every line would carry it twice, and
    /// readers disagree about which of the two a duplicated key means.
    EnvelopeKey {
        /// The event.
        event: String,
        /// The envelope key the payload repeated.
        field: String,
    },
    /// The payload handed over as JSON is not one JSON object on one line.
    Payload {
        /// The event.
        event: String,
        /// What is wrong with it.
        reason: String,
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
            // The shell's sentence, word for word: `mj_ledger_append` says it first, and a
            // person reading either program's refusal must meet the same remedy.
            Self::EnvelopeKey { event, field } => write!(
                f,
                "event '{event}' carries the payload field '{field}', which is a ledger envelope key; every line would carry it twice and readers would disagree about which one it means — rename the field (the provider receipt's own '{field}' became 'provider_{field}')"
            ),
            Self::Payload { event, reason } => write!(
                f,
                "event '{event}' has a payload that is not one JSON object on one line: {reason}"
            ),
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
    /// The directory the declaration was read from: the distribution whose providers the
    /// envelope resolves an episode against. `None` for a vocabulary built in memory.
    dir: Option<PathBuf>,
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
        Ok(Self {
            events,
            order,
            dir: events_yaml.parent().map(Path::to_path_buf),
        })
    }

    /// The distribution this vocabulary was read from, when it was read from one: a directory
    /// holding `kinds.yaml` beside `events.yaml`. `None` for a declaration read from anywhere
    /// else, and then the envelope locates the distribution the usual way.
    fn distribution(&self) -> Option<&Path> {
        self.dir
            .as_deref()
            .filter(|d| d.join(crate::share::KINDS_FILE).is_file())
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

    let mut line = envelope(root, vocabulary, git, now, event);
    for (k, v) in payload {
        line.push(',');
        push_pair(&mut line, k, v);
    }
    line.push('}');
    write_line(root, line)
}

/// The keys the envelope owns, in the order it writes them. A payload may carry none of them:
/// `MJ_LEDGER_ENVELOPE_KEYS` in `lib/common.sh` is the same list.
pub const ENVELOPE_KEYS: [&str; 6] = ["ts", "event", "head", "branch", "by", "session"];

/// Append one line whose payload arrives as a JSON object, written into the line verbatim.
///
/// This is the shell's writer. `mj_ledger_append` has always composed its payload as JSON
/// object members — strings, and also numbers, booleans and nested objects (`task.finished`
/// carries its contract as an object) — and [`append`]'s string pairs cannot say that. So the
/// object is validated here and its members are copied into the line byte for byte: the key
/// order and the spelling of every value are the caller's, which is what keeps a line this
/// writes for the shell identical to the line the shell wrote before it had to ask.
///
/// Refused, and nothing written, when the object does not parse, spans more than one line,
/// repeats an envelope key, or omits a key the vocabulary requires.
///
/// ```
/// use majordomus_cli::git::GitState;
/// use majordomus_cli::ledger::{self, Vocabulary};
/// # let dir = tempfile::tempdir().unwrap();
/// # std::fs::write(dir.path().join("events.yaml"),
/// #   "version: 1\nevents:\n  - id: task.finished\n    requires: [task_id]\n").unwrap();
/// let vocabulary = Vocabulary::load(&dir.path().join("events.yaml")).unwrap();
/// let git = GitState::Unavailable { reason: "not a work tree".into() };
///
/// let line = ledger::append_object(dir.path(), &vocabulary, &git, "2026-09-15T12:00:00Z",
///     "task.finished", r#"{"task_id":"t-1","checkpoints":3,"contract":{"ok":true}}"#).unwrap();
/// assert!(line.ends_with(r#""task_id":"t-1","checkpoints":3,"contract":{"ok":true}}"#), "{line}");
///
/// // a payload that names an envelope key never reaches the file
/// let err = ledger::append_object(dir.path(), &vocabulary, &git, "2026-09-15T12:00:00Z",
///     "task.finished", r#"{"task_id":"t-1","event":"start"}"#).unwrap_err();
/// assert!(err.to_string().contains("which is a ledger envelope key"), "{err}");
/// assert_eq!(ledger::read(dir.path()).0.len(), 1);
/// ```
pub fn append_object(
    root: &Path,
    vocabulary: &Vocabulary,
    git: &GitState,
    now: &str,
    event: &str,
    payload: &str,
) -> Result<String, LedgerError> {
    let requires = vocabulary
        .requires(event)
        .ok_or_else(|| LedgerError::UnknownEvent {
            event: event.to_string(),
            known: vocabulary.ids().to_vec(),
        })?;
    let refuse = |reason: String| LedgerError::Payload {
        event: event.to_string(),
        reason,
    };
    let text = payload.trim();
    // A raw line break is legal whitespace between JSON tokens and fatal in a JSON-lines
    // record: the member would be copied across two lines of the file.
    if text.contains(['\n', '\r']) {
        return Err(refuse("it spans more than one line".into()));
    }
    let members: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(text).map_err(|e| refuse(e.to_string()))?;
    if let Some(field) = ENVELOPE_KEYS.iter().find(|k| members.contains_key(**k)) {
        return Err(LedgerError::EnvelopeKey {
            event: event.to_string(),
            field: (*field).to_string(),
        });
    }
    for key in requires {
        if !members.contains_key(key) {
            return Err(LedgerError::MissingField {
                event: event.to_string(),
                field: key.clone(),
            });
        }
    }
    // The members between the object's own braces, exactly as they were written. The parse
    // above proved `text` is one object, so its first and last bytes are those braces.
    let inner = text[1..text.len() - 1].trim();
    let mut line = envelope(root, vocabulary, git, now, event);
    if !inner.is_empty() {
        line.push(',');
        line.push_str(inner);
    }
    line.push('}');
    write_line(root, line)
}

/// The envelope, open: `{"ts":…,"event":…,"head":…,"branch":…,"by":…[,"session":…]`, with
/// no closing brace, for the payload to follow.
fn envelope(
    root: &Path,
    vocabulary: &Vocabulary,
    git: &GitState,
    now: &str,
    event: &str,
) -> String {
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
    if let Some(session) = open_session_id(root, vocabulary.distribution()) {
        line.push(',');
        push_pair(&mut line, "session", &session);
    }
    line
}

/// Written under the session domain's exclusive lock, never with a bare O_APPEND of its own:
/// the shell, this module and the session domain all append to one file, and this was the one
/// writer in the executable that took no lock.
fn write_line(root: &Path, line: String) -> Result<String, LedgerError> {
    let path = root.join(STATE_DIR).join(LEDGER);
    crate::session::Ledger::at(&path)
        .append_line(&line)
        .map_err(|e| LedgerError::Write {
            path,
            reason: e.to_string(),
        })?;
    Ok(line)
}

/// The two git facts the envelope carries, and nothing else git could be asked.
///
/// [`crate::git::inspect`] also runs `git status`, which costs a walk of the working tree and
/// can rewrite the index — neither of which a ledger append may do, since the shell calls it
/// on every recorded event. Where git cannot answer, the result is what [`append`] spells
/// `NONE` and `DETACHED`.
///
/// ```
/// use majordomus_cli::git::GitState;
/// let nowhere = tempfile::tempdir().unwrap();
/// let git = majordomus_cli::ledger::head_and_branch(nowhere.path());
/// if let GitState::Available(info) = git {
///     // not a repository: git answers nothing about either
///     assert!(info.head.is_none() && info.branch.is_none());
/// }
/// ```
pub fn head_and_branch(root: &Path) -> GitState {
    let ask = |args: &[&str]| {
        crate::git::read_only(root)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    GitState::Available(GitInfo {
        toplevel: root.to_path_buf(),
        head: ask(&["rev-parse", "--verify", "-q", "HEAD"]),
        branch: ask(&["symbolic-ref", "-q", "--short", "HEAD"]),
        working_tree: "unread".into(),
    })
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
/// Resolved by [`crate::session::resolver::resolve`], the one resolution the shell's
/// `mj_open_session_id` also follows: the hook's key strictly, then the provider session this
/// process runs inside, then the pointer. This function used to read `state/session-<key>.yaml`,
/// a layout the store left behind, and never consulted the provider's session variable, so a
/// line a capability wrote was stamped with the last-opened episode where the shell stamped the
/// worker's own.
///
/// The providers are the ones the vocabulary's own distribution declares: the directory
/// `share/events.yaml` was read from is the directory `share/providers.yaml` is read from, so
/// the shell's appends resolve against the shell's distribution and not against whatever the
/// environment would locate.
fn open_session_id(root: &Path, share: Option<&Path>) -> Option<String> {
    let vars = crate::session::resolver::declared_session_vars(root, share);
    let env = |name: &str| std::env::var(name).ok();
    crate::session::resolver::resolve(root, &vars, &env).map(|r| r.episode.as_str().to_string())
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
            dir: None,
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

    /// The shell's writer and the capability's writer produce one line for one event: the
    /// object form is the pair form with the caller's own spelling of the members.
    #[test]
    fn an_object_payload_writes_the_line_the_pairs_write() {
        let dir = tempdir();
        let pairs = append(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_evidence",
            &[("issue", "i-1".into()), ("covers", "a\"b".into())],
        )
        .unwrap();
        let object = append_object(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_evidence",
            " {\"issue\":\"i-1\",\"covers\":\"a\\\"b\"}\n",
        )
        .unwrap();
        assert_eq!(pairs, object);
        assert_eq!(read(dir.path()).0.len(), 2);
    }

    /// Numbers, booleans and nested objects are the shell's payloads too, and are copied as
    /// they were written, in the order they were written.
    #[test]
    fn an_object_payload_keeps_its_members_verbatim() {
        let dir = tempdir();
        let line = append_object(
            dir.path(),
            &vocabulary(),
            &git(),
            "2026-09-11T12:00:00Z",
            "plan_start",
            r#"{"zeta":1.50,"issue":"i-1","ok":true,"nested":{"b":2,"a":[1]}}"#,
        )
        .unwrap();
        assert!(
            line.ends_with(r#","zeta":1.50,"issue":"i-1","ok":true,"nested":{"b":2,"a":[1]}}"#),
            "{line}"
        );
        serde_json::from_str::<serde_json::Value>(&line).expect("the line is one JSON value");
    }

    /// Every refusal of the object form writes nothing, and each says which contract failed.
    #[test]
    fn an_object_payload_is_refused_whole() {
        let dir = tempdir();
        let refuse = |event: &str, payload: &str| {
            append_object(
                dir.path(),
                &vocabulary(),
                &git(),
                "2026-09-11T12:00:00Z",
                event,
                payload,
            )
            .unwrap_err()
        };
        assert!(matches!(
            refuse("plan_strat", r#"{"issue":"i"}"#),
            LedgerError::UnknownEvent { .. }
        ));
        assert!(matches!(
            refuse("plan_start", r#"{"issue":"i","session":"s"}"#),
            LedgerError::EnvelopeKey { ref field, .. } if field == "session"
        ));
        assert_eq!(
            refuse("plan_evidence", r#"{"issue":"i"}"#),
            LedgerError::MissingField {
                event: "plan_evidence".into(),
                field: "covers".into()
            }
        );
        assert!(matches!(
            refuse("plan_start", "{\"issue\":\n\"i\"}"),
            LedgerError::Payload { .. }
        ));
        assert!(matches!(
            refuse("plan_start", r#""issue":"i""#),
            LedgerError::Payload { .. }
        ));
        assert!(matches!(
            refuse("plan_start", r#"["issue"]"#),
            LedgerError::Payload { .. }
        ));
        assert!(
            !dir.path().join(STATE_DIR).join(LEDGER).exists(),
            "a refused payload created the ledger"
        );
    }

    /// The refusal a person reads is the shell's sentence, so either program's remedy is one.
    #[test]
    fn the_envelope_refusal_is_the_shells_sentence() {
        let e = LedgerError::EnvelopeKey {
            event: "provider.event.received".into(),
            field: "event".into(),
        };
        assert!(e.to_string().ends_with(
            "rename the field (the provider receipt's own 'event' became 'provider_event')"
        ));
        assert_eq!(ENVELOPE_KEYS.join(" "), "ts event head branch by session");
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
