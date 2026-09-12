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
//! # The lifecycle, end to end
//!
//! A claim's path becomes an identity, a run becomes an [`Execution`], the ledger keeps the
//! latest one per test, and the digest is what notices that the test itself has moved since
//! — without any commit having to change.
//!
//! ```
//! use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
//!
//! // the identity is derived from the path the claim already names; nothing is entered
//! let id = TestId::of("test/cases/84_distribution_model.sh").unwrap();
//! assert_eq!(id.as_string(), "suite:84_distribution_model");
//! assert_eq!(id.reproduce(), "bash test/run.sh 84_distribution_model");
//!
//! let repo = tempfile::tempdir().unwrap();
//! std::fs::create_dir_all(repo.path().join("test/cases")).unwrap();
//! std::fs::write(repo.path().join(id.source()), "echo ok\n").unwrap();
//!
//! let run = Execution {
//!     test: id.as_string(),
//!     runner: Runner::Suite,
//!     source: id.source(),
//!     outcome: Outcome::parse("ok"),
//!     seconds: 3,
//!     commit: "0".repeat(40),
//!     working_tree: "clean".into(),
//!     digest: digest_of(b"echo ok\n"),
//!     at: "2026-09-11T00:00:00Z".into(),
//!     origin: Origin::Local,
//!     command: id.reproduce(),
//! };
//! assert!(run.outcome.proves());
//!
//! // the ledger is a tracked file, so the evidence survives the process that produced it
//! let mut ledger = Ledger::empty();
//! ledger.merge([run]);
//! ledger.save(repo.path()).unwrap();
//!
//! let recorded = Ledger::load(repo.path()).unwrap();
//! let latest = recorded.latest(&id.as_string()).unwrap();
//! assert_eq!(latest.digest_matches(repo.path()), Some(true));
//!
//! // edit the case afterwards and the recorded run is no longer a run of the test that is
//! // there now — which no diff against a commit would have shown, had the edit been reverted
//! std::fs::write(repo.path().join(id.source()), "echo something else\n").unwrap();
//! assert_eq!(latest.digest_matches(repo.path()), Some(false));
//! ```
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

//! # Example
//!
//! The vocabulary, without a repository: a test is named by the path a claim writes down,
//! a result word is a pass only when the runner said so, and the proof states rank.
//!
//! ```
//! use majordomus_cli::evidence::{Outcome, ProofState, Runner, TestId};
//!
//! let t = TestId::of("test/cases/07_scope.sh").unwrap();
//! assert_eq!(t.runner, Runner::Suite);
//! assert_eq!(t.as_string(), "suite:07_scope");
//! assert!(Outcome::parse("ok").proves());
//! assert!(!Outcome::parse("FAIL").proves());
//! assert!(ProofState::Proven < ProofState::Stale);
//! ```

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
///
/// ```
/// use majordomus_cli::evidence::{Runner, TestId};
///
/// // which runner owns a test is read off where the test lives, never declared
/// assert_eq!(TestId::of("test/cases/07_scope.sh").unwrap().runner, Runner::Suite);
/// assert_eq!(
///     TestId::of("apps/majordomus-cli/tests/why.rs").unwrap().runner,
///     Runner::Crate
/// );
/// // and a path under neither directory belongs to no runner at all
/// assert!(TestId::of("test/lib.sh").is_none());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceRunner")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::Runner;
/// assert_eq!(Runner::Suite.prefix(), "suite");
/// assert_eq!(Runner::Crate.prefix(), "crate");
/// ```
pub enum Runner {
    /// A behavioural case under `test/cases/`, run by `test/run.sh`.
    Suite,
    /// An integration test binary under `apps/majordomus-cli/tests/`, run by `cargo test`.
    Crate,
}

