//! Each task's delivery economics, folded from every checkout's ledger (I3376).
//!
//! The owner's report of 2026-10-19 asks, for each task, its time, its interventions, its CI
//! reruns, its spend and whether its result was accepted (T3). The ledger already records a
//! task's steps — `task.started`, every `task.refused`, `task.finished` — and the episodes that
//! carried it, but it is per checkout and untracked, and the one reader that folded it
//! ([`crate::intent_realization::gather`]) read only the root checkout's. This module reads the
//! ledger of every checkout `git worktree list` reports, with the archives `ledger.rotated`
//! left beside it, and answers one record per task.
//!
//! Wall time, refusals and episodes are measured. Interventions, CI reruns, spend and
//! acceptance are not yet observed by anything; each is present on every record as
//! [`TaskEconomicsFieldStatus::NotObserved`] with the reason and the issue that will supply it,
//! so later issues fill a field without changing the response type.
//!
//! ```
//! use majordomus_cli::economics::tasks::{fold, TaskEconomicsQuery, TaskEconomicsSource};
//! let line = |ts: &str, event: &str, extra: &str| {
//!     format!(r#"{{"ts":"{ts}","event":"{event}","head":"h","branch":"b","by":"me","session":"s-1","task_id":"t-1"{extra}}}"#)
//! };
//! let text = [
//!     line("2026-10-21T10:00:00Z", "task.started", r#","issue":"I3376""#),
//!     line("2026-10-21T10:30:00Z", "task.refused", r#","outcome":"completed","unmet":1"#),
//!     line("2026-10-21T11:00:00Z", "task.finished", r#","outcome":"completed""#),
//! ]
//! .join("\n");
//! let source = TaskEconomicsSource::parse("repo", "ledger.jsonl", &text);
//! let list = fold(vec![source], &TaskEconomicsQuery::default());
//! assert_eq!(list.count, 1);
//! let t = &list.tasks[0];
//! assert_eq!(t.wall_seconds, Some(3600));
//! assert_eq!(t.refusals, 1);
//! assert_eq!(t.issue.as_deref(), Some("I3376"));
//! ```
use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::intent_realization::tasks_from_ledger;
use crate::ledger::Entry;

/// Where a checkout keeps its ledger and the archives a rotation leaves beside it.
pub const STATE_DIR: &str = ".ai/local/state";
/// The live ledger of one checkout, under [`STATE_DIR`].
pub const LEDGER: &str = "ledger.jsonl";
/// The suffix of a rotated ledger archive: `ledger.<compact time>.jsonl.archived`
/// (`lib/history.sh`, `mj_history_rotate`).
pub const ARCHIVE_SUFFIX: &str = ".jsonl.archived";

/// Which tasks to answer. A window filters by each task's `task.started` time, so a task
/// with no start line in the ledgers read is answered only when no window is given.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct TaskEconomicsQuery {
    /// Answer only tasks started at or after this time (`YYYY-MM-DDTHH:MM:SSZ`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// Answer only tasks started before this time (`YYYY-MM-DDTHH:MM:SSZ`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
}

/// Every task the ledgers read carry, and what was read to answer it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskEconomicsList {
    /// How many tasks are answered: the number of distinct task ids within the window.
    pub count: usize,
    /// The window the tasks were filtered by, as given.
    pub window: TaskEconomicsQuery,
    /// One record per task, in the order the tasks started.
    pub tasks: Vec<TaskEconomicsRecord>,
    /// Every ledger file read, with its line and skipped-line counts.
    pub ledgers: Vec<TaskEconomicsLedger>,
    /// Every checkout whose ledger could not be read, and why.
    pub unread: Vec<TaskEconomicsUnread>,
}

