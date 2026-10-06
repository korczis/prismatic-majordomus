//! What a run is made of, beyond the executions it recorded.
//!
//! A recording reads the reports a run wrote: the suite's TSV, `cargo test`'s output, a
//! coverage summary. Every entry of a run belongs to exactly one of those reports, and
//! [`EvidenceProducer`] is the word that names which. Not everything a report holds is
//! something a claim can name: the crate's own unit-test binary and its doctests ran, and no
//! claim of `docs/CLAIMS.yaml` can point at one of their tests yet. Such an entry is listed as
//! an [`EvidenceDropped`], with the report it came from and the reason, rather than ignored,
//! because a recording that silently threw part of a run away reads as a run that was
//! smaller than it was.
//!
//! # What a run is made of
//!
//! A producer word reads back as the producer it names and nothing else reads at all; a
//! `cargo test` output with the crate's unit tests and one integration binary yields one
//! binary a claim can name and one dropped entry that says why the other is not one:
//!
//! ```
//! use majordomus_cli::evidence::{read_crate_output, EvidenceProducer, Outcome};
//!
//! for p in [EvidenceProducer::Suite, EvidenceProducer::Crate, EvidenceProducer::Coverage] {
//!     assert_eq!(EvidenceProducer::parse(p.as_str()), Some(p));
//! }
//! assert_eq!(EvidenceProducer::parse("cargo"), None, "a word no report is named by");
//!
//! let out = "     Running unittests src/lib.rs (target/debug/deps/majordomus_cli-1)\n\
//!            test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
//!            finished in 0.50s\n\
//!                 Running tests/alpha.rs (target/debug/deps/alpha-1)\n\
//!            test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
//!            finished in 1.40s\n";
//! let read = read_crate_output(out);
//! assert_eq!(read.binaries.len(), 1);
//! assert_eq!(read.binaries[0].name, "alpha");
//! assert_eq!(read.binaries[0].outcome, Outcome::Pass);
//!
//! assert_eq!(read.dropped.len(), 1, "the unit tests ran, and are listed rather than ignored");
//! let dropped = &read.dropped[0];
//! assert_eq!(dropped.producer, EvidenceProducer::Crate);
//! assert_eq!(dropped.what, "unittests src/lib.rs");
//! assert!(dropped.reason.contains("no claim can name"), "{}", dropped.reason);
//! ```
//!
//! # A recording, as a typed run
//!
//! Every recording returns an [`EvidenceRunRecord`]: its commit, the tree each report's run
//! measured, its totals, what was absent, dropped or unknown, and the executions, each of
//! which maps back to the record's id:
//!
//! ```
//! use majordomus_cli::evidence::{record, EvidenceProducer, EvidenceRunRecord, Origin, RecordRequest, TreeState};
//! use std::process::Command;
//!
//! let root = tempfile::tempdir().unwrap();
//! let git = |args: &[&str]| {
//!     Command::new("git").arg("-C").arg(root.path()).args(args).output().unwrap()
//! };
//! git(&["init", "-q"]);
//! git(&["config", "user.email", "t@example.com"]);
//! git(&["config", "user.name", "t"]);
//! std::fs::create_dir_all(root.path().join("test/cases")).unwrap();
//! std::fs::write(root.path().join("test/cases/01_a.sh"), "echo a\n").unwrap();
//! git(&["add", "-A"]);
//! git(&["commit", "-qm", "init"]);
//!
//! let reports = tempfile::tempdir().unwrap();
//! let tsv = reports.path().join("run.tsv");
//! std::fs::write(&tsv, "01_a\tok\t1\tparallel\n").unwrap();
//! let out = reports.path().join("run.json");
//! let req = RecordRequest {
//!     suite: Some(tsv),
//!     run_record: Some(out.clone()),
//!     ..RecordRequest::new(Origin::Local)
//! };
//! let run = record(root.path(), &req).unwrap().run_record;
//!
//! // no measurement came with the report, so the run's tree is not vouched for
//! assert_eq!(run.working_tree, TreeState::Unknown);
//! assert_eq!(run.totals.executions, 1);
//! assert_eq!(run.absent[0].producer, EvidenceProducer::Crate);
//! let written: EvidenceRunRecord =
//!     serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
//! assert_eq!(written, run);
//! assert_eq!(EvidenceRunRecord::id_of(&run.executions[0]), run.id);
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::coverage::EvidenceCoverage;
use super::freshness::{weakened_by, Judgement, Presented, Supplementary, TreeState};
use super::ledger::{outcome_counts, LedgerTarget};
use super::provenance::EvidenceProvenance;
use super::{Execution, Origin, RunRef};
use crate::git::Containment;