impl Runner {
    /// The prefix this runner's test ids carry, and the half of [`TestId::as_string`] that
    /// says which runner has to be asked for the proof again.
    ///
    /// The two are distinct words on purpose: a case and a test binary may share a name,
    /// and a ledger keyed on the bare name would let one record overwrite the other's.
    ///
    /// ```
    /// use majordomus_cli::evidence::Runner;
    /// assert_eq!(Runner::Suite.prefix(), "suite");
    /// assert_eq!(Runner::Crate.prefix(), "crate");
    /// assert_ne!(Runner::Suite.prefix(), Runner::Crate.prefix());
    /// ```
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
///
/// ```
/// use majordomus_cli::evidence::Outcome;
///
/// assert!(Outcome::parse("ok").proves());
/// assert!(!Outcome::parse("timeout").proves());
/// // exactly one of the five is evidence of anything; the other four are reasons there
/// // is none, kept apart because "it declined" and "the harness broke" are different bugs
/// assert_eq!(Outcome::parse("skipped"), Outcome::Skip);
/// assert_eq!(Outcome::parse("nonsense"), Outcome::Error);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceOutcome")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::Outcome;
/// assert!(Outcome::parse("ok").proves());
/// assert_eq!(Outcome::parse("wat"), Outcome::Error);
/// ```
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
    /// Did this run prove the behaviour it was about? Only a pass does.
    ///
    /// A skip is a test that declined, and a timeout or an error is a test whose verdict
    /// nobody has; none of the three is a weaker kind of success, and every derivation in
    /// this module asks this question rather than matching on the variants itself.
    ///
    /// ```
    /// use majordomus_cli::evidence::Outcome;
    /// assert!(Outcome::Pass.proves());
    /// assert!(!Outcome::Skip.proves(), "a test that declined to run proved nothing");
    /// assert!(!Outcome::Timeout.proves());
    /// assert!(!Outcome::Error.proves());
    /// ```
    pub fn proves(self) -> bool {
        matches!(self, Outcome::Pass)
    }

    /// The word the runner writes, read back. Anything unrecognised is [`Outcome::Error`]:
    /// a result nobody can classify is not a pass.
    ///
    /// Case and surrounding space are the runner's business, not the ledger's, so both are
    /// normalised away. The fallback is the whole point: a truncated line, a renamed status
    /// word or a new runner's vocabulary fails loudly instead of widening into a green.
    ///
    /// ```
    /// use majordomus_cli::evidence::Outcome;
    /// assert_eq!(Outcome::parse("PASSED"), Outcome::Pass);
    /// assert_eq!(Outcome::parse("  skip  "), Outcome::Skip);
    /// // near-misses included: nothing here guesses at what was meant
    /// assert_eq!(Outcome::parse("okay"), Outcome::Error);
    /// assert!(!Outcome::parse("").proves());
    /// ```
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
///
/// ```
/// use majordomus_cli::evidence::Origin;
///
/// assert_eq!(Origin::parse("CI"), Some(Origin::Ci));
/// assert_eq!(Origin::parse("local"), Some(Origin::Local));
/// // naming the provider is the adapter's business; the ledger records the kind of run,
/// // so a repository that changes CI does not change what its old evidence says
/// assert!(Origin::parse("github-actions").is_none());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceOrigin")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::Origin;
/// assert_eq!(Origin::parse("ci"), Some(Origin::Ci));
/// assert_eq!(Origin::parse("nowhere"), None);
/// ```
pub enum Origin {
    /// Someone ran it on their own machine.
    Local,
    /// A continuous-integration run.
    Ci,
    /// A release run.
    Release,
}

impl Origin {
    /// Read an origin from the word a recorder was given, or `None` when it names none of
    /// the three.
    ///
    /// The asymmetry with [`Outcome::parse`] is deliberate. An unreadable outcome still has
    /// to be recorded as something, and the safe something is an error; an unreadable origin
    /// is a caller passing a word this model does not have, which is worth refusing at the
    /// boundary rather than recording as a local run nobody made.
    ///
    /// ```
    /// use majordomus_cli::evidence::Origin;
    /// assert_eq!(Origin::parse(" Release "), Some(Origin::Release));
    /// assert!(Origin::parse("laptop").is_none());
    /// ```
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
///
/// ```
/// use majordomus_cli::evidence::{Runner, TestId};
///
/// let id = TestId::of("test/cases/07_scope.sh").unwrap();
/// assert_eq!(id, TestId { runner: Runner::Suite, name: "07_scope".into() });
///
/// // the identity and the path are two spellings of one thing, so the derivation is
/// // reversible and no claim in `docs/CLAIMS.yaml` has to be rewritten to carry an id
/// assert_eq!(TestId::of(&id.source()).unwrap(), id);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceTestId")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::TestId;
/// let t = TestId::of("apps/majordomus-cli/tests/product.rs").unwrap();
/// assert_eq!(t.as_string(), "crate:product");
/// assert_eq!(t.reproduce(), "cargo test --test product");
/// assert!(TestId::of("lib/nothing.sh").is_none());
/// ```
pub struct TestId {
    /// Which runner owns it.
    pub runner: Runner,
    /// The name that runner knows it by: a case's file stem, a test binary's file stem.
    pub name: String,
}

impl TestId {
    /// The canonical string form: `suite:84_distribution_model`, `crate:why`.
    ///
    /// This is the ledger's key. Every execution is stored and looked up under it, which is
    /// why the runner is part of it rather than context a reader is expected to carry.
    ///
    /// ```
    /// use majordomus_cli::evidence::TestId;
    /// assert_eq!(
    ///     TestId::of("test/cases/07_scope.sh").unwrap().as_string(),
    ///     "suite:07_scope"
    /// );
    /// assert_eq!(
    ///     TestId::of("apps/majordomus-cli/tests/why.rs").unwrap().as_string(),
    ///     "crate:why"
    /// );
    /// ```
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
            return TestId::named(Runner::Suite, rest.strip_suffix(".sh")?);
        }
        if let Some(rest) = path.strip_prefix("apps/majordomus-cli/tests/") {
            return TestId::named(Runner::Crate, rest.strip_suffix(".rs")?);
        }
        None
    }

