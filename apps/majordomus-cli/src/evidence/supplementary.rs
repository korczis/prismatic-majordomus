//! Supplementary records: the run records read beside a verdict, which may weaken it and
//! never strengthen it (ADR 0087, decision 2).
//!
//! A verdict is derived from the tracked ledger only. A run record — a local run of the
//! local run index ([`super::local_runs`]), or a CI evidence artifact named inside the
//! checkout — is supplementary: a surface lists it beside the verdict and applies it only
//! through the monotone rule, [`weaken_by_records`] over
//! [`weakened_by`](super::freshness::weakened_by). This module is where a surface applies
//! that rule and names what it did:
//!
//! * [`weaken_naming`] is the rule applied to one judgement, with the records that each
//!   alone would have weakened it. The site's publication ([`super::publication`]) and a
//!   subject's answer ([`answer`]) both call it, so the two cannot weaken a verdict apart.
//! * [`answer`] judges one subject of the index with every route passed through it, and lists
//!   the records read beside the verdict, each weakening with the records that made it.
//! * [`EvidenceRunSummary`] and [`EvidenceRunCase`] are what `evidence runs` and `evidence
//!   run <id>` show of a record: they show records only, and decide nothing.
//!
//! ```
//! use majordomus_cli::evidence::supplementary::weaken_naming;
//! use majordomus_cli::evidence::{EvidenceRunRecord, Execution, Judgement, ProofState};
//! use majordomus_cli::git::Containment;
//!
//! let exec = |outcome: &str, commit: &str| -> Execution { serde_json::from_value(serde_json::json!({
//!     "test": "suite:01_a", "runner": "suite", "source": "test/cases/01_a.sh",
//!     "outcome": outcome, "seconds": 1, "commit": commit, "working_tree": "clean",
//!     "digest": "sha256:0", "at": "2026-09-26T12:00:00Z", "origin": "local",
//!     "command": "bash test/run.sh 01_a"
//! })).unwrap() };
//! let record = |id: &str, outcome: &str| -> EvidenceRunRecord { serde_json::from_value(serde_json::json!({
//!     "schema": 1, "id": id, "origin": "local", "commit": "f".repeat(40),
//!     "working_tree": "clean", "recorded_at": "2026-09-26T12:00:00Z", "ledger": "local",
//!     "executions": [exec(outcome, &"f".repeat(40))]
//! })).unwrap() };
//! let evidence = exec("pass", &"e".repeat(40));
//! let all = |_: &str, _: &str| Containment::Contains;
//! let judged = |state| Judgement { state, changed: vec![], detail: None };
//!
//! // a failing record weakens a proven verdict to stale, and is named
//! let (j, by) = weaken_naming(judged(ProofState::Proven), Some(&evidence),
//!     &[record("failed", "fail"), record("passed", "pass")], Some(&"f".repeat(40)), &all);
//! assert_eq!((j.state, by), (ProofState::Stale, vec!["failed".to_string()]));
//!
//! // a passing record never raises a failing one
//! let (j, by) = weaken_naming(judged(ProofState::Failing), Some(&evidence),
//!     &[record("passed", "pass")], Some(&"f".repeat(40)), &all);
//! assert_eq!(j.state, ProofState::Failing);
//! assert!(by.is_empty());
//! ```

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::adapters::EvidenceCase;
use super::freshness::{Judgement, Presented, TreeState};
use super::ledger::{Ledger, LedgerTarget};
use super::local_runs::LocalRun;
use super::run::{weaken_by_records, EvidenceRunRecord, EvidenceRunTotals};
use super::subject::{self, Adjust, Judge, SubjectEvidence, UnknownSubject};
use super::{Execution, Origin, Outcome, ProofState, TestId};
use crate::capability::Context;
use crate::git::Containment;

