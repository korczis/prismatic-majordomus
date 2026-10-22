//! The ledger: the latest execution of every test, as a tracked file.
//!
//! # Why a file, and why this one
//!
//! Evidence that lives only in a CI job's log is evidence nobody can query, and evidence
//! that lives in a database is evidence a clone does not carry. The repository already has
//! a precedent for recorded, committed, reviewable measurement — the benchmark baselines
//! under `.ai/repo/benchmarks/`, which are JSON, per platform, written by a tool and read
//! by a gate. This is the same kind of artifact for the same reason, and it is deliberately
//! shaped like them rather than like something new.
//!
//! # What it holds, and what it refuses to hold
//!
//! One entry per test: the latest execution. Not a history — a run per commit for a hundred
//! tests would grow without bound and the question it answers ("is this claim proven now?")
//! only ever reads the newest. Not stdout, not artifacts: a captured log of every case is
//! megabytes per run and belongs where the run happened. What is kept is the durable
//! semantic part — outcome, duration, commit, tree state, digest, time, origin, and the
//! command that produces it again — which is what a reader needs to decide whether to
//! believe it and how to check.
//!
//! # JSON, not the layer's YAML
//!
//! This repository reads a deliberately small YAML subset (`docs/SCHEMAS.md`), and a file
//! only our own reader can parse is a trap this repository has already fallen into once.
//! The ledger is machine-written and machine-read, so it is JSON: `serde` on both sides,
//! no subset to respect, and `jq` works on it.
//!
//! # The whole lifecycle
//!
//! Load what is there (nothing, in a repository that has recorded nothing), merge the
//! executions a run produced, save, and read back the same value:
//!
//! ```
//! use majordomus_cli::evidence::{Execution, Ledger, Origin, Outcome, Runner};
//!
//! fn execution(name: &str, outcome: Outcome) -> Execution {
//!     Execution {
//!         test: format!("suite:{name}"),
//!         runner: Runner::Suite,
//!         source: format!("test/cases/{name}.sh"),
//!         outcome,
//!         seconds: 1,
//!         commit: "0".repeat(40),
//!         working_tree: "clean".into(),
//!         digest: "sha256:0".into(),
//!         at: "2026-09-11T00:00:00Z".into(),
//!         origin: Origin::Local,
//!         command: format!("bash test/run.sh {name}"),
//!     }
//! }
//!
//! let root = tempfile::tempdir().unwrap();
//!
//! // a repository that has recorded nothing has an empty ledger, and that is not an error
//! assert!(!Ledger::present(root.path()));
//! let mut ledger = Ledger::load(root.path()).unwrap();
//! assert!(ledger.executions.is_empty());
//!
//! ledger.merge([
//!     execution("08_other", Outcome::Fail),
//!     execution("07_scope", Outcome::Pass),
//! ]);
//! ledger.save(root.path()).unwrap();
//!
//! // what came back is what went in, ordered by test id so the tracked file diffs cleanly
//! let read = Ledger::load(root.path()).unwrap();
//! assert_eq!(read, ledger);
//! let ids: Vec<&str> = read.executions.iter().map(|e| e.test.as_str()).collect();
//! assert_eq!(ids, ["suite:07_scope", "suite:08_other"]);
//! assert_eq!(read.summary().executions, 2);
//! assert_eq!(read.by_outcome().get("pass"), Some(&1));
//! ```

//! # Example
//!
//! A tree that has recorded nothing has no ledger, and says so rather than refusing to
//! answer — a repository that could not say "nothing was recorded" would say nothing.
//!
//! ```
//! use majordomus_cli::evidence::Ledger;
//! use majordomus_cli::synthetic::SyntheticRepository;
//! let repo = SyntheticRepository::small().unwrap();
//! let ledger = Ledger::load(repo.root()).unwrap();
//! assert!(!Ledger::present(repo.root()));
//! assert!(ledger.latest("suite:07_scope").is_none());
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Execution, LedgerSummary};
use crate::error::{Error, Result};

/// Where the ledger lives, repository-relative. One spelling, used by the recorder, the
/// reader and the documentation.
pub const LEDGER_PATH: &str = ".ai/repo/evidence/ledger.json";

/// The version of the ledger's own shape. A reader that meets a version it does not know
/// refuses rather than guessing: a misread ledger is worse than an absent one, because an
/// absent one reports `not run` and a misread one could report a pass.
pub const LEDGER_VERSION: u32 = 1;

