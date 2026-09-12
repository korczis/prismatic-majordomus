//! The typed event substrate: the existing append-only ledger, read and written with types.
//!
//! # Why this is not a new store
//!
//! ADR 0041 says it in as many words and this module keeps the promise: the append-only
//! ledger at `.ai/local/state/ledger.jsonl` and the record directories beside it are the
//! substrate, and the write path moves *under* the domain rather than being duplicated
//! beside it. Every defect this subsystem has produced has the same shape — two accounts of
//! one fact — and a second store would be the largest instance of it yet.
//!
//! # Why JSONL and not a database
//!
//! The requirement was stated as concurrency-safe append, corruption detection and
//! recovery. It was *not* stated as query, transactions across records, or indexes, and
//! measuring what the ledger is actually asked for bears that out: every reader in the tree
//! walks the file once and filters. So the question is whether a locked append-only text
//! file meets the three requirements, and it does:
//!
//! * **Concurrent append.** A write opened `O_APPEND` is positioned and written in one
//!   operation by the kernel, so two processes cannot interleave at the offset; below
//!   `PIPE_BUF` the write itself is atomic as well. [`Ledger::append`] additionally takes an
//!   exclusive `flock` for the duration, which covers a long line and a filesystem with
//!   weaker guarantees. A ledger line is one line, so there is no cross-line transaction to
//!   want.
//! * **Corruption detection.** The unit of corruption is a line, because the format has no
//!   cross-line state. [`Ledger::read`] parses line by line and reports each unparseable one
//!   with its number and what was wrong with it.
//! * **Recovery.** A file whose last line was truncated by a machine losing power is still
//!   the complete record of everything before it. Reading skips the bad line, counts it, and
//!   names it; nothing fails, because a ledger with one truncated line that refuses to be
//!   read is worse than one that reports the truncation.
//!
//! A database would add a binary file to a store whose value is partly that a person can
//! `tail -f` it, a second thing to migrate at cutover, a locking model that behaves no
//! better than `flock` over a network filesystem, and a schema that would immediately
//! become a second account of `share/events.yaml`. It is refused, and ADR 0047 records the
//! refusal so that it is a decision rather than an omission.
//!
//! # The vocabulary is not this module's
//!
//! `share/events.yaml` declares every event name the ledger accepts and what each one must
//! carry, and `mj_ledger_append` already refuses an unregistered name. [`Vocabulary`] reads
//! that file. It does not contain a list of event names, and a test below asserts that the
//! names this domain writes are ones the shipped file declares — which is the assertion
//! that fails if somebody adds an event here and forgets the vocabulary, rather than a
//! durable line every reader silently ignores.
//!
//! ```
//! use majordomus_cli::session::{Envelope, Ledger, LedgerEvent, Vocabulary};
//!
//! let dir = tempfile::tempdir().unwrap();
//! let ledger = Ledger::at(dir.path().join("ledger.jsonl"));
//! let vocabulary = Vocabulary::parse(
//!     "version: 1\nevents:\n  - id: session.started\n    requires: [owner]\n    public: true\n",
//! ).expect("a vocabulary");
//! let envelope = Envelope {
//!     ts: "2026-09-11T11:02:35Z".into(), head: "1a651e64".into(),
//!     branch: "master".into(), by: "majordomus/0.5.0".into(), session: None,
//! };
//!
//! ledger.append(&LedgerEvent::new("session.started").with("owner", "korczis"), &vocabulary, &envelope).unwrap();
//! assert_eq!(ledger.read().events.len(), 1);
//!
//! // and a name nothing declares never becomes a durable line
//! assert!(ledger.append(&LedgerEvent::new("session.invented"), &vocabulary, &envelope).is_err());
//! ```

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::metadata::yaml;

/// `share/events.yaml`, relative to the share directory.
pub const EVENTS_FILE: &str = "events.yaml";

/// The ledger, relative to the repository root.
pub const LEDGER_PATH: &str = ".ai/local/state/ledger.jsonl";