/// Which report an entry of a run belongs to: the suite's TSV, `cargo test`'s output, or a
/// coverage summary.
///
/// The serde word is the word a person types and a document carries (`suite`, `crate`,
/// `coverage`), and [`EvidenceProducer::parse`] reads exactly those three back. A word that
/// names no report is not a producer, so a misspelled one is refused where it is read
/// rather than guessed at.
///
/// ```
/// use majordomus_cli::evidence::EvidenceProducer;
///
/// assert_eq!(EvidenceProducer::parse("crate"), Some(EvidenceProducer::Crate));
/// assert_eq!(EvidenceProducer::Coverage.as_str(), "coverage");
/// assert_eq!(serde_json::to_value(EvidenceProducer::Suite).unwrap(), "suite");
/// assert_eq!(EvidenceProducer::parse("Suite"), None, "the word is exact");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProducer {
    /// The suite runner's TSV report (`MJ_TEST_REPORT` of `test/run.sh`).
    Suite,
    /// `cargo test`'s output: one result line per test binary.
    Crate,
    /// A coverage summary (`scripts/rust-coverage --summary-json`).
    Coverage,
}

impl EvidenceProducer {
    /// The producer a word names, or `None` when it names none.
    ///
    /// The three words are exact: no case folding and no trimming, because every caller
    /// reads a word a program wrote and a near-miss is a defect to name, not to absorb.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceProducer;
    /// assert_eq!(EvidenceProducer::parse("suite"), Some(EvidenceProducer::Suite));
    /// assert_eq!(EvidenceProducer::parse(" suite"), None);
    /// assert_eq!(EvidenceProducer::parse(""), None);
    /// ```
    pub fn parse(word: &str) -> Option<EvidenceProducer> {
        match word {
            "suite" => Some(EvidenceProducer::Suite),
            "crate" => Some(EvidenceProducer::Crate),
            "coverage" => Some(EvidenceProducer::Coverage),
            _ => None,
        }
    }

    /// The word this producer is written as: the serde word, and what
    /// [`EvidenceProducer::parse`] reads back.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceProducer;
    /// assert_eq!(EvidenceProducer::Crate.as_str(), "crate");
    /// assert_eq!(
    ///     EvidenceProducer::parse(EvidenceProducer::Suite.as_str()),
    ///     Some(EvidenceProducer::Suite)
    /// );
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceProducer::Suite => "suite",
            EvidenceProducer::Crate => "crate",
            EvidenceProducer::Coverage => "coverage",
        }
    }
}

/// Something a report held that no claim can name yet, listed instead of ignored.
///
/// `cargo test` runs the crate's own unit-test binary and its doctests beside the
/// integration binaries under `tests/`. They ran, and a claim names a test by its path, so
/// none of their tests is anything a claim can point at until a runner records them one by
/// one. A recording lists each such entry with the report it came from ([`producer`]), what
/// it was ([`what`]) and why it is not recorded ([`reason`]), so that the part of a run
/// nobody can use yet is visible rather than silently smaller.
///
/// [`producer`]: EvidenceDropped::producer
/// [`what`]: EvidenceDropped::what
/// [`reason`]: EvidenceDropped::reason
///
/// ```
/// use majordomus_cli::evidence::{EvidenceDropped, EvidenceProducer};
///
/// let d: EvidenceDropped = serde_json::from_value(serde_json::json!({
///     "producer": "crate",
///     "what": "doc-tests majordomus_cli",
///     "reason": "doctests, which no claim can name until a runner records them one by one",
/// }))
/// .unwrap();
/// assert_eq!(d.producer, EvidenceProducer::Crate);
/// assert_eq!(serde_json::to_value(&d).unwrap()["producer"], "crate");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceDropped {
    /// The report the entry was read from.
    pub producer: EvidenceProducer,
    /// What the entry was, in the report's own words: `unittests src/lib.rs`,
    /// `doc-tests <crate>`, the path of a binary that is not under `tests/`, or `a result
    /// line` when a result belonged to no binary at all.
    pub what: String,
    /// Why it is listed rather than recorded. It carries no counts.
    pub reason: String,
}

