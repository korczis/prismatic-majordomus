//! Freshness: whether a recorded run still proves anything at the revision a reader is shown.
//!
//! A passing run is a fact about the tree it measured. Whether it is also a fact about the
//! tree in front of a reader depends on three things the run cannot know: whether the
//! revision presented contains the commit the run was recorded on, what changed between the
//! two, and whether either tree was the commit it claims to be. This module decides that,
//! once, for every surface — the evidence report, the rules report, and every later reader
//! of the ledger — so that no two of them can disagree about what a run still proves.
//!
//! # The pieces
//!
//! * [`Presented`] — what is judged: the reader's working tree, or the checked-out commit
//!   judged as committed ([`presented_commit`]), with the ledger committed in it
//!   ([`ledger_at`]);
//! * [`compare`] — the containment of the evidence commit in the presented revision, and
//!   the paths that changed between them ([`changed_between`]);
//! * [`freshness`] — the one truth table, pure, from a recorded run to a [`Judgement`];
//! * [`weakened_by`] — the monotone rule: a supplementary record may withhold `proven` and
//!   never grant it; its first source is [`uncommitted`], the executions the working ledger
//!   holds that the presented commit's ledger does not;
//! * [`aggregate`] — how several states make one, with a failure outranking an absence.
//!
//! # Example
//!
//! The truth table without a repository: a recorded skip is not a failure, and only a pass
//! the presented revision contains, with nothing but the ledger changed since, on a clean
//! tree at both ends, is `proven`.
//!
//! ```
//! use majordomus_cli::evidence::freshness::{freshness, Comparison, Recorded, TreeState};
//! use majordomus_cli::evidence::{Execution, ProofState};
//! use majordomus_cli::git::Containment;
//!
//! let run = |outcome: &str| -> Execution {
//!     serde_json::from_value(serde_json::json!({
//!         "test": "suite:07_scope", "runner": "suite", "source": "test/cases/07_scope.sh",
//!         "outcome": outcome, "seconds": 1, "commit": "a".repeat(40),
//!         "working_tree": "clean", "digest": "sha256:0", "at": "2026-09-26T00:00:00Z",
//!         "origin": "local", "command": "bash test/run.sh 07_scope"
//!     }))
//!     .unwrap()
//! };
//! let nothing_changed = Comparison {
//!     containment: Containment::Contains,
//!     changed: Some(Default::default()),
//! };
//! let inputs = vec!["test/cases/07_scope.sh".to_string()];
//! let judge = |e: &Execution| {
//!     freshness(
//!         Recorded::Ran(e),
//!         Some(&inputs),
//!         "test/cases/07_scope.sh",
//!         false,
//!         Some(&nothing_changed),
//!         TreeState::Clean,
//!     )
//! };
//! assert_eq!(judge(&run("pass")).state, ProofState::Proven);
//! assert_eq!(judge(&run("skip")).state, ProofState::NotRun);
//! assert_eq!(judge(&run("error")).state, ProofState::Failing);
//! ```

use std::collections::BTreeSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{capped_by_working_tree, Execution, Ledger, Outcome, ProofState, LEDGER_PATH};
use crate::error::{Error, Result};
use crate::git::{self, Containment};

/// The phrase a detail uses for a run the working ledger holds and the presented commit's
/// ledger does not: the first source of [`weakened_by`].
pub const UNCOMMITTED_RUN: &str = "an uncommitted run in the working ledger";

// ---------------------------------------------------------------- the tree

/// Whether a tree was the commit it sits on: the three words the ledger and git already
/// use, typed.
///
/// `unknown` is not a softer `clean`. It is what is recorded when git could not be asked,
/// and not knowing is not proof, so every rule here treats it as not clean.
///
/// ```
/// use majordomus_cli::evidence::freshness::TreeState;
///
/// assert_eq!(TreeState::parse("clean"), TreeState::Clean);
/// assert_eq!(TreeState::parse("dirty"), TreeState::Dirty);
/// // a word nobody wrote down is not a clean tree
/// assert_eq!(TreeState::parse("spotless"), TreeState::Unknown);
/// assert_eq!(TreeState::Dirty.as_str(), "dirty");
/// assert_eq!(serde_json::to_value(TreeState::Unknown).unwrap(), "unknown");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EvidenceTreeState")]
pub enum TreeState {
    /// The tree was the commit: nothing but the evidence ledger differed from it.
    Clean,
    /// Something other than the evidence ledger differed from the commit.
    Dirty,
    /// Git could not be asked, or the word recorded is none of the three.
    Unknown,
}

impl TreeState {
    /// Read the word a recorder or git wrote: `clean` and `dirty` are themselves, and
    /// anything else is [`TreeState::Unknown`], never a guess at one of the other two.
    ///
    /// ```
    /// use majordomus_cli::evidence::freshness::TreeState;
    /// assert_eq!(TreeState::parse("clean"), TreeState::Clean);
    /// assert_eq!(TreeState::parse("Clean"), TreeState::Unknown, "the words are exact");
    /// assert_eq!(TreeState::parse(""), TreeState::Unknown);
    /// ```
    pub fn parse(word: &str) -> TreeState {
        match word {
            "clean" => TreeState::Clean,
            "dirty" => TreeState::Dirty,
            _ => TreeState::Unknown,
        }
    }

    /// The word itself, as the ledger and the report spell it.
    ///
    /// ```
    /// use majordomus_cli::evidence::freshness::TreeState;
    /// for t in [TreeState::Clean, TreeState::Dirty, TreeState::Unknown] {
    ///     assert_eq!(TreeState::parse(t.as_str()), t, "the word reads back as itself");
    /// }
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            TreeState::Clean => "clean",
            TreeState::Dirty => "dirty",
            TreeState::Unknown => "unknown",
        }
    }

    /// The weaker of two statements about one tree: clean only when both say clean, dirty
    /// when either says dirty, and unknown otherwise.
    ///
    /// This is how a caller's own knowledge meets a measurement. A site build that knows its
    /// tree was dirty can weaken a clean measurement; nothing a caller says can make a
    /// measured dirty tree clean.
    ///
    /// ```
    /// use majordomus_cli::evidence::freshness::TreeState::{Clean, Dirty, Unknown};
    /// assert_eq!(Clean.weaker(Clean), Clean);
    /// assert_eq!(Clean.weaker(Dirty), Dirty);
    /// assert_eq!(Dirty.weaker(Clean), Dirty, "a given clean never overrides a dirty tree");
    /// assert_eq!(Unknown.weaker(Dirty), Dirty);
    /// assert_eq!(Clean.weaker(Unknown), Unknown);
    /// ```
    pub fn weaker(self, other: TreeState) -> TreeState {
        match (self, other) {
            (TreeState::Clean, TreeState::Clean) => TreeState::Clean,
            (TreeState::Dirty, _) | (_, TreeState::Dirty) => TreeState::Dirty,
            _ => TreeState::Unknown,
        }
    }
}

// ---------------------------------------------------------------- what is presented