/// What can go wrong writing or reading the ledger. The first two are refusals of a
/// malformed event and happen before anything is written; the last two are the disk.
///
/// ```
/// use majordomus_cli::session::{Envelope, LedgerError, LedgerEvent, Vocabulary};
///
/// let v = Vocabulary::parse("version: 1\nevents:\n  - id: session.started\n    requires: [owner]\n    public: true\n").unwrap();
/// let envelope = Envelope {
///     ts: "2026-09-11T11:02:35Z".into(), head: "1a651e64".into(),
///     branch: "master".into(), by: "majordomus/0.5.0".into(), session: None,
/// };
/// let err = LedgerEvent::new("session.started").render(&v, &envelope).expect_err("a refusal");
/// assert!(matches!(err, LedgerError::MissingField { .. }));
/// ```
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    /// The event name is not one `share/events.yaml` declares. An internal error and not a
    /// finding: a command that writes an event the vocabulary does not define is a bug in
    /// Majordomus, not a fact about the repository being supervised.
    #[error("unregistered event '{name}'; declare it in share/{file} or use one of: {known}")]
    UnregisteredEvent {
        /// The name that was offered.
        name: String,
        /// The vocabulary file it is missing from.
        file: String,
        /// The names that are declared, space separated.
        known: String,
    },
    /// The event is declared and the payload is missing a key its declaration requires.
    #[error("event '{name}' is missing the required field '{field}'")]
    MissingField {
        /// The event.
        name: String,
        /// The key its `requires:` demands.
        field: String,
    },
    /// The vocabulary file could not be read or does not parse.
    #[error("the event vocabulary at {path} could not be read: {reason}")]
    Vocabulary {
        /// Where it was looked for.
        path: String,
        /// What went wrong.
        reason: String,
    },
    /// The ledger file itself could not be opened, locked or written.
    #[error("the ledger at {path} could not be written: {reason}")]
    Io {
        /// The ledger.
        path: String,
        /// What went wrong.
        reason: String,
    },
}

/// One declared event: what it is called, what writes it, what it must carry, and whether
/// it is part of the documented record a user reads back.
///
/// ```
/// use majordomus_cli::session::{DeclaredEvent, Vocabulary};
///
/// let v = Vocabulary::parse(
///     "version: 1\nevents:\n  - id: session.closed\n    emitted_by: session\n    requires: [outcome, session_path]\n    public: true\n",
/// ).expect("a vocabulary");
/// let declared: &DeclaredEvent = &v.declared()[0];
/// assert_eq!(declared.id, "session.closed");
/// assert_eq!(declared.requires, ["outcome", "session_path"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeclaredEvent {
    /// The name written into the `event` field of every line.
    pub id: String,
    /// The command whose module writes it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub emitted_by: String,
    /// Payload keys every line of this type must carry, beyond the envelope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// Whether the event is part of the documented record a user reads back.
    pub public: bool,
}

impl crate::order::Ordered for DeclaredEvent {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey {
            group: Some(&self.emitted_by),
            rank: 0,
            label: &self.id,
            identity: &self.id,
        }
    }
}

/// The event vocabulary, read from `share/events.yaml`.
///
/// ```
/// use majordomus_cli::session::Vocabulary;
///
/// // written as the shipped file is, which is what a repository's own share directory holds
/// let v = Vocabulary::parse(
///     "version: 1\nevents:\n  - id: session.closed\n    emitted_by: session\n    requires: [outcome, session_path]\n    public: true\n",
/// ).expect("a vocabulary");
/// assert!(v.knows("session.closed"));
/// assert!(!v.knows("session.clsoed"), "a typo is not a name");
/// assert_eq!(v.requires("session.closed"), ["outcome", "session_path"]);
/// assert!(v.requires("nothing.declared").is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vocabulary {
    events: BTreeMap<String, DeclaredEvent>,
}