/// One judgement through the monotone rule over `records`, and the ids of the records that
/// each, applied alone, would have weakened it, in the order `records` holds them.
///
/// The judgement is [`weaken_by_records`]'s over all of them, so it is the input state or
/// `stale`, never anything stronger, and its detail names the most recent record that
/// weakened it. With no execution or no presented commit nothing is weakened and nothing is
/// named.
///
/// ```
/// use majordomus_cli::evidence::supplementary::weaken_naming;
/// use majordomus_cli::evidence::{Judgement, ProofState};
/// use majordomus_cli::git::Containment;
/// let proven = Judgement { state: ProofState::Proven, changed: vec![], detail: None };
/// let (j, by) = weaken_naming(proven.clone(), None, &[], None, &|_, _| Containment::Contains);
/// assert_eq!(j, proven);
/// assert!(by.is_empty());
/// ```
pub fn weaken_naming(
    judgement: Judgement,
    execution: Option<&Execution>,
    records: &[EvidenceRunRecord],
    presented: Option<&str>,
    contains: &dyn Fn(&str, &str) -> Containment,
) -> (Judgement, Vec<String>) {
    let by: Vec<String> = records
        .iter()
        .filter(|r| {
            let alone = std::slice::from_ref(*r);
            let j = weaken_by_records(judgement.clone(), execution, alone, presented, contains);
            j.state != judgement.state
        })
        .map(|r| r.id.clone())
        .collect();
    let all = weaken_by_records(judgement, execution, records, presented, contains);
    (all, by)
}

/// A file of a run-record source that was not read as a run record, and why: listed rather
/// than skipped, so that a source that held nothing readable never reads as one that held
/// no runs.
///
/// ```
/// use majordomus_cli::evidence::supplementary::SupplementaryUnread;
/// let u = SupplementaryUnread { file: "manifest.json".into(), reason: "not a run record".into() };
/// assert_eq!(serde_json::to_value(&u).unwrap()["reason"], "not a run record");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSupplementaryUnread")]
pub struct SupplementaryUnread {
    /// The file, repository-relative.
    pub file: String,
    /// Why it was not read.
    pub reason: String,
}

/// The run records of one source, as read: where from, whether it exists, every record with
/// its file in the canonical order (oldest first), and what was not a run record.
///
/// ```
/// use majordomus_cli::evidence::supplementary::SupplementarySource;
/// let none = SupplementarySource::absent(".ai/local/evidence/runs");
/// assert!(!none.present && none.runs.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SupplementarySource {
    /// The file or directory read, repository-relative.
    pub source: String,
    /// Whether it exists.
    pub present: bool,
    /// The run records read, oldest first.
    pub runs: Vec<LocalRun>,
    /// The files that are not run records.
    pub unread: Vec<SupplementaryUnread>,
}

impl SupplementarySource {
    /// A source that does not exist: nothing read, nothing unread.
    ///
    /// ```
    /// use majordomus_cli::evidence::supplementary::SupplementarySource;
    /// assert_eq!(SupplementarySource::absent("x").source, "x");
    /// ```
    pub fn absent(source: &str) -> SupplementarySource {
        SupplementarySource {
            source: source.to_string(),
            present: false,
            runs: Vec::new(),
            unread: Vec::new(),
        }
    }
}

/// One run record read beside a subject's verdict.
///
/// ```
/// use majordomus_cli::evidence::supplementary::SupplementaryRecord;
/// use majordomus_cli::evidence::{Origin, TreeState};
/// let r = SupplementaryRecord {
///     id: "local:ffffffffffff:20261022T120000Z".into(), origin: Origin::Local,
///     commit: "f".repeat(40), working_tree: TreeState::Clean,
///     recorded_at: "2026-10-22T12:00:00Z".into(),
///     file: ".ai/local/evidence/runs/run-x.json".into(), executions: 1,
///     weakened: vec!["suite:00_x".into()],
/// };
/// assert_eq!(serde_json::to_value(&r).unwrap()["weakened"][0], "suite:00_x");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSupplementaryRecord")]
pub struct SupplementaryRecord {
    /// The record's id.
    pub id: String,
    /// Where its run happened.
    pub origin: Origin,
    /// The commit it was recorded at.
    pub commit: String,
    /// The weakest tree over the reports it was recorded from.
    pub working_tree: TreeState,
    /// When it was recorded, RFC 3339 UTC.
    pub recorded_at: String,
    /// The file it was read from, repository-relative.
    pub file: String,
    /// How many executions it holds.
    pub executions: usize,
    /// The tests whose route this record weakened in this answer, through the monotone
    /// rule; empty when it weakened nothing.
    #[serde(default)]
    pub weakened: Vec<String>,
}