    /// A test named directly, validated by the same rule [`TestId::of`] applies to a path.
    ///
    /// The identity form (`suite:84_x`) and the path form (`test/cases/84_x.sh`) name the
    /// same thing, so they must agree about what a name may be. They did not: the path
    /// form rejected a name containing a separator and the identity form did not, so
    /// `suite:../../x` produced the source `test/cases/../../x.sh`. Two parsers for one
    /// grammar is the one-way check this repository keeps finding; this is the grammar,
    /// and both forms go through it.
    ///
    /// ```
    /// use majordomus_cli::evidence::{Runner, TestId};
    /// assert_eq!(TestId::named(Runner::Suite, "84_x").unwrap().as_string(), "suite:84_x");
    /// assert!(TestId::named(Runner::Suite, "../../x").is_none(), "a name is not a path");
    /// assert!(TestId::named(Runner::Suite, "").is_none());
    /// assert!(TestId::named(Runner::Crate, "a/b").is_none());
    /// assert!(TestId::named(Runner::Suite, ".hidden").is_none());
    /// ```
    pub fn named(runner: Runner, name: &str) -> Option<TestId> {
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return None;
        }
        Some(TestId {
            runner,
            name: name.to_string(),
        })
    }

    /// The repository-relative path of the test's own source: the file that is hashed into
    /// [`Execution::digest`], and one of the paths a claim's staleness is measured over.
    ///
    /// ```
    /// use majordomus_cli::evidence::TestId;
    /// let id = TestId::of("apps/majordomus-cli/tests/why.rs").unwrap();
    /// assert_eq!(id.source(), "apps/majordomus-cli/tests/why.rs");
    /// // a case's source is the file the runner executes, not the runner
    /// assert_eq!(
    ///     TestId::of("test/cases/07_scope.sh").unwrap().source(),
    ///     "test/cases/07_scope.sh"
    /// );
    /// ```
    pub fn source(&self) -> String {
        match self.runner {
            Runner::Suite => format!("test/cases/{}.sh", self.name),
            Runner::Crate => format!("apps/majordomus-cli/tests/{}.rs", self.name),
        }
    }

    /// The exact command that runs this one test, for a reader who wants the proof again
    /// rather than the claim that it exists.
    ///
    /// One test, not the suite: a finding a reader can settle in seconds is settled, and
    /// one that costs a full run is argued about instead.
    ///
    /// ```
    /// use majordomus_cli::evidence::TestId;
    /// assert_eq!(
    ///     TestId::of("test/cases/84_distribution_model.sh").unwrap().reproduce(),
    ///     "bash test/run.sh 84_distribution_model"
    /// );
    /// assert_eq!(
    ///     TestId::of("apps/majordomus-cli/tests/why.rs").unwrap().reproduce(),
    ///     "cargo test --test why"
    /// );
    /// ```
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
///
/// ```
/// use majordomus_cli::evidence::{digest_of, Execution, Origin, Outcome, Runner, TestId};
///
/// let id = TestId::of("apps/majordomus-cli/tests/why.rs").unwrap();
/// let run = Execution {
///     test: id.as_string(),
///     runner: Runner::Crate,
///     source: id.source(),
///     outcome: Outcome::Pass,
///     seconds: 12,
///     commit: "0".repeat(40),
///     working_tree: "clean".into(),
///     digest: digest_of(b"fn main() {}"),
///     at: "2026-09-11T00:00:00Z".into(),
///     origin: Origin::Ci,
///     command: id.reproduce(),
/// };
///
/// // it is a value in a tracked JSON file, so it has to survive the file unchanged
/// let back: Execution = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
/// assert_eq!(back, run);
/// assert_eq!(back.test, "crate:why");
/// assert_eq!(back.command, "cargo test --test why");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceExecution")]
/// # Example
///
/// One recorded run, with the provenance the runner did not carry: which commit, which
/// tree state, the digest of the test's own source, when, and how to run it again.
///
/// ```
/// use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
/// let e = Execution {
///     test: "suite:07_scope".into(),
///     runner: Runner::Suite,
///     source: "test/cases/07_scope.sh".into(),
///     outcome: Outcome::Pass,
///     seconds: 3,
///     commit: "a04b65c9".into(),
///     working_tree: "clean".into(),
///     digest: "sha256:00".into(),
///     at: "2026-09-11T00:00:00Z".into(),
///     origin: Origin::Local,
///     command: "bash test/run.sh 07_scope".into(),
/// };
/// assert!(e.outcome.proves());
/// ```
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
    /// `sha256:<hex>` of the test's own source as it was when the run was recorded. An
    /// execution whose test no longer hashes to this did not run the test that is there now.
    pub digest: String,
    /// When it was recorded, RFC 3339, UTC.
    pub at: String,
    /// Where the run happened.
    pub origin: Origin,
    /// The exact command that runs this one test again.
    pub command: String,
}

