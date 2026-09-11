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
//! `test/cases/132_ledger_writers.sh` holds the two writers to byte equality over the same
//! event. Where they disagree the shell is the incumbent and is right; this file is the one
//! that changes.

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

/// Why a line was not written.
///
/// Every variant is a refusal to corrupt the record, never a partial write: the line is
/// composed and validated in full before the file is opened.
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