/// One route state the monotone rule weakened, and the records that weakened it.
///
/// ```
/// use majordomus_cli::evidence::supplementary::SupplementaryWeakening;
/// use majordomus_cli::evidence::ProofState;
/// let w = SupplementaryWeakening {
///     test: "suite:00_x".into(), from: ProofState::Proven, to: ProofState::Stale,
///     by: vec!["local:ffffffffffff:20261022T120000Z".into()], detail: "failed".into(),
/// };
/// assert_eq!(serde_json::to_value(&w).unwrap()["to"], "stale");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSupplementaryWeakening")]
pub struct SupplementaryWeakening {
    /// The test whose route was weakened.
    pub test: String,
    /// The route's state before the rule: `proven` or `inputs_unchanged`.
    pub from: ProofState,
    /// Its state after: `stale`, never stronger than `from`.
    pub to: ProofState,
    /// The ids of the records that each weakened it.
    pub by: Vec<String>,
    /// The rule's sentence, naming the most recent of them.
    pub detail: String,
}

/// The supplementary records read beside a subject's verdict, and what they did to it.
///
/// ```
/// use majordomus_cli::evidence::supplementary::SubjectSupplementary;
/// let s = SubjectSupplementary {
///     source: ".ai/local/evidence/runs".into(), present: false,
///     records: vec![], weakened: vec![], unread: vec![],
/// };
/// assert_eq!(serde_json::to_value(&s).unwrap()["present"], false);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSubjectSupplementary")]
pub struct SubjectSupplementary {
    /// The file or directory the records were read from, repository-relative.
    pub source: String,
    /// Whether it exists.
    pub present: bool,
    /// Every record read, newest first, whether or not it weakened anything.
    pub records: Vec<SupplementaryRecord>,
    /// Every route state the records weakened, by test.
    pub weakened: Vec<SupplementaryWeakening>,
    /// The files of the source that are not run records.
    pub unread: Vec<SupplementaryUnread>,
}

/// The commit the monotone rule places records against: the presented commit, or HEAD for
/// the working tree, and none where git names none.
fn placed_at(root: &Path, presented: &Presented) -> Option<String> {
    match presented {
        Presented::Commit { commit, .. } => Some(commit.clone()),
        Presented::WorkingTree => match crate::git::inspect(root) {
            crate::git::GitState::Available(i) => i.head,
            crate::git::GitState::Unavailable { .. } => None,
        },
    }
}