/// A ledger holds one execution per test, so the test identifies it; the label and the
/// identity are the same string because there is nothing else to read it by.
impl crate::order::Ordered for Execution {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.test, &self.test)
    }
}

impl Execution {
    /// Does the test's source still hash to what was recorded?
    ///
    /// `None` when the source is not there to hash — which is itself a finding, reported by
    /// the path checks, not silently read as a match.
    ///
    /// This is the staleness check that survives a revert: an edit made and undone around a
    /// run leaves no diff against any commit, and the recorded digest still says the test
    /// that ran is not the test that is there.
    ///
    /// ```
    /// use majordomus_cli::evidence::{digest_of, Execution, Origin, Outcome, Runner, TestId};
    ///
    /// let repo = tempfile::tempdir().unwrap();
    /// let id = TestId::of("test/cases/07_scope.sh").unwrap();
    /// std::fs::create_dir_all(repo.path().join("test/cases")).unwrap();
    /// std::fs::write(repo.path().join(id.source()), "echo ok\n").unwrap();
    ///
    /// let run = Execution {
    ///     test: id.as_string(),
    ///     runner: Runner::Suite,
    ///     source: id.source(),
    ///     outcome: Outcome::Pass,
    ///     seconds: 1,
    ///     commit: "0".repeat(40),
    ///     working_tree: "clean".into(),
    ///     digest: digest_of(b"echo ok\n"),
    ///     at: "2026-09-11T00:00:00Z".into(),
    ///     origin: Origin::Local,
    ///     command: id.reproduce(),
    /// };
    /// assert_eq!(run.digest_matches(repo.path()), Some(true));
    ///
    /// std::fs::write(repo.path().join(id.source()), "echo something else\n").unwrap();
    /// assert_eq!(run.digest_matches(repo.path()), Some(false));
    ///
    /// // and a source that is gone cannot be compared, which is not the same as agreeing
    /// std::fs::remove_file(repo.path().join(id.source())).unwrap();
    /// assert_eq!(run.digest_matches(repo.path()), None);
    /// ```
    pub fn digest_matches(&self, root: &Path) -> Option<bool> {
        let text = std::fs::read(root.join(&self.source)).ok()?;
        Some(digest_of(&text) == self.digest)
    }
}

/// `sha256:<hex>` of some bytes, the one spelling used in the ledger.
///
/// The algorithm is named in the value rather than assumed by the reader, so a ledger
/// written before a change of algorithm stays readable and says which one produced it.
///
/// ```
/// use majordomus_cli::evidence::digest_of;
///
/// let d = digest_of(b"echo ok\n");
/// assert!(d.starts_with("sha256:"), "{d}");
/// assert_eq!(d.len(), "sha256:".len() + 64);
/// assert_eq!(d, digest_of(b"echo ok\n"), "the same bytes hash the same");
/// assert_ne!(d, digest_of(b"echo ok"), "a trailing newline is a different file");
/// ```
pub fn digest_of(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("sha256:{:x}", h.finalize())
}