/// The version of the [`EvidenceRunRecord`] document.
pub const RUN_RECORD_SCHEMA: u32 = 1;

/// One recording, as a typed run: what it recorded, against what, measured how, and what
/// the reports held that it could not record.
///
/// Every `evidence record` returns one and writes it with `--run-record <file>`. It is
/// supplementary: a verdict is derived from the tracked ledger only, and a run record can
/// weaken one only through the one monotone rule ([`weakened_by_records`]).
///
/// ```
/// use majordomus_cli::evidence::{EvidenceRunRecord, LedgerTarget, TreeState};
/// let r: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
///     "schema": 1, "id": "local:0123456789ab:20260926T120000Z", "origin": "local",
///     "commit": "0123456789abcdef0123456789abcdef01234567", "working_tree": "clean",
///     "recorded_at": "2026-09-26T12:00:00Z", "ledger": "repo"
/// })).unwrap();
/// assert_eq!((r.working_tree, r.ledger), (TreeState::Clean, LedgerTarget::Repo));
/// assert!(r.executions.is_empty() && r.coverage.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceRunRecord {
    /// [`RUN_RECORD_SCHEMA`].
    pub schema: u32,
    /// [`run_id`] of the recording.
    pub id: String,
    /// Where the run happened.
    pub origin: Origin,
    /// The recorder's HEAD, which every measurement given equals.
    pub commit: String,
    /// The weakest tree over the reports given: `unknown` for a report with no measurement.
    pub working_tree: TreeState,
    /// RFC 3339 UTC, the `at` of every execution.
    pub recorded_at: String,
    /// The ledger the executions were merged into.
    pub ledger: LedgerTarget,
    /// The CI run, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunRef>,
    /// Each report's measurement, ordered suite, crate, coverage, each naming its producer.
    #[serde(default)]
    pub provenance: Vec<EvidenceProvenance>,
    /// Counts over the executions.
    #[serde(default)]
    pub totals: EvidenceRunTotals,
    /// The producers that gave nothing, and why.
    #[serde(default)]
    pub absent: Vec<EvidenceAbsent>,
    /// What the reports held that no claim can name yet.
    #[serde(default)]
    pub dropped: Vec<EvidenceDropped>,
    /// Tests the reports named that this repository does not have.
    #[serde(default)]
    pub unknown: Vec<String>,
    /// The coverage measurement, bound to its commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<EvidenceCoverage>,
    /// The executions recorded.
    #[serde(default)]
    pub executions: Vec<Execution>,
}

/// A recording is ordered by when it was recorded, and its id — unique, the [`run_id`] —
/// makes the order total when two were recorded in the same second. RFC 3339 UTC reads in
/// time order under the canonical comparison, its digit runs being fixed-width.
impl crate::order::Ordered for EvidenceRunRecord {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.recorded_at, &self.id)
    }
}

/// Counts over a run's executions: how many, of each outcome, of each runner. A zero is
/// absent rather than 0.
///
/// ```
/// use majordomus_cli::evidence::EvidenceRunTotals;
/// let t = EvidenceRunTotals::default();
/// assert_eq!(serde_json::to_value(&t).unwrap()["executions"], 0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceRunTotals {
    /// How many executions the run recorded.
    pub executions: usize,
    /// By outcome word, as [`crate::evidence::outcome_counts`] counts them.
    pub outcomes: BTreeMap<String, usize>,
    /// By runner prefix (`suite`, `crate`).
    pub runners: BTreeMap<String, usize>,
}

impl EvidenceRunTotals {
    /// The totals of `executions`: how many there are, how many of each outcome word, and
    /// how many of each runner, with a count nothing reached left out rather than written 0.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceRunTotals;
    /// assert_eq!(EvidenceRunTotals::of(&[]), EvidenceRunTotals::default());
    /// ```
    pub fn of(executions: &[Execution]) -> EvidenceRunTotals {
        let mut runners = BTreeMap::new();
        for e in executions {
            *runners.entry(e.runner.prefix().to_string()).or_insert(0) += 1;
        }
        EvidenceRunTotals {
            executions: executions.len(),
            outcomes: outcome_counts(executions),
            runners,
        }
    }
}

