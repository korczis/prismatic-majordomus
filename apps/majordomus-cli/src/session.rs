//! The session domain: what one execution episode was, derived from evidence.
//!
//! An episode's record is an **envelope**, not a diary. Everything in it is either a fact
//! the repository can prove — the times from the clock, the heads and commits from git, the
//! references from the ledger's own lines for that episode — or a statement that a fact
//! could not be established. Nothing here calls a model, reads a transcript, or infers a
//! relationship from a timestamp or the shape of an identifier.
//!
//! # Why this module exists
//!
//! Attribution used to be derived from *events that happened inside the window*, which
//! meant a worker who spent a whole episode on a task that was already active emitted none
//! of them and closed a record that said:
//!
//! ```yaml
//! task_id: t-20260905034523-a9f1
//! tasks: []
//! ```
//!
//! One field named the task; the list beside it said the episode had touched no task at
//! all. Both were written by the same command, in the same second, from the same state.
//! Nothing refused the pair, because nothing had ever been asked to compare them: the
//! contract required the *keys*, and a key whose value is an empty list satisfies a
//! contract about keys.
//!
//! The repair is not to widen the event filter. It is to separate two things the old
//! derivation folded together — **what the episode did**, which events record, and **what
//! the episode belonged to**, which the lifecycle knows at the boundary and was throwing
//! away. Every reference here therefore carries the [`Source`] that produced it, and a
//! reader can ask *why* the tool believes a relationship rather than being asked to trust
//! a bare list.
//!
//! # The two silences
//!
//! An empty list is not one fact. It is two, and a record that cannot tell them apart is
//! worse than one that says nothing, because the reader cannot tell which they are holding:
//!
//! - **provably nothing.** No task was active, no event was written, no file changed. The
//!   episode really did nothing attributable, and saying so is a complete answer.
//! - **attribution failed.** The episode committed, changed files, ran for hours — and the
//!   record can account for none of it. That is a defect in this subsystem, and it must be
//!   visible as one.
//!
//! [`Completeness`] is that distinction, and it is derived, never authored. `Complete`
//! means every fact the repository can prove about the episode is in the record, *including*
//! the proof that there was nothing to attribute. `Incomplete` means the episode
//! demonstrably moved the repository and the record cannot say what it was for.
//! `Unverifiable` is reserved for records the evidence no longer exists for — a legacy
//! record written before attribution was derived, and one whose ledger has since rotated.
//!
//! # What is deliberately not here
//!
//! No heuristic reads an issue number out of prose or out of a branch name, and no
//! relationship is inferred from time. Where the canonical model cannot prove a link, this
//! module reports that it cannot, and [`Diagnostic`] names the missing relation. An
//! invented link in an append-only record is indistinguishable from a real one the moment
//! it is written, which is strictly worse than the gap it fills.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which class of evidence put a reference into a record.
///
/// Kept distinct rather than flattened into a boolean, because the classes are not equally
/// strong and a reader deciding whether to trust a link needs to know which one it has. A
/// task the lifecycle observed active at the episode boundary is a fact about the episode;
/// a task named by an event inside it is a fact about the work. Both are legitimate, they
/// answer different questions, and a surface that shows only "attributed: yes" has thrown
/// away the half that decides whether to act on it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// The task was active in this worktree when the episode opened, and the lifecycle
    /// recorded that on `session.started`. The evidence class the old derivation had no
    /// way to express, and the one that produced the `task_id` / `tasks: []` contradiction.
    TaskActiveAtOpen,
    /// A task lifecycle event inside the episode named it: `task.started`,
    /// `task.checkpoint`, `task.handed_over`, `task.finished`, `task.evidence`.
    TaskEvent,
    /// An issue transition inside the episode named it: `plan_start`, `plan_verify`,
    /// `plan_done`, `plan_evidence`.
    PlanEvent,
    /// The issue is the one an episode task declares it is executing. The relation is the
    /// task's own, recorded when the task was opened; nothing here parses an id out of
    /// anything.
    IssueOfTask,
    /// The milestone an attributed issue belongs to, from the canonical plan. A session
    /// never decides milestone membership itself, so it cannot disagree with `plan`.
    PlanMembership,
    /// A ledger event of this episode recorded the object directly — a checkpoint, a
    /// handover, a decision, a question, a piece of evidence.
    LedgerEvent,
    /// Git at the close: the commits between the two heads, and the paths the working tree
    /// held changed.
    GitSnapshot,
}

impl Source {
    /// The word as serialised, which is also the word the record's `attribution:` block
    /// carries and the word a diagnostic quotes.
    ///
    /// ```
    /// use majordomus_cli::session::Source;
    /// assert_eq!(Source::TaskActiveAtOpen.as_str(), "task_active_at_open");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Source::TaskActiveAtOpen => "task_active_at_open",
            Source::TaskEvent => "task_event",
            Source::PlanEvent => "plan_event",
            Source::IssueOfTask => "issue_of_task",
            Source::PlanMembership => "plan_membership",
            Source::LedgerEvent => "ledger_event",
            Source::GitSnapshot => "git_snapshot",
        }
    }

    /// One sentence a surface can show beside a reference, in place of making every reader
    /// learn the vocabulary.
    pub fn explain(self) -> &'static str {
        match self {
            Source::TaskActiveAtOpen => {
                "the task was active in this worktree when the episode opened"
            }
            Source::TaskEvent => "a task event inside the episode named it",
            Source::PlanEvent => "an issue transition inside the episode named it",
            Source::IssueOfTask => "an episode task declares it is executing this issue",
            Source::PlanMembership => "the canonical plan puts the attributed issue in it",
            Source::LedgerEvent => "the episode's own ledger line recorded it",
            Source::GitSnapshot => "git, at the moment the episode closed",
        }
    }
}