impl Vocabulary {
    /// Read the vocabulary from a share directory — the one the tool was invoked with,
    /// never a copy compiled into this module.
    ///
    /// ```
    /// use majordomus_cli::session::Vocabulary;
    /// // a directory with no events.yaml is a failure with a path in it, not a default
    /// let empty = tempfile::tempdir().unwrap();
    /// assert!(Vocabulary::load(empty.path()).is_err());
    /// ```
    pub fn load(share_dir: &Path) -> Result<Self, LedgerError> {
        let path = share_dir.join(EVENTS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| LedgerError::Vocabulary {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        Self::parse(&text).map_err(|reason| LedgerError::Vocabulary {
            path: path.display().to_string(),
            reason,
        })
    }

    /// Parse the vocabulary from the file's text. A document that declares no event with
    /// an id is an error rather than an empty vocabulary, because an empty one would refuse
    /// every event and read as "the ledger is broken".
    ///
    /// ```
    /// use majordomus_cli::session::Vocabulary;
    /// assert!(Vocabulary::parse("version: 1\nevents: []\n").is_err());
    /// assert!(Vocabulary::parse("version: 1\nevents:\n  - id: session.started\n").is_ok());
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let map = yaml::parse_mapping(text)?;
        let list = map
            .get("events")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "no `events:` list".to_string())?;
        let mut events = BTreeMap::new();
        for entry in list {
            let Some(obj) = entry.as_object() else {
                continue;
            };
            let Some(id) = obj.get("id").and_then(yaml::scalar_string) else {
                continue;
            };
            let requires = obj
                .get("requires")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(yaml::scalar_string).collect())
                .unwrap_or_default();
            let declared = DeclaredEvent {
                emitted_by: obj
                    .get("emitted_by")
                    .and_then(yaml::scalar_string)
                    .unwrap_or_default(),
                requires,
                public: obj.get("public").and_then(|v| v.as_bool()).unwrap_or(false),
                id: id.clone(),
            };
            events.insert(id, declared);
        }
        if events.is_empty() {
            return Err("the `events:` list declares no event with an id".to_string());
        }
        Ok(Vocabulary { events })
    }

    /// Is `name` declared? The question `mj_ledger_append` asks before it writes, so that
    /// a mistyped name is a refusal rather than a durable line every reader ignores.
    ///
    /// ```
    /// use majordomus_cli::session::Vocabulary;
    /// let v = Vocabulary::parse("version: 1\nevents:\n  - id: session.closed\n").unwrap();
    /// assert!(v.knows("session.closed"));
    /// assert!(!v.knows("session.clsoed"), "a typo is not a name");
    /// ```
    pub fn knows(&self, name: &str) -> bool {
        self.events.contains_key(name)
    }

    /// The payload keys `name` must carry. Empty for a name that is not declared — the
    /// caller has already been told by [`Vocabulary::knows`], and answering "no
    /// requirements" for an unknown name is what keeps this from being a second refusal.
    ///
    /// ```
    /// use majordomus_cli::session::Vocabulary;
    /// let v = Vocabulary::parse(
    ///     "version: 1\nevents:\n  - id: session.closed\n    requires: [outcome, session_path]\n",
    /// ).unwrap();
    /// assert_eq!(v.requires("session.closed"), ["outcome", "session_path"]);
    /// assert!(v.requires("nothing.declared").is_empty());
    /// ```
    pub fn requires(&self, name: &str) -> &[String] {
        self.events
            .get(name)
            .map(|e| e.requires.as_slice())
            .unwrap_or(&[])
    }

    /// Every declared event, in the crate's one canonical order, so two surfaces listing
    /// the vocabulary list it the same way.
    ///
    /// ```
    /// use majordomus_cli::session::Vocabulary;
    /// let v = Vocabulary::parse(
    ///     "version: 1\nevents:\n  - id: session.started\n  - id: session.closed\n",
    /// ).unwrap();
    /// assert_eq!(v.declared().len(), 2);
    /// ```
    pub fn declared(&self) -> Vec<DeclaredEvent> {
        let mut out: Vec<DeclaredEvent> = self.events.values().cloned().collect();
        crate::order::canonical(&mut out);
        out
    }

    /// The declared names, space separated, for a refusal that tells the caller what it
    /// could have said instead.
    fn names(&self) -> String {
        self.events.keys().cloned().collect::<Vec<_>>().join(" ")
    }
}