/// # Example
///
/// ```
/// use majordomus_cli::evidence::Ledger;
/// let l = Ledger::empty();
/// assert_eq!(l.summary().executions, 0);
/// assert!(l.latest("suite:07_scope").is_none());
/// ```
/// The latest execution of every test this repository has recorded.
///
/// One entry per test, never a history: the question it answers ("is this claim proven
/// now?") only ever reads the newest, and a run per commit for a hundred tests would grow
/// without bound. It is a value first and a file second — [`Ledger::load`] and
/// [`Ledger::save`] move it to and from [`LEDGER_PATH`] under a repository root, and a
/// repository with no file has an empty one rather than an error.
///
/// ```
/// # use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
/// # fn execution(name: &str, outcome: Outcome) -> Execution {
/// #     Execution {
/// #         test: format!("suite:{name}"), runner: Runner::Suite,
/// #         source: format!("test/cases/{name}.sh"), outcome, seconds: 1,
/// #         commit: "0".repeat(40), working_tree: "clean".into(), digest: "sha256:0".into(),
/// #         at: "2026-09-11T00:00:00Z".into(), origin: Origin::Local,
/// #         command: format!("bash test/run.sh {name}"),
/// #     }
/// # }
/// use majordomus_cli::evidence::ledger::{Ledger, LEDGER_PATH, LEDGER_VERSION};
///
/// let mut ledger = Ledger::empty();
/// assert_eq!(ledger.version, LEDGER_VERSION);
/// assert_eq!(LEDGER_PATH, ".ai/repo/evidence/ledger.json");
///
/// ledger.merge([execution("07_scope", Outcome::Pass)]);
/// assert_eq!(ledger.executions.len(), 1);
/// assert_eq!(ledger.latest("suite:07_scope").unwrap().outcome, Outcome::Pass);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// The shape's version.
    pub version: u32,
    /// The executions, one per test, ordered by test id so the file diffs cleanly.
    pub executions: Vec<Execution>,
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger {
            version: LEDGER_VERSION,
            executions: Vec::new(),
        }
    }
}

impl Ledger {
    /// An empty ledger — what a repository that has recorded nothing has.
    ///
    /// Indistinguishable from a missing file everywhere it matters: [`Ledger::load`]
    /// answers one for the other, [`Ledger::summary`] reports `present: false` for both,
    /// and both make every claim read `not run`, which is the honest answer in either
    /// case. Only [`Ledger::present`], which asks the filesystem, tells them apart.
    ///
    /// ```
    /// use majordomus_cli::evidence::ledger::{Ledger, LEDGER_VERSION};
    ///
    /// let ledger = Ledger::empty();
    /// assert!(ledger.executions.is_empty());
    /// assert_eq!(ledger.version, LEDGER_VERSION, "it still declares the shape it is");
    /// assert!(!ledger.summary().present, "nothing recorded is not a present ledger");
    /// ```
    pub fn empty() -> Ledger {
        Ledger::default()
    }

    /// Read the ledger of a repository. A repository with no ledger has an empty one; that
    /// is not an error, and the summary says which it was.
    ///
    /// A ledger declaring a version this executable does not know is refused rather than
    /// reinterpreted: an absent ledger reports `not run`, and a misread one could report a
    /// pass that was never recorded.
    ///
    /// ```
    /// use majordomus_cli::evidence::ledger::{Ledger, LEDGER_PATH};
    ///
    /// let root = tempfile::tempdir().unwrap();
    /// assert_eq!(Ledger::load(root.path()).unwrap(), Ledger::empty());
    ///
    /// std::fs::create_dir_all(root.path().join(".ai/repo/evidence")).unwrap();
    /// std::fs::write(
    ///     root.path().join(LEDGER_PATH),
    ///     r#"{"version": 99, "executions": []}"#,
    /// )
    /// .unwrap();
    /// let refused = Ledger::load(root.path()).unwrap_err().to_string();
    /// assert!(refused.contains("version 99"), "{refused}");
    /// ```
    pub fn load(root: &Path) -> Result<Ledger> {
        let path = root.join(LEDGER_PATH);
        if !path.exists() {
            return Ok(Ledger::empty());
        }
        let text = std::fs::read_to_string(&path).map_err(Error::Transport)?;
        let ledger: Ledger = serde_json::from_str(&text).map_err(|e| Error::InvalidSurface {
            surface: "evidence".into(),
            reason: format!("{LEDGER_PATH} is not a ledger this version can read: {e}"),
        })?;
        if ledger.version != LEDGER_VERSION {
            return Err(Error::InvalidSurface {
                surface: "evidence".into(),
                reason: format!(
                    "{LEDGER_PATH} declares version {} and this executable reads version \
                     {LEDGER_VERSION}; a ledger read under the wrong shape could report a pass \
                     that was never recorded",
                    ledger.version
                ),
            });
        }
        Ok(ledger)
    }

