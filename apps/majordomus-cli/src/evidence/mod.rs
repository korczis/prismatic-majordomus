//! Evidence: what actually ran, against which commit, and whether it still proves anything.
//!
//! `docs/CLAIMS.yaml` binds a claim to a test with a **path**. A path is a declaration that
//! a proof exists somewhere; it is not the proof. Until this module, a claim reading
//!
//! ```text
//! status: guaranteed
//! test:   test/cases/84_distribution_model.sh
//! ```
//!
//! was rendered on the public site as a guarantee on the strength of that file existing.
//! Nobody could answer, from the repository, whether the case had ever run, against which
//! revision, with what result, or whether the result still applied. `scripts/generate-site-data
//! --check` verified that the path resolves — which is a check that the *reference* is not
//! broken, not that the *claim* is proven.
//!
//! # The model
//!
//! Three objects, and one derivation over them:
//!
//! * a [`TestId`] — the stable identity of a test, derived from the path a claim already
//!   names, so nothing is entered by hand and no claim has to be migrated;
//! * an [`Execution`] — one recorded run of one test: its outcome, its duration, the commit
//!   it ran against, the digest of the test's own source at the time, when, by which runner
//!   and from where;
//! * a [`Ledger`] — the latest execution of every test, committed to the repository as
//!   durable semantic evidence. Not logs: one line's worth of meaning per test.
//!
//! and [`report`], which joins the claims of the index with the ledger and decides, per
//! claim, a [`ProofState`] that the surfaces render and the gate judges.
//!
//! # What "current" means here, exactly
//!
//! A passing run three commits ago is not automatically proof of the tree in front of you,
//! and pretending otherwise is the failure this module exists to prevent. But declaring
//! every result stale the instant anything anywhere changes is sound and useless — it
//! would mean no claim is ever proven except in the seconds after a full run.
//!
//! So the derivation is conservative and says which of two things it found:
//!
//! * [`ProofState::Proven`] — the test passed, and the tree in front of you is byte for
//!   byte the tree the run measured: the diff against the execution's own commit is empty.
//!   The ledger's own row is excluded from that diff, because the evidence is about the
//!   tree rather than part of what the tests measure — without that exclusion `proven`
//!   would be unreachable by construction, since recording dirties the tree and committing
//!   the record moves HEAD past the commit the record names.
//! * [`ProofState::InputsUnchanged`] — the test passed, and **nothing the claim itself
//!   names** (its source document, its implementation, its test) differs between the
//!   recorded commit and the working tree. This is *not* proof at HEAD: something the claim
//!   does not name may have broken it. It is the absence of any known invalidation, and it
//!   is labelled as that everywhere it is shown.
//! * [`ProofState::Stale`] — the test passed, and something the claim names has changed
//!   since. The changed paths are named.
//!
//! The distinction is the point. A surface that collapsed the second into the first would
//! be the green badge whose derivation cannot be inspected.
//!
//! # What this is not
//!
//! It is not tamper-proof, and it does not pretend to be. The ledger is a tracked file; a
//! person can edit `"outcome": "pass"` into it, and the diff is what a reviewer sees. What
//! the recorded digest *does* buy is staleness detection that survives a revert: an
//! execution whose test source no longer hashes to the recorded digest did not run this
//! test, whatever the commit says. Cryptographic ceremony against an attacker who can
//! already commit to the repository would be ceremony with no threat model.
//!
//! It is also not the execution control plane. [`crate::execution`] is about capability
//! executions of a running server — in-flight work, progress, events, gone with the
//! process. This is about test runs, durable, committed and read long after the process
//! that produced them exited. Two different subjects that share an English word.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::index::Index;

pub mod ledger;
pub mod record;

pub use ledger::{Ledger, LEDGER_PATH};
pub use record::{parse_crate_binaries, record, RecordOutcome, RecordRequest};

