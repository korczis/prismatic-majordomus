//! The recorder: turning a run that happened into evidence that lasts.
//!
//! The runs already write their results down. `test/run.sh` writes a TSV row per case when
//! `MJ_TEST_REPORT` is set — name, result, seconds, phase — and CI already sets it.
//! `cargo test` prints a `Running <binary>` line and a `test result:` line per test binary.
//! Neither is durable, neither carries a commit, and neither is joined to anything.
//!
//! This module reads what those runs wrote, stamps each result with the provenance the run
//! itself did not record (which commit, which tree, which digest, when, from where) and
//! merges it into the ledger.
//!
//! # It records, it does not decide
//!
//! A case that failed is a case the runner said failed. Nothing here re-judges a result,
//! and nothing here can produce one: a test id that appears in no report is simply not
//! recorded, and the report will say `not run` for it, which is true.
//!
//! # Why the commit is taken here rather than by the runner
//!
//! Because the runner is a shell script that runs in a disposable temporary repository and
//! genuinely does not know which checkout it was invoked from. Taking the commit at record
//! time is taking it from the tree the run was made against, which is the same thing,
//! provided the recording happens in that tree. A dirty tree is recorded as dirty for
//! exactly this reason: it is the one case where the commit does not describe what ran.
//!
//! # A run, recorded
//!
//! A repository with one case, a report the runner wrote, and the execution that comes out
//! of joining the two:
//!
//! ```
//! use majordomus_cli::evidence::{record, Ledger, Origin, Outcome, RecordRequest};
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
//! std::fs::write(root.path().join("test/cases/07_scope.sh"), "echo scope\n").unwrap();
//! git(&["add", "-A"]);
//! git(&["commit", "-qm", "init"]);
//!
//! // what `test/run.sh` already writes when `MJ_TEST_REPORT` is set; kept outside the
//! // work tree, so the recorded tree state describes the run and not the report
//! let reports = tempfile::tempdir().unwrap();
//! let tsv = reports.path().join("run.tsv");
//! std::fs::write(&tsv, "07_scope\tok\t12\tparallel\n").unwrap();
//!
//! let outcome = record(
//!     root.path(),
//!     &RecordRequest { suite: Some(tsv), crate_output: None, origin: Origin::Ci },
//! )
//! .unwrap();
//! assert_eq!(outcome.recorded, 1);
//! assert_eq!(outcome.passed, 1);
//! assert_eq!(outcome.commit.len(), 40, "the provenance the run did not record itself");
//! assert_eq!(outcome.working_tree, "clean");
//!
//! // and it is in the ledger, stamped, with the command that produces it again
//! let ledger = Ledger::load(root.path()).unwrap();
//! let execution = ledger.latest("suite:07_scope").unwrap();
//! assert_eq!(execution.outcome, Outcome::Pass);
//! assert_eq!(execution.seconds, 12);
//! assert_eq!(execution.origin, Origin::Ci);
//! assert_eq!(execution.command, "bash test/run.sh 07_scope");
//! assert_eq!(execution.digest_matches(root.path()), Some(true));
//! ```

use std::path::Path;

use super::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use crate::error::{Error, Result};

/// What to record, and where from.
///
/// Both sources are optional and independent — a run of the suite, a run of the crate, or
/// one invocation recording both — but a request that names neither is refused rather than
/// recording an empty run as if nothing had failed.
///
/// ```
/// use majordomus_cli::evidence::{record, Origin, RecordRequest};
///
/// let nothing = RecordRequest { suite: None, crate_output: None, origin: Origin::Local };
/// let root = tempfile::tempdir().unwrap();
/// let refused = record(root.path(), &nothing).unwrap_err().to_string();
/// assert!(refused.contains("nothing to record"), "{refused}");
///
/// let suite_run = RecordRequest {
///     suite: Some("tmp/run.tsv".into()),
///     crate_output: None,
///     origin: Origin::Ci,
/// };
/// assert_eq!(suite_run.origin, Origin::Ci);
/// assert_eq!(suite_run.suite.as_deref(), Some(std::path::Path::new("tmp/run.tsv")));
/// ```
#[derive(Debug, Clone)]
pub struct RecordRequest {
    /// The runner's TSV report, when a suite run is being recorded.
    pub suite: Option<std::path::PathBuf>,
    /// `cargo test`'s output, when a crate run is being recorded.
    pub crate_output: Option<std::path::PathBuf>,
    /// Where the run happened.
    pub origin: Origin,
}