    /// Whether the file is there at all, as opposed to there and empty.
    ///
    /// ```
    /// use majordomus_cli::evidence::Ledger;
    ///
    /// let root = tempfile::tempdir().unwrap();
    /// assert!(!Ledger::present(root.path()));
    ///
    /// Ledger::empty().save(root.path()).unwrap();
    /// assert!(Ledger::present(root.path()), "written and empty is still written");
    /// ```
    pub fn present(root: &Path) -> bool {
        root.join(LEDGER_PATH).exists()
    }

    /// The latest recorded execution of one test, by [`super::TestId::as_string`].
    ///
    /// `None` is the answer for a test no run has ever recorded, which the report renders
    /// as `not run` rather than as a failure.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
    /// # fn execution(name: &str, outcome: Outcome) -> Execution {
    /// #     Execution {
    /// #         test: format!("suite:{name}"), runner: Runner::Suite,
    /// #         source: format!("test/cases/{name}.sh"), outcome, seconds: 1,
    /// #         commit: "0".repeat(40), working_tree: "clean".into(), digest: "sha256:0".into(),
    /// #         at: "2026-09-11T00:00:00Z".into(), origin: Origin::Local,
    /// #         command: format!("bash test/run.sh {name}"),
    /// #     }
    /// # }
    /// use majordomus_cli::evidence::Ledger;
    ///
    /// let mut ledger = Ledger::empty();
    /// ledger.merge([execution("07_scope", Outcome::Fail)]);
    ///
    /// assert_eq!(ledger.latest("suite:07_scope").unwrap().outcome, Outcome::Fail);
    /// assert!(ledger.latest("suite:99_ghost").is_none(), "never run, never recorded");
    /// ```
    pub fn latest(&self, test: &str) -> Option<&Execution> {
        self.executions.iter().find(|e| e.test == test)
    }

    /// Merge executions into the ledger: each one replaces the entry for its test and
    /// leaves every other alone.
    ///
    /// A partial run therefore updates only what it ran, which is what makes `bash
    /// test/run.sh 84_distribution_model` worth recording. The alternative — a whole-ledger
    /// write per run — would silently delete the evidence for every test the run did not
    /// include, turning a one-case run into a repository that has proven one thing.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
    /// # fn execution(name: &str, outcome: Outcome) -> Execution {
    /// #     Execution {
    /// #         test: format!("suite:{name}"), runner: Runner::Suite,
    /// #         source: format!("test/cases/{name}.sh"), outcome, seconds: 1,
    /// #         commit: "0".repeat(40), working_tree: "clean".into(), digest: "sha256:0".into(),
    /// #         at: "2026-09-11T00:00:00Z".into(), origin: Origin::Local,
    /// #         command: format!("bash test/run.sh {name}"),
    /// #     }
    /// # }
    /// use majordomus_cli::evidence::Ledger;
    ///
    /// let mut ledger = Ledger::empty();
    /// ledger.merge([
    ///     execution("07_scope", Outcome::Pass),
    ///     execution("08_other", Outcome::Pass),
    /// ]);
    ///
    /// // a second run of one case replaces that case and touches nothing else
    /// assert_eq!(ledger.merge([execution("07_scope", Outcome::Fail)]), 1);
    /// assert_eq!(ledger.executions.len(), 2, "it replaced rather than appended");
    /// assert_eq!(ledger.latest("suite:07_scope").unwrap().outcome, Outcome::Fail);
    /// assert_eq!(
    ///     ledger.latest("suite:08_other").unwrap().outcome,
    ///     Outcome::Pass,
    ///     "recording one test must never erase another test's evidence",
    /// );
    /// ```
    pub fn merge(&mut self, executions: impl IntoIterator<Item = Execution>) -> usize {
        let mut n = 0;
        for e in executions {
            match self.executions.iter_mut().find(|x| x.test == e.test) {
                Some(slot) => *slot = e,
                None => self.executions.push(e),
            }
            n += 1;
        }
        crate::order::canonical(&mut self.executions);
        n
    }