/// One reference with the evidence class that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attributed {
    /// The identity, as the canonical model spells it.
    pub id: String,
    /// What put it here.
    pub source: Source,
}

impl Attributed {
    /// A reference from one evidence class.
    pub fn new(id: impl Into<String>, source: Source) -> Self {
        Self {
            id: id.into(),
            source,
        }
    }
}

/// How much of what the repository can prove about an episode is in its record.
///
/// Derived on every read, never stored as an authored value: a completeness somebody typed
/// is a claim, and this type exists to replace claims with derivation.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    /// The evidence that would decide it has not been read, or no longer exists: a record
    /// written before attribution was derived, or one whose ledger window has since rotated
    /// away. Not a failure and not a pass — an honest third answer, so that history is not
    /// quietly counted as either, and the default, because a record nothing has judged must
    /// never read as proven.
    #[default]
    Unverifiable,
    /// Every fact the repository can prove is in the record — including the proof that
    /// there was nothing to attribute. An episode that did nothing is complete.
    Complete,
    /// The episode demonstrably moved the repository and the record cannot say what for.
    /// A defect in this subsystem, reported as one.
    Incomplete,
}

impl Completeness {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::session::Completeness;
    /// assert_eq!(Completeness::Incomplete.as_str(), "incomplete");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Completeness::Complete => "complete",
            Completeness::Incomplete => "incomplete",
            Completeness::Unverifiable => "unverifiable",
        }
    }
}

/// One thing wrong with a record, in the shape this repository's findings already take:
/// what is wrong, why it is unsafe, what the claim rests on, and the command that shows it.
///
/// `project.finding-carries-reproduce` is the rule; a finding without a command to
/// reproduce it is a complaint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// The invariant that decided it, by rule id.
    pub rule: String,
    /// The record it is about.
    pub session_id: String,
    /// What is wrong, in one line.
    pub what: String,
    /// Why that is unsafe to leave.
    pub why: String,
    /// What the verdict rests on.
    pub evidence: String,
    /// What to do about it.
    pub remedy: String,
    /// The command that shows it.
    pub command: String,
}

/// The boundary facts of one episode: everything the lifecycle knows without reading the
/// ledger. This is the input to derivation, not its output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Boundary {
    /// The episode identity.
    pub session_id: String,
    /// When it opened, RFC 3339 UTC.
    pub started_at: String,
    /// When it closed.
    pub closed_at: String,
    /// `closed` or `interrupted`. An episode boundary fact, never a statement about a task.
    pub outcome: String,
    /// The task active in this worktree when the episode **opened**, when one was.
    ///
    /// This is the field whose absence produced the contradiction this module exists for.
    /// It is a fact the lifecycle holds at the boundary and nothing else can recover
    /// afterwards: the task may be finished, abandoned or replaced by the time the episode
    /// closes, and "the current task at close" is a different question with a different
    /// answer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task_id_at_open: String,
    /// The issue that task declared it was executing, when it declared one. The canonical
    /// relation; never parsed out of prose.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issue_of_task: String,
    /// The profile the task was worked under.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub profile: String,
    /// What did the work, as it identified itself.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worker: String,
    /// The branch the episode ran on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it started from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_head: String,
    /// The commit it ended at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
    /// Whether the working tree was clean or dirty at the open.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_working_tree: String,
    /// And at the close.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub working_tree: String,
    /// The commits between the two heads, oldest first, or the single entry `diverged`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
    /// Repository-relative paths the working tree held changed at the close.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_files: Vec<String>,
}

/// One ledger line, reduced to the fields the session domain reads.
///
/// The ledger is the canonical account of what happened and it is append-only, so this is a
/// read of a file another process owns. A line that does not parse is skipped rather than
/// failing the read: a truncated last line from a killed write must not take the
/// subsystem's only account of itself away at the moment it is needed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// The episode that wrote it, empty when the line belongs to none. Selection is by this
    /// stamp and never by a time range: one repository has one ledger, and two sessions
    /// writing at once cannot be told apart by a timestamp.
    pub session: String,
    /// The event name, from the vocabulary in `share/events.yaml`.
    pub event: String,
    /// When it was written, as recorded.
    pub ts: String,
    /// The task it names, under either of the two keys the vocabulary uses: `task_id`
    /// everywhere except `task.evidence`, which spells it `task`. Both are read here, so
    /// that an event the ledger proves occurred cannot be silently dropped by a record.
    pub task_id: String,
    /// The issue a `plan_*` line names.
    pub issue: String,
    /// The checkpoint a `task.checkpoint` line wrote, by path.
    pub checkpoint_path: String,
    /// The handover a `task.handed_over` line wrote, by path.
    pub handover_path: String,
    /// The decision a `decision.recorded` line names, by title.
    pub decision: String,
    /// The question a `question.*` line names, by text.
    pub question: String,
    /// The requirement a piece of evidence covers.
    pub covers: String,
}

