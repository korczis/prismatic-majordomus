//! Where the executions of a server are written down, so that they outlive it (ADR 0222).
//!
//! One file per execution, `<dir>/<id>.jsonl`, appended and never rewritten:
//!
//! ```text
//!   {"line":"seed",    ...}   what the execution is: capability, input, actor, repository
//!   {"line":"event",   ...}   every event, in sequence, as the store published it
//!   {"line":"through", ...}   for a reported execution: the reporter's sequence taken so far
//!   {"line":"final",   ...}   the snapshot, once, when the execution reached a final state
//! ```
//!
//! The store writes a line under the same lock that publishes the event, before a subscriber
//! or a caller can see it, so nothing a client was shown is missing from the file. Reading
//! is the inverse: [`Journal::read`] answers every execution in the directory, oldest first,
//! and the store folds each one back (`ExecutionStore::journaled`).
//!
//! The journal changes what a restart means and nothing else. An execution this process
//! was running when it stopped cannot be continued — the thread that ran its handler is
//! gone — so it is recovered as failed with the code `interrupted`, at the sequence it had
//! reached, and never run again: an unknown outcome is not a reason to repeat a mutation.
//! An execution another process runs and reports here is recovered as reported, so the
//! reporter's next report continues it under the same id and cursor; one that never reports
//! again is failed by the silence rule as before.
//!
//! A file is never deleted. The store's bound on how many executions it remembers moves the
//! file of a forgotten execution into `archive/`.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::event::ExecutionEvent;
use super::model::{Actor, Execution, ExecutionId, RepositoryRef};

/// Where a repository's server keeps its executions, relative to the repository root.
pub const DIR: &str = ".ai/local/state/executions";

/// The directory a forgotten execution's file is moved into.
const ARCHIVE: &str = "archive";

/// One line of an execution's file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "line", rename_all = "snake_case")]
pub enum Line {
    /// What the execution is, written once, first.
    Seed(SeedLine),
    /// One published event.
    Event(ExecutionEvent),
    /// The reporter's sequence this store has taken everything up to.
    Through {
        /// The sequence.
        through: u64,
    },
    /// The snapshot at the final state, written once.
    Final(Box<Execution>),
}

/// The part of an execution that does not move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeedLine {
    /// The execution.
    pub id: ExecutionId,
    /// The capability's canonical id.
    pub capability: String,
    /// Its title.
    pub title: String,
    /// The input, already redacted.
    pub input: Value,
    /// Whether asking it to stop does anything.
    pub cancellable: bool,
    /// Who asked for it.
    pub actor: Actor,
    /// The repository it runs against.
    pub repository: RepositoryRef,
    /// When it was created, RFC 3339.
    pub created_at: String,
    /// The process that runs and reports it, when that is not this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_pid: Option<u32>,
}

/// One execution as its file holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    /// What it is.
    pub seed: SeedLine,
    /// Every event, in the order written.
    pub events: Vec<ExecutionEvent>,
    /// The last reporter sequence written, for a reported execution.
    pub through: Option<u64>,
    /// The snapshot at its final state, when it reached one.
    pub final_snapshot: Option<Execution>,
}

/// A directory of execution files.
#[derive(Debug, Clone)]
pub struct Journal {
    dir: PathBuf,
}

impl Journal {
    /// Open the directory, creating it when it is not there.
    pub fn open(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Journal { dir })
    }

    /// The directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, id: &ExecutionId) -> PathBuf {
        // an id is parsed into this shape before it reaches a store, so it names no path
        self.dir.join(format!("{}.jsonl", id.as_str()))
    }

    /// Append one line to an execution's file.
    ///
    /// A failure is logged and swallowed: an execution that cannot be written down still
    /// runs, and the log names the file a restart will not find complete.
    pub fn append(&self, id: &ExecutionId, line: &Line) {
        let path = self.file(id);
        let written = serde_json::to_string(line)
            .map_err(std::io::Error::other)
            .and_then(|text| {
                let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
                file.write_all(text.as_bytes())?;
                file.write_all(b"\n")?;
                file.flush()
            });
        if let Err(e) = written {
            tracing::warn!(
                execution_id = %id,
                path = %path.display(),
                "an execution's journal line was not written; a restart will not recover it whole: {e}"
            );
        }
    }

    /// Move a forgotten execution's file into the archive. Never deletes it.
    pub fn archive(&self, id: &ExecutionId) {
        let from = self.file(id);
        if !from.exists() {
            return;
        }
        let to_dir = self.dir.join(ARCHIVE);
        let moved = std::fs::create_dir_all(&to_dir)
            .and_then(|()| std::fs::rename(&from, to_dir.join(format!("{}.jsonl", id.as_str()))));
        if let Err(e) = moved {
            tracing::warn!(execution_id = %id, "an execution's journal was not archived: {e}");
        }
    }

    /// Every execution in the directory, oldest first. A file that does not start with a
    /// seed, or whose id is not one this executable mints, is skipped with a warning; a
    /// line that does not parse ends that file's reading there, as a torn last write would.
    pub fn read(&self) -> Vec<Recorded> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut recorded: Vec<Recorded> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .filter_map(|p| read_file(&p))
            .collect();
        recorded.sort_by(|a, b| {
            (a.seed.created_at.as_str(), a.seed.id.as_str())
                .cmp(&(b.seed.created_at.as_str(), b.seed.id.as_str()))
        });
        recorded
    }
}