/// Which runner produced a result, and therefore how the test is named and re-run.
///
/// The repository has exactly two that a claim may name today. A third would be a variant
/// here and a branch in [`TestId::of`]; a claim naming something neither runner produces is
/// reported as [`ProofState::Unrunnable`] rather than silently counted as covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceRunner")]
pub enum Runner {
    /// A behavioural case under `test/cases/`, run by `test/run.sh`.
    Suite,
    /// An integration test binary under `apps/majordomus-cli/tests/`, run by `cargo test`.
    Crate,
}

impl Runner {
    /// The prefix this runner's test ids carry.
    pub fn prefix(self) -> &'static str {
        match self {
            Runner::Suite => "suite",
            Runner::Crate => "crate",
        }
    }
}

/// What a run said about one test.
///
/// `NotRun` is deliberately absent: it is the absence of an execution, not an outcome one
/// had. Encoding it here would let a recorder write "this did not run" and have it counted
/// among the things that did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceOutcome")]
pub enum Outcome {
    /// It passed.
    Pass,
    /// It failed.
    Fail,
    /// It declined to run and said so (an unmet precondition the case reports itself).
    Skip,
    /// The bound fired before it finished.
    Timeout,
    /// The harness could not run it at all.
    Error,
}

impl Outcome {
    /// Did this outcome prove anything?
    pub fn proves(self) -> bool {
        matches!(self, Outcome::Pass)
    }

    /// The word the runner writes, read back. Anything unrecognised is [`Outcome::Error`]:
    /// a result nobody can classify is not a pass.
    pub fn parse(word: &str) -> Outcome {
        match word.trim().to_ascii_lowercase().as_str() {
            "ok" | "pass" | "passed" => Outcome::Pass,
            "fail" | "failed" => Outcome::Fail,
            "skip" | "skipped" => Outcome::Skip,
            "timeout" => Outcome::Timeout,
            _ => Outcome::Error,
        }
    }
}

/// Where a run happened. The canonical model is provider-neutral: a CI adapter records
/// `Ci`, and nothing here knows or cares which CI it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceOrigin")]
pub enum Origin {
    /// Someone ran it on their own machine.
    Local,
    /// A continuous-integration run.
    Ci,
    /// A release run.
    Release,
}

impl Origin {
    /// Read an origin from the word a recorder was given.
    pub fn parse(word: &str) -> Option<Origin> {
        match word.trim().to_ascii_lowercase().as_str() {
            "local" => Some(Origin::Local),
            "ci" => Some(Origin::Ci),
            "release" => Some(Origin::Release),
            _ => None,
        }
    }
}

/// The stable identity of a test: the runner that owns it and the name that runner knows
/// it by.
///
/// Derived from the path a claim already names, which is what makes the migration from
/// `test: <path>` deterministic and empty of data entry. `test/cases/84_x.sh` is the case
/// `test/run.sh` calls `84_x`; `apps/majordomus-cli/tests/why.rs` is the binary `cargo test
/// --test why` runs. Both spellings already exist in the repository; this only names the
/// join.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceTestId")]
pub struct TestId {
    /// Which runner owns it.
    pub runner: Runner,
    /// The name that runner knows it by: a case's file stem, a test binary's file stem.
    pub name: String,
}

impl TestId {
    /// The canonical string form: `suite:84_distribution_model`, `crate:why`.
    pub fn as_string(&self) -> String {
        format!("{}:{}", self.runner.prefix(), self.name)
    }

    /// The test a path names, when a runner in this repository produces it.
    ///
    /// `None` is an answer, not a failure: a claim may name a file no runner drives (a
    /// template, a fixture, a document), and the report says so rather than counting it.
    ///
    /// ```
    /// use majordomus_cli::evidence::{Runner, TestId};
    /// assert_eq!(
    ///     TestId::of("test/cases/84_distribution_model.sh").unwrap().as_string(),
    ///     "suite:84_distribution_model"
    /// );
    /// assert_eq!(TestId::of("apps/majordomus-cli/tests/why.rs").unwrap().runner, Runner::Crate);
    /// assert!(TestId::of("share/install/install.sh.in").is_none());
    /// assert!(TestId::of("-").is_none());
    /// ```
    pub fn of(path: &str) -> Option<TestId> {
        let path = path.trim().trim_matches(|c| c == '\'' || c == '"');
        if let Some(rest) = path.strip_prefix("test/cases/") {
            let name = rest.strip_suffix(".sh")?;
            if name.is_empty() || name.contains('/') {
                return None;
            }
            return Some(TestId {
                runner: Runner::Suite,
                name: name.to_string(),
            });
        }
        if let Some(rest) = path.strip_prefix("apps/majordomus-cli/tests/") {
            let name = rest.strip_suffix(".rs")?;
            if name.is_empty() || name.contains('/') {
                return None;
            }
            return Some(TestId {
                runner: Runner::Crate,
                name: name.to_string(),
            });
        }
        None
    }