/// What the repository can say about one claim's proof, right now.
///
/// Ordered from strongest to weakest, so a summary that sorts by this reads as a ranking.
///
/// ```
/// use majordomus_cli::evidence::ProofState;
///
/// let mut states = vec![ProofState::NotRun, ProofState::Stale, ProofState::Proven];
/// states.sort();
/// assert_eq!(
///     states,
///     vec![ProofState::Proven, ProofState::Stale, ProofState::NotRun]
/// );
///
/// // the two strongest are deliberately not one state: `inputs unchanged` is the absence
/// // of a known invalidation, and collapsing it into `proven` is the unaccountable badge
/// assert_ne!(ProofState::Proven, ProofState::InputsUnchanged);
/// assert!(ProofState::Proven < ProofState::InputsUnchanged);
/// assert!(ProofState::InputsUnchanged.passing());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::ProofState;
/// assert!(ProofState::Proven.passing());
/// assert!(!ProofState::NotRun.passing());
/// assert!(!ProofState::InputsUnchanged.meaning().is_empty());
/// ```
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
    /// The word every surface prints for this state, and the key the report's totals are
    /// counted under.
    ///
    /// One spelling, defined here, so that the badge, the terminal rendering and the JSON
    /// cannot disagree about what they are showing the same reader.
    ///
    /// ```
    /// use majordomus_cli::evidence::ProofState;
    /// assert_eq!(ProofState::Proven.label(), "proven");
    /// assert_eq!(ProofState::InputsUnchanged.label(), "inputs unchanged");
    /// assert_ne!(ProofState::Proven.label(), ProofState::InputsUnchanged.label());
    /// ```
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
    ///
    /// The sentence for [`ProofState::InputsUnchanged`] is the one that has to be carried
    /// everywhere, because it is the state a surface is tempted to render as a plain green.
    ///
    /// ```
    /// use majordomus_cli::evidence::ProofState;
    ///
    /// let m = ProofState::InputsUnchanged.meaning();
    /// assert!(m.contains("not proof against the current commit"), "{m}");
    /// assert_ne!(m, ProofState::Proven.meaning());
    /// ```
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
            ProofState::Failing => {
                "The most recent recorded run of this claim's test did not pass."
            }
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
    ///
    /// Freshness and outcome are two questions, and this answers only the second. A count
    /// of passing claims is honest; a count that called them all proven would not be.
    ///
    /// ```
    /// use majordomus_cli::evidence::ProofState;
    /// assert!(ProofState::Proven.passing());
    /// assert!(ProofState::Stale.passing(), "a pass older than its subject is still a pass");
    /// assert!(!ProofState::Failing.passing());
    /// assert!(!ProofState::NotRun.passing());
    /// assert!(!ProofState::Unrunnable.passing());
    /// ```
    pub fn passing(self) -> bool {
        matches!(
            self,
            ProofState::Proven | ProofState::InputsUnchanged | ProofState::Stale
        )
    }
}

/// One claim, joined to whatever the repository actually recorded about it.
///
/// The claim's own fields are carried verbatim beside the derived ones, so that a reader
/// who disbelieves the state can check the join rather than take it.
///
/// ```
/// use majordomus_cli::evidence::{ClaimProof, ProofState};
///
/// let proof = ClaimProof {
///     id: "scope-integrity".into(),
///     claim: "a commit outside the task's scope is refused".into(),
///     status: "guaranteed".into(),
///     source: Some("docs/SCOPE.md".into()),
///     implementation: Some("apps/majordomus-cli/src/scope.rs".into()),
///     test_path: Some("test/cases/07_scope.sh".into()),
///     test: Some("suite:07_scope".into()),
///     state: ProofState::NotRun,
///     meaning: ProofState::NotRun.meaning().to_string(),
///     execution: None,
///     changed: vec![],
///     reproduce: Some("bash test/run.sh 07_scope".into()),
/// };
///
/// let json = serde_json::to_value(&proof).unwrap();
/// // `changed` is emitted empty rather than omitted: a client must not have to tell
/// // "no file changed" from "the server did not say"
/// assert_eq!(json["changed"], serde_json::json!([]));
/// // an execution that does not exist is absent, not a null standing in for one
/// assert!(json.get("execution").is_none());
/// assert_eq!(json["meaning"], serde_json::json!(ProofState::NotRun.meaning()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceClaimProof")]
/// # Example
///
/// A `ClaimProof` is produced by [`report`]; its `state` is the whole answer and its
/// `meaning` is that answer in a sentence, worded once for every surface.
///
/// ```
/// use majordomus_cli::evidence::{report, ClaimProof, Ledger};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ledger = Ledger::load(repo.root()).unwrap();
/// let r = report(&repo.index().unwrap(), &ledger);
/// let proofs: &[ClaimProof] = &r.claims;
/// assert!(proofs.is_empty(), "a synthetic tree declares no claims");
/// ```
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
    ///
    /// Always emitted, empty included: the schema this capability publishes says the key is
    /// there, and a client that has to tell "no files changed" from "the server did not say"
    /// is a client reading two different answers as one.
    pub changed: Vec<String>,
    /// The command that produces the proof again, when a runner owns the test.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
}