/// What a recording did: how much of the run reached the ledger, how much of it passed,
/// the provenance every execution was stamped with, and what the run named that this
/// repository does not have.
///
/// The last is the one worth reading. A report naming a test no runner here owns is
/// counted in [`RecordOutcome::unknown`] and recorded for nobody, because an execution of
/// a test that does not exist is evidence for nothing — and dropping it silently would
/// hide a runner and a matrix that have drifted apart.
///
/// ```
/// # use std::process::Command;
/// # let root = tempfile::tempdir().unwrap();
/// # let git = |args: &[&str]| {
/// #     Command::new("git").arg("-C").arg(root.path()).args(args).output().unwrap()
/// # };
/// # git(&["init", "-q"]);
/// # git(&["config", "user.email", "t@example.com"]);
/// # git(&["config", "user.name", "t"]);
/// # std::fs::create_dir_all(root.path().join("test/cases")).unwrap();
/// # std::fs::write(root.path().join("test/cases/07_scope.sh"), "echo scope\n").unwrap();
/// # std::fs::write(root.path().join("test/cases/08_other.sh"), "echo other\n").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-qm", "init"]);
/// use majordomus_cli::evidence::{record, Ledger, Origin, RecordOutcome, RecordRequest};
///
/// let reports = tempfile::tempdir().unwrap();
/// let tsv = reports.path().join("run.tsv");
/// std::fs::write(&tsv, "07_scope\tok\t1\tparallel\n99_ghost\tok\t1\tparallel\n").unwrap();
///
/// let outcome: RecordOutcome = record(
///     root.path(),
///     &RecordRequest { suite: Some(tsv), crate_output: None, origin: Origin::Local },
/// )
/// .unwrap();
/// assert_eq!(outcome.recorded, 1, "only the case this repository actually has");
/// assert_eq!(outcome.passed, 1);
/// assert_eq!(outcome.working_tree, "clean");
/// assert_eq!(outcome.unknown, ["suite:99_ghost"], "named here, recorded for nobody");
/// assert!(Ledger::load(root.path()).unwrap().latest("suite:99_ghost").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordOutcome {
    /// How many executions were written.
    pub recorded: usize,
    /// How many of them passed.
    pub passed: usize,
    /// The commit they were recorded against.
    pub commit: String,
    /// The tree's state at the time: `clean`, `dirty` or `unknown`.
    pub working_tree: String,
    /// Reports the run named that no runner in this repository owns; recorded for nobody
    /// and named here rather than dropped.
    pub unknown: Vec<String>,
}

/// The instant, RFC 3339, UTC, to whole seconds.
///
/// Written out rather than taken from a date crate: the crate has no time dependency, and
/// one timestamp does not justify one. This is the civil-date algorithm, which is exact.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // days since 1970-01-01 to a civil date (Howard Hinnant's civil_from_days)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// One `Running tests/<name>.rs` / `test result:` pair per test binary, joined.
///
/// `cargo test`'s machine format is still unstable, so this reads what every cargo prints.
/// The `Running` line names the binary the following result belongs to; without it a run
/// of forty binaries is forty anonymous result lines, which is how a per-test-binary
/// outcome becomes an untraceable total.
///
/// ```
/// use majordomus_cli::evidence::parse_crate_binaries;
/// let out = "   Running tests/why.rs (target/debug/deps/why-1a2b)\n\
///             test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
///             finished in 0.41s\n\
///                Running tests/product.rs (target/debug/deps/product-3c4d)\n\
///             test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured\n";
/// let got = parse_crate_binaries(out);
/// assert_eq!(got.len(), 2);
/// assert_eq!(got[0].0, "why");
/// assert!(got[0].1.proves());
/// assert_eq!(got[1].0, "product");
/// assert!(!got[1].1.proves());
/// ```
pub fn parse_crate_binaries(text: &str) -> Vec<(String, Outcome, u64)> {
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Running ") {
            // `tests/why.rs (target/debug/deps/why-1a2b)` — the source path is the name,
            // never the hashed binary, which changes between builds
            let path = rest.split_whitespace().next().unwrap_or("");
            pending = path
                .strip_prefix("tests/")
                .and_then(|p| p.strip_suffix(".rs"))
                .map(str::to_string);
            continue;
        }
        if let Some(rest) = t.strip_prefix("test result: ") {
            let Some(name) = pending.take() else { continue };
            let outcome = if rest.starts_with("ok") {
                Outcome::Pass
            } else {
                Outcome::Fail
            };
            let seconds = rest
                .split("finished in ")
                .nth(1)
                .and_then(|s| s.trim_end_matches('s').trim().parse::<f64>().ok())
                .map(|f| f.round() as u64)
                .unwrap_or(0);
            out.push((name, outcome, seconds));
        }
    }
    out
}

