//! The recorder: turning a run that happened into evidence that lasts.
//!
//! The runs already write their results down. `test/run.sh` writes a TSV row per case when
//! `MJ_TEST_REPORT` is set — name, result, seconds, phase — and CI already sets it.
//! `cargo test` prints a `Running <binary>` line and a `test result:` line per test binary.
//! Neither is durable, neither carries a commit, and neither is joined to anything.
//!
//! This module reads what those runs wrote, carries into each result what the run measured
//! about its own checkout (which commit, which tree) with the digest, the time and the
//! origin, and merges it into the ledger.
//!
//! # It records, it does not decide
//!
//! A case that failed is a case the runner said failed. Nothing here re-judges a result,
//! and nothing here can produce one: a test id that appears in no report is simply not
//! recorded, and the report will say `not run` for it, which is true.
//!
//! # A crate binary states only what ran
//!
//! "Records what the runner said" needs care with `cargo test`, whose result line starts
//! with `ok` for a binary that ran nothing at all: every test ignored, or every test
//! filtered out of a run that named one. So [`read_crate_output`] reads the counts, not
//! only the word. A binary that ran no test, or only a filtered subset, is a skip; one with
//! a failed test is a failure whatever its word says; one whose result line carries no
//! count, or that printed no result line at all, is an error, because the harness could not
//! say what ran. Colour codes are stripped before any line is matched, so a coloured run and
//! a plain one read the same. What the output held that no claim can name yet, the crate's
//! own unit-test binary and its doctests, is listed in [`RecordOutcome::dropped`] rather
//! than ignored.
//!
//! # The run measures; the recorder carries
//!
//! The recorder does not measure its own checkout. A run measures the checkout it left with
//! `majordomus evidence stamp` ([`super::provenance::stamp`]): the commit, and the tree with
//! the evidence ledger ignored and the run's own untracked outputs excluded. The recorder
//! carries that measurement into every execution of the report it names, and refuses one of
//! another commit or of other bytes. A report recorded without a measurement carries the
//! tree `unknown`: the recorder does not vouch for a tree it did not see, and such a row
//! never reads `proven`. When the measured tree was clean, the digest is taken from the
//! commit, which holds the bytes the run executed.
//!
//! A stamp measures the checkout when it is taken, after the report was written, and not
//! the tree while the run executed: a run on a dirty tree that is cleaned before the stamp
//! reads clean.
//!
//! # A run, recorded
//!
//! A repository with one case, a report the runner wrote, and the execution that comes out
//! of joining the two:
//!
//! ```
//! use majordomus_cli::evidence::{
//!     record, stamp, EvidenceProducer, Ledger, Origin, Outcome, RecordRequest, StampRequest,
//! };
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
//! // the run measures its own checkout, naming its report
//! let stamp_req = StampRequest {
//!     producer: Some(EvidenceProducer::Suite),
//!     report: Some(tsv.clone()),
//!     ..StampRequest::default()
//! };
//! let measured = stamp(root.path(), &stamp_req).unwrap();
//!
//! let mut request = RecordRequest { suite: Some(tsv), ..RecordRequest::new(Origin::Ci) };
//! request.provenance.insert(EvidenceProducer::Suite, measured);
//! let outcome = record(root.path(), &request).unwrap();
//! assert_eq!(outcome.recorded, 1);
//! assert_eq!(outcome.passed, 1);
//! assert_eq!(outcome.commit.len(), 40);
//! assert_eq!(outcome.working_tree, "clean", "the tree the run's own measurement stated");
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

//! # Example
//!
//! The recorder reads what a run already wrote and stamps it with the provenance the run
//! did not carry. Parsing is separable from recording, which is what makes it testable.
//!
//! ```
//! use majordomus_cli::evidence::{parse_crate_binaries, Outcome};
//! let got = parse_crate_binaries("     Running tests/product.rs (target/debug/deps/product-1)\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n");
//! assert_eq!(got.len(), 1);
//! assert_eq!(got[0].1, Outcome::Pass);
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use super::coverage::{CommitId, EvidenceCoverage, EvidenceCoverageFloors};
use super::freshness::TreeState;
use super::ledger::{LedgerTarget, LEDGER_PATH, LOCAL_LEDGER_PATH};
use super::provenance::EvidenceProvenance;
use super::run::{
    run_id, EvidenceAbsent, EvidenceDropped, EvidenceProducer, EvidenceRunRecord,
    EvidenceRunTotals, RUN_RECORD_SCHEMA,
};
use super::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use crate::error::{Error, Result};

/// Why the crate's own unit-test binary is listed rather than recorded.
const UNIT_TESTS: &str =
    "the crate's own unit tests, which no claim can name until a runner records them one by one";
/// Why the crate's doctests are listed rather than recorded.
const DOC_TESTS: &str = "doctests, which no claim can name until a runner records them one by one";
/// Why a binary that is not under `tests/` is listed rather than recorded.
const NOT_AN_INTEGRATION_BINARY: &str = "not an integration test binary under tests/";
/// Why a result line with no binary before it is listed rather than recorded.
const UNATTRIBUTED: &str = "no Running line named the binary it belongs to";

/// What to record, and where from.
///
/// Both sources are optional and independent — a run of the suite, a run of the crate, or
/// one invocation recording both — but a request that names neither is refused rather than
/// recording an empty run as if nothing had failed.
///
/// ```
/// use majordomus_cli::evidence::{record, Origin, RecordRequest};
///
/// let nothing = RecordRequest::default();
/// let root = tempfile::tempdir().unwrap();
/// let refused = record(root.path(), &nothing).unwrap_err().to_string();
/// assert!(refused.contains("nothing to record"), "{refused}");
///
/// let suite_run =
///     RecordRequest { suite: Some("tmp/run.tsv".into()), ..RecordRequest::new(Origin::Ci) };
/// assert_eq!(suite_run.origin, Origin::Ci);
/// assert_eq!(suite_run.suite.as_deref(), Some(std::path::Path::new("tmp/run.tsv")));
/// ```
#[derive(Debug, Clone)]
/// # Example
///
/// ```
/// use majordomus_cli::evidence::{Origin, RecordRequest};
/// let r = RecordRequest::new(Origin::Ci);
/// assert_eq!(r.origin, Origin::Ci);
/// ```
pub struct RecordRequest {
    /// The runner's TSV report, when a suite run is being recorded.
    pub suite: Option<std::path::PathBuf>,
    /// `cargo test`'s output, when a crate run is being recorded.
    pub crate_output: Option<std::path::PathBuf>,
    /// Where the run happened.
    pub origin: Origin,
    /// The continuous-integration run to stamp every execution with, when there is one.
    pub run: Option<super::RunRef>,
    /// What each report's run measured about its checkout, by producer: values, not paths,
    /// so a runner can pass the measurement it just took.
    pub provenance: BTreeMap<EvidenceProducer, EvidenceProvenance>,
    /// A coverage summary (`scripts/rust-coverage --summary-json`), when one is recorded.
    pub coverage: Option<std::path::PathBuf>,
    /// The ledger the executions are merged into.
    pub ledger: LedgerTarget,
    /// Where to write the run record as well, when asked.
    pub run_record: Option<std::path::PathBuf>,
}

impl Default for RecordRequest {
    /// A local recording of nothing into the tracked ledger.
    fn default() -> RecordRequest {
        RecordRequest::new(Origin::Local)
    }
}