/// A claim whose declared status the evidence does not support.
///
/// Only a `guaranteed` claim can produce one: the other statuses already say that a current
/// proof is not what they are claiming. A finding says what was claimed, what the evidence
/// supports instead, and — where running something would settle it — what to run.
///
/// ```
/// use majordomus_cli::evidence::{Finding, ProofState};
///
/// let finding = Finding {
///     claim: "scope-integrity".into(),
///     status: "guaranteed".into(),
///     state: ProofState::NotRun,
///     reason: "the claim guarantees a behaviour and names a test, and no run of that \
///              test has ever been recorded"
///         .into(),
///     reproduce: Some("bash test/run.sh 07_scope".into()),
/// };
///
/// // the gap is the finding: what the matrix declares against what the ledger holds
/// assert_ne!(finding.status, finding.state.label());
/// assert!(finding.reproduce.unwrap().starts_with("bash test/run.sh"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceFinding")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::{Finding, ProofState};
/// let f = Finding {
///     claim: "some-claim".into(),
///     status: "guaranteed".into(),
///     state: ProofState::NotRun,
///     reason: "the claim names a test nobody has run".into(),
///     reproduce: Some("bash test/run.sh 07_scope".into()),
/// };
/// assert_eq!(f.state, ProofState::NotRun);
/// ```
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
///
/// A report that did not say where its evidence came from would be asking to be believed.
/// This is the provenance of the whole report: the file, how much it holds, how old the
/// newest entry is, and which commits anything was ever recorded against.
///
/// ```
/// use majordomus_cli::evidence::{Ledger, LedgerSummary, LEDGER_PATH};
///
/// let summary: LedgerSummary = Ledger::empty().summary();
/// assert_eq!(summary.path, LEDGER_PATH);
/// assert!(!summary.present, "a ledger holding nothing has recorded nothing");
/// assert_eq!(summary.executions, 0);
/// assert!(summary.newest.is_none());
/// assert!(summary.commits.is_empty());
/// ```

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceLedgerSummary")]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::LedgerSummary;
/// let s = LedgerSummary {
///     path: ".ai/repo/evidence/ledger.json".into(),
///     present: false,
///     executions: 0,
///     newest: None,
///     commits: vec![],
/// };
/// assert!(!s.present, "a repository that recorded nothing says so");
/// ```
pub struct LedgerSummary {
    /// Where it lives, repository-relative.
    pub path: String,
    /// Whether the ledger holds at least one execution.
    ///
    /// Not whether the file exists. A ledger file holding nothing and no ledger file at
    /// all are the same answer to the only question a reader is asking — has anything
    /// been recorded — and both make every claim read `not_run`. [`Ledger::present`]
    /// answers the filesystem question, and nothing derives a proof state from it.
    pub present: bool,
    /// How many executions it holds.
    pub executions: usize,
    /// The newest `at` in it, when it holds any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest: Option<String>,
    /// The distinct commits its executions were recorded against. Always emitted, empty
    /// included, for the reason [`ClaimProof::changed`] is.
    pub commits: Vec<String>,
}

/// Whether the report could see its whole subject.
///
/// A verdict over a matrix the index silently shrank is the failure this repository met
/// four times in one day: an object is excluded, every projection of the smaller index is a
/// faithful projection, and each check prints a true statement about the half it can see.
/// A freshness check answers "was this generated from this tree" and cannot answer "did the
/// generator see everything in it", so the answer has to carry its own denominator.
///
/// ```
/// use majordomus_cli::evidence::Subject;
/// let whole = Subject { examined: 149, complete: true, excluded: vec![] };
/// assert!(whole.complete);
///
/// let partial = Subject {
///     examined: 148,
///     complete: false,
///     excluded: vec!["docs/CLAIMS.yaml: unknown_key".into()],
/// };
/// assert!(!partial.complete, "a shrunken index must not report a clean verdict");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EvidenceSubject")]
pub struct Subject {
    /// How many claims the index held, and this report therefore examined.
    pub examined: usize,
    /// Whether the index that produced them carried no error diagnostic. When this is
    /// false, every count and every absence below is about a smaller world than the
    /// repository, and no verdict over it means what it says.
    pub complete: bool,
    /// The diagnostics that excluded something, when any did: `<path>: <code>`.
    pub excluded: Vec<String>,
}