/// The runner's TSV: `name <TAB> result <TAB> seconds <TAB> phase`, one case per line.
///
/// A malformed line is refused rather than skipped, for the same reason the renderer
/// refuses one: a recording that silently dropped a case would leave that case's claim
/// reporting `not run` after a run that did run it, which is a lie in the safe direction
/// and still a lie.
fn parse_suite(text: &str) -> Result<Vec<(String, Outcome, u64)>> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(Error::InvalidSurface {
                surface: "evidence".into(),
                reason: format!(
                    "line {} of the run report has {} field(s); it is \
                     name<TAB>result<TAB>seconds<TAB>phase",
                    n + 1,
                    f.len()
                ),
            });
        }
        out.push((
            f[0].to_string(),
            Outcome::parse(f[1]),
            f[2].parse().unwrap_or(0),
        ));
    }
    Ok(out)
}

/// Read the runs a request names, stamp each result with provenance, and merge into the
/// ledger of `root`.
///
/// It decides nothing: a case that failed is a case the runner said failed. It refuses
/// three things — a request naming no run at all, a tree with no commit to stamp, and a
/// malformed report, which is refused rather than partly read.
///
/// Because it merges, a later partial run updates only what it ran:
///
/// ```
/// # use std::process::Command;
/// # let root = tempfile::tempdir().unwrap();
/// # let git = |args: &[&str]| {
/// #     Command::new("git").arg("-C").arg(root.path()).args(args).output().unwrap()
/// # };
/// # git(&["init", "-q"]);
/// # git(&["config", "user.email", "t@example.com"]);
/// # git(&["config", "user.name", "t"]);
/// # std::fs::create_dir_all(root.path().join("test/cases")).unwrap();
/// # std::fs::write(root.path().join("test/cases/07_scope.sh"), "echo scope\n").unwrap();
/// # std::fs::write(root.path().join("test/cases/08_other.sh"), "echo other\n").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-qm", "init"]);
/// use majordomus_cli::evidence::{record, Ledger, Origin, Outcome, RecordRequest};
///
/// let reports = tempfile::tempdir().unwrap();
/// let suite = |name: &str, rows: &str| {
///     let p = reports.path().join(name);
///     std::fs::write(&p, rows).unwrap();
///     RecordRequest { suite: Some(p), crate_output: None, origin: Origin::Local }
/// };
///
/// let both = suite("all.tsv", "07_scope\tok\t1\tparallel\n08_other\tok\t2\tparallel\n");
/// assert_eq!(record(root.path(), &both).unwrap().recorded, 2);
///
/// let again = suite("one.tsv", "07_scope\tFAIL\t9\tserial\n");
/// assert_eq!(record(root.path(), &again).unwrap().recorded, 1);
///
/// let ledger = Ledger::load(root.path()).unwrap();
/// assert_eq!(ledger.latest("suite:07_scope").unwrap().outcome, Outcome::Fail);
/// assert_eq!(
///     ledger.latest("suite:08_other").unwrap().outcome,
///     Outcome::Pass,
///     "the second run must not erase evidence it never measured",
/// );
///
/// // a malformed line is refused, not skipped: an under-recorded run would report
/// // `not run` for a test that ran
/// let short = suite("bad.tsv", "07_scope\tok\t12\n");
/// let refused = record(root.path(), &short).unwrap_err().to_string();
/// assert!(refused.contains("field(s)"), "{refused}");
/// ```
pub fn record(root: &Path, req: &RecordRequest) -> Result<RecordOutcome> {
    if req.suite.is_none() && req.crate_output.is_none() {
        return Err(Error::InvalidSurface {
            surface: "evidence".into(),
            reason: "nothing to record: give --suite, --crate-output, or both".into(),
        });
    }

    let git = crate::git::inspect(root);
    let (commit, working_tree) = match &git {
        crate::git::GitState::Available(i) => (
            i.head.clone().unwrap_or_else(|| "unknown".into()),
            i.working_tree.clone(),
        ),
        crate::git::GitState::Unavailable { .. } => ("unknown".into(), "unknown".into()),
    };
    if commit == "unknown" {
        return Err(Error::InvalidSurface {
            surface: "evidence".into(),
            reason: "this is not a git work tree with a commit, so a run recorded here would \
                     carry no provenance and prove nothing"
                .into(),
        });
    }
    let at = now_rfc3339();

    let mut results: Vec<(TestId, Outcome, u64)> = Vec::new();
    let mut unknown = Vec::new();

    if let Some(p) = &req.suite {
        let text = std::fs::read_to_string(p).map_err(Error::Transport)?;
        for (name, outcome, seconds) in parse_suite(&text)? {
            // the runner names a case by its stem, which is exactly the suite test id
            let id = TestId {
                runner: Runner::Suite,
                name: name.clone(),
            };
            if root.join(id.source()).exists() {
                results.push((id, outcome, seconds));
            } else {
                unknown.push(format!("suite:{name}"));
            }
        }
    }

    if let Some(p) = &req.crate_output {
        let text = std::fs::read_to_string(p).map_err(Error::Transport)?;
        for (name, outcome, seconds) in parse_crate_binaries(&text) {
            let id = TestId {
                runner: Runner::Crate,
                name: name.clone(),
            };
            if root.join(id.source()).exists() {
                results.push((id, outcome, seconds));
            } else {
                unknown.push(format!("crate:{name}"));
            }
        }
    }

    let mut executions = Vec::with_capacity(results.len());
    let mut passed = 0;
    for (id, outcome, seconds) in results {
        if outcome.proves() {
            passed += 1;
        }
        let source = id.source();
        let digest = std::fs::read(root.join(&source))
            .map(|b| digest_of(&b))
            .unwrap_or_else(|_| "sha256:unreadable".into());
        executions.push(Execution {
            test: id.as_string(),
            runner: id.runner,
            source,
            outcome,
            seconds,
            commit: commit.clone(),
            working_tree: working_tree.clone(),
            digest,
            at: at.clone(),
            origin: req.origin,
            command: id.reproduce(),
        });
    }

    let mut ledger = Ledger::load(root)?;
    let recorded = ledger.merge(executions);
    // A run that recorded nothing writes nothing. Saving here would create a ledger
    // holding no executions out of a report that named only tests this repository does
    // not have — a file that says "evidence was recorded" where none was. The malformed
    // report above is refused before any write for the same reason, and the two paths
    // must not disagree about what an empty recording leaves behind.
    if recorded > 0 {
        ledger.save(root)?;
    }

    unknown.sort();
    unknown.dedup();
    Ok(RecordOutcome {
        recorded,
        passed,
        commit,
        working_tree,
        unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        git(d.path(), &["init", "-q"]);
        git(d.path(), &["config", "user.email", "t@example.com"]);
        git(d.path(), &["config", "user.name", "t"]);
        std::fs::create_dir_all(d.path().join("test/cases")).unwrap();
        std::fs::write(d.path().join("test/cases/07_scope.sh"), "echo scope\n").unwrap();
        std::fs::write(d.path().join("test/cases/08_other.sh"), "echo other\n").unwrap();
        git(d.path(), &["add", "-A"]);
        git(d.path(), &["commit", "-qm", "init"]);
        d
    }

    #[test]
    fn a_suite_report_becomes_executions_with_provenance() {
        let d = repo();
        // outside the repository: a report written inside it is an untracked file, and the
        // recorded `working_tree` would then say `dirty` about the report rather than about
        // anything the run measured
        let reports = tempfile::tempdir().unwrap();
        let tsv = reports.path().join("run.tsv");
        std::fs::write(
            &tsv,
            "07_scope\tok\t12\tparallel\n08_other\tFAIL\t3\texclusive\n",
        )
        .unwrap();

        let got = record(
            d.path(),
            &RecordRequest {
                suite: Some(tsv),
                crate_output: None,
                origin: Origin::Ci,
            },
        )
        .unwrap();
        assert_eq!(got.recorded, 2);
        assert_eq!(got.passed, 1);
        assert_eq!(got.commit.len(), 40);
        assert_eq!(got.working_tree, "clean");
        assert!(got.unknown.is_empty());

        let l = Ledger::load(d.path()).unwrap();
        let e = l.latest("suite:07_scope").unwrap();
        assert_eq!(e.outcome, Outcome::Pass);
        assert_eq!(e.seconds, 12);
        assert_eq!(e.origin, Origin::Ci);
        assert_eq!(e.command, "bash test/run.sh 07_scope");
        assert_eq!(e.source, "test/cases/07_scope.sh");
        assert_eq!(e.commit, got.commit);
        // the digest is over the test's own source, so a later edit is detectable
        assert_eq!(e.digest, digest_of(b"echo scope\n"));
        assert_eq!(e.digest_matches(d.path()), Some(true));

        std::fs::write(d.path().join("test/cases/07_scope.sh"), "echo changed\n").unwrap();
        assert_eq!(
            l.latest("suite:07_scope").unwrap().digest_matches(d.path()),
            Some(false),
            "an edited test must not still match the digest that was recorded"
        );
    }

    /// A report naming a case this repository does not have is named, not recorded. An
    /// execution of a test that does not exist would be evidence for nothing, and silently
    /// dropping it would hide a runner and a matrix that have diverged.
    #[test]
    fn a_result_for_a_test_that_does_not_exist_is_reported_not_recorded() {
        let d = repo();
        let tsv = d.path().join("run.tsv");
        std::fs::write(
            &tsv,
            "07_scope\tok\t1\tparallel\n99_ghost\tok\t1\tparallel\n",
        )
        .unwrap();
        let got = record(
            d.path(),
            &RecordRequest {
                suite: Some(tsv),
                crate_output: None,
                origin: Origin::Local,
            },
        )
        .unwrap();
        assert_eq!(got.recorded, 1);
        assert_eq!(got.unknown, vec!["suite:99_ghost"]);
        assert!(Ledger::load(d.path())
            .unwrap()
            .latest("suite:99_ghost")
            .is_none());
    }

    /// Recording twice replaces, and a second partial run does not erase the first run's
    /// evidence for tests it did not include.
    #[test]
    fn a_later_partial_run_updates_only_what_it_ran() {
        let d = repo();
        let all = d.path().join("all.tsv");
        std::fs::write(
            &all,
            "07_scope\tok\t1\tparallel\n08_other\tok\t2\tparallel\n",
        )
        .unwrap();
        record(
            d.path(),
            &RecordRequest {
                suite: Some(all),
                crate_output: None,
                origin: Origin::Local,
            },
        )
        .unwrap();

        let one = d.path().join("one.tsv");
        std::fs::write(&one, "07_scope\tFAIL\t9\tserial\n").unwrap();
        record(
            d.path(),
            &RecordRequest {
                suite: Some(one),
                crate_output: None,
                origin: Origin::Local,
            },
        )
        .unwrap();

        let l = Ledger::load(d.path()).unwrap();
        assert_eq!(l.executions.len(), 2);
        assert_eq!(l.latest("suite:07_scope").unwrap().outcome, Outcome::Fail);
        assert_eq!(l.latest("suite:07_scope").unwrap().seconds, 9);
        assert_eq!(
            l.latest("suite:08_other").unwrap().outcome,
            Outcome::Pass,
            "the second run erased evidence it never measured"
        );
    }

    /// A malformed report is refused. Skipping the bad line would under-record a run and
    /// leave a claim reporting `not run` for a test that ran.
    #[test]
    fn a_malformed_report_is_refused_rather_than_partly_read() {
        let d = repo();
        let tsv = d.path().join("run.tsv");
        std::fs::write(&tsv, "07_scope\tok\t12\n").unwrap();
        let err = record(
            d.path(),
            &RecordRequest {
                suite: Some(tsv),
                crate_output: None,
                origin: Origin::Local,
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("field(s)"), "{err}");
        assert!(
            !Ledger::present(d.path()),
            "a refused recording wrote a ledger"
        );
    }

    /// A report whose every result names a test this repository does not have records
    /// nothing, and must leave nothing behind. A ledger holding no executions is a file
    /// that says evidence was recorded where none was, and the malformed-report path
    /// already refuses before any write — the two must not disagree.
    #[test]
    fn a_recording_that_recorded_nothing_writes_no_ledger() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = reports.path().join("run.tsv");
        std::fs::write(
            &tsv,
            "99_ghost\tok\t1\tparallel\n98_gone\tok\t1\tparallel\n",
        )
        .unwrap();
        let got = record(
            d.path(),
            &RecordRequest {
                suite: Some(tsv),
                crate_output: None,
                origin: Origin::Local,
            },
        )
        .unwrap();
        assert_eq!(got.recorded, 0);
        assert_eq!(got.unknown, vec!["suite:98_gone", "suite:99_ghost"]);
        assert!(
            !Ledger::present(d.path()),
            "a recording that recorded nothing created a ledger"
        );
    }

    #[test]
    fn recording_nothing_is_refused() {
        let d = repo();
        assert!(record(
            d.path(),
            &RecordRequest {
                suite: None,
                crate_output: None,
                origin: Origin::Local
            }
        )
        .is_err());
    }

    /// A tree that is not a git work tree cannot produce evidence: there is no commit to
    /// stamp, and an execution with no commit is the anonymous green the whole subsystem
    /// exists to refuse.
    #[test]
    fn a_run_with_no_commit_to_name_is_refused() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("test/cases")).unwrap();
        std::fs::write(d.path().join("test/cases/07_scope.sh"), "x").unwrap();
        let tsv = d.path().join("run.tsv");
        std::fs::write(&tsv, "07_scope\tok\t1\tparallel\n").unwrap();
        let err = record(
            d.path(),
            &RecordRequest {
                suite: Some(tsv),
                crate_output: None,
                origin: Origin::Local,
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("provenance"), "{err}");
    }

    #[test]
    fn the_timestamp_is_rfc3339_utc() {
        let t = now_rfc3339();
        assert_eq!(t.len(), 20, "{t}");
        assert!(t.ends_with('Z'), "{t}");
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
        // the civil-date arithmetic must land in this decade, not in 1970 or 33658
        let year: i32 = t[..4].parse().unwrap();
        assert!((2020..2100).contains(&year), "{t}");
    }

    /// A `test result:` line with no `Running` line before it belongs to no binary and is
    /// dropped rather than attributed to the previous one.
    #[test]
    fn an_unattributed_crate_result_is_not_credited_to_the_last_binary() {
        let got = parse_crate_binaries(
            "test result: ok. 1 passed; 0 failed\n\
                Running tests/why.rs (target/debug/deps/why-1)\n\
             test result: ok. 2 passed; 0 failed; finished in 1.60s\n\
             test result: FAILED. 0 passed; 1 failed\n",
        );
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "why");
        assert_eq!(got[0].2, 2, "1.60s rounds to 2");
    }

    /// A unit-test binary of the crate itself (`Running unittests src/lib.rs`) is not an
    /// integration test a claim can name, and must not be recorded as one.
    #[test]
    fn the_crates_own_unit_test_binary_is_not_an_integration_test() {
        let got = parse_crate_binaries(
            "   Running unittests src/lib.rs (target/debug/deps/majordomus_cli-9)\n\
             test result: ok. 400 passed; 0 failed\n",
        );
        assert!(got.is_empty(), "{got:?}");
    }
}