/// One event on its way into the ledger: the name, the payload, and the envelope the
/// writer computes.
///
/// The envelope — `ts`, `event`, `head`, `branch`, `by`, and `session` when an episode is
/// open — is computed and never authored, exactly as `share/events.yaml` says. This type
/// takes the computed values from the caller rather than reaching for git itself, because
/// a domain that shells out to git while holding a lock on the ledger is a domain that
/// holds the lock for a hundred milliseconds on a loaded machine.
///
/// ```
/// use majordomus_cli::session::{Envelope, LedgerEvent, Vocabulary};
///
/// let v = Vocabulary::parse(
///     "version: 1\nevents:\n  - id: session.closed\n    requires: [outcome, session_path]\n    public: true\n",
/// ).unwrap();
/// let envelope = Envelope {
///     ts: "2026-09-11T11:02:35Z".into(),
///     head: "1a651e644".into(),
///     branch: "feature/the-session-domain-is-typed".into(),
///     by: "majordomus/0.5.0".into(),
///     session: Some("s-20260910205542-e2a6".into()),
/// };
///
/// // the payload its declaration requires
/// let ok = LedgerEvent::new("session.closed")
///     .with("outcome", "closed")
///     .with("session_path", ".ai/repo/sessions/x.md");
/// let line = ok.render(&v, &envelope).expect("a line");
/// assert!(line.starts_with('{') && line.ends_with('}'));
/// assert!(line.contains("\"event\":\"session.closed\""));
/// assert!(line.contains("\"session\":\"s-20260910205542-e2a6\""));
///
/// // one key short, and it is refused before anything is written
/// let short = LedgerEvent::new("session.closed").with("outcome", "closed");
/// assert!(short.render(&v, &envelope).is_err());
///
/// // a name the vocabulary does not declare is refused too
/// assert!(LedgerEvent::new("session.clsoed").render(&v, &envelope).is_err());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LedgerEvent {
    name: String,
    payload: Map<String, Value>,
}

/// The computed half of a ledger line: what the writer knows and the event does not.
///
/// Every field here is computed and never authored, exactly as `share/events.yaml` says.
/// It is passed in rather than gathered here, because a domain that shells out to git
/// while holding the ledger's lock holds it for a hundred milliseconds on a loaded machine.
///
/// ```
/// use majordomus_cli::session::Envelope;
///
/// let envelope = Envelope {
///     ts: "2026-09-11T11:02:35Z".into(), head: "1a651e64".into(),
///     branch: "master".into(), by: "majordomus/0.5.0".into(),
///     session: Some("s-20260910205542-e2a6".into()),
/// };
/// // a line carrying no session belongs to no episode, which is an answer and not a gap
/// assert!(envelope.session.is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// When, RFC 3339 in UTC.
    pub ts: String,
    /// The commit the writer was on.
    pub head: String,
    /// The branch it was on.
    pub branch: String,
    /// The writer, `majordomus/<version>`.
    pub by: String,
    /// The episode this line belongs to, when one is open. A line carrying no session id
    /// belongs to no episode, and that is the correct answer rather than a gap: work done
    /// outside an episode is attributed to nobody rather than to whoever had one open
    /// nearby.
    pub session: Option<String>,
}