/// A producer that gave the run nothing, and why.
///
/// ```
/// use majordomus_cli::evidence::{EvidenceAbsent, EvidenceProducer};
/// let a = EvidenceAbsent { producer: EvidenceProducer::Coverage, reason: "no coverage report was given".into() };
/// assert_eq!(serde_json::to_value(&a).unwrap()["producer"], "coverage");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceAbsent {
    /// The producer.
    pub producer: EvidenceProducer,
    /// Why it gave nothing.
    pub reason: String,
}

/// The id of a recording: `<origin>:<provider>:<run id>:<attempt>` for a CI run, else
/// `<origin>:<commit[..12]>:<at without '-' and ':'>`.
///
/// ```
/// use majordomus_cli::evidence::{run_id, Origin};
/// let c = "0123456789abcdef0123456789abcdef01234567";
/// assert_eq!(run_id(Origin::Local, None, c, "2026-09-26T12:00:00Z"),
///     "local:0123456789ab:20260926T120000Z");
/// ```
pub fn run_id(origin: Origin, run: Option<&RunRef>, commit: &str, at: &str) -> String {
    match run {
        Some(r) => format!(
            "{}:{}:{}:{}",
            origin_word(origin),
            r.provider,
            r.id,
            r.attempt
        ),
        None => format!(
            "{}:{}:{}",
            origin_word(origin),
            commit.get(..12).unwrap_or(commit),
            at.replace(['-', ':'], "")
        ),
    }
}

impl EvidenceRunRecord {
    /// The id of the run record `execution` belongs to: every execution of a recording
    /// shares its origin, run, commit and `at`.
    ///
    /// ```
    /// use majordomus_cli::evidence::{EvidenceRunRecord, Execution};
    /// let e: Execution = serde_json::from_value(serde_json::json!({
    ///     "test": "suite:01_a", "runner": "suite", "source": "test/cases/01_a.sh",
    ///     "outcome": "pass", "seconds": 1, "commit": "a".repeat(40), "working_tree": "clean",
    ///     "digest": "sha256:0", "at": "2026-09-26T12:00:00Z", "origin": "local",
    ///     "command": "bash test/run.sh 01_a"
    /// })).unwrap();
    /// assert_eq!(EvidenceRunRecord::id_of(&e), "local:aaaaaaaaaaaa:20260926T120000Z");
    /// ```
    pub fn id_of(execution: &Execution) -> String {
        run_id(
            execution.origin,
            execution.run.as_ref(),
            &execution.commit,
            &execution.at,
        )
    }
}

/// The origin's serde word.
fn origin_word(origin: Origin) -> &'static str {
    match origin {
        Origin::Local => "local",
        Origin::Ci => "ci",
        Origin::Release => "release",
    }
}

/// A judgement, weakened by the run records that ran the same test: the second source of
/// the one monotone rule, [`crate::evidence::freshness::weakened_by`].
///
/// It holds no predicate of its own. Each execution of a record that ran `execution`'s test
/// becomes one [`Supplementary`], placed by `contains(descendant, ancestor)`: after the
/// evidence when the record's commit contains the evidence commit, and in the presented
/// history when the presented commit contains the record's. The most recent record comes
/// first. The result is the input state or `stale`, never anything stronger.
///
/// ```
/// use majordomus_cli::evidence::{weaken_by_records, EvidenceRunRecord, Execution, Judgement, ProofState};
/// use majordomus_cli::git::Containment;
/// let exec = |outcome: &str, commit: &str| -> Execution { serde_json::from_value(serde_json::json!({
///     "test": "suite:01_a", "runner": "suite", "source": "test/cases/01_a.sh",
///     "outcome": outcome, "seconds": 1, "commit": commit, "working_tree": "clean",
///     "digest": "sha256:0", "at": "2026-09-26T12:00:00Z", "origin": "ci",
///     "command": "bash test/run.sh 01_a"
/// })).unwrap() };
/// let (e, r) = ("e".repeat(40), "f".repeat(40));
/// let evidence = exec("pass", &e);
/// let record: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
///     "schema": 1, "id": "ci:x", "origin": "ci", "commit": r, "working_tree": "clean",
///     "recorded_at": "2026-09-26T12:00:00Z", "ledger": "repo",
///     "executions": [exec("fail", &r)]
/// })).unwrap();
/// let proven = Judgement { state: ProofState::Proven, changed: vec![], detail: None };
/// let j = weaken_by_records(proven, Some(&evidence), &[record], Some(&r), |_, _| Containment::Contains);
/// assert_eq!(j.state, ProofState::Stale);
/// ```
pub fn weaken_by_records(
    judgement: Judgement,
    execution: Option<&Execution>,
    records: &[EvidenceRunRecord],
    presented_commit: Option<&str>,
    contains: impl Fn(&str, &str) -> Containment,
) -> Judgement {
    let (Some(execution), Some(presented)) = (execution, presented_commit) else {
        return judgement;
    };
    // newest first: the canonical order, which is oldest first, read backwards
    let mut ordered: Vec<&EvidenceRunRecord> = records.iter().collect();
    crate::order::canonical(&mut ordered);
    ordered.reverse();
    let named: Vec<String> = ordered
        .iter()
        .map(|r| format!("the {} run {}", origin_word(r.origin), r.id))
        .collect();
    let mut supplementaries = Vec::new();
    for (r, name) in ordered.iter().zip(&named) {
        for x in r.executions.iter().filter(|x| x.test == execution.test) {
            supplementaries.push(Supplementary {
                execution: x,
                named: name,
                after_evidence: contains(&r.commit, &execution.commit),
                in_presented: contains(presented, &r.commit),
            });
        }
    }
    weakened_by(judgement, &supplementaries)
}

