//! The local run index: where a local run is kept, bounded, and never in the tracked ledger.
//!
//! ADR 0087 (decision 2) keeps run records in three places: a CI run's artifact, its copy on
//! gh-pages, and, for a run on a person's machine, the local run index under
//! [`LOCAL_RUNS_DIR`], inside the ignored local layer. This module is that index. A
//! recording into the local ledger is a local run, and [`super::record()`] appends its run
//! record here; a recording into the tracked ledger never is. The index holds the newest
//! [`LOCAL_RUNS_BOUND`] run records and drops the oldest past that, so it cannot grow
//! without bound: history lives in run records, and the ledger still keeps one execution per
//! test (ADR 0041 as decision 2 amends it).
//!
//! [`run_suite`] is `evidence record --run suite`: it runs the repository's suite runner
//! itself, stamps the checkout the run left, reads every line the runner wrote and printed
//! through the suite adapter ([`super::adapters::read_suite`]), and records the run into the
//! local ledger, so that it lands in the index and nowhere tracked.
//!
//! ```
//! use majordomus_cli::evidence::local_runs::{append, entries, LOCAL_RUNS_DIR};
//! use majordomus_cli::evidence::EvidenceRunRecord;
//!
//! let root = tempfile::tempdir().unwrap();
//! let run: EvidenceRunRecord = serde_json::from_value(serde_json::json!({
//!     "schema": 1, "id": "local:0123456789ab:20260926T120000Z", "origin": "local",
//!     "commit": "0123456789abcdef0123456789abcdef01234567", "working_tree": "clean",
//!     "recorded_at": "2026-09-26T12:00:00Z", "ledger": "local"
//! }))
//! .unwrap();
//! let kept = append(root.path(), &run).unwrap();
//! assert_eq!(kept, format!("{LOCAL_RUNS_DIR}/run-local-0123456789ab-20260926T120000Z.json"));
//! assert_eq!(entries(root.path())[0].record, run);
//!
//! // a run recorded into the tracked ledger is not a local run, and the index refuses it
//! let tracked = EvidenceRunRecord { ledger: majordomus_cli::evidence::LedgerTarget::Repo, ..run };
//! assert!(append(root.path(), &tracked).is_err());
//! ```

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::ledger::LedgerTarget;
use super::provenance::{stamp, StampRequest};
use super::record::{record, RecordOutcome, RecordRequest};
use super::run::{EvidenceProducer, EvidenceRunRecord, RUN_RECORD_SCHEMA};
use super::Origin;
use crate::error::{Error, Result};

/// Where the local run index lives, repository-relative: inside the ignored local layer.
pub const LOCAL_RUNS_DIR: &str = ".ai/local/evidence/runs";

/// How many run records the local run index keeps: the newest fifty (ADR 0087, decision 2).
/// Appending the fifty-first drops the oldest.
pub const LOCAL_RUNS_BOUND: usize = 50;

/// The prefix of every file the index writes. A file under [`LOCAL_RUNS_DIR`] without it (a
/// CI artifact a person copied there to read) is read by the readers of the directory and
/// never counted against the bound or dropped by it.
const PREFIX: &str = "run-";

/// The runner `evidence record --run suite` runs, repository-relative.
const SUITE_RUNNER: &str = "test/run.sh";

fn refused(reason: impl Into<String>) -> Error {
    Error::InvalidSurface {
        surface: "evidence".into(),
        reason: reason.into(),
    }
}

/// One run record the index holds, with its file, repository-relative.
///
/// ```
/// use majordomus_cli::evidence::local_runs::{entries, LocalRun};
/// let root = tempfile::tempdir().unwrap();
/// let held: Vec<LocalRun> = entries(root.path());
/// assert!(held.is_empty(), "an index nothing was appended to is empty");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LocalRun {
    /// The file, repository-relative.
    pub file: String,
    /// The run record it holds.
    pub record: EvidenceRunRecord,
}

/// A local run is ordered by when it was recorded, and its file, unique in the index, makes
/// the order total.
impl crate::order::Ordered for LocalRun {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.record.recorded_at, &self.file)
    }
}