impl LedgerEvent {
    /// An event with no payload yet. The name is not checked here: it is checked against
    /// the vocabulary at [`LedgerEvent::render`], which is the last moment before a line
    /// could become durable.
    ///
    /// ```
    /// use majordomus_cli::session::LedgerEvent;
    /// assert_eq!(LedgerEvent::new("session.closed").name(), "session.closed");
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        LedgerEvent {
            name: name.into(),
            payload: Map::new(),
        }
    }

    /// Add one payload key. Strings are the common case and have their own method so that
    /// the ordinary call site carries no `json!`.
    ///
    /// ```
    /// use majordomus_cli::session::{Envelope, LedgerEvent, Vocabulary};
    /// let v = Vocabulary::parse("version: 1\nevents:\n  - id: session.closed\n    requires: [outcome]\n").unwrap();
    /// let envelope = Envelope {
    ///     ts: "t".into(), head: "h".into(), branch: "b".into(), by: "m".into(), session: None,
    /// };
    /// let line = LedgerEvent::new("session.closed").with("outcome", "closed").render(&v, &envelope).unwrap();
    /// assert!(line.contains("\"outcome\":\"closed\""));
    /// ```
    pub fn with(mut self, key: &str, value: impl Into<String>) -> Self {
        self.payload
            .insert(key.to_string(), Value::String(value.into()));
        self
    }

    /// Add one payload key whose value is not a string — a count, a flag, a list.
    ///
    /// ```
    /// use majordomus_cli::session::{Envelope, LedgerEvent, Vocabulary};
    /// let v = Vocabulary::parse("version: 1\nevents:\n  - id: ledger.rotated\n    requires: [kept]\n").unwrap();
    /// let envelope = Envelope {
    ///     ts: "t".into(), head: "h".into(), branch: "b".into(), by: "m".into(), session: None,
    /// };
    /// let line = LedgerEvent::new("ledger.rotated")
    ///     .with_value("kept", serde_json::json!(120))
    ///     .render(&v, &envelope)
    ///     .unwrap();
    /// assert!(line.contains("\"kept\":120"));
    /// ```
    pub fn with_value(mut self, key: &str, value: Value) -> Self {
        self.payload.insert(key.to_string(), value);
        self
    }

    /// The event's name, as offered. It is a declared name only once the vocabulary has
    /// been asked, which [`LedgerEvent::render`] does.
    ///
    /// ```
    /// use majordomus_cli::session::LedgerEvent;
    /// assert_eq!(LedgerEvent::new("session.started").name(), "session.started");
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Render the line, refusing an unregistered name or a missing required key. The
    /// refusal happens here — before the lock is taken and before anything is written — so
    /// that a malformed event never becomes a durable line.
    ///
    /// ```
    /// use majordomus_cli::session::{Envelope, LedgerEvent, Vocabulary};
    ///
    /// let v = Vocabulary::parse(
    ///     "version: 1\nevents:\n  - id: session.closed\n    requires: [outcome, session_path]\n",
    /// ).unwrap();
    /// let envelope = Envelope {
    ///     ts: "2026-09-11T11:02:35Z".into(), head: "1a651e64".into(),
    ///     branch: "master".into(), by: "majordomus/0.5.0".into(),
    ///     session: Some("s-20260910205542-e2a6".into()),
    /// };
    ///
    /// let line = LedgerEvent::new("session.closed")
    ///     .with("outcome", "closed")
    ///     .with("session_path", ".ai/repo/sessions/x.md")
    ///     .render(&v, &envelope)
    ///     .expect("a line");
    /// assert!(line.contains("\"event\":\"session.closed\""));
    /// assert!(line.contains("\"session\":\"s-20260910205542-e2a6\""));
    ///
    /// // one key short, and nothing is rendered at all
    /// assert!(LedgerEvent::new("session.closed").with("outcome", "closed").render(&v, &envelope).is_err());
    /// ```
    pub fn render(
        &self,
        vocabulary: &Vocabulary,
        envelope: &Envelope,
    ) -> Result<String, LedgerError> {
        if !vocabulary.knows(&self.name) {
            return Err(LedgerError::UnregisteredEvent {
                name: self.name.clone(),
                file: EVENTS_FILE.to_string(),
                known: vocabulary.names(),
            });
        }
        for field in vocabulary.requires(&self.name) {
            if !self.payload.contains_key(field) {
                return Err(LedgerError::MissingField {
                    name: self.name.clone(),
                    field: field.clone(),
                });
            }
        }
        // The envelope's key order is the shell writer's, so a reader that has learned to
        // read one ledger reads both and a diff of two lines is legible.
        let mut line = Map::new();
        line.insert("ts".into(), Value::String(envelope.ts.clone()));
        line.insert("event".into(), Value::String(self.name.clone()));
        line.insert("head".into(), Value::String(envelope.head.clone()));
        line.insert("branch".into(), Value::String(envelope.branch.clone()));
        line.insert("by".into(), Value::String(envelope.by.clone()));
        if let Some(session) = &envelope.session {
            line.insert("session".into(), Value::String(session.clone()));
        }
        for (k, v) in &self.payload {
            line.insert(k.clone(), v.clone());
        }
        serde_json::to_string(&Value::Object(line)).map_err(|e| LedgerError::Io {
            path: LEDGER_PATH.to_string(),
            reason: e.to_string(),
        })
    }
}