/// One subject judged at the presented revision, exactly as
/// [`subject::answer`] judges it, with every route that names a test passed through
/// [`weaken_naming`] over the records `read` holds, and those records listed beside the
/// verdict under `supplementary`.
///
/// A record can only weaken: a route keeps the weaker of its own state and the rule's, and the
/// rule caps at `stale`. Each weakening names the records that made it, and each record names
/// the tests it weakened. An unknown key is refused before anything is judged.
///
/// ```
/// use majordomus_cli::evidence::supplementary::{answer, SupplementarySource};
/// use majordomus_cli::evidence::{Ledger, Presented};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let ctx = repo.context().unwrap();
/// let (ledger, p) = (Ledger::empty(), Presented::WorkingTree);
/// let read = SupplementarySource::absent(".ai/local/evidence/runs");
/// let key = "capability:evidence.subject";
/// let a = answer(&ctx, &ledger, &[], &p, key, &read).unwrap();
/// let s = a.supplementary.as_ref().unwrap();
/// assert!(!s.present && s.records.is_empty() && s.weakened.is_empty());
/// assert!(answer(&ctx, &ledger, &[], &p, "feature:no-such", &read).is_err());
/// ```
pub fn answer(
    ctx: &Context,
    ledger: &Ledger,
    uncommitted: &[Execution],
    presented: &Presented,
    key: &str,
    read: &SupplementarySource,
) -> Result<SubjectEvidence, UnknownSubject> {
    let s = subject::index(&ctx.index, &ctx.product, &ctx.registry);
    s.get(key)?;
    let claims = crate::evidence::report_at(&ctx.index, ledger, presented, uncommitted);
    let rules = crate::rules::report(&ctx.index, ledger);
    let root = PathBuf::from(&ctx.index.repository.root);

    // newest first, as the rule reads them and as the answer lists them
    let mut newest: Vec<&LocalRun> = read.runs.iter().collect();
    crate::order::canonical(&mut newest);
    newest.reverse();
    let records: Vec<EvidenceRunRecord> = newest.iter().map(|r| r.record.clone()).collect();
    let head = if records.is_empty() {
        None
    } else {
        placed_at(&root, presented)
    };
    let memo: RefCell<BTreeMap<(String, String), Containment>> = RefCell::new(BTreeMap::new());
    let contains = |descendant: &str, ancestor: &str| -> Containment {
        let key = (descendant.to_string(), ancestor.to_string());
        let known = memo.borrow().get(&key).copied();
        known.unwrap_or_else(|| {
            let c = crate::git::contains(&root, descendant, ancestor);
            memo.borrow_mut().insert(key, c);
            c
        })
    };
    type Weakened = BTreeMap<(String, ProofState), (ProofState, Vec<String>, String)>;
    let weakened: RefCell<Weakened> = RefCell::new(BTreeMap::new());
    let adjust = |_: &TestId, execution: Option<&Execution>, judgement: Judgement| {
        let from = judgement.state;
        let (after, by) = weaken_naming(judgement, execution, &records, head.as_deref(), &contains);
        if let Some(e) = execution.filter(|_| after.state != from) {
            weakened
                .borrow_mut()
                .entry((e.test.clone(), from))
                .or_insert_with(|| (after.state, by, after.detail.clone().unwrap_or_default()));
        }
        after
    };
    let adjust: &Adjust<'_> = &adjust;
    let judge = Judge::new(&s, &claims, &rules, ledger, uncommitted, &root, presented);
    let mut evidence = if head.is_some() {
        judge.with_adjust(adjust).subject(key)?
    } else {
        // no record, or nothing to place one against: the rule weakens nothing
        let mut judge = judge;
        judge.subject(key)?
    };

    let weakened: Vec<SupplementaryWeakening> = weakened
        .into_inner()
        .into_iter()
        .map(|((test, from), (to, by, detail))| SupplementaryWeakening {
            test,
            from,
            to,
            by,
            detail,
        })
        .collect();
    let records = newest
        .iter()
        .map(|r| {
            let tests: BTreeSet<&str> = weakened
                .iter()
                .filter(|w| w.by.contains(&r.record.id))
                .map(|w| w.test.as_str())
                .collect();
            SupplementaryRecord {
                id: r.record.id.clone(),
                origin: r.record.origin,
                commit: r.record.commit.clone(),
                working_tree: r.record.working_tree,
                recorded_at: r.record.recorded_at.clone(),
                file: r.file.clone(),
                executions: r.record.executions.len(),
                weakened: tests.into_iter().map(str::to_string).collect(),
            }
        })
        .collect();
    evidence.supplementary = Some(SubjectSupplementary {
        source: read.source.clone(),
        present: read.present,
        records,
        weakened,
        unread: read.unread.clone(),
    });
    Ok(evidence)
}

/// The outcome a run stands for: the weakest over its executions, in the order error,
/// timeout, fail, skip, pass, so that a run with one failure is a failing run and a run
/// that skipped what it did not fail is not a pass. `None` for a run that recorded nothing.
///
/// ```
/// use majordomus_cli::evidence::supplementary::run_outcome;
/// use majordomus_cli::evidence::Outcome::{Fail, Pass, Skip};
/// assert_eq!(run_outcome([Pass, Skip, Pass]), Some(Skip));
/// assert_eq!(run_outcome([Pass, Fail, Skip]), Some(Fail));
/// assert_eq!(run_outcome([]), None);
/// ```
pub fn run_outcome(outcomes: impl IntoIterator<Item = Outcome>) -> Option<Outcome> {
    let rank = |o: Outcome| match o {
        Outcome::Error => 0,
        Outcome::Timeout => 1,
        Outcome::Fail => 2,
        Outcome::Skip => 3,
        Outcome::Pass => 4,
    };
    outcomes.into_iter().min_by_key(|o| rank(*o))
}