/// The revision a verdict is about.
///
/// Every verdict is a verdict *at* something, and saying which is half of the answer. The
/// working tree is what a person at a terminal is looking at; the checked-out commit judged
/// as committed is what a site built from that commit shows its readers.
///
/// ```
/// use majordomus_cli::evidence::freshness::Presented;
///
/// // the default of every existing reader: the checkout, with whatever it carries
/// let here = Presented::WorkingTree;
/// assert_eq!(here.commit(), None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presented {
    /// The reader's checkout: HEAD plus every tracked, staged and untracked change in it.
    WorkingTree,
    /// The checked-out commit, judged as committed: its own ledger, its own tree. Only
    /// [`presented_commit`] builds one, because only it checks that the commit is the one
    /// checked out and measures the tree.
    #[non_exhaustive]
    Commit {
        /// The full commit id.
        commit: String,
        /// Whether the checkout was that commit, ignoring the evidence ledger's working
        /// copy, weakened by anything the caller knew.
        tree: TreeState,
    },
}

impl Presented {
    /// The presented commit, when a commit rather than the working tree is presented.
    ///
    /// ```
    /// use majordomus_cli::evidence::freshness::{presented_commit, Presented};
    /// use std::process::Command;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// let git = |args: &[&str]| {
    ///     let out = Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap();
    ///     String::from_utf8_lossy(&out.stdout).trim().to_string()
    /// };
    /// git(&["init", "-q"]);
    /// git(&["config", "user.email", "t@example.com"]);
    /// git(&["config", "user.name", "t"]);
    /// git(&["commit", "-q", "--allow-empty", "-m", "one"]);
    ///
    /// let p = presented_commit(dir.path(), "HEAD", None).unwrap();
    /// assert_eq!(p.commit(), Some(git(&["rev-parse", "HEAD"]).as_str()));
    /// assert_eq!(Presented::WorkingTree.commit(), None);
    /// ```
    pub fn commit(&self) -> Option<&str> {
        match self {
            Presented::WorkingTree => None,
            Presented::Commit { commit, .. } => Some(commit),
        }
    }

    /// The presented tree as the truth table reads it: [`TreeState::Clean`] for the working
    /// tree, whose changes are already in the diff, and the measured tree for a commit.
    ///
    /// ```
    /// use majordomus_cli::evidence::freshness::{Presented, TreeState};
    /// assert_eq!(Presented::WorkingTree.tree(), TreeState::Clean);
    /// ```
    pub fn tree(&self) -> TreeState {
        match self {
            Presented::WorkingTree => TreeState::Clean,
            Presented::Commit { tree, .. } => *tree,
        }
    }
}

/// The first twelve characters of a commit, for a sentence a person reads. The full id is
/// what every payload carries.
fn short12(commit: &str) -> String {
    commit.chars().take(12).collect()
}

/// The full commit a revision names, or `None` when it names none here.
fn commit_of(root: &Path, rev: &str) -> Option<String> {
    let out = git::read_only(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!id.is_empty()).then_some(id)
}

/// The checked-out commit, presented as committed.
///
/// `rev` must name the commit that is checked out: the claims matrix, the tests' sources and
/// the digests a verdict reads come from the checkout, so a verdict "at" some other commit
/// would mix two trees. The tree is measured, ignoring the evidence ledger's working copy —
/// the ledger is read from the commit, so its working copy is not part of what is judged —
/// and `given` can only weaken what was measured.
///
/// ```
/// use majordomus_cli::evidence::freshness::{presented_commit, TreeState};
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap()
/// };
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// git(&["commit", "-q", "--allow-empty", "-m", "one"]);
///
/// assert_eq!(presented_commit(dir.path(), "HEAD", None).unwrap().tree(), TreeState::Clean);
/// // a caller that knows better can weaken it
/// let told = presented_commit(dir.path(), "HEAD", Some(TreeState::Dirty)).unwrap();
/// assert_eq!(told.tree(), TreeState::Dirty);
/// // a revision that names nothing is refused, and the refusal names it
/// let refused = presented_commit(dir.path(), "no-such-branch", None).unwrap_err();
/// assert!(refused.contains("no-such-branch"), "{refused}");
/// ```
pub fn presented_commit(
    root: &Path,
    rev: &str,
    given: Option<TreeState>,
) -> std::result::Result<Presented, String> {
    let commit = match git::resolves(root, rev) {
        true => commit_of(root, rev),
        false => None,
    }
    .ok_or_else(|| format!("`{rev}` names no commit in this repository"))?;
    let head = commit_of(root, "HEAD").ok_or_else(|| {
        "this repository has no checked-out commit, so no commit can be judged as committed"
            .to_string()
    })?;
    if commit != head {
        return Err(format!(
            "{rev} is {}, not the checked-out commit {}; check it out to judge at it",
            short12(&commit),
            short12(&head)
        ));
    }
    let measured = TreeState::parse(&git::working_tree_ignoring(root, &[LEDGER_PATH]));
    let tree = measured.weaker(given.unwrap_or(TreeState::Clean));
    Ok(Presented::Commit { commit, tree })
}

/// The ledger as `commit` holds it: `git show <commit>:.ai/repo/evidence/ledger.json`.
///
/// A commit that holds no ledger has an empty one, as a repository with no ledger file does.
/// A commit this clone does not have is an error, not an empty ledger: "nothing recorded"
/// and "nothing to read" are different answers.
///
/// ```
/// use majordomus_cli::evidence::freshness::ledger_at;
/// use majordomus_cli::evidence::Ledger;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     let out = Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap();
///     String::from_utf8_lossy(&out.stdout).trim().to_string()
/// };
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// git(&["commit", "-q", "--allow-empty", "-m", "nothing recorded yet"]);
/// let head = git(&["rev-parse", "HEAD"]);
///
/// assert_eq!(ledger_at(dir.path(), &head).unwrap(), Ledger::empty());
/// assert!(ledger_at(dir.path(), &"0".repeat(40)).is_err());
/// ```
pub fn ledger_at(root: &Path, commit: &str) -> Result<Ledger> {
    let spec = format!("{commit}:{LEDGER_PATH}");
    let out = git::read_only(root)
        .args(["show", &spec])
        .output()
        .map_err(Error::Transport)?;
    if out.status.success() {
        return Ledger::parse(&String::from_utf8_lossy(&out.stdout));
    }
    if git::resolves(root, commit) {
        // the commit is here and holds no ledger: nothing was recorded at it
        return Ok(Ledger::empty());
    }
    Err(Error::Git {
        reason: format!("`{commit}` names no commit in this repository"),
    })
}

/// Every execution the working copy of the ledger holds that `committed` does not.
///
/// The ledger keeps one execution per test, so a row that differs from the committed row of
/// its test, or that the committed ledger has no row for, is a run recorded since. These are
/// read only through [`weakened_by`]: they withhold `proven` and never decide a verdict.
///
/// An absent working copy holds none. An unreadable one is an error rather than none: a run
/// it may hold cannot be ruled out, so a verdict that judged past it could read `proven` over
/// a failure the checkout has.
///
/// ```
/// use majordomus_cli::evidence::freshness::uncommitted;
/// use majordomus_cli::evidence::{Ledger, LEDGER_PATH};
///
/// let root = tempfile::tempdir().unwrap();
/// // nothing in the working tree: nothing uncommitted
/// assert!(uncommitted(root.path(), &Ledger::empty()).unwrap().is_empty());
///
/// std::fs::create_dir_all(root.path().join(".ai/repo/evidence")).unwrap();
/// std::fs::write(root.path().join(LEDGER_PATH), "not json").unwrap();
/// let refused = uncommitted(root.path(), &Ledger::empty()).unwrap_err().to_string();
/// assert!(refused.contains(LEDGER_PATH), "{refused}");
/// ```
pub fn uncommitted(root: &Path, committed: &Ledger) -> Result<Vec<Execution>> {
    if !Ledger::present(root) {
        return Ok(Vec::new());
    }
    let working = Ledger::load(root).map_err(|e| Error::InvalidSurface {
        surface: "evidence".into(),
        reason: format!(
            "the working copy of {LEDGER_PATH} cannot be read, so a run it may hold cannot be \
             ruled out: {e}"
        ),
    })?;
    Ok(working
        .executions
        .into_iter()
        .filter(|e| committed.latest(&e.test) != Some(e))
        .collect())
}