/// Every ledger line of a file, oldest first. Order is the file's own, which is the order
/// the commands ran; two events inside one second therefore need no tiebreak.
pub fn ledger(path: &Path) -> Vec<Line> {
    std::fs::read_to_string(path)
        .map(|t| parse_ledger(&t))
        .unwrap_or_default()
}

/// The parse, separated from the read so that it can be exercised without a filesystem.
///
/// ```
/// use majordomus_cli::session::parse_ledger;
/// let lines = parse_ledger(
///     "{\"event\":\"task.evidence\",\"session\":\"s-1\",\"task\":\"t-1\"}\nnot json\n",
/// );
/// assert_eq!(lines.len(), 1);
/// // `task.evidence` spells the task `task`; the reader normalises it
/// assert_eq!(lines[0].task_id, "t-1");
/// ```
pub fn parse_ledger(text: &str) -> Vec<Line> {
    let field = |v: &serde_json::Value, k: &str| -> String {
        v.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string()
    };
    text.lines()
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).ok()?;
            // `task.evidence` is the one event that spells the task `task` rather than
            // `task_id`. The inconsistency is registered in docs/HARDCODING_LEDGER.yaml
            // rather than renamed, because renaming orphans every line already written —
            // so the reader normalises instead, and the event stops being invisible.
            let mut task_id = field(&v, "task_id");
            if task_id.is_empty() {
                task_id = field(&v, "task");
            }
            Some(Line {
                session: field(&v, "session"),
                event: field(&v, "event"),
                ts: field(&v, "ts"),
                task_id,
                issue: field(&v, "issue"),
                checkpoint_path: field(&v, "checkpoint_path"),
                handover_path: field(&v, "handover_path"),
                decision: field(&v, "decision"),
                question: field(&v, "question"),
                covers: field(&v, "covers"),
            })
        })
        .collect()
}

/// The lines one episode wrote, in ledger order.
///
/// ```
/// use majordomus_cli::session::{parse_ledger, window};
/// let all = parse_ledger(
///     "{\"event\":\"a\",\"session\":\"s-1\"}\n{\"event\":\"b\",\"session\":\"s-2\"}\n{\"event\":\"c\"}\n",
/// );
/// // by the stamp the writer put on the line, never by a time range
/// assert_eq!(window(&all, "s-1").len(), 1);
/// // a line belonging to no episode belongs to no episode: that is the answer, not a gap
/// assert_eq!(window(&all, "").len(), 0);
/// ```
pub fn window<'a>(all: &'a [Line], session_id: &str) -> Vec<&'a Line> {
    if session_id.is_empty() {
        return Vec::new();
    }
    all.iter().filter(|l| l.session == session_id).collect()
}

/// Everything derived about one episode: its boundary, what it is attributed to and why,
/// how complete that account is, and what is wrong with it.
///
/// One model. The CLI, the HTTP routes, OpenAPI, MCP, the Cockpit and the record's own
/// rendered body are all projections of this type; none of them derives anything of its own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Facts {
    /// What the lifecycle knew at the episode's edges.
    pub boundary: Boundary,
    /// The tasks the episode belonged to or touched.
    pub tasks: Vec<Attributed>,
    /// The issues, by the canonical model's identity.
    pub issues: Vec<Attributed>,
    /// The milestones those issues belong to.
    pub milestones: Vec<Attributed>,
    /// The checkpoints it recorded, by path.
    pub checkpoints: Vec<Attributed>,
    /// The continuation records it wrote, by path.
    pub handovers: Vec<Attributed>,
    /// The decisions it recorded, by title.
    pub decisions: Vec<Attributed>,
    /// The questions it opened or resolved.
    pub questions: Vec<Attributed>,
    /// The evidence it attached, `issue:requirement` as the ledger recorded it.
    pub evidence: Vec<Attributed>,
    /// How much of the provable account is present.
    pub completeness: Completeness,
    /// What is wrong, each with its remedy.
    pub diagnostics: Vec<Diagnostic>,
}

impl Facts {
    /// Whether the episode left any mark on the repository at all: a commit, a changed
    /// path, or any ledger line of its own. The denominator of the completeness question —
    /// an episode that left no mark has nothing to fail to attribute.
    pub fn moved_the_repository(&self) -> bool {
        !self.boundary.commits.is_empty() || !self.boundary.changed_files.is_empty()
    }

    /// Whether anything at all was attributed to work.
    pub fn attributed_any_work(&self) -> bool {
        !self.tasks.is_empty() || !self.issues.is_empty()
    }
}