/// One run record as `evidence runs` lists it: its id, its file, where and on what it ran,
/// the outcome it stands for and its counts. It shows a record and decides nothing.
///
/// ```
/// use majordomus_cli::evidence::supplementary::EvidenceRunSummary;
/// use majordomus_cli::evidence::EvidenceRunRecord;
/// let r: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
///     "schema": 1, "id": "local:0123456789ab:20260926T120000Z", "origin": "local",
///     "commit": "0123456789abcdef0123456789abcdef01234567", "working_tree": "clean",
///     "recorded_at": "2026-09-26T12:00:00Z", "ledger": "local"
/// })).unwrap();
/// let s = EvidenceRunSummary::of(".ai/local/evidence/runs/run-x.json", &r);
/// let json = serde_json::to_value(&s).unwrap();
/// assert!(json["outcome"].is_null(), "a run that recorded nothing stands for no outcome");
/// assert_eq!(json["totals"]["executions"], 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceRunSummary {
    /// The record's id, which `evidence run <id>` takes.
    pub id: String,
    /// The file it was read from, repository-relative; `evidence run` takes it as well.
    pub file: String,
    /// Where the run happened.
    pub origin: Origin,
    /// The commit it ran at.
    pub commit: String,
    /// The weakest tree over the reports it was recorded from.
    pub working_tree: TreeState,
    /// When it was recorded, RFC 3339 UTC.
    pub recorded_at: String,
    /// The ledger its executions were merged into.
    pub ledger: LedgerTarget,
    /// Where a reader finds its CI run, when it was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The outcome it stands for ([`run_outcome`]); `null` when it recorded nothing.
    pub outcome: Option<Outcome>,
    /// Its executions, by outcome and by runner.
    pub totals: EvidenceRunTotals,
    /// How many per-test results its executions carry.
    pub cases: usize,
    /// How many entries its reports held that no claim can name.
    pub dropped: usize,
    /// How many lines its reports and outputs held that no case result holds.
    pub dropped_lines: usize,
    /// How many tests its reports named that the repository does not have.
    pub unknown: usize,
    /// Whether it carries a coverage measurement.
    pub coverage: bool,
}

impl EvidenceRunSummary {
    /// The summary of `record`, read from `file`.
    ///
    /// ```
    /// use majordomus_cli::evidence::supplementary::EvidenceRunSummary;
    /// use majordomus_cli::evidence::EvidenceRunRecord;
    /// let r: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
    ///     "schema": 1, "id": "ci:github:1:1", "origin": "ci", "commit": "a".repeat(40),
    ///     "working_tree": "clean", "recorded_at": "2026-09-26T12:00:00Z", "ledger": "repo"
    /// })).unwrap();
    /// assert_eq!(EvidenceRunSummary::of("record.json", &r).id, "ci:github:1:1");
    /// ```
    pub fn of(file: &str, record: &EvidenceRunRecord) -> EvidenceRunSummary {
        EvidenceRunSummary {
            id: record.id.clone(),
            file: file.to_string(),
            origin: record.origin,
            commit: record.commit.clone(),
            working_tree: record.working_tree,
            recorded_at: record.recorded_at.clone(),
            ledger: record.ledger,
            url: record.run.as_ref().map(|r| r.url.clone()),
            outcome: run_outcome(record.executions.iter().map(|e| e.outcome)),
            totals: record.totals.clone(),
            cases: record.executions.iter().map(|e| e.cases.len()).sum(),
            dropped: record.dropped.len(),
            dropped_lines: record.dropped_lines.len(),
            unknown: record.unknown.len(),
            coverage: record.coverage.is_some(),
        }
    }
}

/// One per-test result of a run, with the test whose execution carries it.
///
/// ```
/// use majordomus_cli::evidence::supplementary::EvidenceRunCase;
/// let c: EvidenceRunCase = serde_json::from_value(serde_json::json!({
///     "test": "crate:why", "name": "a_test", "outcome": "fail"
/// })).unwrap();
/// assert_eq!(c.case.name, "a_test");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceRunCase {
    /// The test whose execution carries the result.
    pub test: String,
    /// The result, as its runner's own line reported it.
    #[serde(flatten)]
    pub case: EvidenceCase,
}