/// [`weaken_by_records`] against a checkout: the presented commit is the one `presented`
/// names, or HEAD for the working tree (none when HEAD is unborn), and containment is
/// [`crate::git::contains`] in `root`.
///
/// ```
/// use majordomus_cli::evidence::{weakened_by_records, Judgement, Presented, ProofState};
/// let dir = tempfile::tempdir().unwrap();
/// let proven = Judgement { state: ProofState::Proven, changed: vec![], detail: None };
/// let j = weakened_by_records(proven.clone(), None, &[], dir.path(), &Presented::WorkingTree);
/// assert_eq!(j, proven);
/// ```
pub fn weakened_by_records(
    judgement: Judgement,
    execution: Option<&Execution>,
    records: &[EvidenceRunRecord],
    root: &Path,
    presented: &Presented,
) -> Judgement {
    let presented_commit = match presented {
        Presented::Commit { commit, .. } => Some(commit.clone()),
        Presented::WorkingTree => match crate::git::inspect(root) {
            crate::git::GitState::Available(i) => i.head,
            crate::git::GitState::Unavailable { .. } => None,
        },
    };
    weaken_by_records(
        judgement,
        execution,
        records,
        presented_commit.as_deref(),
        |d, a| crate::git::contains(root, d, a),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each producer's word reads back as that producer, the serde word is the same word,
    /// and a word that names no report reads as nothing rather than as a guess.
    #[test]
    fn a_producer_word_round_trips_and_nothing_else_parses() {
        for p in [
            EvidenceProducer::Suite,
            EvidenceProducer::Crate,
            EvidenceProducer::Coverage,
        ] {
            assert_eq!(EvidenceProducer::parse(p.as_str()), Some(p));
            assert_eq!(serde_json::to_value(p).unwrap(), p.as_str());
            let back: EvidenceProducer =
                serde_json::from_value(serde_json::json!(p.as_str())).unwrap();
            assert_eq!(back, p);
        }
        for word in [
            "",
            "Suite",
            "CRATE",
            " coverage",
            "cargo",
            "unittests",
            "doc-tests",
        ] {
            assert_eq!(EvidenceProducer::parse(word), None, "{word:?}");
        }
        assert!(serde_json::from_value::<EvidenceProducer>(serde_json::json!("cargo")).is_err());
    }

    /// A dropped entry is a document with its producer's word, what it was and why, and it
    /// reads back as the same entry.
    #[test]
    fn a_dropped_entry_serialises_with_its_producer_word() {
        let d: EvidenceDropped = serde_json::from_value(serde_json::json!({
            "producer": "crate",
            "what": "unittests src/lib.rs",
            "reason": "the crate's own unit tests",
        }))
        .unwrap();
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["producer"], "crate");
        assert_eq!(v["what"], "unittests src/lib.rs");
        assert_eq!(v["reason"], "the crate's own unit tests");
        assert_eq!(
            v.as_object().unwrap().len(),
            3,
            "a field nobody declared: {v}"
        );
        let back: EvidenceDropped = serde_json::from_value(v).unwrap();
        assert_eq!(back, d);
    }

    use crate::evidence::ProofState;

    const E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const R: &str = "ffffffffffffffffffffffffffffffffffffffff";
    const P: &str = "dddddddddddddddddddddddddddddddddddddddd";

    fn exec(test: &str, outcome: &str, commit: &str, tree: &str, origin: &str) -> Execution {
        let name = test
            .trim_start_matches("suite:")
            .trim_start_matches("crate:");
        let (runner, source) = if test.starts_with("crate:") {
            ("crate", format!("apps/majordomus-cli/tests/{name}.rs"))
        } else {
            ("suite", format!("test/cases/{name}.sh"))
        };
        serde_json::from_value(serde_json::json!({
            "test": test, "runner": runner, "source": source, "outcome": outcome, "seconds": 1,
            "commit": commit, "working_tree": tree, "digest": "sha256:0",
            "at": "2026-09-26T12:00:00Z", "origin": origin, "command": "run it"
        }))
        .unwrap()
    }

    fn record_of(
        id: &str,
        at: &str,
        commit: &str,
        executions: Vec<Execution>,
    ) -> EvidenceRunRecord {
        serde_json::from_value(serde_json::json!({
            "schema": 1, "id": id, "origin": "ci", "commit": commit, "working_tree": "clean",
            "recorded_at": at, "ledger": "repo", "executions": executions
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

    /// Containment in a line of history E < R < P: a later commit contains every earlier one.
    fn linear(d: &str, a: &str) -> Containment {
        let pos = |c: &str| [E, R, P].iter().position(|x| *x == c);
        match (pos(d), pos(a)) {
            (Some(d), Some(a)) if d >= a => Containment::Contains,
            (Some(_), Some(_)) => Containment::DoesNotContain,
            _ => Containment::CommitUnknown,
        }
    }

    fn weaken(state: ProofState, records: &[EvidenceRunRecord]) -> Judgement {
        let evidence = exec("suite:01_a", "pass", E, "clean", "local");
        weaken_by_records(judged(state), Some(&evidence), records, Some(P), linear)
    }

    #[test]
    fn the_run_id_names_the_ci_run_or_the_local_commit_and_time() {
        let run: RunRef = serde_json::from_value(serde_json::json!({
            "provider": "github_actions", "id": "77", "attempt": 3, "workflow": "w",
            "job": "j", "url": "u"
        }))
        .unwrap();
        assert_eq!(
            run_id(Origin::Ci, Some(&run), E, "x"),
            "ci:github_actions:77:3"
        );
        assert_eq!(
            run_id(
                Origin::Local,
                None,
                "0123456789abcdef",
                "2026-09-26T12:00:00Z"
            ),
            "local:0123456789ab:20260926T120000Z"
        );
        assert_eq!(
            run_id(Origin::Release, None, "abc", "2026-01-02T03:04:05Z"),
            "release:abc:20260102T030405Z"
        );
    }

    #[test]
    fn every_execution_maps_back_to_its_run_record_id() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output();
            assert!(out.unwrap().status.success());
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        for f in ["test/cases/01_a.sh", "test/cases/02_b.sh"] {
            std::fs::create_dir_all(dir.path().join("test/cases")).unwrap();
            std::fs::write(dir.path().join(f), "x").unwrap();
        }
        git(&["add", "-A"]);
        git(&["commit", "-qm", "init"]);
        let reports = tempfile::tempdir().unwrap();
        let tsv = reports.path().join("r.tsv");
        std::fs::write(&tsv, "01_a\tok\t1\tparallel\n02_b\tFAIL\t2\tparallel\n").unwrap();
        let req = crate::evidence::RecordRequest {
            suite: Some(tsv),
            ..crate::evidence::RecordRequest::new(Origin::Local)
        };
        let got = crate::evidence::record(dir.path(), &req)
            .unwrap()
            .run_record;
        assert_eq!(got.executions.len(), 2);
        for e in &got.executions {
            assert_eq!(EvidenceRunRecord::id_of(e), got.id);
        }
    }

    #[test]
    fn totals_count_outcomes_and_runners() {
        let t = EvidenceRunTotals::of(&[
            exec("suite:01_a", "pass", E, "clean", "ci"),
            exec("suite:02_b", "fail", E, "clean", "ci"),
            exec("crate:why", "pass", E, "clean", "ci"),
        ]);
        assert_eq!(t.executions, 3);
        assert_eq!(
            t.outcomes,
            BTreeMap::from([("fail".into(), 1), ("pass".into(), 2)])
        );
        assert_eq!(
            t.runners,
            BTreeMap::from([("crate".into(), 1), ("suite".into(), 2)])
        );
        assert_eq!(
            serde_json::to_value(EvidenceRunTotals::of(&[])).unwrap()["outcomes"],
            serde_json::json!({})
        );
    }

    #[test]
    fn a_failing_clean_record_between_e_and_p_caps_proven_at_stale() {
        let r = record_of(
            "ci:x:1:1",
            "2026-09-27T00:00:00Z",
            R,
            vec![exec("suite:01_a", "fail", R, "clean", "ci")],
        );
        let j = weaken(ProofState::Proven, &[r]);
        assert_eq!(j.state, ProofState::Stale);
        assert!(j
            .detail
            .unwrap()
            .starts_with("the ci run ci:x:1:1 recorded on ffffffffffff failed"));
    }

    #[test]
    fn inputs_unchanged_is_capped_too() {
        let r = record_of(
            "ci:x:1:1",
            "t",
            R,
            vec![exec("suite:01_a", "error", R, "clean", "ci")],
        );
        assert_eq!(
            weaken(ProofState::InputsUnchanged, &[r]).state,
            ProofState::Stale
        );
    }

    #[test]
    fn a_record_the_presented_revision_does_not_contain_weakens_nothing() {
        let other = "cccccccccccccccccccccccccccccccccccccccc";
        let r = record_of(
            "ci:x:1:1",
            "t",
            other,
            vec![exec("suite:01_a", "fail", other, "clean", "ci")],
        );
        let contains = |d: &str, a: &str| {
            if d == P && a == other {
                Containment::DoesNotContain
            } else {
                Containment::Contains
            }
        };
        let evidence = exec("suite:01_a", "pass", E, "clean", "local");
        let j = weaken_by_records(
            judged(ProofState::Proven),
            Some(&evidence),
            &[r],
            Some(P),
            contains,
        );
        assert_eq!(j, judged(ProofState::Proven));
    }

    #[test]
    fn a_record_that_does_not_contain_e_weakens_nothing() {
        // a record at E's parent: it ran before the evidence
        let before = "0000000000000000000000000000000000000000";
        let r = record_of(
            "ci:x:1:1",
            "t",
            before,
            vec![exec("suite:01_a", "fail", before, "clean", "ci")],
        );
        let contains = |d: &str, a: &str| {
            if d == before && a == E {
                Containment::DoesNotContain
            } else {
                Containment::Contains
            }
        };
        let evidence = exec("suite:01_a", "pass", E, "clean", "local");
        let j = weaken_by_records(
            judged(ProofState::Proven),
            Some(&evidence),
            &[r],
            Some(P),
            contains,
        );
        assert_eq!(j, judged(ProofState::Proven));
    }

    #[test]
    fn a_dirty_record_weakens_nothing() {
        for tree in ["dirty", "unknown"] {
            let r = record_of(
                "ci:x:1:1",
                "t",
                R,
                vec![exec("suite:01_a", "fail", R, tree, "ci")],
            );
            assert_eq!(
                weaken(ProofState::Proven, &[r]),
                judged(ProofState::Proven),
                "{tree}"
            );
        }
    }

    #[test]
    fn a_passing_or_skipped_record_weakens_nothing() {
        for outcome in ["pass", "skip"] {
            let r = record_of(
                "ci:x:1:1",
                "t",
                R,
                vec![exec("suite:01_a", outcome, R, "clean", "ci")],
            );
            assert_eq!(
                weaken(ProofState::Proven, &[r]),
                judged(ProofState::Proven),
                "{outcome}"
            );
        }
        // and a failing record of another test says nothing about this one
        let r = record_of(
            "ci:x:1:1",
            "t",
            R,
            vec![exec("suite:02_b", "fail", R, "clean", "ci")],
        );
        assert_eq!(weaken(ProofState::Proven, &[r]), judged(ProofState::Proven));
    }

    #[test]
    fn nothing_to_weaken_without_evidence_or_a_presented_commit() {
        let r = record_of(
            "ci:x:1:1",
            "t",
            R,
            vec![exec("suite:01_a", "fail", R, "clean", "ci")],
        );
        let evidence = exec("suite:01_a", "pass", E, "clean", "local");
        let proven = judged(ProofState::Proven);
        assert_eq!(
            weaken_by_records(
                proven.clone(),
                None,
                std::slice::from_ref(&r),
                Some(P),
                linear
            ),
            proven
        );
        assert_eq!(
            weaken_by_records(proven.clone(), Some(&evidence), &[r], None, linear),
            proven
        );
    }

    #[test]
    fn weakening_never_strengthens() {
        let states = [
            ProofState::Proven,
            ProofState::InputsUnchanged,
            ProofState::Stale,
            ProofState::Failing,
            ProofState::NotRun,
            ProofState::Unrunnable,
            ProofState::NoTest,
        ];
        for s in states {
            for outcome in ["pass", "fail", "skip", "timeout", "error"] {
                let r = record_of(
                    "ci:x:1:1",
                    "t",
                    R,
                    vec![exec("suite:01_a", outcome, R, "clean", "ci")],
                );
                let got = weaken(s, &[r]).state;
                assert!(
                    got == s || (got == ProofState::Stale && s < ProofState::Stale),
                    "{s:?} {outcome} -> {got:?}"
                );
            }
        }
    }

    #[test]
    fn the_most_recent_counter_record_is_named() {
        let old = record_of(
            "ci:old:1:1",
            "2026-09-26T00:00:00Z",
            R,
            vec![exec("suite:01_a", "fail", R, "clean", "ci")],
        );
        let new = record_of(
            "ci:new:1:1",
            "2026-09-27T00:00:00Z",
            R,
            vec![exec("suite:01_a", "timeout", R, "clean", "ci")],
        );
        let j = weaken(ProofState::Proven, &[old, new]);
        let detail = j.detail.unwrap();
        assert!(
            detail.starts_with("the ci run ci:new:1:1") && detail.contains("timed out"),
            "{detail}"
        );
    }

    #[test]
    fn weakened_by_records_resolves_head_for_the_working_tree() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}");
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        let proven = judged(ProofState::Proven);
        let evidence_at = |c: &str| exec("suite:01_a", "pass", c, "clean", "local");
        // unborn HEAD: no presented commit, nothing to weaken
        let unborn = weakened_by_records(
            proven.clone(),
            Some(&evidence_at(E)),
            &[],
            dir.path(),
            &Presented::WorkingTree,
        );
        assert_eq!(unborn, proven);
        git(&["commit", "-q", "--allow-empty", "-m", "e"]);
        let e = git(&["rev-parse", "HEAD"]);
        git(&["commit", "-q", "--allow-empty", "-m", "r"]);
        let r = git(&["rev-parse", "HEAD"]);
        let rec = record_of(
            "ci:x:1:1",
            "t",
            &r,
            vec![exec("suite:01_a", "fail", &r, "clean", "ci")],
        );
        let j = weakened_by_records(
            proven.clone(),
            Some(&evidence_at(&e)),
            std::slice::from_ref(&rec),
            dir.path(),
            &Presented::WorkingTree,
        );
        assert_eq!(
            j.state,
            ProofState::Stale,
            "HEAD is r, which contains the record"
        );
        // a record at a commit HEAD does not contain weakens nothing
        git(&["checkout", "-q", &e]);
        let j = weakened_by_records(
            proven.clone(),
            Some(&evidence_at(&e)),
            &[rec],
            dir.path(),
            &Presented::WorkingTree,
        );
        assert_eq!(j, proven);
    }

    /// A presented commit is taken as given, and a working tree outside any git work tree
    /// presents no commit: with no record to weigh, either leaves the judgement as it was.
    #[test]
    fn weakened_by_records_takes_a_presented_commit_and_no_head_outside_git() {
        let dir = tempfile::tempdir().unwrap();
        let proven = judged(ProofState::Proven);
        let evidence = exec("suite:01_a", "pass", E, "clean", "local");
        for presented in [
            Presented::Commit {
                commit: P.to_string(),
                tree: TreeState::Clean,
            },
            Presented::WorkingTree,
        ] {
            let j =
                weakened_by_records(proven.clone(), Some(&evidence), &[], dir.path(), &presented);
            assert_eq!(j, proven, "{presented:?}");
        }
    }
}