impl RecordRequest {
    /// A recording of nothing yet, from `origin`, into the tracked ledger: the base every
    /// request is built on, so that a field added later breaks no caller.
    ///
    /// ```
    /// use majordomus_cli::evidence::{LedgerTarget, Origin, RecordRequest};
    /// let r = RecordRequest { suite: Some("run.tsv".into()), ..RecordRequest::new(Origin::Ci) };
    /// assert_eq!((r.origin, r.ledger), (Origin::Ci, LedgerTarget::Repo));
    /// assert!(r.provenance.is_empty() && r.coverage.is_none() && r.run_record.is_none());
    /// ```
    pub fn new(origin: Origin) -> RecordRequest {
        RecordRequest {
            suite: None,
            crate_output: None,
            origin,
            run: None,
            provenance: BTreeMap::new(),
            coverage: None,
            ledger: LedgerTarget::Repo,
            run_record: None,
        }
    }
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
///     &RecordRequest { suite: Some(tsv), crate_output: None, ..RecordRequest::new(Origin::Local) },
/// )
/// .unwrap();
/// assert_eq!(outcome.recorded, 1, "only the case this repository actually has");
/// assert_eq!(outcome.passed, 1);
/// assert_eq!(outcome.working_tree, "unknown", "no measurement came with the report");
/// assert_eq!(outcome.run_record.unknown, outcome.unknown);
/// assert_eq!(outcome.unknown, ["suite:99_ghost"], "named here, recorded for nobody");
/// assert!(Ledger::load(root.path()).unwrap().latest("suite:99_ghost").is_none());
/// ```
#[derive(Debug, Clone, PartialEq)]
/// # Example
///
/// A crate run's recording: the integration binary a claim can name is recorded, and the
/// doctests, which ran and which no claim can name yet, are listed in
/// [`RecordOutcome::dropped`] rather than ignored.
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
/// # std::fs::create_dir_all(root.path().join("apps/majordomus-cli/tests")).unwrap();
/// # std::fs::write(root.path().join("apps/majordomus-cli/tests/why.rs"), "// why\n").unwrap();
/// # git(&["add", "-A"]);
/// # git(&["commit", "-qm", "init"]);
/// use majordomus_cli::evidence::{record, EvidenceProducer, Origin, RecordOutcome, RecordRequest};
///
/// let reports = tempfile::tempdir().unwrap();
/// let log = reports.path().join("crate.log");
/// std::fs::write(
///     &log,
///     "     Running tests/why.rs (target/debug/deps/why-1)\n\
///      test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
///      finished in 0.10s\n\
///         Doc-tests majordomus_cli\n\
///      test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
///      finished in 1.00s\n",
/// )
/// .unwrap();
///
/// let request =
///     RecordRequest { suite: None, crate_output: Some(log), ..RecordRequest::new(Origin::Local) };
/// let outcome: RecordOutcome = record(root.path(), &request).unwrap();
/// assert_eq!((outcome.recorded, outcome.passed), (1, 1), "the binary under tests/");
/// assert_eq!(outcome.dropped.len(), 1, "the doctests, listed");
/// assert_eq!(outcome.dropped[0].producer, EvidenceProducer::Crate);
/// assert_eq!(outcome.dropped[0].what, "doc-tests majordomus_cli");
/// ```
pub struct RecordOutcome {
    /// How many executions were written.
    pub recorded: usize,
    /// How many of them passed.
    pub passed: usize,
    /// The commit they were recorded against.
    pub commit: String,
    /// The weakest tree the reports' own measurements stated: `clean`, `dirty`, or
    /// `unknown` when a report came without one. Read from [`RecordOutcome::run_record`].
    pub working_tree: String,
    /// Reports the run named that no runner in this repository owns; recorded for nobody
    /// and named here rather than dropped.
    pub unknown: Vec<String>,
    /// What the reports held that no claim can name yet, in the order the reports held it:
    /// the crate's own unit-test binary, its doctests, a binary outside `tests/`, a result
    /// line no binary owned. Listed rather than ignored, so a run is never silently smaller
    /// than it was. The same list as the run record's.
    pub dropped: Vec<EvidenceDropped>,
    /// The recording as a typed run.
    pub run_record: EvidenceRunRecord,
}

/// The instant, RFC 3339, UTC, to whole seconds.
///
/// Written out rather than taken from a date crate: the crate has no time dependency, and
/// one timestamp does not justify one. This is the civil-date algorithm, which is exact.
pub(crate) fn now_rfc3339() -> String {
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

/// One integration test binary of a `cargo test` run, as its result line stated it.
///
/// `name` is the binary's source under `tests/` without the extension, never the hashed
/// file cargo built, which changes between builds. The counts are the result line's own
/// (a count the line did not carry is 0), and `outcome` is decided from them by the rule
/// [`read_crate_output`] states: a binary that ran nothing is not a pass.
///
/// ```
/// use majordomus_cli::evidence::{read_crate_output, CrateBinary, Outcome};
///
/// let read = read_crate_output(
///     "     Running tests/why.rs (target/debug/deps/why-1)\n\
///      test result: ok. 1 passed; 0 failed; 2 ignored; 0 measured; 4 filtered out; \
///      finished in 0.60s\n",
/// );
/// let why: &CrateBinary = &read.binaries[0];
/// assert_eq!(why.name, "why");
/// assert_eq!((why.passed, why.failed, why.ignored, why.filtered_out), (1, 0, 2, 4));
/// assert_eq!(why.seconds, 1, "0.60s rounds to the nearest second");
/// assert_eq!(why.outcome, Outcome::Skip, "a filtered subset is not the binary's proof");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateBinary {
    /// The binary's name: `tests/<name>.rs` without the directory and the extension.
    pub name: String,
    /// What the binary's result line, read by its counts, says happened.
    pub outcome: Outcome,
    /// `finished in` rounded to whole seconds; 0 when the line carried none.
    pub seconds: u64,
    /// Tests that ran and passed.
    pub passed: u64,
    /// Tests that ran and failed.
    pub failed: u64,
    /// Tests that were compiled in and not run.
    pub ignored: u64,
    /// Tests a name filter left out of this run.
    pub filtered_out: u64,
}

/// What `cargo test`'s output held: the integration binaries a claim can name, and what it
/// cannot, listed in [`CrateRead::dropped`] in the order the output held it.
///
/// ```
/// use majordomus_cli::evidence::{read_crate_output, CrateRead};
///
/// let read: CrateRead = read_crate_output(
///     "   Doc-tests majordomus_cli\n\
///      test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
///      finished in 2.00s\n",
/// );
/// assert!(read.binaries.is_empty(), "no claim can name a doctest yet");
/// assert_eq!(read.dropped[0].what, "doc-tests majordomus_cli");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrateRead {
    /// Each binary under `tests/` that the output named, in order, with its outcome.
    pub binaries: Vec<CrateBinary>,
    /// What the output held that no claim can name yet, with the reason for each.
    pub dropped: Vec<EvidenceDropped>,
}