/// Derive everything about one episode from its boundary and its ledger window.
///
/// `milestone_of` is the canonical plan's issue-to-milestone mapping, passed in rather than
/// looked up here: a session must never decide milestone membership, so it is given the
/// answer the plan already derived and cannot disagree with it.
///
/// Order is first appearance in the ledger, never sorted. The sequence a worker moved
/// through is itself information and sorting throws it away.
pub fn derive(
    boundary: Boundary,
    window: &[&Line],
    milestone_of: &dyn Fn(&str) -> Option<String>,
) -> Facts {
    let mut f = Facts {
        boundary,
        ..Default::default()
    };

    // --- tasks. Two evidence classes, and the boundary one comes first because it is the
    // relationship the episode *had*, not merely one it touched. The old derivation had
    // only the second, which is why an episode spent entirely inside an already-active task
    // attributed nothing.
    push(&mut f.tasks, &f.boundary.task_id_at_open, Source::TaskActiveAtOpen);
    for l in window {
        if matches!(
            l.event.as_str(),
            "task.started" | "task.checkpoint" | "task.handed_over" | "task.finished" | "task.evidence"
        ) {
            push(&mut f.tasks, &l.task_id, Source::TaskEvent);
        }
    }

    // --- issues. Direct evidence first, then the canonical relation an episode task
    // declares. Nothing reads an id out of prose, a path or a branch name.
    for l in window {
        if matches!(
            l.event.as_str(),
            "plan_start" | "plan_verify" | "plan_done" | "plan_evidence"
        ) {
            push(&mut f.issues, &l.issue, Source::PlanEvent);
        }
    }
    push(&mut f.issues, &f.boundary.issue_of_task, Source::IssueOfTask);

    // --- milestones, from the plan and only from the plan.
    let issue_ids: Vec<String> = f.issues.iter().map(|a| a.id.clone()).collect();
    for id in issue_ids {
        if let Some(m) = milestone_of(&id) {
            push(&mut f.milestones, &m, Source::PlanMembership);
        }
    }

    // --- the records the episode wrote. Each is referenced by the identity it actually
    // has; nothing invents one.
    for l in window {
        match l.event.as_str() {
            "task.checkpoint" => push(&mut f.checkpoints, &l.checkpoint_path, Source::LedgerEvent),
            "task.handed_over" => push(&mut f.handovers, &l.handover_path, Source::LedgerEvent),
            "decision.recorded" => push(&mut f.decisions, &l.decision, Source::LedgerEvent),
            "question.opened" | "question.resolved" => {
                push(&mut f.questions, &l.question, Source::LedgerEvent)
            }
            "plan_evidence" | "task.evidence" => {
                // `plan_evidence` names an issue and `task.evidence` a task; both record
                // what the evidence covers, and the pair is the identity.
                let subject = if l.issue.is_empty() {
                    &l.task_id
                } else {
                    &l.issue
                };
                if !subject.is_empty() && !l.covers.is_empty() {
                    push(
                        &mut f.evidence,
                        &format!("{subject}:{}", l.covers),
                        Source::LedgerEvent,
                    );
                }
            }
            _ => {}
        }
    }

    let (completeness, diagnostics) = judge(&f);
    f.completeness = completeness;
    f.diagnostics = diagnostics;
    f
}

/// Append a reference unless it is empty or already present. First appearance wins, so the
/// evidence class recorded is the strongest one that saw it — the boundary before an event,
/// a direct event before a derived relation.
fn push(into: &mut Vec<Attributed>, id: &str, source: Source) {
    if id.is_empty() || id == "none" {
        return;
    }
    if into.iter().any(|a| a.id == id) {
        return;
    }
    into.push(Attributed::new(id, source));
}

/// Decide how complete the account is, and say what is missing.
///
/// The rule has exactly one shape: *the episode moved the repository* and *the record can
/// attribute none of it* is a defect. Everything else is either a complete account or an
/// honest absence.
fn judge(f: &Facts) -> (Completeness, Vec<Diagnostic>) {
    let mut d = Vec::new();
    let id = &f.boundary.session_id;

    // The contradiction this module was written for. It cannot arise from the derivation
    // above — the boundary task is pushed first — so this is the guard that keeps it from
    // arising again from somewhere else.
    if !f.boundary.task_id_at_open.is_empty()
        && !f.tasks.iter().any(|t| t.id == f.boundary.task_id_at_open)
    {
        d.push(Diagnostic {
            rule: "majordomus.session-records".into(),
            session_id: id.clone(),
            what: format!(
                "the episode opened inside task {} and the attributed task set does not contain it",
                f.boundary.task_id_at_open
            ),
            why: "a record whose two task fields disagree cannot be used to answer what the episode was for, and nothing downstream can tell which half to believe".into(),
            evidence: "session.started recorded the task active at the boundary".into(),
            remedy: "re-derive the record from its ledger window".into(),
            command: format!("majordomus session show {id}"),
        });
    }

    if f.moved_the_repository() && !f.attributed_any_work() {
        d.push(Diagnostic {
            rule: "majordomus.session-records".into(),
            session_id: id.clone(),
            what: format!(
                "the episode produced {} commit(s) and left {} changed path(s), and no task or issue is attributed to it",
                f.boundary.commits.len(),
                f.boundary.changed_files.len()
            ),
            why: "work that reached the repository under no recorded task is work the repository cannot account for afterwards; an empty attribution here is a failure to attribute and not a proof of absence".into(),
            evidence: "git at the close against the episode's ledger window".into(),
            remedy: "open a task before the work, or record why this episode ran outside one".into(),
            command: format!("majordomus session show {id}"),
        });
    }

    // Continuity: an episode cut short while a task was still open owes the next worker a
    // continuation. `closed` makes no such promise — a worker who ends an episode
    // deliberately has said what they mean.
    if f.boundary.outcome == "interrupted"
        && !f.boundary.task_id_at_open.is_empty()
        && f.handovers.is_empty()
    {
        d.push(Diagnostic {
            rule: "majordomus.lifecycle-observed".into(),
            session_id: id.clone(),
            what: format!(
                "the episode was interrupted with task {} still active and wrote no continuation record",
                f.boundary.task_id_at_open
            ),
            why: "an interrupted episode is exactly the case whose next worker has no other account of where the work stood".into(),
            evidence: "outcome: interrupted, and the episode's ledger window holds no task.handed_over".into(),
            remedy: "the end event writes one when session.handover_on_end is true; check that the provider hook reached this repository".into(),
            command: "majordomus doctor".into(),
        });
    }

    let c = if d.is_empty() {
        Completeness::Complete
    } else {
        Completeness::Incomplete
    };
    (c, d)
}