/// Keep `run` in the local run index of `root`, drop the oldest past [`LOCAL_RUNS_BOUND`],
/// and return the file it was kept in, repository-relative.
///
/// Refused for a run recorded into the tracked ledger: the index holds local runs only, so
/// that what it holds can never be mistaken for, or reach, the tracked ledger. The file is
/// named after the run's id; a second run with the same id (two recordings in one second)
/// gets a numbered name rather than replacing the first.
///
/// ```
/// use majordomus_cli::evidence::local_runs::{append, entries, LOCAL_RUNS_BOUND};
/// use majordomus_cli::evidence::EvidenceRunRecord;
///
/// let root = tempfile::tempdir().unwrap();
/// let run = |n: usize| -> EvidenceRunRecord {
///     let at = format!("2026-09-26T12:{:02}:{:02}Z", n / 60, n % 60);
///     serde_json::from_value(serde_json::json!({
///         "schema": 1, "id": format!("local:0123456789ab:{at}"), "origin": "local",
///         "commit": "0123456789abcdef0123456789abcdef01234567", "working_tree": "clean",
///         "recorded_at": at, "ledger": "local"
///     }))
///     .unwrap()
/// };
/// for n in 0..=LOCAL_RUNS_BOUND {
///     append(root.path(), &run(n)).unwrap();
/// }
/// let kept = entries(root.path());
/// assert_eq!(kept.len(), LOCAL_RUNS_BOUND, "bounded");
/// assert_eq!(kept[0].record, run(1), "the oldest was dropped");
/// ```
pub fn append(root: &Path, run: &EvidenceRunRecord) -> Result<String> {
    if run.ledger != LedgerTarget::Local {
        return Err(refused(format!(
            "the run {} was recorded into the tracked ledger, and the local run index under \
             {LOCAL_RUNS_DIR} keeps local runs only",
            run.id
        )));
    }
    let dir = root.join(LOCAL_RUNS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
    let stem: String = run
        .id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut name = format!("{PREFIX}{stem}.json");
    let mut n = 1;
    while dir.join(&name).exists() {
        n += 1;
        name = format!("{PREFIX}{stem}-{n}.json");
    }
    // plain data with string keys: serialising it cannot fail, so there is no error to carry
    let mut text = serde_json::to_string_pretty(run).unwrap_or_default();
    text.push('\n');
    // written beside its name and moved into place, so a reader never sees half a record
    let partial = dir.join(format!(".{name}.partial"));
    std::fs::write(&partial, text).map_err(|e| Error::io(partial.display().to_string(), e))?;
    let file = dir.join(&name);
    std::fs::rename(&partial, &file).map_err(|e| Error::io(file.display().to_string(), e))?;
    prune(root, LOCAL_RUNS_BOUND)?;
    Ok(format!("{LOCAL_RUNS_DIR}/{name}"))
}

/// Every run record the index holds, oldest first: the files it wrote that read as a run
/// record of the schema this executable reads. Anything else in the directory is not the
/// index's, and is not listed here.
///
/// ```
/// use majordomus_cli::evidence::local_runs::{entries, LOCAL_RUNS_DIR};
/// let root = tempfile::tempdir().unwrap();
/// let dir = root.path().join(LOCAL_RUNS_DIR);
/// std::fs::create_dir_all(&dir).unwrap();
/// std::fs::write(dir.join("run-broken.json"), "not json").unwrap();
/// std::fs::write(dir.join("copied.json"), "{}").unwrap();
/// assert!(entries(root.path()).is_empty());
/// ```
pub fn entries(root: &Path) -> Vec<LocalRun> {
    let dir = root.join(LOCAL_RUNS_DIR);
    let mut runs: Vec<LocalRun> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !(name.starts_with(PREFIX) && name.ends_with(".json")) || !e.path().is_file() {
                return None;
            }
            let text = std::fs::read_to_string(e.path()).ok()?;
            let record: EvidenceRunRecord = serde_json::from_str(&text).ok()?;
            (record.schema == RUN_RECORD_SCHEMA).then(|| LocalRun {
                file: format!("{LOCAL_RUNS_DIR}/{name}"),
                record,
            })
        })
        .collect();
    crate::order::canonical(&mut runs);
    runs
}