/// One task's delivery economics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskEconomicsRecord {
    /// The task's id, as `start` wrote it.
    pub task_id: String,
    /// The issue the task was started under, when it named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The intent the task was started under, when it named one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// The checkout whose ledger carries the task.
    pub checkout: String,
    /// The branch the task started on.
    pub branch: String,
    /// `finished` once a `task.finished` line is read, `open` otherwise.
    pub state: TaskEconomicsState,
    /// The outcome as recorded: the finish's outcome, or `active` / `handed_over` while open.
    pub outcome: String,
    /// The time of the task's first `task.started` line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// The time of the task's last `task.finished` line, or of its last `task.refused` line
    /// while it has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Seconds from `started_at` to `ended_at`; absent while either is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall_seconds: Option<i64>,
    /// How many times `finish` refused the task.
    pub refusals: u32,
    /// The episodes that carried the task, each with the span of its lines about it.
    pub episodes: Vec<TaskEconomicsEpisode>,
    /// Human interventions during the task.
    pub interventions: TaskEconomicsField,
    /// CI runs of the task's work that were rerun.
    pub ci_reruns: TaskEconomicsField,
    /// What the task cost.
    pub spend: TaskEconomicsField,
    /// Whether the task's result was accepted.
    pub accepted: TaskEconomicsField,
}

/// Tasks in the order they started, then by id: an RFC 3339 start orders as text, and a task
/// with no recorded start, whose label is empty, comes first. The order is the type's, so
/// every surface that lists tasks reads them in the same sequence.
impl crate::order::Ordered for TaskEconomicsRecord {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(self.started_at.as_deref().unwrap_or(""), &self.task_id)
    }
}

/// Whether a task has finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskEconomicsState {
    /// No `task.finished` line was read for the task.
    Open,
    /// A `task.finished` line was read for the task.
    Finished,
}

/// One episode that carried a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskEconomicsEpisode {
    /// The episode's session id.
    pub session: String,
    /// The provider that ran the episode, when the ledger names it.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// The time of the episode's first line about the task.
    pub first_at: String,
    /// The time of the episode's last line about the task.
    pub last_at: String,
    /// How many of the episode's lines are about the task.
    pub lines: usize,
}

/// A field of the record that nothing observes yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskEconomicsField {
    /// Always `not_observed` until the field's supplier lands.
    pub status: TaskEconomicsFieldStatus,
    /// Why there is no value, naming what will supply it.
    pub reason: String,
}

/// Whether a field holds an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskEconomicsFieldStatus {
    /// Nothing observes the field yet: there is no value, which is not a zero.
    NotObserved,
}

/// One ledger file that was read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskEconomicsLedger {
    /// The checkout the ledger belongs to.
    pub checkout: String,
    /// The file, relative to the checkout.
    pub path: String,
    /// How many lines were read as ledger entries.
    pub lines: usize,
    /// How many non-empty lines could not be read as ledger entries.
    pub skipped: usize,
}

/// A checkout whose ledger was not read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TaskEconomicsUnread {
    /// The checkout.
    pub checkout: String,
    /// Why its ledger was not read.
    pub reason: String,
}

/// The entries of one checkout's ledgers, in the order they were written, and what was read.
#[derive(Debug, Clone, Default)]
pub struct TaskEconomicsSource {
    /// The checkout the entries come from.
    pub checkout: String,
    /// The entries, archives first, then the live ledger.
    pub entries: Vec<Entry>,
    /// The files read.
    pub ledgers: Vec<TaskEconomicsLedger>,
}

impl TaskEconomicsSource {
    /// A source holding one ledger file's text.
    ///
    /// ```
    /// use majordomus_cli::economics::tasks::TaskEconomicsSource;
    /// let s = TaskEconomicsSource::parse("repo", "ledger.jsonl", "not json\n\n");
    /// assert_eq!((s.ledgers[0].lines, s.ledgers[0].skipped), (0, 1));
    /// ```
    pub fn parse(checkout: &str, path: &str, text: &str) -> Self {
        let mut s = TaskEconomicsSource {
            checkout: checkout.to_string(),
            ..Default::default()
        };
        s.push(path, text);
        s
    }