// --------------------------------------------------------------------- rendering

/// The front-matter keys this module writes, in the order it writes them. Declared once so
/// that the writer and the reader cannot drift apart, and so that the order is stable —
/// a record whose key order moved would show as a diff in every clone that rewrote it.
pub const FRONT_MATTER_ORDER: &[&str] = &[
    "schema",
    "kind",
    "created_at",
    "task_id",
    "profile",
    "repository_id",
    "worktree_id",
    "branch",
    "head",
    "working_tree",
    "changed_files",
    "session_id",
    "started_at",
    "closed_at",
    "outcome",
    "completeness",
    "title",
    "worker",
    "start_head",
    "start_working_tree",
    "commits",
    "tasks",
    "issues",
    "milestones",
    "checkpoints",
    "handovers",
    "decisions",
    "questions",
    "evidence",
    "attribution",
];

/// The deterministic human report: what a person opening the Markdown file needs, composed
/// from facts and nothing else.
///
/// Every sentence here is derivable from [`Facts`]. No model is called, no remote request
/// is made, the output depends on nothing but its input, and running it twice on the same
/// input produces the same bytes — which is what lets a validator re-render a stored record
/// and refuse one whose body has drifted from its front matter.
///
/// It is deliberately short. The record is an envelope; the checkpoints, handovers and
/// decisions it references are where the detail lives, and copying them in here would make
/// one immutable document the place four mutable ones are read from.
///
/// ```
/// use majordomus_cli::session::{render_report, Facts, Boundary, Completeness};
/// let f = Facts {
///     boundary: Boundary { outcome: "closed".into(), branch: "master".into(), ..Default::default() },
///     completeness: Completeness::Complete,
///     ..Default::default()
/// };
/// let r = render_report(&f);
/// // an episode with nothing to attribute says so, rather than printing empty headings
/// assert!(r.contains("No attributable repository work was observed"));
/// // and closing an episode never claims a task was finished
/// assert!(r.contains("does not mean"));
/// ```
pub fn render_report(f: &Facts) -> String {
    let b = &f.boundary;
    let mut s = String::new();
    s.push_str("# Session report\n\n");
    s.push_str(
        "Generated from this record's own front matter. Deterministic: the same facts render the same bytes, and `majordomus doctor` refuses a body that has drifted from them.\n\n",
    );

    // --- outcome
    s.push_str("## Outcome\n\n");
    let how = if b.outcome == "interrupted" {
        "Interrupted"
    } else {
        "Closed"
    };
    match duration(&b.started_at, &b.closed_at) {
        Some(d) => s.push_str(&format!("{how} after {d} on `{}`.\n", nonempty(&b.branch))),
        None => s.push_str(&format!("{how} on `{}`.\n", nonempty(&b.branch))),
    }
    s.push('\n');

    // --- work
    s.push_str("## Work\n\n");
    if f.attributed_any_work() {
        if let Some(t) = f.tasks.first() {
            s.push_str(&format!(
                "Primary task: `{}` ({}).\n",
                t.id,
                t.source.explain()
            ));
        }
        s.push_str(&format!(
            "Tasks: {}. Issues: {}. Milestones: {}.\n",
            f.tasks.len(),
            f.issues.len(),
            f.milestones.len()
        ));
        for i in &f.issues {
            s.push_str(&format!("- issue `{}` — {}\n", i.id, i.source.explain()));
        }
    } else if f.moved_the_repository() {
        s.push_str(
            "No task or issue could be attributed to this episode, and the episode did change the repository. That is a gap in the account, not a proof that the work belonged to nothing; the diagnostics below name it.\n",
        );
    } else {
        s.push_str(
            "No attributable repository work was observed: no task was active, no issue moved, no commit was produced and no path was left changed. This is an observed absence and not a failure to observe.\n",
        );
    }
    s.push('\n');

    // --- progress
    s.push_str("## Progress\n\n");
    s.push_str(&format!(
        "- {} commit(s) produced\n- {} changed path(s) at close\n- {} checkpoint(s) recorded\n- {} decision(s) recorded\n- {} question(s) touched\n- {} evidence item(s) attached\n",
        b.commits.len(),
        b.changed_files.len(),
        f.checkpoints.len(),
        f.decisions.len(),
        f.questions.len(),
        f.evidence.len()
    ));
    s.push('\n');

    // --- continuation
    s.push_str("## Continuation\n\n");
    if let Some(h) = f.handovers.first() {
        s.push_str(&format!(
            "Handover: `{}`. The next worker should follow it rather than this record: an envelope says what an episode produced, a handover says what to do next.\n",
            h.id
        ));
    } else if b.outcome == "interrupted" {
        s.push_str("None. The episode was interrupted and wrote no continuation record; the diagnostics below say so.\n");
    } else {
        s.push_str("None was written, and none was owed: the episode ended deliberately.\n");
    }
    s.push('\n');

    // --- verification
    s.push_str("## Verification\n\n");
    s.push_str(&format!(
        "Completeness: **{}**. {}\n",
        f.completeness.as_str(),
        match f.completeness {
            Completeness::Complete =>
                "Every fact the repository can prove about this episode is in this record.",
            Completeness::Incomplete =>
                "The repository can prove something about this episode that this record does not carry.",
            Completeness::Unverifiable =>
                "The evidence that would decide this record's account no longer exists.",
        }
    ));
    s.push_str(
        "\nA closed session does not mean a completed task. This record is an envelope over one execution episode; whether the work it names is finished is `majordomus finish`'s answer and is recorded against the task, not here.\n",
    );
    if !f.diagnostics.is_empty() {
        s.push('\n');
        for d in &f.diagnostics {
            s.push_str(&format!(
                "- **{}** — {}. {} Reproduce: `{}`\n",
                d.rule, d.what, d.why, d.command
            ));
        }
    }
    s
}