/// One line that could not be read as an event: which line it was, and what was wrong with
/// it.
///
/// ```
/// use majordomus_cli::session::{Corruption, Ledger};
///
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("ledger.jsonl");
/// std::fs::write(&path, "not json at all\n").unwrap();
/// let found: Vec<Corruption> = Ledger::at(&path).read().corrupt;
/// assert_eq!(found[0].line, 1, "counted from one, so a person can go to it");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Corruption {
    /// Which line, counting from one, so a person can go to it.
    pub line: usize,
    /// What was wrong with it.
    pub reason: String,
}

/// What a read of the ledger found: the lines that parsed, in ledger order, and the lines
/// that did not, named rather than dropped.
///
/// ```
/// use majordomus_cli::session::{Ledger, LedgerRead};
///
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("ledger.jsonl");
/// std::fs::write(&path, "{\"ts\":\"a\"}\nbroken\n").unwrap();
/// let read: LedgerRead = Ledger::at(&path).read();
/// assert_eq!(read.events.len(), 1);
/// assert_eq!(read.corrupt.len(), 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LedgerRead {
    /// The lines that parsed, in ledger order — which is the order the commands ran, and
    /// is what makes two events inside one second need no tiebreak.
    pub events: Vec<Value>,
    /// The lines that did not. Named rather than dropped: a reader that silently omits a
    /// line cannot tell a ledger with a truncated tail from one that recorded less.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corrupt: Vec<Corruption>,
}