impl EvidenceRunCase {
    /// Every per-test result of `record`, in the order its executions and their results
    /// hold them.
    ///
    /// ```
    /// use majordomus_cli::evidence::supplementary::EvidenceRunCase;
    /// use majordomus_cli::evidence::EvidenceRunRecord;
    /// let r: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
    ///     "schema": 1, "id": "x", "origin": "local", "commit": "a".repeat(40),
    ///     "working_tree": "clean", "recorded_at": "2026-09-26T12:00:00Z", "ledger": "local"
    /// })).unwrap();
    /// assert!(EvidenceRunCase::all(&r).is_empty());
    /// ```
    pub fn all(record: &EvidenceRunRecord) -> Vec<EvidenceRunCase> {
        record
            .executions
            .iter()
            .flat_map(|e| {
                e.cases.iter().map(|c| EvidenceRunCase {
                    test: e.test.clone(),
                    case: c.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec(test: &str, outcome: &str, commit: &str) -> Execution {
        serde_json::from_value(serde_json::json!({
            "test": test, "runner": "suite", "source": "test/cases/01_a.sh",
            "outcome": outcome, "seconds": 1, "commit": commit, "working_tree": "clean",
            "digest": "sha256:0", "at": "2026-09-26T12:00:00Z", "origin": "local",
            "command": "bash test/run.sh 01_a"
        }))
        .unwrap()
    }

    fn record(id: &str, at: &str, executions: Vec<Execution>) -> EvidenceRunRecord {
        serde_json::from_value(serde_json::json!({
            "schema": 1, "id": id, "origin": "local", "commit": "f".repeat(40),
            "working_tree": "clean", "recorded_at": at, "ledger": "local",
            "executions": executions
        }))
        .unwrap()
    }

    fn judged(state: ProofState) -> Judgement {
        Judgement {
            state,
            changed: vec![],
            detail: None,
        }
    }

    /// Every state through every record: the result is the input or `stale`, never
    /// stronger, and a state changes only when some record is named for it.
    #[test]
    fn a_record_only_ever_weakens_and_every_weakening_is_named() {
        let f = "f".repeat(40);
        let evidence = exec("suite:01_a", "pass", &"e".repeat(40));
        let all = |_: &str, _: &str| Containment::Contains;
        let states = [
            ProofState::Proven,
            ProofState::InputsUnchanged,
            ProofState::Stale,
            ProofState::Failing,
            ProofState::NotRun,
            ProofState::Unrunnable,
            ProofState::NoTest,
        ];
        for outcome in ["pass", "fail", "skip", "timeout", "error"] {
            let records = [record(
                "r",
                "2026-09-26T12:00:00Z",
                vec![exec("suite:01_a", outcome, &f)],
            )];
            for s in states {
                let (j, by) = weaken_naming(judged(s), Some(&evidence), &records, Some(&f), &all);
                assert!(
                    j.state == s || j.state == ProofState::Stale,
                    "{outcome} moved {s:?} to {:?}",
                    j.state
                );
                assert!(
                    j.state >= s,
                    "{outcome} strengthened {s:?} to {:?}",
                    j.state
                );
                assert_eq!(j.state != s, !by.is_empty(), "{outcome} {s:?}: {by:?}");
            }
        }
    }

    /// Each record that alone would weaken is named, in the order given, and the detail
    /// names the newest of them, as the rule reads them.
    #[test]
    fn every_record_that_weakens_is_named_and_the_newest_is_in_the_detail() {
        let f = "f".repeat(40);
        let evidence = exec("suite:01_a", "pass", &"e".repeat(40));
        let all = |_: &str, _: &str| Containment::Contains;
        let records = [
            record(
                "old",
                "2026-09-26T12:00:00Z",
                vec![exec("suite:01_a", "fail", &f)],
            ),
            record(
                "other-test",
                "2026-09-26T12:30:00Z",
                vec![exec("suite:02_b", "fail", &f)],
            ),
            record(
                "new",
                "2026-09-26T13:00:00Z",
                vec![exec("suite:01_a", "error", &f)],
            ),
        ];
        let (j, by) = weaken_naming(
            judged(ProofState::Proven),
            Some(&evidence),
            &records,
            Some(&f),
            &all,
        );
        assert_eq!(j.state, ProofState::Stale);
        assert_eq!(by, ["old", "new"]);
        assert!(j.detail.unwrap().contains("new"));
    }

    /// The outcome of a run is its weakest execution's, and a run of nothing has none.
    #[test]
    fn a_run_stands_for_its_weakest_outcome() {
        use Outcome::*;
        assert_eq!(run_outcome([Pass, Pass]), Some(Pass));
        assert_eq!(run_outcome([Pass, Timeout, Fail]), Some(Timeout));
        assert_eq!(run_outcome([Error, Timeout]), Some(Error));
        assert_eq!(run_outcome(Vec::new()), None);
    }
}