    /// The repository-relative path of the test's own source.
    pub fn source(&self) -> String {
        match self.runner {
            Runner::Suite => format!("test/cases/{}.sh", self.name),
            Runner::Crate => format!("apps/majordomus-cli/tests/{}.rs", self.name),
        }
    }

    /// The exact command that runs this one test, for a reader who wants the proof again
    /// rather than the claim that it exists.
    pub fn reproduce(&self) -> String {
        match self.runner {
            Runner::Suite => format!("bash test/run.sh {}", self.name),
            Runner::Crate => format!("cargo test --test {}", self.name),
        }
    }
}

/// One recorded run of one test: the whole of what this repository durably remembers about
/// it.
///
/// Every field is provenance. A result with no commit is an anonymous green, which is the
/// thing a badge must never be derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceExecution")]
pub struct Execution {
    /// The test, in [`TestId::as_string`] form.
    pub test: String,
    /// Which runner produced it.
    pub runner: Runner,
    /// The test's own source, repository-relative.
    pub source: String,
    /// What it said.
    pub outcome: Outcome,
    /// How long it took, in whole seconds.
    pub seconds: u64,
    /// The full commit id the run was made against.
    pub commit: String,
    /// `clean`, `dirty` or `unknown`: whether the tree the run measured was the commit.
    pub working_tree: String,
    /// `sha256:<hex>` of the test's own source as it was when the run was recorded. A
    /// execution whose test no longer hashes to this did not run the test that is there now.
    pub digest: String,
    /// When it was recorded, RFC 3339, UTC.
    pub at: String,
    /// Where the run happened.
    pub origin: Origin,
    /// The exact command that runs this one test again.
    pub command: String,
}

impl Execution {
    /// Does the test's source still hash to what was recorded?
    ///
    /// `None` when the source is not there to hash — which is itself a finding, reported by
    /// the path checks, not silently read as a match.
    pub fn digest_matches(&self, root: &Path) -> Option<bool> {
        let text = std::fs::read(root.join(&self.source)).ok()?;
        Some(digest_of(&text) == self.digest)
    }
}

/// `sha256:<hex>` of some bytes, the one spelling used in the ledger.
pub fn digest_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{:x}", h.finalize())
}

/// What the repository can say about one claim's proof, right now.
///
/// Ordered from strongest to weakest, so a summary that sorts by this reads as a ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProofState {
    /// A passing run, and nothing has changed since it — the diff against the execution's
    /// own commit is empty but for the ledger. Proof of the tree in front of you.
    Proven,
    /// A passing run, and nothing this claim names has changed since. Not proof at HEAD:
    /// something the claim does not name may have broken it.
    InputsUnchanged,
    /// A passing run, but something this claim names has changed since. The proof is older
    /// than its subject.
    Stale,
    /// The latest run of this claim's test did not pass.
    Failing,
    /// The claim names a test a runner owns, and no run of it has ever been recorded.
    NotRun,
    /// The claim names a path no runner in this repository drives. Nothing can record it,
    /// so nothing can prove it this way.
    Unrunnable,
    /// The claim names no test at all. Correct for a `planned` or `rejected` claim, and a
    /// defect for any other.
    NoTest,
}