/// The entry a `Running` or `Doc-tests` line opened, waiting for its result line.
enum Pending {
    /// An integration binary under `tests/`: a claim can name it.
    Binary(String),
    /// Anything else: listed, with why, when it closes.
    Dropped(String, &'static str),
}

impl CrateRead {
    /// Close the entry that was waiting for a result line and got none before the next
    /// `Running`, the next `Doc-tests` or the end of the output. A binary that started and
    /// never reported is an error, because the harness could not say what ran; anything
    /// else is listed as it is.
    fn close(&mut self, pending: Option<Pending>) {
        match pending {
            None => {}
            Some(Pending::Binary(name)) => self.binaries.push(CrateBinary {
                name,
                outcome: Outcome::Error,
                seconds: 0,
                passed: 0,
                failed: 0,
                ignored: 0,
                filtered_out: 0,
            }),
            Some(Pending::Dropped(what, reason)) => self.dropped.push(dropped(what, reason)),
        }
    }
}

/// A crate entry listed rather than recorded.
fn dropped(what: impl Into<String>, reason: &str) -> EvidenceDropped {
    EvidenceDropped {
        producer: EvidenceProducer::Crate,
        what: what.into(),
        reason: reason.to_string(),
    }
}

/// Read `cargo test`'s output: each `Running tests/<name>.rs` line joined to the
/// `test result:` line that follows it, judged by its counts, and everything else it ran
/// listed rather than ignored.
///
/// `cargo test`'s machine format is still unstable, so this reads what every cargo prints.
/// The `Running` line names the binary the following result belongs to; without it a run of
/// forty binaries is forty anonymous result lines, which is how a per-test-binary outcome
/// becomes an untraceable total. Escape sequences are stripped from every line before it is
/// matched, so a coloured run reads as a plain one.
///
/// A result line is judged by the first of these that holds:
///
/// | The result line | Outcome |
/// |---|---|
/// | a failed count above zero | fail |
/// | the word `FAILED` | fail |
/// | the word `ok` and no `passed` count | error |
/// | the word `ok` and nothing passed (every test ignored, say) | skip |
/// | the word `ok` and a filtered-out count above zero (a subset) | skip |
/// | the word `ok` | pass |
/// | any other word | error |
///
/// A binary whose `Running` line is followed by no result line is an error. The crate's own
/// unit-test binary (`Running unittests <path>`), its doctests (`Doc-tests <crate>`), a
/// binary outside `tests/` and a result line with no `Running` line before it are listed in
/// [`CrateRead::dropped`] with the reason, and never credited to a neighbour.
///
/// ```
/// use majordomus_cli::evidence::{read_crate_output, Outcome};
///
/// let read = read_crate_output(
///     "     Running tests/all_ignored.rs (target/debug/deps/all_ignored-1)\n\
///      test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; \
///      finished in 0.00s\n\
///           Running tests/broken.rs (target/debug/deps/broken-1)\n\
///      test result: ok. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; \
///      finished in 0.20s\n\
///           Running tests/silent.rs (target/debug/deps/silent-1)\n",
/// );
/// let outcomes: Vec<(&str, Outcome)> =
///     read.binaries.iter().map(|b| (b.name.as_str(), b.outcome)).collect();
/// assert_eq!(
///     outcomes,
///     [("all_ignored", Outcome::Skip), ("broken", Outcome::Fail), ("silent", Outcome::Error)]
/// );
/// ```
pub fn read_crate_output(text: &str) -> CrateRead {
    let mut read = CrateRead::default();
    let mut pending: Option<Pending> = None;
    for line in text.lines() {
        let plain = strip_ansi(line);
        let t = plain.trim();
        if let Some(rest) = t.strip_prefix("Running ") {
            read.close(pending.take());
            // `tests/why.rs (target/debug/deps/why-1a2b)` — the source path is the name,
            // never the hashed binary, which changes between builds
            let mut words = rest.split_whitespace();
            let first = words.next().unwrap_or("");
            pending = Some(if first == "unittests" {
                let path = words.next().unwrap_or("");
                Pending::Dropped(format!("unittests {path}"), UNIT_TESTS)
            } else if let Some(name) = first
                .strip_prefix("tests/")
                .and_then(|p| p.strip_suffix(".rs"))
            {
                Pending::Binary(name.to_string())
            } else {
                Pending::Dropped(first.to_string(), NOT_AN_INTEGRATION_BINARY)
            });
            continue;
        }
        if let Some(rest) = t.strip_prefix("Doc-tests ") {
            read.close(pending.take());
            let krate = rest.split_whitespace().next().unwrap_or("");
            pending = Some(Pending::Dropped(format!("doc-tests {krate}"), DOC_TESTS));
            continue;
        }
        if let Some(rest) = t.strip_prefix("test result: ") {
            match pending.take() {
                None => read.dropped.push(dropped("a result line", UNATTRIBUTED)),
                Some(Pending::Dropped(what, reason)) => read.dropped.push(dropped(what, reason)),
                Some(Pending::Binary(name)) => read.binaries.push(judge(name, rest)),
            }
        }
    }
    read.close(pending.take());
    read
}

/// A binary's result line, `<word>. <counts>; finished in <S>s`, judged by the table of
/// [`read_crate_output`].
fn judge(name: String, rest: &str) -> CrateBinary {
    let (word, counts) = rest.split_once('.').unwrap_or((rest, ""));
    let word = word.trim();
    let (mut passed, mut failed, mut ignored, mut filtered_out) = (None, 0, 0, 0);
    let mut seconds = 0;
    for part in counts.split(';').map(str::trim) {
        if let Some(s) = part.strip_prefix("finished in ") {
            seconds = s
                .trim_end_matches('s')
                .trim()
                .parse::<f64>()
                .map(|f| f.round() as u64)
                .unwrap_or(0);
            continue;
        }
        let Some((n, label)) = part.split_once(' ') else {
            continue;
        };
        let Ok(n) = n.parse::<u64>() else { continue };
        match label.trim() {
            "passed" => passed = Some(n),
            "failed" => failed = n,
            "ignored" => ignored = n,
            "filtered out" => filtered_out = n,
            _ => {}
        }
    }
    let outcome = if failed > 0 || word == "FAILED" {
        Outcome::Fail
    } else if word == "ok" {
        match passed {
            // the line said `ok` and not what ran: nothing here can say it passed
            None => Outcome::Error,
            // it ran nothing, or only a subset a filter chose: a skip, not the proof
            Some(0) => Outcome::Skip,
            Some(_) if filtered_out > 0 => Outcome::Skip,
            Some(_) => Outcome::Pass,
        }
    } else {
        Outcome::Error
    };
    CrateBinary {
        name,
        outcome,
        seconds,
        passed: passed.unwrap_or(0),
        failed,
        ignored,
        filtered_out,
    }
}

/// A line with its terminal escape sequences removed: `ESC [` up to its final byte (`@`
/// to `~`), `ESC ]` up to BEL or `ESC \`, `ESC` with intermediate bytes (space to `/`) up to
/// the final byte after them, and any other `ESC` with the character after it.
/// `cargo test` colours its output when it thinks a person is watching, and a coloured
/// `Running` or `ok` is the same line as a plain one. The test harness resets its colour
/// through the terminal's own `sgr0`, which on an xterm is `ESC ( B ESC [ m`: the `( B`
/// selects the character set, and reading it as a two-character pair would leave its `B`
/// behind and turn `ok` into `okB`.
fn strip_ansi(line: &str) -> String {
    const ESC: char = '\u{1b}';
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != ESC {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                let mut after_esc = false;
                for c in chars.by_ref() {
                    if c == '\u{7}' || (after_esc && c == '\\') {
                        break;
                    }
                    after_esc = c == ESC;
                }
            }
            // intermediate bytes, then the one final byte that ends the sequence
            Some(' '..='/') => {
                while chars.next_if(|c| (' '..='/').contains(c)).is_some() {}
                chars.next();
            }
            Some(_) | None => {}
        }
    }
    out
}