/// The whole joined picture: every claim of the matrix against every execution recorded.
///
/// What the capability answers, what the gate judges and what the site renders are this one
/// value; the totals and the findings are derived from the claims, never counted twice.
///
/// ```
/// use majordomus_cli::evidence::{EvidenceReport, Finding, Ledger, ProofState, Subject};
///
/// let mut report = EvidenceReport {
///     head: None,
///     working_tree: "unknown".into(),
///     ledger: Ledger::empty().summary(),
///     subject: Subject { examined: 0, complete: true, excluded: vec![] },
///     claims: vec![],
///     totals: Default::default(),
///     findings: vec![],
/// };
/// assert!(report.satisfied(), "nothing claimed, so nothing unsupported");
///
/// // and the same report over a subject it could not wholly read is not a pass
/// let mut partial = report.clone();
/// partial.subject.complete = false;
/// assert!(!partial.satisfied(), "an absent answer is not a clean one");
///
/// report.findings.push(Finding {
///     claim: "scope-integrity".into(),
///     status: "guaranteed".into(),
///     state: ProofState::Failing,
///     reason: "the claim guarantees a behaviour whose test most recently failed".into(),
///     reproduce: None,
/// });
/// // one unsupported guarantee is enough: the gate is not a proportion
/// assert!(!report.satisfied());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// # Example
///
/// The whole matrix joined to the ledger. `satisfied` is true when no claim declares a
/// guarantee the recorded evidence does not support.
///
/// ```
/// use majordomus_cli::evidence::{report, EvidenceReport, Ledger};
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let ledger = Ledger::load(repo.root()).unwrap();
/// let r: EvidenceReport = report(&repo.index().unwrap(), &ledger);
/// assert!(r.satisfied(), "no claim, so no unsupported guarantee");
/// ```
pub struct EvidenceReport {
    /// The commit the report was derived against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `clean`, `dirty` or `unknown`.
    pub working_tree: String,
    /// The ledger this was joined against.
    pub ledger: LedgerSummary,
    /// What the report could reach. Read this before reading the tallies: they are counts
    /// over the claims the index held, and an index that dropped a file holds fewer.
    pub subject: Subject,
    /// Every claim, in the matrix's own order.
    pub claims: Vec<ClaimProof>,
    /// How many claims are in each state.
    pub totals: BTreeMap<String, usize>,
    /// Claims whose declared status the evidence does not support.
    pub findings: Vec<Finding>,
}