impl ProofState {
    /// The word a surface prints.
    pub fn label(self) -> &'static str {
        match self {
            ProofState::Proven => "proven",
            ProofState::InputsUnchanged => "inputs unchanged",
            ProofState::Stale => "stale",
            ProofState::Failing => "failing",
            ProofState::NotRun => "not run",
            ProofState::Unrunnable => "unrunnable",
            ProofState::NoTest => "no test",
        }
    }

    /// One sentence: what the state means, for the reader who clicked the badge. This is
    /// the derivation, in words, and it lives here so that every surface says the same
    /// thing rather than each inventing its own gloss.
    pub fn meaning(self) -> &'static str {
        match self {
            ProofState::Proven => {
                "A passing run, and nothing in the repository has changed since it — the tree \
                 in front of you is the tree the run measured."
            }
            ProofState::InputsUnchanged => {
                "A passing run, and nothing this claim names has changed since it. This is the \
                 absence of a known invalidation, not proof against the current commit: a change \
                 the claim does not name could have broken it."
            }
            ProofState::Stale => {
                "A passing run, but a file this claim names has changed since. The proof is older \
                 than what it is about."
            }
            ProofState::Failing => "The most recent recorded run of this claim's test did not pass.",
            ProofState::NotRun => {
                "The claim names a test, and no run of that test has ever been recorded."
            }
            ProofState::Unrunnable => {
                "The claim names a path that no runner in this repository drives, so no execution \
                 of it can ever be recorded."
            }
            ProofState::NoTest => "The claim names no test.",
        }
    }

    /// Does this state carry a passing execution behind it, of any freshness?
    pub fn passing(self) -> bool {
        matches!(
            self,
            ProofState::Proven | ProofState::InputsUnchanged | ProofState::Stale
        )
    }
}

/// One claim, joined to whatever the repository actually recorded about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceClaimProof")]
pub struct ClaimProof {
    /// The claim's id, as `docs/CLAIMS.yaml` spells it.
    pub id: String,
    /// The claim sentence.
    pub claim: String,
    /// The status the claim declares: `guaranteed`, `advisory`, `planned`, `rejected`.
    pub status: String,
    /// The document that defines the behaviour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The file that implements it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implementation: Option<String>,
    /// The path the claim names as its test, verbatim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_path: Option<String>,
    /// The test's stable identity, when a runner owns it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// What the repository can say about the proof.
    pub state: ProofState,
    /// That state, in one sentence.
    pub meaning: String,
    /// The execution the state was derived from, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<Execution>,
    /// The files this claim names that differ between the recorded commit and the working
    /// tree. Empty unless the state is [`ProofState::Stale`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    /// The command that produces the proof again, when a runner owns the test.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
}

/// A claim whose declared status the evidence does not support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceFinding")]
pub struct Finding {
    /// The claim.
    pub claim: String,
    /// What the claim says it is.
    pub status: String,
    /// What the evidence supports.
    pub state: ProofState,
    /// Why this is a finding.
    pub reason: String,
    /// What to run, when running something would settle it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
}

/// What the ledger is and whether it is there at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceLedgerSummary")]
pub struct LedgerSummary {
    /// Where it lives, repository-relative.
    pub path: String,
    /// Is there one?
    pub present: bool,
    /// How many executions it holds.
    pub executions: usize,
    /// The newest `at` in it, when it holds any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest: Option<String>,
    /// The distinct commits its executions were recorded against.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commits: Vec<String>,
}

/// The whole joined picture: every claim of the matrix against every execution recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceReport {
    /// The commit the report was derived against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `clean`, `dirty` or `unknown`.
    pub working_tree: String,
    /// The ledger this was joined against.
    pub ledger: LedgerSummary,
    /// Every claim, in the matrix's own order.
    pub claims: Vec<ClaimProof>,
    /// How many claims are in each state.
    pub totals: BTreeMap<String, usize>,
    /// Claims whose declared status the evidence does not support.
    pub findings: Vec<Finding>,
}

impl EvidenceReport {
    /// Does the evidence support every claim that declares a guarantee?
    pub fn satisfied(&self) -> bool {
        self.findings.is_empty()
    }
}

// ---------------------------------------------------------------- the derivation

/// A claim as the index holds it: the fields of one `docs/CLAIMS.yaml` entry.
struct IndexedClaim {
    id: String,
    claim: String,
    status: String,
    source: Option<String>,
    implementation: Option<String>,
    test: Option<String>,
}