fn read_file(path: &Path) -> Option<Recorded> {
    let file = File::open(path).ok()?;
    let mut lines = BufReader::new(file).lines();
    let seed = match lines
        .next()?
        .ok()
        .and_then(|l| serde_json::from_str(&l).ok())
    {
        Some(Line::Seed(seed)) if ExecutionId::parse(seed.id.as_str()).is_some() => seed,
        _ => {
            tracing::warn!(path = %path.display(), "an execution journal without a seed was skipped");
            return None;
        }
    };
    let mut recorded = Recorded {
        seed,
        events: Vec::new(),
        through: None,
        final_snapshot: None,
    };
    for line in lines {
        let Some(line) = line
            .ok()
            .and_then(|l| serde_json::from_str::<Line>(&l).ok())
        else {
            tracing::warn!(path = %path.display(), "an execution journal ends in a line that does not parse; read up to it");
            break;
        };
        match line {
            Line::Seed(_) => {}
            Line::Event(event) => recorded.events.push(event),
            Line::Through { through } => recorded.through = Some(through),
            Line::Final(snapshot) => recorded.final_snapshot = Some(*snapshot),
        }
    }
    Some(recorded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::event::EventPayload;
    use crate::execution::model::ActorKind;

    fn seed(id: &ExecutionId) -> SeedLine {
        SeedLine {
            id: id.clone(),
            capability: "executions.demonstrate".into(),
            title: "Demonstrate".into(),
            input: serde_json::json!({ "steps": 1 }),
            cancellable: true,
            actor: Actor::of(ActorKind::Http),
            repository: RepositoryRef {
                name: "r".into(),
                id: "id".into(),
                branch: None,
            },
            created_at: "2026-10-23T00:00:00Z".into(),
            reported_pid: None,
        }
    }

    #[test]
    fn what_is_appended_is_read_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open(dir.path()).unwrap();
        let id = ExecutionId::fresh();
        journal.append(&id, &Line::Seed(seed(&id)));
        journal.append(
            &id,
            &Line::Event(ExecutionEvent::new(id.clone(), 1, EventPayload::Started)),
        );
        journal.append(&id, &Line::Through { through: 1 });
        let read = journal.read();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].seed.id, id);
        assert_eq!(read[0].events.len(), 1);
        assert_eq!(read[0].through, Some(1));
        assert!(read[0].final_snapshot.is_none());
    }

    #[test]
    fn a_torn_last_line_ends_the_reading_and_keeps_what_came_before() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open(dir.path()).unwrap();
        let id = ExecutionId::fresh();
        journal.append(&id, &Line::Seed(seed(&id)));
        journal.append(
            &id,
            &Line::Event(ExecutionEvent::new(id.clone(), 1, EventPayload::Started)),
        );
        let mut file = OpenOptions::new()
            .append(true)
            .open(dir.path().join(format!("{}.jsonl", id.as_str())))
            .unwrap();
        file.write_all(b"{\"line\":\"event\",\"sequ").unwrap();
        let read = journal.read();
        assert_eq!(read[0].events.len(), 1, "the torn line is not believed");
    }

    #[test]
    fn an_archived_execution_is_moved_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open(dir.path()).unwrap();
        let id = ExecutionId::fresh();
        journal.append(&id, &Line::Seed(seed(&id)));
        journal.archive(&id);
        assert!(journal.read().is_empty(), "no longer recovered");
        assert!(dir
            .path()
            .join("archive")
            .join(format!("{}.jsonl", id.as_str()))
            .exists());
    }
}