// ---------------------------------------------------------------- the comparison

/// Every path that differs between the evidence commit and the presented revision, or
/// `None` when git cannot answer — which no caller may read as "nothing changed".
///
/// For the working tree this is the evidence module's own working-tree diff: tracked
/// changes committed since, staged or edited, and untracked files. For a presented commit
/// it is the committed difference between the two commits, and nothing in the checkout.
///
/// ```
/// use majordomus_cli::evidence::freshness::{changed_between, presented_commit, Presented};
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     let out = Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap();
///     String::from_utf8_lossy(&out.stdout).trim().to_string()
/// };
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// git(&["commit", "-q", "--allow-empty", "-m", "one"]);
/// let e = git(&["rev-parse", "HEAD"]);
/// std::fs::write(dir.path().join("a.md"), "a").unwrap();
/// git(&["add", "-A"]);
/// git(&["commit", "-q", "-m", "two"]);
/// std::fs::write(dir.path().join("b.md"), "b").unwrap();
///
/// // the working tree carries the untracked file; the commit does not
/// let here = changed_between(dir.path(), &e, &Presented::WorkingTree).unwrap();
/// assert!(here.contains("a.md") && here.contains("b.md"));
/// let committed = presented_commit(dir.path(), "HEAD", None).unwrap();
/// let there = changed_between(dir.path(), &e, &committed).unwrap();
/// assert_eq!(there.into_iter().collect::<Vec<_>>(), ["a.md"]);
/// ```
pub fn changed_between(
    root: &Path,
    evidence_commit: &str,
    presented: &Presented,
) -> Option<BTreeSet<String>> {
    match presented {
        Presented::WorkingTree => super::changed_since(root, evidence_commit),
        Presented::Commit { commit, .. } => {
            let out = git::read_only(root)
                .args([
                    "diff",
                    "--name-only",
                    evidence_commit,
                    commit.as_str(),
                    "--",
                ])
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
    }
}

/// What git says about one evidence commit against the presented revision: whether the
/// revision contains it, and what changed between them.
///
/// ```
/// use majordomus_cli::evidence::freshness::Comparison;
/// use majordomus_cli::git::Containment;
///
/// // git could not be asked: the truth table reads this as `stale`, never as unchanged
/// let unknown = Comparison { containment: Containment::CommitUnknown, changed: None };
/// assert!(unknown.changed.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// Whether the presented revision contains the evidence commit.
    pub containment: Containment,
    /// The paths that differ between the two, or `None` when git could not answer.
    pub changed: Option<BTreeSet<String>>,
}

/// Compare one evidence commit with the presented revision. For the working tree the
/// containing commit is HEAD (an unborn HEAD contains nothing anyone can name); for a
/// presented commit it is that commit.
///
/// ```
/// use majordomus_cli::evidence::freshness::{compare, Presented};
/// use majordomus_cli::git::Containment;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let git = |args: &[&str]| {
///     let out = Command::new("git").arg("-C").arg(dir.path()).args(args).output().unwrap();
///     String::from_utf8_lossy(&out.stdout).trim().to_string()
/// };
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// git(&["commit", "-q", "--allow-empty", "-m", "one"]);
/// let e = git(&["rev-parse", "HEAD"]);
///
/// let c = compare(dir.path(), &e, &Presented::WorkingTree);
/// assert_eq!(c.containment, Containment::Contains);
/// assert_eq!(c.changed, Some(Default::default()));
/// let gone = compare(dir.path(), &"0".repeat(40), &Presented::WorkingTree);
/// assert_eq!(gone.containment, Containment::CommitUnknown);
/// ```
pub fn compare(root: &Path, evidence_commit: &str, presented: &Presented) -> Comparison {
    let containing = presented.commit().unwrap_or("HEAD");
    Comparison {
        containment: git::contains(root, containing, evidence_commit),
        changed: changed_between(root, evidence_commit, presented),
    }
}

// ---------------------------------------------------------------- the judgement

/// What the ledger holds for one route's test, before any comparison.
///
/// ```
/// use majordomus_cli::evidence::freshness::Recorded;
/// // a claim that names no test is a different fact from one whose test never ran
/// assert!(!matches!(Recorded::NoTest, Recorded::NotRun));
/// ```
#[derive(Debug, Clone, Copy)]
pub enum Recorded<'a> {
    /// The route names no test.
    NoTest,
    /// The route names a path no runner drives.
    Unrunnable,
    /// A runner owns the test, and the ledger holds no execution of it.
    NotRun,
    /// The ledger's execution of the test.
    Ran(&'a Execution),
}

/// One verdict, with what it rests on.
///
/// ```
/// use majordomus_cli::evidence::freshness::Judgement;
/// use majordomus_cli::evidence::ProofState;
///
/// let j = Judgement {
///     state: ProofState::Stale,
///     changed: vec!["lib/alpha.sh".into()],
///     detail: None,
/// };
/// assert!(j.state.passing(), "a stale pass is still a pass");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judgement {
    /// The state.
    pub state: ProofState,
    /// The declared inputs that changed since the run, in the route's own order, when they
    /// are what made it `stale`.
    pub changed: Vec<String>,
    /// Why, in a sentence, when the state alone does not say.
    pub detail: Option<String>,
}

impl Judgement {
    fn of(state: ProofState) -> Judgement {
        Judgement {
            state,
            changed: Vec::new(),
            detail: None,
        }
    }

    fn because(state: ProofState, detail: impl Into<String>) -> Judgement {
        Judgement {
            state,
            changed: Vec::new(),
            detail: Some(detail.into()),
        }
    }
}