/// One `Running tests/<name>.rs` / `test result:` pair per test binary, joined: the
/// binaries of [`read_crate_output`] as `(name, outcome, seconds)`, without what it listed.
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
/// ```
/// use majordomus_cli::evidence::parse_crate_binaries;
/// assert!(parse_crate_binaries("nothing to see here").is_empty());
/// ```
/// The outputs of several lanes are joined end to end, so a binary whose run was cut short
/// must not take the next result line, a doctest run's least of all: it stands as an error
/// that ran nothing, and the doctest result belongs to no binary.
/// ```
/// use majordomus_cli::evidence::{parse_crate_binaries, Outcome};
/// let joined = "     Running tests/cut.rs (target/debug/deps/cut-1a2b)\n\
///                  Doc-tests majordomus_cli\n\
///               test result: ok. 900 passed; 0 failed; finished in 600.00s\n";
/// assert_eq!(parse_crate_binaries(joined), [("cut".to_string(), Outcome::Error, 0)]);
/// ```
pub fn parse_crate_binaries(text: &str) -> Vec<(String, Outcome, u64)> {
    read_crate_output(text)
        .binaries
        .into_iter()
        .map(|b| (b.name, b.outcome, b.seconds))
        .collect()
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
/// It decides nothing: a case that failed is a case the runner said failed. Every refusal
/// comes before any write: a request naming no report, a tree with no commit, a
/// measurement of a report not given, of another producer, of another commit or of other
/// bytes, a local or release measurement that names no report, a CI report without a
/// measurement, a malformed report (refused rather than partly read), a coverage summary
/// of another schema, and a run-record path that is a directory or a ledger.
///
/// Each execution carries its report's measured tree, or `unknown` without one, and the
/// recording is returned as an [`EvidenceRunRecord`] in [`RecordOutcome::run_record`].
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
///     RecordRequest { suite: Some(p), crate_output: None, ..RecordRequest::new(Origin::Local) }
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
    use EvidenceProducer as P;
    if req.suite.is_none() && req.crate_output.is_none() && req.coverage.is_none() {
        return Err(refused(
            "nothing to record: give --suite, --crate-output, --coverage or several",
        ));
    }

    let commit = match crate::git::inspect(root) {
        crate::git::GitState::Available(i) => i.head,
        crate::git::GitState::Unavailable { .. } => None,
    }
    .ok_or_else(|| {
        refused(
            "this is not a git work tree with a commit, so a run recorded here would carry no \
             provenance and prove nothing",
        )
    })?;

    let report_of = |p: EvidenceProducer| match p {
        P::Suite => req.suite.as_deref(),
        P::Crate => req.crate_output.as_deref(),
        P::Coverage => req.coverage.as_deref(),
    };
    for (&p, prov) in &req.provenance {
        let w = p.as_str();
        if report_of(p).is_none() {
            return Err(refused(format!(
                "a measurement of the {w} report was given, and no {w} report"
            )));
        }
        if let Some(x) = prov.producer.filter(|x| *x != p) {
            return Err(refused(format!(
                "the {w} measurement measures the {} report, not the {w} one",
                x.as_str()
            )));
        }
        if CommitId::parse(&prov.commit).is_err() || prov.commit != commit {
            return Err(refused(format!(
                "the {w} report was measured at {}, and this checkout is at {}: record it in a \
                 checkout of the commit it ran",
                short12(&prov.commit),
                short12(&commit)
            )));
        }
        if req.origin != Origin::Ci && prov.report.is_none() {
            return Err(refused(format!(
                "the {w} measurement names no report, so it cannot say which run it measured: \
                 stamp with --report <file>"
            )));
        }
    }
    if req.origin == Origin::Ci {
        for p in [P::Suite, P::Crate] {
            if report_of(p).is_some() && !req.provenance.contains_key(&p) {
                return Err(refused(format!(
                    "a CI report is recorded only with the measurement its own run made: give \
                     --provenance {}=<file> from `majordomus evidence stamp`",
                    p.as_str()
                )));
            }
        }
    }
    // the bytes a report holds, refused when its measurement describes other bytes
    let read_report = |p: EvidenceProducer, path: &Path| -> Result<Vec<u8>> {
        let bytes = std::fs::read(path).map_err(Error::Transport)?;
        if let Some(artifact) = req.provenance.get(&p).and_then(|m| m.report.as_ref()) {
            let actual = digest_of(&bytes);
            if artifact.digest != actual {
                return Err(refused(format!(
                    "the measurement describes another report ({} ≠ {})",
                    short12(artifact.digest.trim_start_matches("sha256:")),
                    short12(actual.trim_start_matches("sha256:"))
                )));
            }
        }
        Ok(bytes)
    };
    let tree_of = |p: EvidenceProducer| {
        req.provenance
            .get(&p)
            .map_or(TreeState::Unknown, |m| m.working_tree)
    };

    let mut results: Vec<(TestId, Outcome, u64)> = Vec::new();
    let mut unknown = Vec::new();
    let mut absent = Vec::new();

    if let Some(p) = &req.suite {
        let bytes = read_report(P::Suite, p)?;
        let text = String::from_utf8_lossy(&bytes);
        let before = results.len();
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
        if results.len() == before {
            absent.push(absence(
                P::Suite,
                "the report named no test this repository has",
            ));
        }
    } else {
        absent.push(absence(P::Suite, "no suite report was given"));
    }

    let mut dropped = Vec::new();
    if let Some(p) = &req.crate_output {
        let bytes = read_report(P::Crate, p)?;
        let read = read_crate_output(&String::from_utf8_lossy(&bytes));
        if read.binaries.is_empty() {
            absent.push(absence(
                P::Crate,
                "the output named no integration test binary",
            ));
        }
        for binary in read.binaries {
            let id = TestId {
                runner: Runner::Crate,
                name: binary.name.clone(),
            };
            if root.join(id.source()).exists() {
                results.push((id, binary.outcome, binary.seconds));
            } else {
                unknown.push(format!("crate:{}", binary.name));
            }
        }
        // what the output held that no claim can name yet, in the order it held it
        dropped = read.dropped;
    } else {
        absent.push(absence(P::Crate, "no crate report was given"));
    }

    let coverage = match &req.coverage {
        Some(p) => {
            let bytes = read_report(P::Coverage, p)?;
            let id = CommitId::parse(&commit).map_err(refused)?;
            let floors = EvidenceCoverageFloors::read(root);
            let text = String::from_utf8_lossy(&bytes);
            let cov = EvidenceCoverage::from_summary(&text, id, tree_of(P::Coverage), floors)
                .map_err(|e| refused(format!("{}: {e}", p.display())))?;
            Some(cov)
        }
        None => {
            absent.push(absence(P::Coverage, "no coverage report was given"));
            None
        }
    };

    if let Some(out) = &req.run_record {
        let is_ledger = [LEDGER_PATH, LOCAL_LEDGER_PATH].iter().any(|l| {
            let ledger = root.join(l);
            out == &ledger
                || matches!(
                    (std::fs::canonicalize(out), std::fs::canonicalize(&ledger)),
                    (Ok(a), Ok(b)) if a == b
                )
        });
        if out.as_os_str().is_empty() || out.is_dir() || is_ledger {
            return Err(refused(format!(
                "`{}` cannot hold a run record: it is empty, a directory or a ledger",
                out.display()
            )));
        }
    }

    let at = now_rfc3339();
    let mut executions = Vec::with_capacity(results.len());
    let mut passed = 0;
    for (id, outcome, seconds) in results {
        if outcome.proves() {
            passed += 1;
        }
        let producer = match id.runner {
            Runner::Suite => P::Suite,
            Runner::Crate => P::Crate,
        };
        let tree = tree_of(producer);
        let source = id.source();
        let digest = digest_ran(root, &commit, &source, tree);
        executions.push(Execution {
            test: id.as_string(),
            runner: id.runner,
            source,
            outcome,
            seconds,
            commit: commit.clone(),
            working_tree: tree.as_str().into(),
            digest,
            at: at.clone(),
            origin: req.origin,
            command: id.reproduce(),
            run: req.run.clone(),
        });
    }

    let mut ledger = Ledger::load_from(root, req.ledger)?;
    let recorded = ledger.merge(executions.clone());
    // A run that recorded nothing writes nothing. Saving here would create a ledger
    // holding no executions out of a report that named only tests this repository does
    // not have — a file that says "evidence was recorded" where none was. The malformed
    // report above is refused before any write for the same reason, and the two paths
    // must not disagree about what an empty recording leaves behind.
    if recorded > 0 {
        ledger.save_to(root, req.ledger)?;
    }

    crate::order::canonical_strings(&mut unknown);
    unknown.dedup();
    let given = [P::Suite, P::Crate, P::Coverage]
        .into_iter()
        .filter(|p| report_of(*p).is_some());
    let working_tree = given.fold(TreeState::Clean, |t, p| t.weaker(tree_of(p)));
    let run_record = EvidenceRunRecord {
        schema: RUN_RECORD_SCHEMA,
        id: run_id(req.origin, req.run.as_ref(), &commit, &at),
        origin: req.origin,
        commit: commit.clone(),
        working_tree,
        recorded_at: at,
        ledger: req.ledger,
        run: req.run.clone(),
        provenance: req.provenance.values().cloned().collect(),
        totals: EvidenceRunTotals::of(&executions),
        absent,
        dropped,
        unknown,
        coverage,
        executions,
    };

    if let Some(out) = &req.run_record {
        write_run_record(out, &run_record).map_err(|e| {
            refused(format!(
                "the executions were recorded into {}, and the run record could not be \
                 written to {}: {e}",
                req.ledger.path(),
                out.display()
            ))
        })?;
    }

    Ok(RecordOutcome {
        recorded,
        passed,
        commit,
        working_tree: run_record.working_tree.as_str().to_string(),
        unknown: run_record.unknown.clone(),
        dropped: run_record.dropped.clone(),
        run_record,
    })
}