/// Drop the oldest of the index's run records until at most `bound` are left; the files
/// dropped, repository-relative.
fn prune(root: &Path, bound: usize) -> Result<Vec<String>> {
    let runs = entries(root);
    let excess = runs.len().saturating_sub(bound);
    let mut dropped = Vec::new();
    for run in runs.into_iter().take(excess) {
        let path = root.join(&run.file);
        std::fs::remove_file(&path).map_err(|e| Error::io(path.display().to_string(), e))?;
        dropped.push(run.file);
    }
    Ok(dropped)
}

/// What `evidence record --run suite` runs: the cases to run, every case when none is named,
/// and where to write the run record as well.
///
/// ```
/// use majordomus_cli::evidence::SuiteRunRequest;
/// let r = SuiteRunRequest { cases: vec!["07_scope".into()], ..SuiteRunRequest::default() };
/// assert!(r.run_record.is_none());
/// ```
#[derive(Debug, Clone, Default)]
pub struct SuiteRunRequest {
    /// The cases to run, by name; every case when empty.
    pub cases: Vec<String>,
    /// Where to write the run record as well, when asked.
    pub run_record: Option<PathBuf>,
}

/// Run the suite of `root` and record it as a local run: `evidence record --run suite`.
///
/// It runs `bash test/run.sh <cases>` in `root` with its report written outside the
/// checkout, echoing what the runner prints to `echo` as it prints it; measures the
/// checkout the run left ([`stamp`]), naming that report; and records the report and
/// everything the runner printed into the local ledger, which appends the run record to the
/// local run index. Nothing reaches the tracked ledger.
///
/// Refused, with nothing written: a checkout without `test/run.sh`, a case name that is an
/// option or a path, a runner that exits with anything but 0 (every case passed) or 1 (some
/// failed) — a usage error or an interrupted run states nothing about the cases — and a
/// runner that wrote no report row.
///
/// ```
/// use majordomus_cli::evidence::{run_suite, SuiteRunRequest};
/// let root = tempfile::tempdir().unwrap();
/// let refused = run_suite(root.path(), &SuiteRunRequest::default(), &mut std::io::sink())
///     .unwrap_err()
///     .to_string();
/// assert!(refused.contains("test/run.sh"), "{refused}");
/// ```
pub fn run_suite(
    root: &Path,
    req: &SuiteRunRequest,
    echo: &mut dyn Write,
) -> Result<RecordOutcome> {
    if !root.join(SUITE_RUNNER).is_file() {
        return Err(refused(format!(
            "there is no {SUITE_RUNNER} here, so there is no suite to run"
        )));
    }
    if let Some(bad) = req.cases.iter().find(|c| {
        c.is_empty()
            || c.starts_with('-')
            || !c
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    }) {
        return Err(refused(format!(
            "`{bad}` is not a case name: a case is named by its file's stem under test/cases/"
        )));
    }
    let scratch = Scratch::new()?;
    let outcome = run_in(root, req, echo, &scratch);
    drop(scratch);
    outcome
}