/// The one truth table: what a recorded run proves at the presented revision.
///
/// Pure: every fact it needs is an argument. `inputs` is what the route names — a claim's
/// source, implementation and test source, a rule's test source and definition — or `None`
/// for a route that declares none. `test_moved` is whether the test's source no longer
/// hashes to the recorded digest. `presented_tree` is [`TreeState::Clean`] for the working
/// tree, whose changes are already in `comparison`, and the measured tree for a commit.
///
/// Let `changed'` be the comparison's paths without the ledger's own. The first row that
/// matches decides:
///
/// | # | condition | state |
/// |---|---|---|
/// | 1–3 | no test, unrunnable, nothing recorded | `no_test`, `unrunnable`, `not_run` |
/// | 4 | the run was a skip | `not_run` |
/// | 5 | it failed, timed out or errored | `failing` |
/// | 6 | a pass git could not compare | `stale` |
/// | 7 | a pass the presented revision does not contain | `stale` |
/// | 8 | a pass whose test moved, no declared input changed | `stale`, naming the test |
/// | 9 | a pass, a declared input changed | `stale`, naming those inputs |
/// | 10 | a pass, `changed'` empty, both trees clean | `proven` |
/// | 11 | as 10, the recorded tree not clean | `inputs_unchanged` |
/// | 12 | as 10, the presented tree not clean | `inputs_unchanged` |
/// | 13 | a pass, `changed'` not empty, no declared input among it | `inputs_unchanged` |
/// | 14 | as 13, but the route declares no inputs | `stale` |
///
/// ```
/// use majordomus_cli::evidence::freshness::{freshness, Comparison, Recorded, TreeState};
/// use majordomus_cli::evidence::{Execution, ProofState};
/// use majordomus_cli::git::Containment;
///
/// let e: Execution = serde_json::from_value(serde_json::json!({
///     "test": "suite:07_scope", "runner": "suite", "source": "test/cases/07_scope.sh",
///     "outcome": "pass", "seconds": 1, "commit": "a".repeat(40), "working_tree": "clean",
///     "digest": "sha256:0", "at": "2026-09-26T00:00:00Z", "origin": "local",
///     "command": "bash test/run.sh 07_scope"
/// }))
/// .unwrap();
/// let inputs = vec!["docs/SCOPE.md".to_string()];
/// let other_history = Comparison {
///     containment: Containment::DoesNotContain,
///     changed: Some(Default::default()),
/// };
/// let j = freshness(
///     Recorded::Ran(&e),
///     Some(&inputs),
///     "test/cases/07_scope.sh",
///     false,
///     Some(&other_history),
///     TreeState::Clean,
/// );
/// // a run on a commit the presented revision does not contain proves some other history
/// assert_eq!(j.state, ProofState::Stale);
/// assert!(j.detail.unwrap().contains("does not contain"));
/// ```
pub fn freshness(
    recorded: Recorded<'_>,
    inputs: Option<&[String]>,
    test_source: &str,
    test_moved: bool,
    comparison: Option<&Comparison>,
    presented_tree: TreeState,
) -> Judgement {
    let e = match recorded {
        Recorded::NoTest => return Judgement::of(ProofState::NoTest),
        Recorded::Unrunnable => return Judgement::of(ProofState::Unrunnable),
        Recorded::NotRun => return Judgement::of(ProofState::NotRun),
        Recorded::Ran(e) => e,
    };
    match e.outcome {
        Outcome::Pass => {}
        Outcome::Skip => {
            return Judgement::because(ProofState::NotRun, "the test declined to run");
        }
        Outcome::Fail => return Judgement::because(ProofState::Failing, "failed"),
        Outcome::Timeout => return Judgement::because(ProofState::Failing, "timed out"),
        Outcome::Error => {
            return Judgement::because(ProofState::Failing, "the harness could not run it");
        }
    }

    let uncompared = || {
        Judgement::because(
            ProofState::Stale,
            format!(
                "git could not compare {} with the presented revision",
                short12(&e.commit)
            ),
        )
    };
    let Some(comparison) = comparison else {
        return uncompared();
    };
    let changed = match (comparison.containment, &comparison.changed) {
        (Containment::CommitUnknown, _) | (_, None) => return uncompared(),
        (Containment::DoesNotContain, Some(_)) => {
            return Judgement::because(
                ProofState::Stale,
                format!(
                    "recorded on {}, which the presented revision does not contain",
                    short12(&e.commit)
                ),
            );
        }
        (Containment::Contains, Some(changed)) => changed,
    };

    let declared_changed: Vec<String> = inputs
        .unwrap_or_default()
        .iter()
        .filter(|p| changed.contains(*p))
        .cloned()
        .collect();
    if test_moved && declared_changed.is_empty() {
        return Judgement {
            state: ProofState::Stale,
            changed: vec![test_source.to_string()],
            detail: None,
        };
    }
    if !declared_changed.is_empty() {
        return Judgement {
            state: ProofState::Stale,
            changed: declared_changed,
            detail: None,
        };
    }

    let changed_at_all = changed.iter().any(|p| p != LEDGER_PATH);
    if !changed_at_all {
        // the diff says the presented revision is the commit the run sat on; the run's own
        // tree says whether it measured that commit, and the presented tree whether what is
        // shown is that commit. All three, or it is not proven.
        let capped = capped_by_working_tree(ProofState::Proven, &e.working_tree);
        if capped != ProofState::Proven {
            return Judgement::because(capped, "the run measured a tree that was not its commit");
        }
        if presented_tree != TreeState::Clean {
            return Judgement::because(
                ProofState::InputsUnchanged,
                "the presented revision was built from a tree that was not its commit",
            );
        }
        return Judgement::of(ProofState::Proven);
    }
    match inputs {
        Some(_) => Judgement::of(ProofState::InputsUnchanged),
        None => Judgement::because(
            ProofState::Stale,
            "the route names no inputs, so a change since the run cannot be ruled out",
        ),
    }
}

// ---------------------------------------------------------------- the monotone rule

/// A record read beside the verdict's own execution, which may weaken it and never
/// strengthens it.
///
/// Its two containments are computed by the caller with [`git::contains`], so that
/// [`weakened_by`] stays pure: `after_evidence` is whether the record's commit contains the
/// evidence commit, `in_presented` whether the presented commit contains the record's.
///
/// ```
/// use majordomus_cli::evidence::freshness::{Supplementary, UNCOMMITTED_RUN};
/// use majordomus_cli::evidence::Execution;
/// use majordomus_cli::git::Containment;
///
/// let e: Execution = serde_json::from_value(serde_json::json!({
///     "test": "suite:07_scope", "runner": "suite", "source": "test/cases/07_scope.sh",
///     "outcome": "fail", "seconds": 1, "commit": "b".repeat(40), "working_tree": "clean",
///     "digest": "sha256:0", "at": "2026-09-26T00:00:00Z", "origin": "local",
///     "command": "bash test/run.sh 07_scope"
/// }))
/// .unwrap();
/// let record = Supplementary {
///     execution: &e,
///     named: UNCOMMITTED_RUN,
///     after_evidence: Containment::Contains,
///     in_presented: Containment::Contains,
/// };
/// assert_eq!(record.named, "an uncommitted run in the working ledger");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Supplementary<'a> {
    /// The record's execution.
    pub execution: &'a Execution,
    /// How a detail names the record, for example [`UNCOMMITTED_RUN`].
    pub named: &'a str,
    /// Whether the record's commit contains the evidence commit: a record from before the
    /// evidence says nothing against it.
    pub after_evidence: Containment,
    /// Whether the presented commit contains the record's: a record from another history
    /// says nothing about this one.
    pub in_presented: Containment,
}