    /// Append one more ledger file's text to this source.
    pub fn push(&mut self, path: &str, text: &str) {
        let (mut lines, mut skipped) = (0, 0);
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<Entry>(line) {
                Ok(e) => {
                    self.entries.push(e);
                    lines += 1;
                }
                Err(_) => skipped += 1,
            }
        }
        self.ledgers.push(TaskEconomicsLedger {
            checkout: self.checkout.clone(),
            path: path.to_string(),
            lines,
            skipped,
        });
    }
}

fn payload(e: &Entry, key: &str) -> String {
    e.payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn not_observed(reason: &str) -> TaskEconomicsField {
    TaskEconomicsField {
        status: TaskEconomicsFieldStatus::NotObserved,
        reason: reason.to_string(),
    }
}

fn in_window(started_at: Option<&str>, q: &TaskEconomicsQuery) -> bool {
    if q.since.is_none() && q.until.is_none() {
        return true;
    }
    let Some(at) = started_at.and_then(crate::peers::epoch_seconds) else {
        return false;
    };
    let after = |b: &Option<String>| b.as_deref().and_then(crate::peers::epoch_seconds);
    after(&q.since).is_none_or(|s| at >= s) && after(&q.until).is_none_or(|u| at < u)
}

/// One record per distinct task id across the sources, filtered by the window.
///
/// Each source is folded on its own with [`tasks_from_ledger`], which names the episodes and
/// the recorded outcome; the times, the refusals and each episode's span are read from the
/// same lines here. A task id met in two sources is one task.
pub fn fold(sources: Vec<TaskEconomicsSource>, q: &TaskEconomicsQuery) -> TaskEconomicsList {
    let mut ledgers = Vec::new();
    let mut records: BTreeMap<String, TaskEconomicsRecord> = BTreeMap::new();
    for source in sources {
        ledgers.extend(source.ledgers.iter().cloned());
        let units = tasks_from_ledger(&source.entries, &BTreeMap::new());
        for unit in units {
            let lines: Vec<&Entry> = source
                .entries
                .iter()
                .filter(|e| e.event.starts_with("task.") && payload(e, "task_id") == unit.id)
                .collect();
            let started = lines.iter().find(|e| e.event == "task.started");
            let finished = lines.iter().rev().find(|e| e.event == "task.finished");
            let refused: Vec<&&Entry> =
                lines.iter().filter(|e| e.event == "task.refused").collect();
            let r = records.entry(unit.id.clone()).or_insert_with(|| TaskEconomicsRecord {
                task_id: unit.id.clone(),
                issue: None,
                intent: None,
                checkout: source.checkout.clone(),
                branch: started.map(|e| e.branch.clone()).unwrap_or_default(),
                state: TaskEconomicsState::Open,
                outcome: String::new(),
                started_at: None,
                ended_at: None,
                wall_seconds: None,
                refusals: 0,
                episodes: Vec::new(),
                interventions: not_observed(
                    "nothing records a human intervention per task yet; I3377 supplies it",
                ),
                ci_reruns: not_observed(
                    "no CI run is joined to a task yet; I3378 supplies the rerun count",
                ),
                spend: not_observed("no token or cost reading is joined to a task yet; I3379 supplies it"),
                accepted: not_observed(
                    "acceptance of a task's result is not recorded yet; I3380 and I3381 supply it",
                ),
            });
            if let Some(s) = started {
                let issue = payload(s, "issue");
                let intent = payload(s, "intent");
                if r.issue.is_none() && !issue.is_empty() {
                    r.issue = Some(issue);
                }
                if r.intent.is_none() && !intent.is_empty() {
                    r.intent = Some(intent);
                }
                if r.started_at.as_deref().is_none_or(|at| s.ts.as_str() < at) {
                    r.started_at = Some(s.ts.clone());
                }
            }
            r.refusals += refused.len() as u32;
            if let Some(f) = finished {
                r.state = TaskEconomicsState::Finished;
                r.ended_at = Some(f.ts.clone());
            } else if r.state == TaskEconomicsState::Open {
                if let Some(last) = refused.last() {
                    r.ended_at = Some(last.ts.clone());
                }
            }
            r.outcome = if unit.outcome.is_empty() {
                "active".to_string()
            } else {
                unit.outcome.clone()
            };
            for ep in &unit.episodes {
                let mine: Vec<&&Entry> = lines
                    .iter()
                    .filter(|e| e.session.as_deref() == Some(ep.session.as_str()))
                    .collect();
                let (Some(first), Some(last)) = (mine.first(), mine.last()) else {
                    continue;
                };
                if !r.episodes.iter().any(|x| x.session == ep.session) {
                    r.episodes.push(TaskEconomicsEpisode {
                        session: ep.session.clone(),
                        provider: ep.provider.clone(),
                        first_at: first.ts.clone(),
                        last_at: last.ts.clone(),
                        lines: mine.len(),
                    });
                }
            }
        }
    }
    let mut tasks: Vec<TaskEconomicsRecord> = records
        .into_values()
        .filter(|r| in_window(r.started_at.as_deref(), q))
        .map(|mut r| {
            r.wall_seconds = match (&r.started_at, &r.ended_at) {
                (Some(a), Some(b)) => crate::peers::epoch_seconds(b)
                    .zip(crate::peers::epoch_seconds(a))
                    .map(|(b, a)| b - a),
                _ => None,
            };
            r
        })
        .collect();
    crate::order::canonical(&mut tasks);
    TaskEconomicsList {
        count: tasks.len(),
        window: q.clone(),
        tasks,
        ledgers,
        unread: Vec::new(),
    }
}

/// The ledgers of one checkout: its rotated archives, oldest first, then its live ledger.
/// `None` when it holds neither.
pub fn read_checkout(dir: &Path, checkout: &str) -> Option<TaskEconomicsSource> {
    let state = dir.join(STATE_DIR);
    let mut archives: Vec<String> = std::fs::read_dir(&state)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("ledger.") && n.ends_with(ARCHIVE_SUFFIX))
                .collect()
        })
        .unwrap_or_default();
    // oldest first: an archive's name carries its rotation stamp, which natural order reads
    crate::order::canonical_strings(&mut archives);
    let mut source = TaskEconomicsSource {
        checkout: checkout.to_string(),
        ..Default::default()
    };
    for name in archives
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(LEDGER))
    {
        let Ok(text) = std::fs::read_to_string(state.join(name)) else {
            continue;
        };
        source.push(&format!("{STATE_DIR}/{name}"), &text);
    }
    (!source.ledgers.is_empty()).then_some(source)
}