fn refused(reason: impl Into<String>) -> Error {
    Error::InvalidSurface {
        surface: "evidence".into(),
        reason: reason.into(),
    }
}

fn absence(producer: EvidenceProducer, reason: &str) -> EvidenceAbsent {
    EvidenceAbsent {
        producer,
        reason: reason.into(),
    }
}

/// The first twelve characters of an id, or the whole of a shorter one.
fn short12(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// The digest of the bytes the run executed: the commit's blob when the run measured a
/// clean tree, else the working copy, else `sha256:unreadable`.
fn digest_ran(root: &Path, commit: &str, source: &str, tree: TreeState) -> String {
    if tree == TreeState::Clean {
        let blob = crate::git::read_only(root)
            .args(["cat-file", "blob", &format!("{commit}:{source}")])
            .output();
        if let Ok(out) = blob {
            if out.status.success() {
                return digest_of(&out.stdout);
            }
        }
    }
    std::fs::read(root.join(source))
        .map(|b| digest_of(&b))
        .unwrap_or_else(|_| "sha256:unreadable".into())
}

fn write_run_record(out: &Path, run_record: &EvidenceRunRecord) -> std::io::Result<()> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = serde_json::to_string_pretty(run_record)?;
    text.push('\n');
    std::fs::write(out, text)
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

    /// The measurement a suite run leaves at `root` now, naming its report.
    fn stamp_of(root: &Path, producer: EvidenceProducer, report: &Path) -> EvidenceProvenance {
        let req = super::super::provenance::StampRequest {
            producer: Some(producer),
            report: Some(report.to_path_buf()),
            ..Default::default()
        };
        super::super::provenance::stamp(root, &req).unwrap()
    }

    /// A suite recording from `origin` that carries the measurement its run just took.
    fn measured(root: &Path, origin: Origin, tsv: &Path) -> RecordRequest {
        let mut req = RecordRequest {
            suite: Some(tsv.to_path_buf()),
            ..RecordRequest::new(origin)
        };
        req.provenance.insert(
            EvidenceProducer::Suite,
            stamp_of(root, EvidenceProducer::Suite, tsv),
        );
        req
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

        let got = record(d.path(), &measured(d.path(), Origin::Ci, &tsv)).unwrap();
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

    /// The ledger is not part of what a run measures: a stamp taken when the only pending
    /// change is the previous recording's own row is `clean`. Any other pending change is
    /// `dirty`, because the commit the run is joined to is then not what ran.
    #[test]
    fn the_ledgers_own_row_does_not_make_the_tree_dirty() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = reports.path().join("run.tsv");
        std::fs::write(&tsv, "07_scope\tok\t1\tparallel\n").unwrap();

        // stamp, record: the ledger now has a pending row; stamp again, and it is still clean
        let first = record(d.path(), &measured(d.path(), Origin::Local, &tsv)).unwrap();
        assert_eq!(first.working_tree, "clean");
        let second = record(d.path(), &measured(d.path(), Origin::Local, &tsv)).unwrap();
        assert_eq!(
            second.working_tree, "clean",
            "the previous recording's own row made the next run read as measured on a dirty tree"
        );
        let row = |d: &Path| {
            Ledger::load(d)
                .unwrap()
                .latest("suite:07_scope")
                .unwrap()
                .clone()
        };
        assert_eq!(row(d.path()).working_tree, "clean");

        // an edited unrelated file makes the stamp dirty, and the recorded row says so
        std::fs::write(d.path().join("test/cases/08_other.sh"), "echo edited\n").unwrap();
        let third = record(d.path(), &measured(d.path(), Origin::Local, &tsv)).unwrap();
        assert_eq!(third.working_tree, "dirty");
        assert_eq!(row(d.path()).working_tree, "dirty");
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
                ..RecordRequest::new(Origin::Local)
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
                ..RecordRequest::new(Origin::Local)
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
                ..RecordRequest::new(Origin::Local)
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
                ..RecordRequest::new(Origin::Local)
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
                ..RecordRequest::new(Origin::Local)
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
                ..RecordRequest::new(Origin::Local)
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
                ..RecordRequest::new(Origin::Local)
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

    /// The one binary of a crate output, read: a helper for the tests below, which each
    /// state one row of the rule.
    fn only(text: &str) -> CrateBinary {
        let read = read_crate_output(text);
        assert_eq!(read.binaries.len(), 1, "{read:?}");
        read.binaries.into_iter().next().unwrap()
    }

    /// `ok` over a binary that ran nothing is not a pass: every test ignored, or no test at
    /// all, is a skip. A pass would prove a claim with a run that measured nothing.
    #[test]
    fn a_binary_that_ran_no_test_is_a_skip() {
        let all_ignored = only(
            "     Running tests/beta.rs (target/debug/deps/beta-1)\n\
             test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; \
             finished in 0.00s\n",
        );
        assert_eq!(all_ignored.outcome, Outcome::Skip);
        assert_eq!((all_ignored.passed, all_ignored.ignored), (0, 2));

        let empty = only(
            "     Running tests/empty.rs (target/debug/deps/empty-1)\n\
             test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
             finished in 0.00s\n",
        );
        assert_eq!(empty.outcome, Outcome::Skip);
    }

    /// A run that a name filter narrowed to a subset ran some of the binary's tests and not
    /// the rest: it is not the binary's proof, so it is a skip, however many passed.
    #[test]
    fn a_filtered_subset_is_a_skip() {
        let subset = only(
            "     Running tests/gamma.rs (target/debug/deps/gamma-1)\n\
             test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; \
             finished in 0.00s\n",
        );
        assert_eq!(subset.outcome, Outcome::Skip);
        assert_eq!((subset.passed, subset.filtered_out), (1, 4));

        let whole = only(
            "     Running tests/gamma.rs (target/debug/deps/gamma-1)\n\
             test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
             finished in 0.00s\n",
        );
        assert_eq!(whole.outcome, Outcome::Pass, "the same binary, run whole");
    }

    /// A failed count is a failure whatever word the line starts with, and `FAILED` is a
    /// failure whatever the counts say.
    #[test]
    fn a_failed_count_is_a_failure_whatever_the_word() {
        for line in [
            "test result: ok. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out",
            "test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out",
            "test result: FAILED. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
            "test result: interrupted. 0 passed; 1 failed",
        ] {
            let b = only(&format!(
                "     Running tests/delta.rs (target/debug/deps/delta-1)\n{line}\n"
            ));
            assert_eq!(b.outcome, Outcome::Fail, "{line}");
        }
    }

    /// The lines cargo and the harness print between a `Running` line and its result, the
    /// per-test lines among them, close nothing: the result that follows is still the
    /// binary's. A part of the result line that is not a count is passed over, and the counts
    /// after it are still read.
    #[test]
    fn the_lines_between_a_binary_and_its_result_are_read_past() {
        let b = only(
            "     Running tests/alpha.rs (target/debug/deps/alpha-1)\n\
             \n\
             running 3 tests\n\
             test alpha_holds ... ok\n\
             test alpha_is_fast ... ok\n\
             test alpha_later ... ignored\n\
             \n\
             test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; \
             finished in 0.40s\n\
             \n",
        );
        assert_eq!((b.name.as_str(), b.outcome), ("alpha", Outcome::Pass));
        assert_eq!((b.passed, b.ignored), (2, 1));

        let noted = only(
            "     Running tests/alpha.rs (target/debug/deps/alpha-1)\n\
             test result: ok. a note; 3 passed; 0 failed; 0 ignored; 0 measured; \
             0 filtered out; finished in 1.40s\n",
        );
        assert_eq!(noted.outcome, Outcome::Pass, "the counts after the note");
        assert_eq!((noted.passed, noted.seconds), (3, 1));
    }

    /// A crate binary the output names and this repository does not have is named, not
    /// recorded, exactly as a suite case would be: its execution would prove nothing here.
    #[test]
    fn a_crate_binary_this_repository_does_not_have_is_named_not_recorded() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let log = reports.path().join("crate.log");
        std::fs::write(
            &log,
            "     Running tests/ghost.rs (target/debug/deps/ghost-1)\n\
             test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n",
        )
        .unwrap();
        let got = record(
            d.path(),
            &RecordRequest {
                suite: None,
                crate_output: Some(log),
                ..RecordRequest::new(Origin::Local)
            },
        )
        .unwrap();
        assert_eq!(got.recorded, 0);
        assert_eq!(got.unknown, ["crate:ghost"]);
        assert!(
            !d.path().join(crate::evidence::LEDGER_PATH).exists(),
            "a recording of nothing this repository has writes no ledger"
        );
    }

    /// A result line that says `ok` and no count says nothing about what ran, and a word
    /// nobody knows is not a verdict: both are errors, never a pass.
    #[test]
    fn a_result_line_without_counts_is_an_error() {
        for line in [
            "test result: ok.",
            "test result: ok",
            "test result: ok. finished in 0.10s",
            "test result: okay. 3 passed; 0 failed",
            "test result: . 3 passed",
        ] {
            let b = only(&format!(
                "     Running tests/alpha.rs (target/debug/deps/alpha-1)\n{line}\n"
            ));
            assert_eq!(b.outcome, Outcome::Error, "{line}");
        }
    }

    /// A binary that started and never printed a result line, before the next binary, before
    /// the doctests or at the end of the output, is an error: the harness could not run it.
    #[test]
    fn a_binary_that_printed_no_result_is_an_error() {
        let read = read_crate_output(
            "     Running tests/eps.rs (target/debug/deps/eps-1)\n\
                  Running tests/zeta.rs (target/debug/deps/zeta-1)\n\
             test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
             finished in 0.00s\n\
                  Running tests/eta.rs (target/debug/deps/eta-1)\n\
                Doc-tests majordomus_cli\n\
                  Running tests/theta.rs (target/debug/deps/theta-1)\n",
        );
        let got: Vec<(&str, Outcome, u64)> = read
            .binaries
            .iter()
            .map(|b| (b.name.as_str(), b.outcome, b.seconds))
            .collect();
        assert_eq!(
            got,
            [
                ("eps", Outcome::Error, 0),
                ("zeta", Outcome::Pass, 0),
                ("eta", Outcome::Error, 0),
                ("theta", Outcome::Error, 0),
            ]
        );
        // the doctests got no result line either, and are listed as they are
        assert_eq!(read.dropped.len(), 1, "{:?}", read.dropped);
        assert_eq!(read.dropped[0].what, "doc-tests majordomus_cli");
    }

    /// Colour codes are stripped before a line is matched: a coloured run reads exactly as
    /// the plain one. Each escape form is stripped on its own as well.
    #[test]
    fn colour_codes_are_stripped_before_matching() {
        let e = '\u{1b}';
        let coloured = format!(
            "{e}[1m{e}[92m     Running{e}[0m tests/alpha.rs (target/debug/deps/alpha-1)\n\
             test result: {e}[32mok{e}(B{e}[m. 3 passed; 0 failed; 0 ignored; 0 measured; \
             0 filtered out; finished in 1.40s\n\
             {e}[1m{e}[92m     Running{e}[0m tests/delta.rs (target/debug/deps/delta-1)\n\
             test result: {e}[31mFAILED{e}[0m. 2 passed; 1 failed; 0 ignored; 0 measured; \
             0 filtered out; finished in 0.20s\n\
             {e}[1m{e}[92m   Doc-tests{e}[0m majordomus_cli\n\
             test result: ok. 5 passed; 0 failed\n"
        );
        let plain = strip_ansi(&coloured);
        assert!(!plain.contains(e), "{plain:?}");
        assert_eq!(read_crate_output(&coloured), read_crate_output(&plain));
        let read = read_crate_output(&coloured);
        let got: Vec<(&str, Outcome, u64)> = read
            .binaries
            .iter()
            .map(|b| (b.name.as_str(), b.outcome, b.seconds))
            .collect();
        assert_eq!(
            got,
            [("alpha", Outcome::Pass, 1), ("delta", Outcome::Fail, 0)]
        );
        assert_eq!(read.dropped[0].what, "doc-tests majordomus_cli");

        // each form alone: CSI up to its final byte, OSC up to BEL or ESC \, intermediate
        // bytes up to their final byte, and a pair
        assert_eq!(strip_ansi(&format!("a{e}[1;32mb{e}[0mc")), "abc");
        assert_eq!(
            strip_ansi(&format!("ok{e}(B{e}[m.")),
            "ok.",
            "the harness's own reset on an xterm, whose `B` is part of the escape"
        );
        assert_eq!(strip_ansi(&format!("a{e}#8b{e})0c{e} Fd")), "abcd");
        assert_eq!(
            strip_ansi(&format!("a{e}$(Bb")),
            "ab",
            "every intermediate byte, then the final one"
        );
        assert_eq!(strip_ansi(&format!("a{e}[?25lb")), "ab");
        assert_eq!(strip_ansi(&format!("a{e}]0;title\u{7}b")), "ab");
        assert_eq!(strip_ansi(&format!("a{e}]8;;http://x{e}\\b")), "ab");
        assert_eq!(strip_ansi(&format!("a{e}7b{e}8c")), "abc");
        assert_eq!(
            strip_ansi(&format!("a{e}")),
            "a",
            "a trailing ESC is dropped"
        );
        assert_eq!(strip_ansi("plain [brackets] stay"), "plain [brackets] stay");
    }

    /// The crate's own unit-test binary and its doctests ran, and no claim can name one of
    /// their tests: they are listed as dropped, with the report and the reason, in the
    /// order the output held them, and the recording carries the same list.
    #[test]
    fn unit_and_doc_tests_are_listed_as_dropped() {
        let d = repo();
        std::fs::create_dir_all(d.path().join("apps/majordomus-cli/tests")).unwrap();
        std::fs::write(
            d.path().join("apps/majordomus-cli/tests/why.rs"),
            "// why\n",
        )
        .unwrap();
        git(d.path(), &["add", "-A"]);
        git(d.path(), &["commit", "-qm", "why"]);
        let out = "     Running unittests src/lib.rs (target/debug/deps/majordomus_cli-1)\n\
                   test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\
                        Running tests/why.rs (target/debug/deps/why-1)\n\
                   test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\
                        Running benches/speed.rs (target/debug/deps/speed-1)\n\
                   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n\
                      Doc-tests majordomus_cli\n\
                   test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";
        let read = read_crate_output(out);
        assert_eq!(read.binaries.len(), 1);
        assert_eq!(read.binaries[0].name, "why");
        let listed: Vec<(EvidenceProducer, &str, &str)> = read
            .dropped
            .iter()
            .map(|d| (d.producer, d.what.as_str(), d.reason.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                (EvidenceProducer::Crate, "unittests src/lib.rs", UNIT_TESTS),
                (
                    EvidenceProducer::Crate,
                    "benches/speed.rs",
                    NOT_AN_INTEGRATION_BINARY
                ),
                (
                    EvidenceProducer::Crate,
                    "doc-tests majordomus_cli",
                    DOC_TESTS
                ),
            ]
        );
        for entry in &read.dropped {
            assert!(
                !entry.reason.chars().any(|c| c.is_ascii_digit()),
                "a reason carries no counts: {}",
                entry.reason
            );
        }

        let reports = tempfile::tempdir().unwrap();
        let log = reports.path().join("crate.log");
        std::fs::write(&log, out).unwrap();
        let got = record(
            d.path(),
            &RecordRequest {
                suite: None,
                crate_output: Some(log),
                ..RecordRequest::new(Origin::Local)
            },
        )
        .unwrap();
        assert_eq!(got.recorded, 1);
        assert_eq!(
            got.dropped, read.dropped,
            "the recording carries the same list"
        );
        let ledger = Ledger::load(d.path()).unwrap();
        assert_eq!(
            ledger.executions.len(),
            1,
            "nothing dropped reached the ledger"
        );
        assert_eq!(ledger.executions[0].test, "crate:why");
    }

    /// A result line that no `Running` line named belongs to no binary: it is listed, and
    /// never credited to the binary before it or after it.
    #[test]
    fn a_result_line_no_running_line_named_is_dropped() {
        let read = read_crate_output(
            "test result: ok. 1 passed; 0 failed\n\
                  Running tests/why.rs (target/debug/deps/why-1)\n\
             test result: ok. 2 passed; 0 failed; finished in 1.60s\n\
             test result: FAILED. 0 passed; 1 failed\n",
        );
        assert_eq!(read.binaries.len(), 1);
        assert_eq!(
            read.binaries[0].outcome,
            Outcome::Pass,
            "not the stray FAILED"
        );
        assert_eq!(read.dropped.len(), 2);
        for entry in &read.dropped {
            assert_eq!(
                (entry.producer, entry.what.as_str(), entry.reason.as_str()),
                (EvidenceProducer::Crate, "a result line", UNATTRIBUTED)
            );
        }
    }

    fn report(dir: &tempfile::TempDir, name: &str, rows: &str) -> std::path::PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, rows).unwrap();
        p
    }

    fn refusal(root: &Path, req: &RecordRequest) -> String {
        record(root, req).unwrap_err().to_string()
    }

    fn ledger_bytes(root: &Path) -> Option<Vec<u8>> {
        std::fs::read(root.join(LEDGER_PATH)).ok()
    }

    #[test]
    fn a_report_is_stamped_with_its_runs_measurement_not_the_recorders() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        // a dirty stamp, then a clean recorder
        std::fs::write(d.path().join("pending.txt"), "x").unwrap();
        let dirty = measured(d.path(), Origin::Local, &tsv);
        std::fs::remove_file(d.path().join("pending.txt")).unwrap();
        assert_eq!(record(d.path(), &dirty).unwrap().working_tree, "dirty");
        // a clean stamp, then a dirty recorder
        let clean = measured(d.path(), Origin::Local, &tsv);
        std::fs::write(d.path().join("pending.txt"), "x").unwrap();
        assert_eq!(record(d.path(), &clean).unwrap().working_tree, "clean");
    }

    #[test]
    fn a_report_without_a_measurement_records_an_unknown_tree() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        for origin in [Origin::Local, Origin::Release] {
            let got = record(
                d.path(),
                &RecordRequest {
                    suite: Some(tsv.clone()),
                    ..RecordRequest::new(origin)
                },
            );
            let got = got.unwrap();
            assert_eq!(got.working_tree, "unknown");
            assert!(got.run_record.provenance.is_empty());
            let row = Ledger::load(d.path())
                .unwrap()
                .latest("suite:07_scope")
                .unwrap()
                .clone();
            assert_eq!(row.working_tree, "unknown");
        }
    }

    #[test]
    fn a_local_measurement_that_names_no_report_is_refused() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        for origin in [Origin::Local, Origin::Release] {
            let mut req = measured(d.path(), origin, &tsv);
            req.provenance
                .get_mut(&EvidenceProducer::Suite)
                .unwrap()
                .report = None;
            let err = refusal(d.path(), &req);
            assert!(err.contains("stamp with --report"), "{err}");
        }
        assert_eq!(ledger_bytes(d.path()), None);
    }

    #[test]
    fn a_ci_measurement_without_a_report_is_accepted() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let mut req = measured(d.path(), Origin::Ci, &tsv);
        req.provenance
            .get_mut(&EvidenceProducer::Suite)
            .unwrap()
            .report = None;
        assert_eq!(record(d.path(), &req).unwrap().working_tree, "clean");
    }

    #[test]
    fn a_ci_report_without_a_measurement_is_refused_before_anything_is_written() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let log = report(&reports, "crate.log", "");
        for req in [
            RecordRequest {
                suite: Some(tsv.clone()),
                ..RecordRequest::new(Origin::Ci)
            },
            RecordRequest {
                crate_output: Some(log),
                ..measured(d.path(), Origin::Ci, &tsv)
            },
        ] {
            let err = refusal(d.path(), &req);
            assert!(err.contains("give --provenance"), "{err}");
        }
        assert_eq!(ledger_bytes(d.path()), None);
    }

    #[test]
    fn a_measurement_of_another_commit_is_refused() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let old = measured(d.path(), Origin::Local, &tsv);
        std::fs::write(d.path().join("next.txt"), "x").unwrap();
        git(d.path(), &["add", "-A"]);
        git(d.path(), &["commit", "-qm", "next"]);
        let err = refusal(d.path(), &old);
        let then = &old.provenance[&EvidenceProducer::Suite].commit[..12];
        assert!(
            err.contains(then) && err.contains("in a checkout of the commit it ran"),
            "{err}"
        );
        let mut short = measured(d.path(), Origin::Local, &tsv);
        let m = short.provenance.get_mut(&EvidenceProducer::Suite).unwrap();
        m.commit.truncate(12);
        assert!(refusal(d.path(), &short).contains("was measured at"));
    }

    #[test]
    fn a_measurement_of_another_report_is_refused() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let other = report(&reports, "other.tsv", "07_scope\tok\t2\tparallel\n");
        let mut req = measured(d.path(), Origin::Local, &other);
        req.suite = Some(tsv);
        let err = refusal(d.path(), &req);
        assert!(err.contains("describes another report"), "{err}");
        assert_eq!(ledger_bytes(d.path()), None);
    }

    #[test]
    fn a_measurement_for_a_report_not_given_is_refused() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let mut req = measured(d.path(), Origin::Local, &tsv);
        let m = req.provenance[&EvidenceProducer::Suite].clone();
        for p in [EvidenceProducer::Crate, EvidenceProducer::Coverage] {
            let mut m = m.clone();
            m.producer = Some(p);
            req.provenance.insert(p, m);
            let err = refusal(d.path(), &req);
            assert!(
                err.contains(&format!("and no {} report", p.as_str())),
                "{err}"
            );
            req.provenance.remove(&p);
        }
    }

    #[test]
    fn a_measurement_whose_producer_disagrees_with_its_key_is_refused() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let mut req = measured(d.path(), Origin::Local, &tsv);
        req.provenance
            .get_mut(&EvidenceProducer::Suite)
            .unwrap()
            .producer = Some(EvidenceProducer::Crate);
        let err = refusal(d.path(), &req);
        assert!(
            err.contains("measures the crate report, not the suite one"),
            "{err}"
        );
    }

    #[test]
    fn every_refusal_leaves_no_ledger_and_no_run_record() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let other = report(&reports, "other.tsv", "08_other\tok\t1\tparallel\n");
        let bad = report(&reports, "bad.tsv", "07_scope\tok\n");
        let cov = report(&reports, "cov.json", r#"{"schema": 2}"#);
        let out = reports.path().join("out/run.json");
        let with_out = |mut r: RecordRequest| {
            r.run_record = Some(out.clone());
            r
        };
        let unreported = {
            let mut r = measured(d.path(), Origin::Local, &tsv);
            r.provenance
                .get_mut(&EvidenceProducer::Suite)
                .unwrap()
                .report = None;
            r
        };
        let another = RecordRequest {
            suite: Some(tsv.clone()),
            ..measured(d.path(), Origin::Local, &other)
        };
        let foreign = {
            let mut r = measured(d.path(), Origin::Local, &tsv);
            r.provenance
                .get_mut(&EvidenceProducer::Suite)
                .unwrap()
                .commit = "a".repeat(40);
            r
        };
        let refusals = [
            RecordRequest::default(),
            RecordRequest {
                suite: Some(tsv.clone()),
                ..RecordRequest::new(Origin::Ci)
            },
            unreported,
            another,
            foreign,
            RecordRequest {
                suite: Some(bad),
                ..RecordRequest::default()
            },
            RecordRequest {
                coverage: Some(cov),
                ..RecordRequest::default()
            },
            RecordRequest {
                suite: Some(tsv.clone()),
                run_record: Some(reports.path().to_path_buf()),
                ..RecordRequest::default()
            },
            RecordRequest {
                suite: Some(tsv.clone()),
                run_record: Some(d.path().join(LEDGER_PATH)),
                ..RecordRequest::default()
            },
            RecordRequest {
                suite: Some(tsv.clone()),
                run_record: Some(d.path().join(LOCAL_LEDGER_PATH)),
                ..RecordRequest::default()
            },
            RecordRequest {
                suite: Some(tsv.clone()),
                run_record: Some("".into()),
                ..RecordRequest::default()
            },
        ];
        for (n, req) in refusals.into_iter().enumerate() {
            let keep = req.run_record.is_some();
            let req = if keep { req } else { with_out(req) };
            assert!(record(d.path(), &req).is_err(), "refusal {n} recorded");
            assert_eq!(ledger_bytes(d.path()), None, "refusal {n} wrote the ledger");
            assert!(!d.path().join(LOCAL_LEDGER_PATH).exists(), "refusal {n}");
            assert!(!out.exists(), "refusal {n} wrote a run record");
        }
    }

    #[test]
    fn the_digest_is_taken_from_the_commit_when_the_run_was_clean() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let req = measured(d.path(), Origin::Local, &tsv);
        std::fs::write(d.path().join("test/cases/07_scope.sh"), "echo edited\n").unwrap();
        let got = record(d.path(), &req).unwrap();
        assert_eq!(
            got.run_record.executions[0].digest,
            digest_of(b"echo scope\n")
        );
    }

    #[test]
    fn the_digest_is_taken_from_the_checkout_when_the_tree_is_not_known_clean() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        std::fs::write(d.path().join("test/cases/07_scope.sh"), "echo edited\n").unwrap();
        let dirty = measured(d.path(), Origin::Local, &tsv);
        let got = record(d.path(), &dirty).unwrap();
        assert_eq!(
            got.run_record.executions[0].digest,
            digest_of(b"echo edited\n")
        );
        let unmeasured = RecordRequest {
            suite: Some(tsv.clone()),
            ..RecordRequest::default()
        };
        let got = record(d.path(), &unmeasured).unwrap();
        assert_eq!(
            got.run_record.executions[0].digest,
            digest_of(b"echo edited\n")
        );
        // a source git cannot answer for, measured clean, is read from the checkout
        std::fs::write(d.path().join("test/cases/09_new.sh"), "echo new\n").unwrap();
        assert_eq!(
            digest_ran(
                d.path(),
                &got.commit,
                "test/cases/09_new.sh",
                TreeState::Clean
            ),
            digest_of(b"echo new\n")
        );
        assert_eq!(
            digest_ran(
                d.path(),
                &got.commit,
                "test/cases/none.sh",
                TreeState::Dirty
            ),
            "sha256:unreadable"
        );
    }

    #[test]
    fn a_local_recording_changes_no_tracked_file() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let req = RecordRequest {
            ledger: LedgerTarget::Local,
            ..measured(d.path(), Origin::Local, &tsv)
        };
        let got = record(d.path(), &req).unwrap();
        assert_eq!(got.run_record.ledger, LedgerTarget::Local);
        assert_eq!(
            ledger_bytes(d.path()),
            None,
            "the tracked ledger is not written"
        );
        let local = Ledger::load_from(d.path(), LedgerTarget::Local).unwrap();
        assert!(local.latest("suite:07_scope").is_some());
    }

    #[test]
    fn the_run_record_written_is_the_run_record_returned() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(
            &reports,
            "run.tsv",
            "07_scope\tok\t1\tparallel\n99_ghost\tok\t1\tparallel\n",
        );
        let out = reports.path().join("deep/run.json");
        let req = RecordRequest {
            run_record: Some(out.clone()),
            ..measured(d.path(), Origin::Local, &tsv)
        };
        let got = record(d.path(), &req).unwrap();
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(text.ends_with("}\n"));
        let back: EvidenceRunRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(back, got.run_record);
        assert_eq!(back.unknown, ["suite:99_ghost"]);
        assert_eq!(back.schema, RUN_RECORD_SCHEMA);
        let absent: Vec<_> = back.absent.iter().map(|a| a.producer).collect();
        assert_eq!(
            absent,
            [EvidenceProducer::Crate, EvidenceProducer::Coverage]
        );
    }

    #[test]
    fn a_run_record_that_cannot_be_written_says_the_executions_were_recorded() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let tsv = report(&reports, "run.tsv", "07_scope\tok\t1\tparallel\n");
        let blocker = report(&reports, "file", "x");
        let req = RecordRequest {
            run_record: Some(blocker.join("run.json")),
            ..measured(d.path(), Origin::Local, &tsv)
        };
        let err = refusal(d.path(), &req);
        assert!(
            err.contains("the executions were recorded into")
                && err.contains("run record could not be written"),
            "{err}"
        );
        assert!(ledger_bytes(d.path()).is_some());
    }

    #[test]
    fn the_outcomes_dropped_list_is_the_run_records() {
        let d = repo();
        let reports = tempfile::tempdir().unwrap();
        let log = report(&reports, "crate.log", "     Running unittests src/lib.rs (x)\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n");
        let got = record(
            d.path(),
            &RecordRequest {
                crate_output: Some(log),
                ..RecordRequest::default()
            },
        )
        .unwrap();
        assert_eq!(got.dropped.len(), 1);
        assert_eq!(got.dropped, got.run_record.dropped);
        assert_eq!(
            got.run_record.absent[1].reason,
            "the output named no integration test binary"
        );
    }
}