    /// Write the ledger, creating its directory. Trailing newline, two-space indent: a
    /// tracked file a person reads in a diff.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
    /// # fn execution(name: &str, outcome: Outcome) -> Execution {
    /// #     Execution {
    /// #         test: format!("suite:{name}"), runner: Runner::Suite,
    /// #         source: format!("test/cases/{name}.sh"), outcome, seconds: 1,
    /// #         commit: "0".repeat(40), working_tree: "clean".into(), digest: "sha256:0".into(),
    /// #         at: "2026-09-11T00:00:00Z".into(), origin: Origin::Local,
    /// #         command: format!("bash test/run.sh {name}"),
    /// #     }
    /// # }
    /// use majordomus_cli::evidence::ledger::{Ledger, LEDGER_PATH};
    ///
    /// let root = tempfile::tempdir().unwrap();
    /// let mut ledger = Ledger::empty();
    /// ledger.merge([execution("07_scope", Outcome::Pass)]);
    /// ledger.save(root.path()).unwrap();
    ///
    /// let text = std::fs::read_to_string(root.path().join(LEDGER_PATH)).unwrap();
    /// assert!(text.ends_with('\n'), "a tracked file ends in a newline");
    /// assert!(text.contains("\n  \"version\": 1"), "two-space indent: {text}");
    /// assert_eq!(Ledger::load(root.path()).unwrap(), ledger);
    /// ```
    pub fn save(&self, root: &Path) -> Result<()> {
        let path = root.join(LEDGER_PATH);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(Error::Transport)?;
        }
        let mut text = serde_json::to_string_pretty(self).map_err(|e| Error::InvalidSurface {
            surface: "evidence".into(),
            reason: e.to_string(),
        })?;
        text.push('\n');
        std::fs::write(&path, text).map_err(Error::Transport)
    }

    /// What the ledger is, for a report that has to say where its evidence came from:
    /// where it lives, how much it holds, how new the newest of it is, and every distinct
    /// commit its executions were recorded against.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
    /// # fn execution(name: &str, outcome: Outcome) -> Execution {
    /// #     Execution {
    /// #         test: format!("suite:{name}"), runner: Runner::Suite,
    /// #         source: format!("test/cases/{name}.sh"), outcome, seconds: 1,
    /// #         commit: "0".repeat(40), working_tree: "clean".into(), digest: "sha256:0".into(),
    /// #         at: "2026-09-11T00:00:00Z".into(), origin: Origin::Local,
    /// #         command: format!("bash test/run.sh {name}"),
    /// #     }
    /// # }
    /// use majordomus_cli::evidence::ledger::{Ledger, LEDGER_PATH};
    ///
    /// let mut ledger = Ledger::empty();
    /// ledger.merge([
    ///     execution("07_scope", Outcome::Pass),
    ///     execution("08_other", Outcome::Fail),
    /// ]);
    ///
    /// let summary = ledger.summary();
    /// assert_eq!(summary.path, LEDGER_PATH);
    /// assert_eq!(summary.executions, 2);
    /// assert_eq!(summary.newest.as_deref(), Some("2026-09-11T00:00:00Z"));
    /// assert_eq!(summary.commits, ["0".repeat(40)], "distinct commits, deduplicated");
    /// assert!(!Ledger::empty().summary().present);
    /// ```
    pub fn summary(&self) -> LedgerSummary {
        // the BTreeSet is already in order; collecting it is the ordering
        let commits: Vec<String> = self
            .executions
            .iter()
            .map(|e| e.commit.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        LedgerSummary {
            path: LEDGER_PATH.to_string(),
            present: !self.executions.is_empty(),
            executions: self.executions.len(),
            newest: self.executions.iter().map(|e| e.at.clone()).max(),
            commits,
        }
    }

    /// How many executions there are of each outcome, for a one-line summary. An outcome
    /// nothing recorded is absent from the map rather than present as a zero.
    ///
    /// ```
    /// # use majordomus_cli::evidence::{Execution, Origin, Outcome, Runner};
    /// # fn execution(name: &str, outcome: Outcome) -> Execution {
    /// #     Execution {
    /// #         test: format!("suite:{name}"), runner: Runner::Suite,
    /// #         source: format!("test/cases/{name}.sh"), outcome, seconds: 1,
    /// #         commit: "0".repeat(40), working_tree: "clean".into(), digest: "sha256:0".into(),
    /// #         at: "2026-09-11T00:00:00Z".into(), origin: Origin::Local,
    /// #         command: format!("bash test/run.sh {name}"),
    /// #     }
    /// # }
    /// use majordomus_cli::evidence::Ledger;
    ///
    /// let mut ledger = Ledger::empty();
    /// ledger.merge([
    ///     execution("07_scope", Outcome::Pass),
    ///     execution("08_other", Outcome::Fail),
    ///     execution("09_third", Outcome::Pass),
    /// ]);
    ///
    /// let counts = ledger.by_outcome();
    /// assert_eq!(counts.get("pass"), Some(&2));
    /// assert_eq!(counts.get("fail"), Some(&1));
    /// assert_eq!(counts.get("skip"), None, "an outcome nothing recorded is absent");
    /// ```
    pub fn by_outcome(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for e in &self.executions {
            let k = serde_json::to_value(e.outcome)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "unknown".into());
            *m.entry(k).or_insert(0) += 1;
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{Origin, Outcome, Runner};

    fn execution(test: &str, outcome: Outcome) -> Execution {
        Execution {
            test: test.into(),
            runner: Runner::Suite,
            source: format!("test/cases/{test}.sh"),
            outcome,
            seconds: 1,
            commit: "0".repeat(40),
            working_tree: "clean".into(),
            digest: "sha256:0".into(),
            at: "2026-09-11T00:00:00Z".into(),
            origin: Origin::Local,
            command: format!("bash test/run.sh {test}"),
        }
    }

    /// The property the whole recorder depends on: recording one test leaves every other
    /// test's evidence exactly as it was. Without it, running one case would erase the
    /// repository's proof of everything else.
    #[test]
    fn merging_one_test_leaves_the_others_untouched() {
        let mut l = Ledger::empty();
        l.merge([execution("a", Outcome::Pass), execution("b", Outcome::Pass)]);
        assert_eq!(l.executions.len(), 2);

        l.merge([execution("a", Outcome::Fail)]);
        assert_eq!(
            l.executions.len(),
            2,
            "merging replaced rather than appended"
        );
        assert_eq!(l.latest("a").unwrap().outcome, Outcome::Fail);
        assert_eq!(
            l.latest("b").unwrap().outcome,
            Outcome::Pass,
            "recording one test destroyed another's evidence"
        );
        assert!(l.latest("c").is_none());
    }

    /// Ordered by test id, so that two runs of the same set produce the same file and a
    /// diff shows what changed rather than what moved.
    #[test]
    fn the_file_is_ordered_by_test_so_it_diffs_cleanly() {
        let mut l = Ledger::empty();
        l.merge([
            execution("z", Outcome::Pass),
            execution("m", Outcome::Pass),
            execution("a", Outcome::Pass),
        ]);
        let ids: Vec<&str> = l.executions.iter().map(|e| e.test.as_str()).collect();
        assert_eq!(ids, vec!["a", "m", "z"]);
    }

    #[test]
    fn a_round_trip_through_the_file_preserves_every_execution() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = Ledger::empty();
        l.merge([execution("a", Outcome::Pass), execution("b", Outcome::Fail)]);

        assert!(!Ledger::present(dir.path()));
        assert_eq!(Ledger::load(dir.path()).unwrap(), Ledger::empty());

        l.save(dir.path()).unwrap();
        assert!(Ledger::present(dir.path()));
        assert_eq!(Ledger::load(dir.path()).unwrap(), l);

        let s = l.summary();
        assert_eq!(s.executions, 2);
        assert!(s.present);
        assert_eq!(s.path, LEDGER_PATH);
        assert_eq!(s.commits.len(), 1);
        assert_eq!(l.by_outcome().get("pass"), Some(&1));
        assert_eq!(l.by_outcome().get("fail"), Some(&1));
    }

    /// A ledger written by a future version is refused, not reinterpreted. A reader that
    /// guessed could report a pass that was never recorded, which is the one failure this
    /// subsystem must not have.
    #[test]
    fn a_ledger_of_an_unknown_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai/repo/evidence")).unwrap();
        std::fs::write(
            dir.path().join(LEDGER_PATH),
            r#"{"version": 99, "executions": []}"#,
        )
        .unwrap();
        let err = Ledger::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("version 99"), "{err}");

        std::fs::write(dir.path().join(LEDGER_PATH), "not json at all").unwrap();
        assert!(Ledger::load(dir.path()).is_err());
    }

    /// A missing ledger and an empty one both mean "nothing is recorded". Neither may be
    /// an error, because a fresh clone has the first and a repository mid-adoption has the
    /// second, and both must report every claim as `not run` rather than refusing to
    /// answer.
    #[test]
    fn a_repository_that_recorded_nothing_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let l = Ledger::load(dir.path()).unwrap();
        assert!(l.executions.is_empty());
        assert!(!l.summary().present);
        assert_eq!(l.summary().executions, 0);
        assert!(l.summary().newest.is_none());
    }
}