/// Every task of every checkout of the repository at `root`.
///
/// The checkouts are the ones `git worktree list` reports; when git cannot be asked, `root`
/// alone is read. A checkout git marks prunable, or one holding no ledger, is listed as not
/// read with the reason.
pub fn tasks(root: &Path, q: &TaskEconomicsQuery) -> TaskEconomicsList {
    let records = crate::worktree::topology::read(root).unwrap_or_default();
    let mut sources = Vec::new();
    let mut unread = Vec::new();
    let checkouts: Vec<(std::path::PathBuf, Option<String>)> = if records.is_empty() {
        vec![(root.to_path_buf(), None)]
    } else {
        records
            .iter()
            .filter(|r| !r.bare)
            .map(|r| (r.path.clone(), r.prunable.clone()))
            .collect()
    };
    for (dir, prunable) in checkouts {
        let name = dir.display().to_string();
        if let Some(why) = prunable {
            unread.push(TaskEconomicsUnread {
                checkout: name,
                reason: format!("git marks it prunable: {why}"),
            });
            continue;
        }
        match read_checkout(&dir, &name) {
            Some(s) => sources.push(s),
            None => unread.push(TaskEconomicsUnread {
                checkout: name,
                reason: format!("no ledger under {STATE_DIR}"),
            }),
        }
    }
    let mut list = fold(sources, q);
    list.unread = unread;
    list
}