/// The append-only ledger of one checkout: the canonical record of what happened, written
/// only by Majordomus and in the order the commands ran.
///
/// ```
/// use majordomus_cli::session::Ledger;
/// use std::path::Path;
/// assert!(Ledger::of(Path::new("/r")).path().ends_with("ledger.jsonl"));
/// ```
#[derive(Debug, Clone)]
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// The ledger of the repository rooted at `root`.
    ///
    /// ```
    /// use majordomus_cli::session::Ledger;
    /// use std::path::Path;
    /// assert!(Ledger::of(Path::new("/r")).path().ends_with("ledger.jsonl"));
    /// ```
    pub fn of(root: &Path) -> Self {
        Ledger {
            path: root.join(LEDGER_PATH),
        }
    }

    /// The ledger at an explicit path, for a test or a store that is not a repository root.
    ///
    /// ```
    /// use majordomus_cli::session::Ledger;
    /// let dir = tempfile::tempdir().unwrap();
    /// let ledger = Ledger::at(dir.path().join("elsewhere.jsonl"));
    /// assert!(ledger.read().events.is_empty(), "a ledger nobody has written is empty");
    /// ```
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Ledger { path: path.into() }
    }

    /// Where it is. A reader that wants to name the file in a diagnostic asks here rather
    /// than composing the path a second time.
    ///
    /// ```
    /// use majordomus_cli::session::Ledger;
    /// use std::path::Path;
    /// assert!(Ledger::of(Path::new("/r")).path().starts_with("/r"));
    /// ```
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one rendered line, under an exclusive lock, opened `O_APPEND`.
    ///
    /// The local half is never tracked, so a second worktree of one repository starts
    /// without it and the first write creates the directory — the same thing
    /// `mj_ledger_append` does, for the same reason.
    ///
    /// ```
    /// use majordomus_cli::session::{Envelope, Ledger, LedgerEvent, Vocabulary};
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let ledger = Ledger::at(dir.path().join("ledger.jsonl"));
    /// let v = Vocabulary::parse("version: 1\nevents:\n  - id: session.started\n    requires: [owner]\n    public: true\n").unwrap();
    /// let envelope = Envelope {
    ///     ts: "2026-09-11T11:02:35Z".into(), head: "1a651e64".into(),
    ///     branch: "master".into(), by: "majordomus/0.5.0".into(), session: None,
    /// };
    ///
    /// ledger.append(&LedgerEvent::new("session.started").with("owner", "korczis"), &v, &envelope).unwrap();
    /// ledger.append(&LedgerEvent::new("session.started").with("owner", "korczis"), &v, &envelope).unwrap();
    ///
    /// let read = ledger.read();
    /// assert_eq!(read.events.len(), 2);
    /// assert!(read.corrupt.is_empty());
    /// ```
    pub fn append(
        &self,
        event: &LedgerEvent,
        vocabulary: &Vocabulary,
        envelope: &Envelope,
    ) -> Result<(), LedgerError> {
        let line = event.render(vocabulary, envelope)?;
        let io = |reason: String| LedgerError::Io {
            path: self.path.display().to_string(),
            reason,
        };
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io(e.to_string()))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| io(e.to_string()))?;
        // The descriptor number is taken before the lock so that the guard borrows
        // nothing: `file` stays exclusively ours to write through.
        let fd = {
            use std::os::unix::io::AsRawFd;
            file.as_raw_fd()
        };
        let _guard = FileLock::exclusive(fd).map_err(io)?;
        file.write_all(line.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|e| io(e.to_string()))?;
        file.flush().map_err(|e| io(e.to_string()))
    }

    /// Read the whole ledger, reporting what did not parse rather than failing on it.
    ///
    /// An absent ledger reads as empty: a fresh clone has never run the lifecycle, and
    /// that is not a fault.
    ///
    /// ```
    /// use majordomus_cli::session::Ledger;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let path = dir.path().join("ledger.jsonl");
    /// std::fs::write(
    ///     &path,
    ///     "{\"ts\":\"a\",\"event\":\"session.started\"}\nnot json at all\n{\"ts\":\"b\",\"event\":\"session.closed\"}\n{\"ts\":\"c\",\"event\"\n",
    /// ).unwrap();
    ///
    /// let read = Ledger::at(&path).read();
    /// assert_eq!(read.events.len(), 2, "everything that parsed");
    /// assert_eq!(read.corrupt.len(), 2, "and both lines that did not, by number");
    /// assert_eq!(read.corrupt[0].line, 2);
    /// assert_eq!(read.corrupt[1].line, 4, "a truncated tail is one bad line, not a failed read");
    ///
    /// // absence is an answer
    /// assert!(Ledger::at(dir.path().join("nothing.jsonl")).read().events.is_empty());
    /// ```
    pub fn read(&self) -> LedgerRead {
        let Ok(mut file) = std::fs::File::open(&self.path) else {
            return LedgerRead::default();
        };
        let mut text = String::new();
        if file.read_to_string(&mut text).is_err() {
            return LedgerRead {
                events: Vec::new(),
                corrupt: vec![Corruption {
                    line: 0,
                    reason: "the ledger is not valid UTF-8".to_string(),
                }],
            };
        }
        let mut out = LedgerRead::default();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(v) if v.is_object() => out.events.push(v),
                Ok(_) => out.corrupt.push(Corruption {
                    line: n + 1,
                    reason: "a ledger line is a JSON object; this one is not".to_string(),
                }),
                Err(e) => out.corrupt.push(Corruption {
                    line: n + 1,
                    reason: e.to_string(),
                }),
            }
        }
        out
    }
}

/// An advisory exclusive lock held for the life of the value.
///
/// `O_APPEND` alone positions and writes in one kernel operation, which is what stops two
/// appends from landing at the same offset. The lock is what stops a *long* line from
/// interleaving on a filesystem whose atomicity guarantee stops at `PIPE_BUF`, and it is
/// cheap: an uncontended `flock` is a syscall.
///
/// The guard holds the descriptor *number* and no borrow of the file, so that the writes it
/// protects can still take `&mut File`. The caller keeps the file alive for the guard's
/// whole life; [`Ledger::append`] does, in one function, which is the only place this type
/// is constructed.
struct FileLock {
    fd: std::os::unix::io::RawFd,
}