impl EvidenceReport {
    /// Does the evidence support every claim that declares a guarantee?
    ///
    /// False when the index was incomplete, whatever the findings say. An empty finding
    /// list over a matrix that lost entries is not a pass — it is the absence of an
    /// answer, and the two must never be spelled the same way.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{EvidenceReport, LedgerSummary, Subject};
    /// # use std::collections::BTreeMap;
    /// let mut r = EvidenceReport {
    ///     head: None,
    ///     working_tree: "clean".into(),
    ///     ledger: LedgerSummary {
    ///         path: ".ai/repo/evidence/ledger.json".into(),
    ///         present: false,
    ///         executions: 0,
    ///         newest: None,
    ///         commits: vec![],
    ///     },
    ///     subject: Subject { examined: 0, complete: true, excluded: vec![] },
    ///     claims: vec![],
    ///     totals: BTreeMap::new(),
    ///     findings: vec![],
    /// };
    /// assert!(r.satisfied(), "no findings over a whole subject is a pass");
    ///
    /// r.subject.complete = false;
    /// assert!(!r.satisfied(), "no findings over a partial subject is not a pass");
    /// ```
    pub fn satisfied(&self) -> bool {
        self.subject.complete && self.findings.is_empty()
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
    let git = |args: &[&str]| -> Option<Vec<String>> {
        let out = crate::git::read_only(root).args(args).output().ok()?;
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
    };
    // Tracked differences, and then the files that are there but git is not tracking.
    // `git diff` never lists an untracked file, so a tree whose only difference from the
    // recorded commit is a new source file would compare equal — and `proven` would say
    // "the tree in front of you is the tree the run measured" about a tree carrying code
    // the run never saw. Ignored files stay ignored: `--exclude-standard` is what makes
    // this the set a person would call "new here" rather than every build artifact.
    let mut changed: BTreeSet<String> = git(&["diff", "--name-only", commit, "--"])?
        .into_iter()
        .collect();
    changed.extend(git(&["ls-files", "--others", "--exclude-standard"])?);
    Some(changed)
}

/// Join the claims of the index with the executions of the ledger and decide, per claim,
/// what the repository can honestly say.
///
/// The git comparison is done once per distinct recorded commit and shared by every claim
/// that recorded against it.
///
/// Claims come from the index and nothing else; executions come from the ledger and nothing
/// else. A claim the ledger has never heard of is [`ProofState::NotRun`], and if it declares
/// a guarantee, that is a finding — the file existing was never the proof.
///
/// ```
/// use majordomus_cli::evidence::{self, Ledger, ProofState};
/// use majordomus_cli::git::GitState;
/// use majordomus_cli::index::{Index, RepositoryInfo, State};
/// use majordomus_cli::{Object, Provenance};
///
/// let repo = tempfile::tempdir().unwrap();
/// let claim = Object {
///     kind: "claim".into(),
///     identity: "scope-integrity".into(),
///     uri: "majordomus://claim/scope-integrity".into(),
///     title: None,
///     description: None,
///     metadata: serde_json::json!({
///         "claim": "a commit outside the task's scope is refused",
///         "status": "guaranteed",
///         "source": "docs/SCOPE.md",
///         "test": "test/cases/07_scope.sh",
///     }),
///     body: String::new(),
///     content: String::new(),
///     media_type: "application/yaml",
///     provenance: Provenance {
///         path: "docs/CLAIMS.yaml".into(),
///         directory: "docs".into(),
///         source_class: "claim".into(),
///         section: None,
///         bytes: 0,
///         member: Some("claims.0".into()),
///     },
/// };
/// let index = Index {
///     repository: RepositoryInfo {
///         root: repo.path().display().to_string(),
///         layer_schema: "ai-repository/v1".into(),
///         sections: Default::default(),
///         git: GitState::Unavailable { reason: "doc".into() },
///         discovery: "filesystem".into(),
///         source_classes: vec![],
///         kind_sources: vec![],
///         scope_origin: majordomus_cli::scope::Origin::Distribution,
///         scope_path: String::new(),
///     },
///     objects: vec![claim],
///     diagnostics: vec![],
///     state: State::Ok,
///     fingerprint: String::new(),
///     scoped: Default::default(),
///     distribution: None,
///     providers: Default::default(),
///     share: None,
/// };
///
/// let report = evidence::report(&index, &Ledger::empty());
/// let proof = &report.claims[0];
/// assert_eq!(proof.test.as_deref(), Some("suite:07_scope"), "the id came from the path");
/// assert_eq!(proof.state, ProofState::NotRun);
/// assert_eq!(report.totals.get("not run"), Some(&1));
///
/// // a guarantee with nothing recorded behind it is a finding, and the reader is told
/// // exactly what would settle it
/// assert!(!report.satisfied());
/// assert_eq!(report.findings[0].claim, "scope-integrity");
/// assert_eq!(
///     report.findings[0].reproduce.as_deref(),
///     Some("bash test/run.sh 07_scope")
/// );
/// ```
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
                    let inputs: Vec<String> =
                        [c.source.clone(), c.implementation.clone(), Some(t.source())]
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

    // What the index could not read is what this report cannot be a verdict over. An
    // excluded file takes its objects out of the index, and every count below is then a
    // count over what survived.
    let excluded: Vec<String> = index
        .diagnostics
        .iter()
        .filter(|d| d.severity == crate::Severity::Error)
        .map(|d| match &d.path {
            Some(p) => format!("{p}: {}", d.code),
            None => d.code.clone(),
        })
        .collect();
    let subject = Subject {
        examined: claims.len(),
        complete: excluded.is_empty(),
        excluded,
    };

    EvidenceReport {
        head,
        working_tree,
        ledger: ledger.summary(),
        subject,
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

    /// A verdict over a subject the reader could not wholly see is not a verdict. This is
    /// the property the gate and `--check` both depend on: an empty finding list over an
    /// index that dropped a file must not spell the same as a pass.
    #[test]
    fn an_incomplete_subject_is_never_satisfied() {
        let base = EvidenceReport {
            head: None,
            working_tree: "clean".into(),
            ledger: LedgerSummary {
                path: LEDGER_PATH.into(),
                present: false,
                executions: 0,
                newest: None,
                commits: vec![],
            },
            subject: Subject {
                examined: 3,
                complete: true,
                excluded: vec![],
            },
            claims: vec![],
            totals: BTreeMap::new(),
            findings: vec![],
        };
        assert!(
            base.satisfied(),
            "no findings over a whole subject is a pass"
        );

        let mut partial = base.clone();
        partial.subject.complete = false;
        partial.subject.excluded = vec!["docs/CLAIMS.yaml: unknown_key".into()];
        assert!(
            !partial.satisfied(),
            "an empty finding list over a shrunken index read as a pass"
        );

        // and a whole subject with a finding is still not satisfied, for the ordinary reason
        let mut failing = base;
        failing.findings.push(Finding {
            claim: "x".into(),
            status: "guaranteed".into(),
            state: ProofState::NotRun,
            reason: "never run".into(),
            reproduce: None,
        });
        assert!(!failing.satisfied());
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