/// A value, or a dash. Keeps a rendered report free of empty backticks without letting an
/// absent fact read as a present one.
fn nonempty(s: &str) -> &str {
    if s.is_empty() {
        "(unrecorded)"
    } else {
        s
    }
}

/// The gap between two RFC 3339 stamps, in the coarsest useful unit.
///
/// `None` when either is absent or unparseable, because a duration computed from a stamp
/// nobody wrote is a number with no referent.
///
/// ```
/// use majordomus_cli::session::duration;
/// assert_eq!(duration("2026-09-11T01:00:00Z", "2026-09-11T03:30:00Z").as_deref(), Some("2h 30m"));
/// assert_eq!(duration("2026-09-11T01:00:00Z", "2026-09-11T01:00:45Z").as_deref(), Some("45s"));
/// assert_eq!(duration("", "2026-09-11T01:00:00Z"), None);
/// ```
pub fn duration(from: &str, to: &str) -> Option<String> {
    let secs = epoch(to)?.checked_sub(epoch(from)?)?;
    Some(if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    })
}

/// Seconds since the epoch for `YYYY-MM-DDTHH:MM:SSZ`, by the civil-date algorithm, so that
/// nothing here depends on a date library or on the machine's timezone.
fn epoch(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| s.get(a..z)?.parse::<i64>().ok();
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (hh, mm, ss) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    // days from civil (Howard Hinnant's algorithm), valid for the proleptic Gregorian
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// The `attribution:` block of a record's front matter: every reference that is not
/// self-evident, with the evidence class that produced it.
///
/// Flat keys rather than a nested mapping, because the record's reader is a line-oriented
/// YAML subset and a nested block would be one more thing only our own parser could read
/// (`majordomus.the-yaml-subset-is-a-subset`).
pub fn render_attribution(f: &Facts) -> String {
    let mut by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for a in f
        .tasks
        .iter()
        .chain(&f.issues)
        .chain(&f.milestones)
        .chain(&f.checkpoints)
        .chain(&f.handovers)
        .chain(&f.decisions)
        .chain(&f.questions)
        .chain(&f.evidence)
    {
        by.entry(a.source.as_str()).or_default().push(&a.id);
    }
    if by.is_empty() {
        return "attribution: []\n".to_string();
    }
    let mut s = String::from("attribution:\n");
    for (source, ids) in by {
        for id in ids {
            s.push_str(&format!("  - \"{}: {}\"\n", source, escape(id)));
        }
    }
    s
}

/// The escaping one double-quoted YAML scalar needs.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(event: &str, pairs: &[(&str, &str)]) -> Line {
        let mut l = Line {
            session: "s-1".into(),
            event: event.into(),
            ..Default::default()
        };
        for (k, v) in pairs {
            match *k {
                "task_id" => l.task_id = (*v).into(),
                "issue" => l.issue = (*v).into(),
                "checkpoint_path" => l.checkpoint_path = (*v).into(),
                "handover_path" => l.handover_path = (*v).into(),
                "decision" => l.decision = (*v).into(),
                "question" => l.question = (*v).into(),
                "covers" => l.covers = (*v).into(),
                _ => unreachable!("unknown field in test fixture: {k}"),
            }
        }
        l
    }

    fn no_milestone(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_task_active_at_open_is_attributed_without_any_event() {
        // The defect this module exists for: the whole episode is spent inside a task that
        // was already active, so not one task event falls inside the window.
        let b = Boundary {
            session_id: "s-1".into(),
            task_id_at_open: "t-1".into(),
            commits: vec!["abc1234".into()],
            ..Default::default()
        };
        let f = derive(b, &[], &no_milestone);
        assert_eq!(f.tasks, vec![Attributed::new("t-1", Source::TaskActiveAtOpen)]);
        assert_eq!(f.completeness, Completeness::Complete);
        assert!(f.diagnostics.is_empty());
    }

    #[test]
    fn work_with_no_attribution_is_incomplete_and_named() {
        let b = Boundary {
            session_id: "s-1".into(),
            commits: vec!["abc1234".into()],
            changed_files: vec!["lib/a".into()],
            ..Default::default()
        };
        let f = derive(b, &[], &no_milestone);
        assert_eq!(f.completeness, Completeness::Incomplete);
        assert_eq!(f.diagnostics.len(), 1);
        assert!(f.diagnostics[0].what.contains("no task or issue is attributed"));
        assert!(!f.diagnostics[0].command.is_empty());
    }

    #[test]
    fn an_episode_that_did_nothing_is_complete() {
        let f = derive(Boundary::default(), &[], &no_milestone);
        assert_eq!(f.completeness, Completeness::Complete);
        assert!(!f.moved_the_repository());
        assert!(render_report(&f).contains("No attributable repository work was observed"));
    }

    #[test]
    fn both_task_evidence_classes_are_kept_and_ordered_by_strength() {
        let l = [
            line("task.started", &[("task_id", "t-2")]),
            line("task.checkpoint", &[("task_id", "t-1"), ("checkpoint_path", "p")]),
        ];
        let w: Vec<&Line> = l.iter().collect();
        let b = Boundary {
            session_id: "s-1".into(),
            task_id_at_open: "t-1".into(),
            ..Default::default()
        };
        let f = derive(b, &w, &no_milestone);
        // the boundary task first and labelled by the boundary, even though an event also
        // named it later
        assert_eq!(f.tasks[0], Attributed::new("t-1", Source::TaskActiveAtOpen));
        assert_eq!(f.tasks[1], Attributed::new("t-2", Source::TaskEvent));
        assert_eq!(f.checkpoints.len(), 1);
    }

    #[test]
    fn task_evidence_events_are_not_dropped() {
        // `task.evidence` spells the task `task`, and the old derivation read only
        // `task_id`, so the ledger held an event the record never showed.
        let all = parse_ledger(
            "{\"event\":\"task.evidence\",\"session\":\"s-1\",\"task\":\"t-9\",\"covers\":\"tests\"}\n",
        );
        let w: Vec<&Line> = all.iter().collect();
        let f = derive(
            Boundary {
                session_id: "s-1".into(),
                ..Default::default()
            },
            &w,
            &no_milestone,
        );
        assert_eq!(f.tasks, vec![Attributed::new("t-9", Source::TaskEvent)]);
        assert_eq!(f.evidence, vec![Attributed::new("t-9:tests", Source::LedgerEvent)]);
    }

    #[test]
    fn milestones_come_only_from_the_plan() {
        let l = [line("plan_start", &[("issue", "i-1")])];
        let w: Vec<&Line> = l.iter().collect();
        let f = derive(
            Boundary {
                session_id: "s-1".into(),
                ..Default::default()
            },
            &w,
            &|id| (id == "i-1").then(|| "m-1".to_string()),
        );
        assert_eq!(f.issues, vec![Attributed::new("i-1", Source::PlanEvent)]);
        assert_eq!(
            f.milestones,
            vec![Attributed::new("m-1", Source::PlanMembership)]
        );
    }

    #[test]
    fn the_issue_a_task_declares_is_a_canonical_relation() {
        let b = Boundary {
            session_id: "s-1".into(),
            task_id_at_open: "t-1".into(),
            issue_of_task: "i-7".into(),
            ..Default::default()
        };
        let f = derive(b, &[], &no_milestone);
        assert_eq!(f.issues, vec![Attributed::new("i-7", Source::IssueOfTask)]);
    }

    #[test]
    fn an_interrupted_episode_with_an_active_task_owes_a_continuation() {
        let b = Boundary {
            session_id: "s-1".into(),
            outcome: "interrupted".into(),
            task_id_at_open: "t-1".into(),
            ..Default::default()
        };
        let f = derive(b, &[], &no_milestone);
        assert_eq!(f.completeness, Completeness::Incomplete);
        assert!(f
            .diagnostics
            .iter()
            .any(|d| d.rule == "majordomus.lifecycle-observed"));
        assert!(render_report(&f).contains("wrote no continuation record"));
    }

    #[test]
    fn an_interrupted_episode_that_handed_over_is_complete() {
        let l = [line("task.handed_over", &[("task_id", "t-1"), ("handover_path", ".ai/x.md")])];
        let w: Vec<&Line> = l.iter().collect();
        let b = Boundary {
            session_id: "s-1".into(),
            outcome: "interrupted".into(),
            task_id_at_open: "t-1".into(),
            ..Default::default()
        };
        let f = derive(b, &w, &no_milestone);
        assert_eq!(f.completeness, Completeness::Complete);
        assert!(render_report(&f).contains(".ai/x.md"));
    }

    #[test]
    fn a_closed_episode_owes_no_continuation() {
        let b = Boundary {
            session_id: "s-1".into(),
            outcome: "closed".into(),
            task_id_at_open: "t-1".into(),
            ..Default::default()
        };
        let f = derive(b, &[], &no_milestone);
        assert_eq!(f.completeness, Completeness::Complete);
        assert!(render_report(&f).contains("none was owed"));
    }

    #[test]
    fn rendering_is_deterministic() {
        let l = [line("decision.recorded", &[("task_id", "t-1"), ("decision", "use X")])];
        let w: Vec<&Line> = l.iter().collect();
        let b = Boundary {
            session_id: "s-1".into(),
            started_at: "2026-09-11T01:00:00Z".into(),
            closed_at: "2026-09-11T04:05:00Z".into(),
            outcome: "closed".into(),
            branch: "master".into(),
            task_id_at_open: "t-1".into(),
            ..Default::default()
        };
        let f = derive(b, &w, &no_milestone);
        assert_eq!(render_report(&f), render_report(&f));
        assert!(render_report(&f).contains("Closed after 3h 5m on `master`"));
        assert_eq!(f.decisions.len(), 1);
    }

    #[test]
    fn the_attribution_block_names_every_class_it_used() {
        let b = Boundary {
            session_id: "s-1".into(),
            task_id_at_open: "t-1".into(),
            issue_of_task: "i-1".into(),
            ..Default::default()
        };
        let f = derive(b, &[], &no_milestone);
        let a = render_attribution(&f);
        assert!(a.contains("task_active_at_open: t-1"));
        assert!(a.contains("issue_of_task: i-1"));
        // an episode with nothing attributed writes the empty list, not a bare key
        assert_eq!(
            render_attribution(&Facts::default()),
            "attribution: []\n"
        );
    }

    #[test]
    fn a_reference_is_never_duplicated_and_none_is_never_a_reference() {
        let l = [
            line("task.started", &[("task_id", "t-1")]),
            line("task.finished", &[("task_id", "t-1")]),
            line("task.checkpoint", &[("task_id", "none")]),
        ];
        let w: Vec<&Line> = l.iter().collect();
        let f = derive(
            Boundary {
                session_id: "s-1".into(),
                ..Default::default()
            },
            &w,
            &no_milestone,
        );
        assert_eq!(f.tasks.len(), 1);
    }

    #[test]
    fn a_window_is_selected_by_stamp_and_a_bad_line_is_skipped() {
        let all = parse_ledger(
            "{\"event\":\"a\",\"session\":\"s-1\"}\n{{bad\n{\"event\":\"b\",\"session\":\"s-2\"}\n",
        );
        assert_eq!(all.len(), 2);
        assert_eq!(window(&all, "s-1").len(), 1);
        assert_eq!(window(&all, "s-3").len(), 0);
        assert_eq!(window(&all, "").len(), 0);
    }

    #[test]
    fn durations_and_their_absence() {
        assert_eq!(duration("2026-09-11T01:00:00Z", "2026-09-11T01:00:05Z").as_deref(), Some("5s"));
        assert_eq!(duration("2026-09-11T01:00:00Z", "2026-09-11T01:20:00Z").as_deref(), Some("20m"));
        assert_eq!(duration("2026-09-10T23:00:00Z", "2026-09-11T01:00:00Z").as_deref(), Some("2h 0m"));
        assert_eq!(duration("nonsense", "2026-09-11T01:00:00Z"), None);
        assert_eq!(duration("2026-09-11T01:00:00Z", "2026-09-11T01:00"), None);
        // a report still renders when the clock says nothing
        let f = Facts::default();
        assert!(render_report(&f).contains("(unrecorded)"));
    }

    #[test]
    fn every_source_has_a_word_and_a_sentence() {
        for s in [
            Source::TaskActiveAtOpen,
            Source::TaskEvent,
            Source::PlanEvent,
            Source::IssueOfTask,
            Source::PlanMembership,
            Source::LedgerEvent,
            Source::GitSnapshot,
        ] {
            assert!(!s.as_str().is_empty());
            assert!(!s.explain().is_empty());
        }
        for c in [
            Completeness::Complete,
            Completeness::Incomplete,
            Completeness::Unverifiable,
        ] {
            assert!(!c.as_str().is_empty());
        }
    }

    #[test]
    fn a_contradiction_between_the_two_task_fields_is_refused() {
        // Constructed by hand: derivation cannot produce it, and this is the guard that
        // keeps it from arriving from anywhere else.
        let f = Facts {
            boundary: Boundary {
                session_id: "s-1".into(),
                task_id_at_open: "t-1".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (c, d) = judge(&f);
        assert_eq!(c, Completeness::Incomplete);
        assert!(d[0].what.contains("does not contain it"));
    }

    #[test]
    fn quoting_survives_a_title_with_quotes() {
        let f = Facts {
            decisions: vec![Attributed::new("he said \"no\"", Source::LedgerEvent)],
            ..Default::default()
        };
        assert!(render_attribution(&f).contains("he said \\\"no\\\""));
    }

    #[test]
    fn the_front_matter_order_is_a_superset_of_what_a_record_carries() {
        // A key written but not declared here would sort into a different place in the next
        // clone that rewrote the record.
        for k in ["session_id", "completeness", "attribution", "tasks"] {
            assert!(FRONT_MATTER_ORDER.contains(&k), "{k} is not declared");
        }
    }
}