impl FileLock {
    fn exclusive(fd: std::os::unix::io::RawFd) -> Result<Self, String> {
        // SAFETY: the caller holds the open file whose descriptor this is for the whole
        // life of the guard; flock has no other precondition.
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX) };
        if rc != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(FileLock { fd })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // SAFETY: as above; the descriptor is still open because the caller's file is.
        unsafe {
            libc::flock(self.fd, libc::LOCK_UN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> Envelope {
        Envelope {
            ts: "2026-09-11T11:02:35Z".into(),
            head: "1a651e644".into(),
            branch: "master".into(),
            by: "majordomus/test".into(),
            session: None,
        }
    }

    #[test]
    fn the_names_this_domain_writes_are_ones_the_shipped_vocabulary_declares() {
        // The assertion that fails when somebody adds an event here and forgets
        // share/events.yaml — instead of a durable line every reader silently ignores.
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../share")
            .join(EVENTS_FILE);
        let Ok(text) = std::fs::read_to_string(&shipped) else {
            // a checkout without the share directory is not this test's subject
            return;
        };
        let v = Vocabulary::parse(&text).expect("the shipped vocabulary parses");
        for name in super::super::WRITTEN_EVENTS {
            assert!(
                v.knows(name),
                "{name} is written by the session domain and is not declared in share/{EVENTS_FILE}"
            );
        }
    }

    #[test]
    fn concurrent_appends_from_many_threads_all_land_whole() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let ledger = Ledger::at(dir.path().join("ledger.jsonl"));
        let v = Vocabulary::parse(
            "version: 1\nevents:\n  - id: session.started\n    requires: [owner]\n    public: true\n",
        )
        .expect("a vocabulary");

        // A payload long enough that the line exceeds any plausible PIPE_BUF, which is
        // precisely the case O_APPEND alone does not cover and the lock does.
        let long = "x".repeat(9_000);
        std::thread::scope(|scope| {
            for n in 0..16 {
                let ledger = &ledger;
                let v = &v;
                let long = &long;
                scope.spawn(move || {
                    let event = LedgerEvent::new("session.started")
                        .with("owner", format!("worker-{n}"))
                        .with("filler", long.clone());
                    ledger.append(&event, v, &envelope()).expect("an append");
                });
            }
        });

        let read = ledger.read();
        assert_eq!(read.events.len(), 16, "every line landed");
        assert!(read.corrupt.is_empty(), "and none of them interleaved");
    }

    #[test]
    fn an_unregistered_name_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("ledger.jsonl");
        let ledger = Ledger::at(&path);
        let v = Vocabulary::parse(
            "version: 1\nevents:\n  - id: session.started\n    requires: [owner]\n    public: true\n",
        )
        .expect("a vocabulary");
        let err = ledger
            .append(&LedgerEvent::new("session.invented"), &v, &envelope())
            .expect_err("a refusal");
        assert!(matches!(err, LedgerError::UnregisteredEvent { .. }));
        assert!(!path.exists(), "and the ledger was not even created");
    }

    #[test]
    fn a_missing_required_field_is_refused_before_anything_is_written() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("ledger.jsonl");
        let v = Vocabulary::parse(
            "version: 1\nevents:\n  - id: session.closed\n    requires: [outcome, session_path]\n    public: true\n",
        )
        .expect("a vocabulary");
        let err = Ledger::at(&path)
            .append(
                &LedgerEvent::new("session.closed").with("outcome", "closed"),
                &v,
                &envelope(),
            )
            .expect_err("a refusal");
        match err {
            LedgerError::MissingField { field, .. } => assert_eq!(field, "session_path"),
            other => panic!("the wrong refusal: {other}"),
        }
        assert!(!path.exists());
    }
}