/// Read one metadata field, treating the matrix's `'-'` placeholder as absence — which is
/// what it means, and what every reader of this file has to agree on.
fn field(meta: &Value, key: &str) -> Option<String> {
    let v = meta.get(key)?.as_str()?.trim().trim_matches('\'');
    if v.is_empty() || v == "-" {
        None
    } else {
        Some(v.to_string())
    }
}

/// Every claim the index holds, in the order the index holds them, which is the matrix's.
fn claims_of(index: &Index) -> Vec<IndexedClaim> {
    index
        .objects
        .iter()
        .filter(|o| o.kind == "claim")
        .map(|o| IndexedClaim {
            id: o.identity.clone(),
            claim: field(&o.metadata, "claim")
                .or_else(|| o.title.clone())
                .unwrap_or_else(|| o.identity.clone()),
            status: field(&o.metadata, "status").unwrap_or_else(|| "unknown".into()),
            source: field(&o.metadata, "source"),
            implementation: field(&o.metadata, "implementation"),
            test: field(&o.metadata, "test"),
        })
        .collect()
}

/// Every path that differs between `commit` and the working tree — committed since, staged,
/// or merely edited. One subprocess per distinct commit in the ledger, which in practice is
/// one.
///
/// `None` when git could not answer, which the caller must not read as "nothing changed":
/// an unanswerable comparison is why [`ProofState`] has to be able to say it does not know.
fn changed_since(root: &Path, commit: &str) -> Option<BTreeSet<String>> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-only", commit, "--"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    )
}