/// A directory of the run's own, outside the checkout, removed when the run is done.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("mj-evidence-run-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(dir.display().to_string(), e))?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The run itself: see [`run_suite`].
fn run_in(
    root: &Path,
    req: &SuiteRunRequest,
    echo: &mut dyn Write,
    scratch: &Scratch,
) -> Result<RecordOutcome> {
    let report = scratch.0.join("suite.tsv");
    let printed = scratch.0.join("suite-output.txt");
    let mut command = Command::new("bash");
    command
        // one stream, in the order the runner wrote it: its stderr joined to its stdout,
        // which is how a person watching it reads it and how CI keeps it
        .arg("-c")
        .arg(format!("exec bash {SUITE_RUNNER} \"$@\" 2>&1"))
        .arg("evidence-record")
        .args(&req.cases)
        .current_dir(root)
        // the runner's own modes are not this run's: a worker, a listing or a shard would
        // record a part of the suite, or none of it, as if it were the run asked for
        .env_remove("MJ_TEST_WORKER")
        .env_remove("MJ_TEST_LOGDIR")
        .env_remove("MJ_TEST_LIST")
        .env_remove("MJ_TEST_SHARD")
        .env("MJ_TEST_REPORT", &report)
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| refused(format!("the suite runner could not be started: {e}")))?;
    let reader = child
        .stdout
        .take()
        .ok_or_else(|| refused("the suite runner's output could not be read"))?;
    let mut output = Vec::new();
    let mut lines = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        match lines.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(_) => {
                // a person watching sees the run as it goes; a write that fails costs the
                // echo, never the recording
                let _ = echo.write_all(&line);
                output.extend_from_slice(&line);
            }
            Err(e) => {
                // the run this process started is stopped, never left behind it
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Transport(e));
            }
        }
    }
    let status = child.wait().map_err(Error::Transport)?;
    std::fs::write(&printed, &output).map_err(|e| Error::io(printed.display().to_string(), e))?;
    match status.code() {
        Some(0 | 1) => {}
        code => {
            let text = String::from_utf8_lossy(&output);
            let tail: Vec<&str> = text.lines().rev().take(3).collect();
            let said: Vec<&str> = tail.into_iter().rev().collect();
            return Err(refused(format!(
                "the suite runner exited with {}, which states nothing about the cases, so \
                 nothing was recorded: {}",
                code.map_or("a signal".to_string(), |c| format!("status {c}")),
                said.join(" / ")
            )));
        }
    }
    let rows = std::fs::read_to_string(&report).unwrap_or_default();
    if rows.trim().is_empty() {
        return Err(refused(
            "the suite runner wrote no report row, so there is no run to record",
        ));
    }
    // the run measures the checkout it left, naming its report: the recorder stamps its own
    let measured = stamp(
        root,
        &StampRequest {
            producer: Some(EvidenceProducer::Suite),
            report: Some(report.clone()),
            ..StampRequest::default()
        },
    )?;
    let mut recording = RecordRequest {
        suite: Some(report),
        suite_output: Some(printed),
        ledger: LedgerTarget::Local,
        run_record: req.run_record.clone(),
        ..RecordRequest::new(Origin::Local)
    };
    recording
        .provenance
        .insert(EvidenceProducer::Suite, measured);
    record(root, &recording)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: &str, at: &str) -> EvidenceRunRecord {
        serde_json::from_value(serde_json::json!({
            "schema": 1, "id": id, "origin": "local",
            "commit": "0123456789abcdef0123456789abcdef01234567", "working_tree": "clean",
            "recorded_at": at, "ledger": "local"
        }))
        .unwrap()
    }

    #[test]
    fn two_runs_with_one_id_are_both_kept() {
        let root = tempfile::tempdir().unwrap();
        let r = run("local:x:1", "2026-09-26T12:00:00Z");
        let first = append(root.path(), &r).unwrap();
        let second = append(root.path(), &r).unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("-2.json"), "{second}");
        assert_eq!(entries(root.path()).len(), 2);
    }

    #[test]
    fn a_file_the_index_did_not_write_is_never_dropped() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(LOCAL_RUNS_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ci-artifact.json"), "{}").unwrap();
        for n in 0..3 {
            append(
                root.path(),
                &run(&format!("local:x:{n}"), &format!("2026-09-26T12:00:0{n}Z")),
            )
            .unwrap();
        }
        assert_eq!(prune(root.path(), 1).unwrap().len(), 2);
        assert!(dir.join("ci-artifact.json").is_file());
        assert_eq!(entries(root.path())[0].record.id, "local:x:2", "the newest");
    }

    #[test]
    fn a_case_name_that_is_an_option_or_a_path_is_refused() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("test")).unwrap();
        std::fs::write(root.path().join(SUITE_RUNNER), "exit 0\n").unwrap();
        for bad in ["--no-skips", "../x", "a b", ""] {
            let req = SuiteRunRequest {
                cases: vec![bad.into()],
                run_record: None,
            };
            let err = run_suite(root.path(), &req, &mut std::io::sink())
                .unwrap_err()
                .to_string();
            assert!(err.contains("is not a case name"), "{bad:?}: {err}");
        }
    }

    #[test]
    fn a_runner_that_refuses_its_arguments_records_nothing() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("test")).unwrap();
        std::fs::write(
            root.path().join(SUITE_RUNNER),
            "echo \"run.sh: no case matches '$1'\" >&2\nexit 2\n",
        )
        .unwrap();
        let req = SuiteRunRequest {
            cases: vec!["99_none".into()],
            run_record: None,
        };
        let err = run_suite(root.path(), &req, &mut std::io::sink())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("status 2") && err.contains("no case matches '99_none'"),
            "{err}"
        );
        assert!(!root.path().join(LOCAL_RUNS_DIR).exists());
    }
}