/// The monotone rule: a supplementary record may withhold `proven` and never grant it.
///
/// It changes a judgement only when the judgement is `proven` or `inputs_unchanged` and some
/// record failed, timed out or errored on a clean tree, at a commit that contains the
/// evidence commit and that the presented revision contains. Then the state becomes
/// `stale`, naming the record. The cap is `stale` and not `failing`: a supplementary record
/// withholds `proven` and never decides a verdict.
///
/// ```
/// use majordomus_cli::evidence::freshness::{weakened_by, Judgement, Supplementary, UNCOMMITTED_RUN};
/// use majordomus_cli::evidence::{Execution, ProofState};
/// use majordomus_cli::git::Containment;
///
/// let failed: Execution = serde_json::from_value(serde_json::json!({
///     "test": "suite:07_scope", "runner": "suite", "source": "test/cases/07_scope.sh",
///     "outcome": "fail", "seconds": 1, "commit": "b".repeat(40), "working_tree": "clean",
///     "digest": "sha256:0", "at": "2026-09-26T00:00:00Z", "origin": "local",
///     "command": "bash test/run.sh 07_scope"
/// }))
/// .unwrap();
/// let record = Supplementary {
///     execution: &failed,
///     named: UNCOMMITTED_RUN,
///     after_evidence: Containment::Contains,
///     in_presented: Containment::Contains,
/// };
/// let proven = Judgement { state: ProofState::Proven, changed: vec![], detail: None };
/// let j = weakened_by(proven, &[record]);
/// assert_eq!(j.state, ProofState::Stale);
/// assert!(j.detail.unwrap().contains(&"b".repeat(12)));
///
/// // and never the other way
/// let failing = Judgement { state: ProofState::Failing, changed: vec![], detail: None };
/// assert_eq!(weakened_by(failing.clone(), &[record]), failing);
/// ```
pub fn weakened_by(judgement: Judgement, records: &[Supplementary<'_>]) -> Judgement {
    if judgement.state >= ProofState::Stale {
        return judgement;
    }
    let counter = records.iter().find_map(|r| {
        let verb = match r.execution.outcome {
            Outcome::Fail => "failed",
            Outcome::Timeout => "timed out",
            Outcome::Error => "errored",
            Outcome::Pass | Outcome::Skip => return None,
        };
        let clean = TreeState::parse(&r.execution.working_tree) == TreeState::Clean;
        let placed =
            r.after_evidence == Containment::Contains && r.in_presented == Containment::Contains;
        (clean && placed).then_some((r, verb))
    });
    match counter {
        None => judgement,
        Some((r, verb)) => Judgement {
            state: ProofState::Stale,
            changed: judgement.changed,
            detail: Some(format!(
                "{} recorded on {} {verb}, so the pass before it proves nothing at the presented \
                 revision",
                r.named,
                short12(&r.execution.commit)
            )),
        },
    }
}

// ---------------------------------------------------------------- aggregation

/// Several states made one: a failure outranks an absence, and otherwise the weakest part
/// that can carry proof decides.
///
/// Each part is a state and whether it can carry proof. The result is `failing` when any
/// part is `failing`; otherwise the weakest (the greatest, by the enum's declared order) of
/// the parts that can carry proof; otherwise the weakest of all parts; and `None` for none.
///
/// Why the first rule: the declared order ranks `failing` above `not_run`, so a plain
/// maximum over a failing test and a test that never ran reads `not_run`, and hides the
/// counter-evidence behind an absence. Why the second: a path no runner and no gate drives
/// can never carry proof, so it counts only when it is all there is.
///
/// ```
/// use majordomus_cli::evidence::freshness::aggregate;
/// use majordomus_cli::evidence::ProofState::{Failing, NotRun, Proven, Unrunnable};
///
/// assert_eq!(aggregate(&[(Failing, true), (NotRun, true)], Failing), Some(Failing));
/// assert_eq!(aggregate(&[(Proven, true), (NotRun, true)], Failing), Some(NotRun));
/// assert_eq!(aggregate(&[(Proven, true), (Unrunnable, false)], Failing), Some(Proven));
/// assert_eq!(aggregate(&[(Unrunnable, false)], Failing), Some(Unrunnable));
/// assert_eq!(aggregate::<majordomus_cli::evidence::ProofState>(&[], Failing), None);
/// ```
pub fn aggregate<S: Ord + Copy>(parts: &[(S, bool)], failing: S) -> Option<S> {
    if parts.iter().any(|(s, _)| *s == failing) {
        return Some(failing);
    }
    parts
        .iter()
        .filter(|(_, carries)| *carries)
        .map(|(s, _)| *s)
        .max()
        .or_else(|| parts.iter().map(|(s, _)| *s).max())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const R: &str = "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr";
    const SOURCE: &str = "test/cases/01_alpha.sh";

    /// One execution, built from the ledger's own JSON so that a field added to
    /// [`Execution`] with a serde default reaches every test here unchanged.
    fn ran(outcome: &str, commit: &str, tree: &str) -> Execution {
        serde_json::from_value(serde_json::json!({
            "test": "suite:01_alpha",
            "runner": "suite",
            "source": SOURCE,
            "outcome": outcome,
            "seconds": 1,
            "commit": commit,
            "working_tree": tree,
            "digest": "sha256:0",
            "at": "2026-09-26T00:00:00Z",
            "origin": "local",
            "command": "bash test/run.sh 01_alpha"
        }))
        .unwrap()
    }

    fn inputs() -> Vec<String> {
        vec![
            "docs/ALPHA.md".to_string(),
            "lib/alpha.sh".to_string(),
            SOURCE.to_string(),
        ]
    }

    fn contains(paths: &[&str]) -> Comparison {
        Comparison {
            containment: Containment::Contains,
            changed: Some(paths.iter().map(|p| p.to_string()).collect()),
        }
    }

    /// A pass on a clean tree judged against `comparison` and `presented`, with the claim's
    /// three inputs and a test that has not moved.
    fn judge(e: &Execution, comparison: Option<&Comparison>, presented: TreeState) -> Judgement {
        let i = inputs();
        freshness(
            Recorded::Ran(e),
            Some(&i),
            SOURCE,
            false,
            comparison,
            presented,
        )
    }

    /// A git repository with one commit, for the rules that ask git.
    struct Repo {
        dir: tempfile::TempDir,
    }

    impl Repo {
        fn new() -> Repo {
            let r = Repo {
                dir: tempfile::tempdir().unwrap(),
            };
            r.git(&["init", "-q"]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "t"]);
            r.git(&["commit", "-q", "--allow-empty", "-m", "init"]);
            r
        }
        fn root(&self) -> &Path {
            self.dir.path()
        }
        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .arg("-C")
                .arg(self.root())
                .args(args)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        fn write(&self, path: &str, text: &str) {
            let p = self.root().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        fn commit(&self, path: &str, text: &str) -> String {
            self.write(path, text);
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", path]);
            self.git(&["rev-parse", "HEAD"])
        }
        fn ledger(&self, executions: &[Execution]) {
            let mut l = Ledger::empty();
            l.merge(executions.iter().cloned());
            l.save(self.root()).unwrap();
        }
    }

    // ---------------------------------------------------------------- the rows

    #[test]
    fn row_01_no_test_is_no_test() {
        let j = freshness(Recorded::NoTest, None, "", false, None, TreeState::Clean);
        assert_eq!(j, Judgement::of(ProofState::NoTest));
    }

    #[test]
    fn row_02_a_path_no_runner_drives_is_unrunnable() {
        let j = freshness(
            Recorded::Unrunnable,
            None,
            "",
            false,
            None,
            TreeState::Clean,
        );
        assert_eq!(j, Judgement::of(ProofState::Unrunnable));
    }

    #[test]
    fn row_03_nothing_recorded_is_not_run() {
        let j = freshness(
            Recorded::NotRun,
            None,
            SOURCE,
            false,
            None,
            TreeState::Clean,
        );
        assert_eq!(j, Judgement::of(ProofState::NotRun));
    }

    /// A test that declined to run proved nothing and failed nothing: `not_run`, with the
    /// reason, however fresh everything else is.
    #[test]
    fn row_04_a_skip_is_not_run() {
        let e = ran("skip", E, "clean");
        let j = judge(&e, Some(&contains(&[])), TreeState::Clean);
        assert_eq!(j.state, ProofState::NotRun);
        assert_eq!(j.detail.as_deref(), Some("the test declined to run"));
        assert!(j.changed.is_empty());
    }

    #[test]
    fn row_05_a_failure_a_timeout_and_an_error_are_failing() {
        for (outcome, detail) in [
            ("fail", "failed"),
            ("timeout", "timed out"),
            ("error", "the harness could not run it"),
        ] {
            let e = ran(outcome, E, "clean");
            let j = judge(&e, Some(&contains(&[])), TreeState::Clean);
            assert_eq!(j.state, ProofState::Failing, "{outcome}");
            assert_eq!(j.detail.as_deref(), Some(detail), "{outcome}");
        }
    }

    /// Not knowing is not proof: no comparison, a commit git cannot find, and a diff git
    /// could not produce are all `stale`, and the detail names the commit.
    #[test]
    fn row_06_a_pass_git_could_not_compare_is_stale() {
        let e = ran("pass", E, "clean");
        let unknown = Comparison {
            containment: Containment::CommitUnknown,
            changed: Some(BTreeSet::new()),
        };
        let no_diff = Comparison {
            containment: Containment::Contains,
            changed: None,
        };
        let no_diff_elsewhere = Comparison {
            containment: Containment::DoesNotContain,
            changed: None,
        };
        for c in [
            None,
            Some(&unknown),
            Some(&no_diff),
            Some(&no_diff_elsewhere),
        ] {
            let j = judge(&e, c, TreeState::Clean);
            assert_eq!(j.state, ProofState::Stale, "{c:?}");
            let d = j.detail.unwrap();
            assert!(d.contains("git could not compare"), "{d}");
            assert!(d.contains(&E[..12]), "{d}");
        }
    }

    #[test]
    fn row_07_a_pass_the_presented_revision_does_not_contain_is_stale() {
        let e = ran("pass", E, "clean");
        let elsewhere = Comparison {
            containment: Containment::DoesNotContain,
            changed: Some(BTreeSet::new()),
        };
        let j = judge(&e, Some(&elsewhere), TreeState::Clean);
        assert_eq!(j.state, ProofState::Stale);
        assert_eq!(
            j.detail.as_deref(),
            Some(
                format!(
                    "recorded on {}, which the presented revision does not contain",
                    &E[..12]
                )
                .as_str()
            )
        );
    }

    /// A test whose source no longer hashes to what ran is not the test that ran, even
    /// with no diff to show for it (an edit made and reverted around the run).
    #[test]
    fn row_08_a_test_that_moved_is_stale_and_named() {
        let e = ran("pass", E, "clean");
        let i = inputs();
        let j = freshness(
            Recorded::Ran(&e),
            Some(&i),
            SOURCE,
            true,
            Some(&contains(&[])),
            TreeState::Clean,
        );
        assert_eq!(j.state, ProofState::Stale);
        assert_eq!(j.changed, [SOURCE]);
        assert_eq!(j.detail, None);
    }

    #[test]
    fn row_09_a_declared_input_that_changed_is_stale_and_named_in_input_order() {
        let e = ran("pass", E, "clean");
        let c = contains(&[SOURCE, "lib/alpha.sh", "unrelated.txt", LEDGER_PATH]);
        let j = judge(&e, Some(&c), TreeState::Clean);
        assert_eq!(j.state, ProofState::Stale);
        assert_eq!(
            j.changed,
            ["lib/alpha.sh", SOURCE],
            "the route's order, not the diff's"
        );
        // and a moved test whose file also shows in the diff is named once, by the diff
        let i = inputs();
        let moved = freshness(
            Recorded::Ran(&e),
            Some(&i),
            SOURCE,
            true,
            Some(&contains(&[SOURCE])),
            TreeState::Clean,
        );
        assert_eq!(moved.changed, [SOURCE]);
    }

    /// The only proven: contained, nothing but the ledger changed, both trees clean.
    #[test]
    fn row_10_a_contained_pass_on_clean_trees_with_nothing_changed_is_proven() {
        let e = ran("pass", E, "clean");
        assert_eq!(
            judge(&e, Some(&contains(&[])), TreeState::Clean),
            Judgement::of(ProofState::Proven)
        );
        // the ledger's own row does not age the proof it records
        assert_eq!(
            judge(&e, Some(&contains(&[LEDGER_PATH])), TreeState::Clean),
            Judgement::of(ProofState::Proven)
        );
    }

    #[test]
    fn row_11_a_run_on_a_tree_that_was_not_its_commit_is_inputs_unchanged() {
        for tree in ["dirty", "unknown", "anything else"] {
            let e = ran("pass", E, tree);
            for presented in [TreeState::Clean, TreeState::Dirty, TreeState::Unknown] {
                let j = judge(&e, Some(&contains(&[])), presented);
                assert_eq!(j.state, ProofState::InputsUnchanged, "{tree}");
                assert_eq!(
                    j.detail.as_deref(),
                    Some("the run measured a tree that was not its commit"),
                    "{tree} / {presented:?}: row 11 comes before row 12"
                );
            }
        }
    }

    #[test]
    fn row_12_a_presented_tree_that_was_not_its_commit_is_inputs_unchanged() {
        let e = ran("pass", E, "clean");
        for presented in [TreeState::Dirty, TreeState::Unknown] {
            let j = judge(&e, Some(&contains(&[])), presented);
            assert_eq!(j.state, ProofState::InputsUnchanged);
            assert_eq!(
                j.detail.as_deref(),
                Some("the presented revision was built from a tree that was not its commit")
            );
        }
    }

    #[test]
    fn row_13_a_change_the_route_does_not_name_is_inputs_unchanged_for_any_tree() {
        for tree in ["clean", "dirty", "unknown"] {
            let e = ran("pass", E, tree);
            for presented in [TreeState::Clean, TreeState::Dirty, TreeState::Unknown] {
                let j = judge(&e, Some(&contains(&["unrelated.txt"])), presented);
                assert_eq!(j, Judgement::of(ProofState::InputsUnchanged), "{tree}");
            }
        }
    }

    #[test]
    fn row_14_a_change_a_route_with_no_inputs_cannot_rule_out_is_stale() {
        let e = ran("pass", E, "clean");
        let j = freshness(
            Recorded::Ran(&e),
            None,
            SOURCE,
            false,
            Some(&contains(&["unrelated.txt"])),
            TreeState::Clean,
        );
        assert_eq!(j.state, ProofState::Stale);
        assert_eq!(
            j.detail.as_deref(),
            Some("the route names no inputs, so a change since the run cannot be ruled out")
        );
        // with nothing changed there is nothing to rule out
        let none = freshness(
            Recorded::Ran(&e),
            None,
            SOURCE,
            false,
            Some(&contains(&[])),
            TreeState::Clean,
        );
        assert_eq!(none.state, ProofState::Proven);
    }

    /// The working tree's untracked files are part of what is presented: a new file no
    /// claim names takes a run from `proven` to `inputs_unchanged` (row 13).
    #[test]
    fn an_untracked_file_in_the_working_tree_is_a_change_since_the_run() {
        let r = Repo::new();
        let e = r.git(&["rev-parse", "HEAD"]);
        let run = ran("pass", &e, "clean");
        let before = compare(r.root(), &e, &Presented::WorkingTree);
        assert_eq!(
            judge(&run, Some(&before), TreeState::Clean).state,
            ProofState::Proven
        );

        r.write("new-and-untracked.txt", "x");
        let after = compare(r.root(), &e, &Presented::WorkingTree);
        assert!(after
            .changed
            .as_ref()
            .unwrap()
            .contains("new-and-untracked.txt"));
        assert_eq!(
            judge(&run, Some(&after), TreeState::Clean),
            Judgement::of(ProofState::InputsUnchanged)
        );
    }

    /// Row 10 is the only way to `proven`, over every combination of every input: a result
    /// of `proven` implies a pass, containment, a clean recorded tree, a clean presented
    /// tree and nothing but the ledger changed.
    #[test]
    fn no_combination_is_proven_unless_every_condition_holds() {
        let outcomes = ["pass", "fail", "skip", "timeout", "error"];
        let containments = [
            Containment::Contains,
            Containment::DoesNotContain,
            Containment::CommitUnknown,
        ];
        let trees = ["clean", "dirty", "unknown"];
        let presented = [TreeState::Clean, TreeState::Dirty, TreeState::Unknown];
        let diffs: [Option<&[&str]>; 5] = [
            None,
            Some(&[]),
            Some(&[LEDGER_PATH]),
            Some(&["unrelated.txt"]),
            Some(&["lib/alpha.sh"]),
        ];
        let i = inputs();
        let mut proven = 0;
        for outcome in outcomes {
            for containment in containments {
                for tree in trees {
                    for q in presented {
                        for diff in diffs {
                            for moved in [false, true] {
                                for declared in [Some(i.as_slice()), None] {
                                    let e = ran(outcome, E, tree);
                                    let c = Comparison {
                                        containment,
                                        changed: diff
                                            .map(|d| d.iter().map(|p| p.to_string()).collect()),
                                    };
                                    let j = freshness(
                                        Recorded::Ran(&e),
                                        declared,
                                        SOURCE,
                                        moved,
                                        Some(&c),
                                        q,
                                    );
                                    if j.state != ProofState::Proven {
                                        continue;
                                    }
                                    proven += 1;
                                    let only_ledger =
                                        diff.is_some_and(|d| d.iter().all(|p| *p == LEDGER_PATH));
                                    assert!(
                                        outcome == "pass"
                                            && containment == Containment::Contains
                                            && tree == "clean"
                                            && q == TreeState::Clean
                                            && only_ledger
                                            && !moved,
                                        "proven from {outcome} {containment:?} {tree} {q:?} \
                                         {diff:?} moved={moved}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            proven > 0,
            "the property held over a set with no proven member"
        );
    }

    // ---------------------------------------------------------------- aggregation

    #[test]
    fn a_failure_outranks_an_absence() {
        use ProofState::*;
        assert_eq!(
            aggregate(&[(Failing, true), (NotRun, true)], Failing),
            Some(Failing)
        );
        // whichever order, and whether or not the failing part is the one that carries proof
        assert_eq!(
            aggregate(&[(NotRun, true), (Failing, false)], Failing),
            Some(Failing)
        );
    }

    #[test]
    fn a_part_nothing_drives_counts_only_when_it_is_all_there_is() {
        use ProofState::*;
        assert_eq!(
            aggregate(&[(Proven, true), (Unrunnable, false)], Failing),
            Some(Proven)
        );
        assert_eq!(aggregate(&[(Unrunnable, false)], Failing), Some(Unrunnable));
    }

    #[test]
    fn otherwise_the_weakest_part_decides_and_nothing_is_nothing() {
        use ProofState::*;
        assert_eq!(
            aggregate(&[(Proven, true), (NotRun, true)], Failing),
            Some(NotRun)
        );
        assert_eq!(
            aggregate(&[(InputsUnchanged, true), (Proven, true)], Failing),
            Some(InputsUnchanged)
        );
        assert_eq!(aggregate::<ProofState>(&[], Failing), None);
    }

    // ---------------------------------------------------------------- the presented commit

    #[test]
    fn a_revision_other_than_the_checked_out_commit_is_refused() {
        let r = Repo::new();
        let first = r.git(&["rev-parse", "HEAD"]);
        let second = r.commit("a.md", "a");
        let refused = presented_commit(r.root(), &first, None).unwrap_err();
        assert!(refused.contains(&first[..12]), "{refused}");
        assert!(refused.contains(&second[..12]), "{refused}");
        assert!(refused.contains("not the checked-out commit"), "{refused}");
        let p = presented_commit(r.root(), &second, None).unwrap();
        assert_eq!(p.commit(), Some(second.as_str()));

        let zeros = "0".repeat(40);
        let unknown = presented_commit(r.root(), &zeros, None).unwrap_err();
        assert!(unknown.contains(&zeros), "{unknown}");
    }

    /// A checkout with nothing checked out has no commit to judge as committed, even when
    /// the revision named is a commit this clone has.
    #[test]
    fn an_unborn_head_has_no_commit_to_present() {
        let r = Repo::new();
        let first = r.git(&["rev-parse", "HEAD"]);
        r.git(&["checkout", "-q", "--orphan", "nothing-yet"]);
        let refused = presented_commit(r.root(), &first, None).unwrap_err();
        assert!(refused.contains("no checked-out commit"), "{refused}");
    }

    #[test]
    fn a_measured_dirty_tree_is_not_overridden_by_a_given_clean_one() {
        let r = Repo::new();
        r.write("pending.txt", "x");
        let p = presented_commit(r.root(), "HEAD", Some(TreeState::Clean)).unwrap();
        assert_eq!(p.tree(), TreeState::Dirty);
    }

    #[test]
    fn a_given_dirty_tree_weakens_a_clean_measurement() {
        let r = Repo::new();
        assert_eq!(
            presented_commit(r.root(), "HEAD", None).unwrap().tree(),
            TreeState::Clean
        );
        for given in [TreeState::Dirty, TreeState::Unknown] {
            let p = presented_commit(r.root(), "HEAD", Some(given)).unwrap();
            assert_eq!(p.tree(), given);
        }
    }

    #[test]
    fn a_change_to_the_ledgers_working_copy_alone_keeps_the_presented_tree_clean() {
        let r = Repo::new();
        r.ledger(&[ran("pass", E, "clean")]);
        let p = presented_commit(r.root(), "HEAD", None).unwrap();
        assert_eq!(p.tree(), TreeState::Clean, "an untracked ledger");
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "ledger"]);
        r.ledger(&[ran("fail", E, "clean")]);
        let p = presented_commit(r.root(), "HEAD", None).unwrap();
        assert_eq!(p.tree(), TreeState::Clean, "a modified ledger");
    }

    #[test]
    fn a_commit_without_a_ledger_has_an_empty_one_and_a_commit_with_one_has_that_one() {
        let r = Repo::new();
        let before = r.git(&["rev-parse", "HEAD"]);
        r.ledger(&[ran("pass", E, "clean")]);
        r.git(&["add", "-A"]);
        r.git(&["commit", "-q", "-m", "ledger"]);
        let after = r.git(&["rev-parse", "HEAD"]);
        assert_eq!(ledger_at(r.root(), &before).unwrap(), Ledger::empty());
        let at = ledger_at(r.root(), &after).unwrap();
        assert_eq!(at.executions, [ran("pass", E, "clean")]);
        // the working copy moving on does not move what the commit holds
        r.ledger(&[ran("fail", E, "clean")]);
        assert_eq!(ledger_at(r.root(), &after).unwrap(), at);
        assert!(ledger_at(r.root(), &"0".repeat(40)).is_err());
    }

    #[test]
    fn the_diff_to_a_presented_commit_is_what_was_committed_between_the_two() {
        let r = Repo::new();
        let e = r.git(&["rev-parse", "HEAD"]);
        r.commit("a.md", "a");
        r.write("pending.txt", "x");
        let p = presented_commit(r.root(), "HEAD", None).unwrap();
        let c = compare(r.root(), &e, &p);
        assert_eq!(c.containment, Containment::Contains);
        assert_eq!(c.changed.unwrap().into_iter().collect::<Vec<_>>(), ["a.md"]);
        let gone = compare(r.root(), &"0".repeat(40), &p);
        assert_eq!(gone.containment, Containment::CommitUnknown);
        assert!(gone.changed.is_none());
    }

    // ---------------------------------------------------------------- the monotone rule

    fn record(e: &Execution, after: Containment, within: Containment) -> Supplementary<'_> {
        Supplementary {
            execution: e,
            named: UNCOMMITTED_RUN,
            after_evidence: after,
            in_presented: within,
        }
    }

    #[test]
    fn a_clean_failing_record_after_the_evidence_caps_a_pass_at_stale() {
        use Containment::Contains;
        for (outcome, verb) in [
            ("fail", "failed"),
            ("timeout", "timed out"),
            ("error", "errored"),
        ] {
            let e = ran(outcome, R, "clean");
            for state in [ProofState::Proven, ProofState::InputsUnchanged] {
                let before = Judgement {
                    state,
                    changed: vec!["kept".into()],
                    detail: None,
                };
                let j = weakened_by(before, &[record(&e, Contains, Contains)]);
                assert_eq!(j.state, ProofState::Stale, "{outcome}");
                assert_eq!(j.changed, ["kept"]);
                let d = j.detail.unwrap();
                assert!(d.starts_with(UNCOMMITTED_RUN), "{d}");
                assert!(d.contains(&format!("{} {verb}", &R[..12])), "{d}");
            }
        }
    }

    #[test]
    fn a_passing_or_skipped_record_changes_nothing() {
        use Containment::Contains;
        for outcome in ["pass", "skip"] {
            let e = ran(outcome, R, "clean");
            let j = Judgement::of(ProofState::Proven);
            assert_eq!(weakened_by(j.clone(), &[record(&e, Contains, Contains)]), j);
        }
    }

    #[test]
    fn a_failing_record_on_a_tree_that_was_not_its_commit_changes_nothing() {
        use Containment::Contains;
        for tree in ["dirty", "unknown"] {
            let e = ran("fail", R, tree);
            let j = Judgement::of(ProofState::Proven);
            assert_eq!(weakened_by(j.clone(), &[record(&e, Contains, Contains)]), j);
        }
    }

    #[test]
    fn a_failing_record_from_another_history_changes_nothing() {
        use Containment::*;
        let e = ran("fail", R, "clean");
        let j = Judgement::of(ProofState::Proven);
        for (after, within) in [
            (DoesNotContain, Contains),
            (Contains, DoesNotContain),
            (CommitUnknown, Contains),
            (Contains, CommitUnknown),
        ] {
            assert_eq!(
                weakened_by(j.clone(), &[record(&e, after, within)]),
                j,
                "{after:?} {within:?}"
            );
        }
        assert_eq!(weakened_by(j.clone(), &[]), j);
    }

    /// Monotone: whatever the records, the result is never stronger than what went in.
    #[test]
    fn no_record_ever_strengthens_a_verdict() {
        use Containment::*;
        let states = [
            ProofState::Proven,
            ProofState::InputsUnchanged,
            ProofState::Stale,
            ProofState::Failing,
            ProofState::NotRun,
            ProofState::Unrunnable,
            ProofState::NoTest,
        ];
        for state in states {
            for outcome in ["pass", "fail", "skip", "timeout", "error"] {
                for tree in ["clean", "dirty", "unknown"] {
                    for after in [Contains, DoesNotContain, CommitUnknown] {
                        for within in [Contains, DoesNotContain, CommitUnknown] {
                            let e = ran(outcome, R, tree);
                            let j = weakened_by(Judgement::of(state), &[record(&e, after, within)]);
                            assert!(j.state >= state, "{state:?} became {:?}", j.state);
                            // and a verdict at or below stale is returned exactly as it was
                            if state >= ProofState::Stale {
                                assert_eq!(j, Judgement::of(state));
                            }
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- uncommitted runs

    #[test]
    fn the_uncommitted_runs_are_exactly_the_rows_that_differ() {
        let r = Repo::new();
        let pass = ran("pass", E, "clean");
        let mut other = ran("pass", E, "clean");
        other.test = "suite:02_beta".into();
        other.source = "test/cases/02_beta.sh".into();
        r.ledger(&[pass.clone(), other.clone()]);
        let committed = Ledger::load(r.root()).unwrap();
        assert!(uncommitted(r.root(), &committed).unwrap().is_empty());

        // one row replaced, one row new, one row as committed
        let failed = ran("fail", R, "clean");
        let mut third = ran("pass", R, "clean");
        third.test = "suite:03_gamma".into();
        r.ledger(&[failed.clone(), other.clone(), third.clone()]);
        let got = uncommitted(r.root(), &committed).unwrap();
        assert_eq!(got, [failed, third]);
    }

    #[test]
    fn an_absent_working_ledger_holds_no_uncommitted_run() {
        let r = Repo::new();
        let mut committed = Ledger::empty();
        committed.merge([ran("pass", E, "clean")]);
        assert!(uncommitted(r.root(), &committed).unwrap().is_empty());
    }

    #[test]
    fn an_unreadable_working_ledger_is_an_error_and_not_an_absence() {
        let r = Repo::new();
        r.write(LEDGER_PATH, "{ not a ledger");
        let err = uncommitted(r.root(), &Ledger::empty())
            .unwrap_err()
            .to_string();
        assert!(err.contains(LEDGER_PATH), "{err}");
        assert!(err.contains("cannot be ruled out"), "{err}");
    }

    #[test]
    fn a_tree_word_reads_back_and_the_weaker_of_two_is_never_cleaner() {
        use TreeState::*;
        for a in [Clean, Dirty, Unknown] {
            assert_eq!(TreeState::parse(a.as_str()), a);
            for b in [Clean, Dirty, Unknown] {
                let w = a.weaker(b);
                assert_eq!(w, b.weaker(a), "weaker is symmetric");
                assert_eq!(w == Clean, a == Clean && b == Clean);
                assert_eq!(w == Dirty, a == Dirty || b == Dirty);
            }
        }
    }
}