/// Join the claims of the index with the executions of the ledger and decide, per claim,
/// what the repository can honestly say.
///
/// The git comparison is done once per distinct recorded commit and shared by every claim
/// that recorded against it.
pub fn report(index: &Index, ledger: &Ledger) -> EvidenceReport {
    let root = PathBuf::from(&index.repository.root);
    let git = crate::git::inspect(&root);
    // Reported so a reader knows what the report was derived against; the proof states
    // are decided by the diff against each execution's own commit, never by HEAD.
    let (head, working_tree) = match &git {
        crate::git::GitState::Available(i) => (i.head.clone(), i.working_tree.clone()),
        crate::git::GitState::Unavailable { .. } => (None, "unknown".to_string()),
    };

    // one comparison per commit the ledger names, not one per claim
    let mut diffs: BTreeMap<String, Option<BTreeSet<String>>> = BTreeMap::new();
    for e in &ledger.executions {
        diffs
            .entry(e.commit.clone())
            .or_insert_with(|| changed_since(&root, &e.commit));
    }

    let mut claims = Vec::new();
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    let mut findings = Vec::new();

    for c in claims_of(index) {
        let test_id = c.test.as_deref().and_then(TestId::of);
        let execution = test_id
            .as_ref()
            .and_then(|t| ledger.latest(&t.as_string()))
            .cloned();

        let (state, changed) = match (&c.test, &test_id, &execution) {
            (None, _, _) => (ProofState::NoTest, Vec::new()),
            (Some(_), None, _) => (ProofState::Unrunnable, Vec::new()),
            (Some(_), Some(_), None) => (ProofState::NotRun, Vec::new()),
            (Some(_), Some(t), Some(e)) => {
                if !e.outcome.proves() {
                    (ProofState::Failing, Vec::new())
                } else {
                    // what this claim names, and nothing else: the derivation is explicit
                    // about its own reach, and a surface can repeat it
                    let inputs: Vec<String> = [
                        c.source.clone(),
                        c.implementation.clone(),
                        Some(t.source()),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    match diffs.get(&e.commit).and_then(|d| d.as_ref()) {
                        // git could not compare: not knowing is not proof
                        None => (ProofState::Stale, Vec::new()),
                        Some(d) => {
                            // The ledger is evidence *about* the tree, not part of what the
                            // tests measure, so its own row does not age the proof it
                            // records. Without this exclusion `proven` is unreachable by
                            // construction: recording dirties the tree, and committing the
                            // record moves HEAD past the commit the record names.
                            let changed_at_all: Vec<&String> =
                                d.iter().filter(|p| p.as_str() != LEDGER_PATH).collect();
                            let changed: Vec<String> =
                                inputs.iter().filter(|p| d.contains(*p)).cloned().collect();
                            // A test whose source no longer hashes to what ran did not run
                            // in the form it is in now, whatever the diff says — an edit
                            // made and reverted around the run leaves no diff and is still
                            // not the thing that was measured.
                            let test_moved = e.digest_matches(&root) == Some(false);
                            if test_moved && changed.is_empty() {
                                (ProofState::Stale, vec![t.source()])
                            } else if !changed.is_empty() {
                                (ProofState::Stale, changed)
                            } else if changed_at_all.is_empty() {
                                (ProofState::Proven, Vec::new())
                            } else {
                                (ProofState::InputsUnchanged, Vec::new())
                            }
                        }
                    }
                }
            }
        };

        *totals.entry(state.label().to_string()).or_insert(0) += 1;

        if let Some(reason) = unsupported(&c.status, state) {
            findings.push(Finding {
                claim: c.id.clone(),
                status: c.status.clone(),
                state,
                reason,
                reproduce: test_id.as_ref().map(TestId::reproduce),
            });
        }

        claims.push(ClaimProof {
            id: c.id,
            claim: c.claim,
            status: c.status,
            source: c.source,
            implementation: c.implementation,
            test_path: c.test,
            test: test_id.as_ref().map(TestId::as_string),
            state,
            meaning: state.meaning().to_string(),
            execution,
            changed,
            reproduce: test_id.as_ref().map(TestId::reproduce),
        });
    }

    EvidenceReport {
        head,
        working_tree,
        ledger: ledger.summary(),
        claims,
        totals,
        findings,
    }
}

/// Does a claim's declared status survive the evidence found for it? `Some(reason)` when it
/// does not.
///
/// Only `guaranteed` is judged. `advisory` states that enforcement is not observable from
/// outside, `planned` states that nothing implements it and `rejected` that nothing will;
/// demanding a current proof of those would be demanding proof of a thing the claim already
/// says is not there.
fn unsupported(status: &str, state: ProofState) -> Option<String> {
    if status != "guaranteed" {
        return None;
    }
    match state {
        ProofState::Proven | ProofState::InputsUnchanged | ProofState::Stale => None,
        ProofState::Failing => {
            Some("the claim guarantees a behaviour whose test most recently failed".into())
        }
        ProofState::NotRun => Some(
            "the claim guarantees a behaviour and names a test, and no run of that test has ever \
             been recorded"
                .into(),
        ),
        ProofState::Unrunnable => Some(
            "the claim guarantees a behaviour and names a path no runner drives, so no execution \
             of it can be recorded"
                .into(),
        ),
        ProofState::NoTest => {
            Some("the claim guarantees a behaviour and names no test at all".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_case_path_and_a_crate_path_each_name_their_runner() {
        let s = TestId::of("test/cases/07_scope.sh").unwrap();
        assert_eq!(s.runner, Runner::Suite);
        assert_eq!(s.name, "07_scope");
        assert_eq!(s.source(), "test/cases/07_scope.sh");
        assert_eq!(s.reproduce(), "bash test/run.sh 07_scope");

        let c = TestId::of("apps/majordomus-cli/tests/product.rs").unwrap();
        assert_eq!(c.runner, Runner::Crate);
        assert_eq!(c.as_string(), "crate:product");
        assert_eq!(c.reproduce(), "cargo test --test product");
    }

    /// The matrix's own placeholder for "nothing", and a path that is real but that no
    /// runner drives, must both fail to name a test — the first is a claim with no proof,
    /// the second a claim whose proof can never be recorded, and conflating either with a
    /// test id would invent evidence.
    #[test]
    fn a_path_no_runner_drives_names_no_test() {
        assert!(TestId::of("-").is_none());
        assert!(TestId::of("share/install/install.sh.in").is_none());
        assert!(TestId::of("test/cases/").is_none());
        assert!(TestId::of("test/lib.sh").is_none());
        assert!(TestId::of("apps/majordomus-cli/src/why.rs").is_none());
        // a nested path under the cases directory is not a case the runner names
        assert!(TestId::of("test/cases/sub/x.sh").is_none());
    }

    /// Anything the runner did not spell `ok` is not a pass. A result word nobody
    /// recognises is an error, never silently the good outcome.
    #[test]
    fn only_a_recognised_pass_proves_anything() {
        assert!(Outcome::parse("ok").proves());
        assert!(Outcome::parse("PASS").proves());
        assert!(!Outcome::parse("FAIL").proves());
        assert!(!Outcome::parse("TIMEOUT").proves());
        assert_eq!(Outcome::parse("TIMEOUT"), Outcome::Timeout);
        assert_eq!(Outcome::parse("wat"), Outcome::Error);
        assert!(!Outcome::parse("").proves());
    }

    /// Only a guarantee is judged against the evidence; the other three statuses already
    /// say that a current proof is not what they are claiming.
    #[test]
    fn only_a_guarantee_is_held_to_its_evidence() {
        assert!(unsupported("guaranteed", ProofState::NotRun).is_some());
        assert!(unsupported("guaranteed", ProofState::Failing).is_some());
        assert!(unsupported("guaranteed", ProofState::NoTest).is_some());
        assert!(unsupported("guaranteed", ProofState::Unrunnable).is_some());
        assert!(unsupported("guaranteed", ProofState::Stale).is_none());
        assert!(unsupported("guaranteed", ProofState::Proven).is_none());
        for status in ["advisory", "planned", "rejected"] {
            for state in [ProofState::NotRun, ProofState::NoTest, ProofState::Failing] {
                assert!(
                    unsupported(status, state).is_none(),
                    "{status} must not be held to a current proof"
                );
            }
        }
    }

    /// The ranking a summary sorts by, and the guarantee that the two passing-but-not-current
    /// states are never collapsed into the strongest one.
    #[test]
    fn the_states_rank_from_strongest_to_weakest() {
        assert!(ProofState::Proven < ProofState::InputsUnchanged);
        assert!(ProofState::InputsUnchanged < ProofState::Stale);
        assert!(ProofState::Stale < ProofState::Failing);
        assert!(ProofState::Failing < ProofState::NotRun);
        assert!(ProofState::Proven.passing());
        assert!(ProofState::InputsUnchanged.passing());
        assert!(ProofState::Stale.passing());
        assert!(!ProofState::NotRun.passing());
        // every state says what it means, so no surface has to invent a gloss
        for s in [
            ProofState::Proven,
            ProofState::InputsUnchanged,
            ProofState::Stale,
            ProofState::Failing,
            ProofState::NotRun,
            ProofState::Unrunnable,
            ProofState::NoTest,
        ] {
            assert!(!s.meaning().is_empty(), "{} has no meaning", s.label());
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn a_digest_is_over_the_bytes_and_is_prefixed() {
        let d = digest_of(b"majordomus");
        assert!(d.starts_with("sha256:"), "{d}");
        assert_eq!(d.len(), "sha256:".len() + 64);
        assert_eq!(d, digest_of(b"majordomus"));
        assert_ne!(d, digest_of(b"majordomu"));
    }

    /// The matrix writes absence as `-`, sometimes quoted. Every reader has to agree, or a
    /// claim with no implementation reads as a claim implemented by a file called `-`.
    #[test]
    fn the_matrixs_placeholder_for_nothing_reads_as_nothing() {
        let meta = serde_json::json!({
            "a": "-", "b": "'-'", "c": "", "d": "docs/CLI.md", "e": "  docs/X.md  "
        });
        assert_eq!(field(&meta, "a"), None);
        assert_eq!(field(&meta, "b"), None);
        assert_eq!(field(&meta, "c"), None);
        assert_eq!(field(&meta, "d"), Some("docs/CLI.md".into()));
        assert_eq!(field(&meta, "e"), Some("docs/X.md".into()));
        assert_eq!(field(&meta, "missing"), None);
    }
}
