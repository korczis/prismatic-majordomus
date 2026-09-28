//! Token economics, measured rather than claimed.
//!
//! The question is the one `.ai/repo/benchmarks/economics/methodology.yaml` states: for the
//! same task, from the same repository state, with the same model and the same acceptance
//! tests, how many tokens does a coding session consume with Majordomus installed and
//! without it? This module is the one calculator that answers it. The CLI, the HTTP API,
//! MCP, the Cockpit, the generated report and the public site are projections of
//! [`summarize`]; none of them computes a number of its own.
//!
//! The data flow:
//!
//! ```text
//! methodology.yaml, suites/*.yaml, tasks/*/task.yaml    declared
//!          │
//!   runner (live sessions) ─► runs/<suite>/<run>.json   raw facts: provider usage, gates
//!   context (no model)     ─► runs/context/<rev>.json   raw facts: counted tokens
//!          │
//!   summarize ─► pairs ─► per-pair reductions ─► distributions, intervals ─► verdict
//! ```
//!
//! Nothing derived is stored. A pair, a metric and a verdict are recomputed from the raw
//! records on every read, so the records never need rewriting when the derivation changes,
//! and a changed derivation is a methodology version, not an edit to history.
//!
//! What the verdict may say is decided by the methodology's publication rule, not by the
//! caller: below it, the statement is that no verified total-token claim exists, with the
//! preliminary observation labelled as such. The rule is evaluated for the primary metric
//! over all the evidence and for nothing else, so only that metric, unnarrowed, can ever
//! be `verified`.
//!
//! What a generated artifact publishes comes from [`summarize_tracked`], which reads only
//! the records git tracks, so that two checkouts of one commit generate the same bytes.
//!
//! ```
//! use majordomus_cli::economics::{summarize, model::EconomicsQuery};
//! let dir = tempfile::tempdir().unwrap();
//! let s = summarize(dir.path(), &EconomicsQuery::default());
//! assert!(!s.present);
//! assert!(!s.verdict.publishable);
//! assert!(s.verdict.statement.starts_with("No verified total-token-savings claim"));
//! ```

pub mod claims;
pub mod context;
pub mod model;
pub mod report;
pub mod runner;
pub mod stats;
pub mod usage;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::metadata::yaml;

use model::*;
use stats::{bootstrap_median, bootstrap_median_clustered, distribution, percent_text, reduction};

/// Where the declarations and the recorded evidence live, relative to the repository root.
pub const DIR: &str = ".ai/repo/benchmarks/economics";

/// The schema a live run record carries.
pub const RUN_SCHEMA: &str = "economics-run/v1";
/// The schema a context run record carries.
pub const CONTEXT_RUN_SCHEMA: &str = "economics-context-run/v1";

/// Every declaration of the economics subsystem, read from [`DIR`].
///
/// Only [`load`] builds one, and only when every file it names was read: a value of this
/// type is a complete benchmark, never one with a task silently missing. Suites and tasks
/// are keyed by their declared id, so iteration order is the id order, not the file order.
///
/// ```
/// use majordomus_cli::economics::{load, Declarations};
/// let dir = tempfile::tempdir().unwrap();
/// # use majordomus_cli::economics::DIR;
/// # let decl_dir = dir.path().join(DIR);
/// # std::fs::create_dir_all(decl_dir.join("suites")).unwrap();
/// # std::fs::write(decl_dir.join("methodology.yaml"), [
/// #     "schema: economics-methodology/v1", "version: 1", "title: t", "question: q",
/// #     "unit: tokens", "primary_metric: effective_token_reduction",
/// #     "classes: []", "variants: []", "success: []",
/// #     "pairing:", "  key: []", "  comparable: []", "  valid: v",
/// #     "statistics:", "  per_pair: p", "  location: median", "  interval: bootstrap",
/// #     "  confidence_bp: 9500", "  resamples: 100", "  seed: 1", "  min_pairs_for_interval: 5",
/// #     "publication:", "  min_valid_pairs: 30", "  min_categories: 4",
/// #     "  min_pairs_per_category: 5", "  min_repetitions: 3", "  min_valid_pair_rate_bp: 8000",
/// #     "  max_interval_width_bp: 2000", "  require_current: true",
/// #     "outliers:", "  rule: none", "",
/// # ].join("\n")).unwrap();
/// # std::fs::write(decl_dir.join("suites/pilot.yaml"), [
/// #     "schema: economics-suite/v1", "id: pilot", "version: 1", "kind: live", "title: Pilot",
/// #     "control: baseline", "treatment: majordomus", "freshness_inputs: [methodology.yaml]", "",
/// # ].join("\n")).unwrap();
/// // a methodology and one live suite `pilot` declaring no task yet
/// let decl: Declarations = load(dir.path()).unwrap().expect("a methodology is declared");
/// assert_eq!(decl.methodology.version, 1);
/// assert_eq!(decl.suites.keys().collect::<Vec<_>>(), ["pilot"]);
/// assert!(decl.tasks.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct Declarations {
    /// The methodology.
    pub methodology: EconomicsMethodology,
    /// The suites, by id.
    pub suites: BTreeMap<String, EconomicsSuite>,
    /// The tasks, by id.
    pub tasks: BTreeMap<String, EconomicsTask>,
}

fn read_yaml<T: serde::de::DeserializeOwned>(root: &Path, path: &Path) -> Result<T, String> {
    let shown = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
    yaml::parse_into(&text).map_err(|e| format!("{shown}: {e}"))
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    out.sort();
    out
}

/// Read every declaration under `root`. `Ok(None)` when no methodology is declared, which
/// is the ordinary state of a repository that does not benchmark itself; an error names
/// every file that could not be read, because a benchmark with a silently missing task is
/// a different benchmark.
///
/// ```
/// use majordomus_cli::economics::{load, DIR};
/// let dir = tempfile::tempdir().unwrap();
/// assert!(load(dir.path()).unwrap().is_none(), "no methodology: nothing is benchmarked");
///
/// std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
/// std::fs::write(dir.path().join(DIR).join("methodology.yaml"), "schema: x\n").unwrap();
/// let errors = load(dir.path()).unwrap_err();
/// assert_eq!(errors.len(), 1);
/// assert!(errors[0].starts_with(&format!("{DIR}/methodology.yaml: ")), "{}", errors[0]);
/// ```
pub fn load(root: &Path) -> Result<Option<Declarations>, Vec<String>> {
    let dir = root.join(DIR);
    let methodology_path = dir.join("methodology.yaml");
    if !methodology_path.is_file() {
        return Ok(None);
    }
    let mut errors = Vec::new();
    let methodology = match read_yaml::<EconomicsMethodology>(root, &methodology_path) {
        Ok(m) => Some(m),
        Err(e) => {
            errors.push(e);
            None
        }
    };
    let mut suites = BTreeMap::new();
    for p in sorted_entries(&dir.join("suites")) {
        if p.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        match read_yaml::<EconomicsSuite>(root, &p) {
            Ok(s) => {
                suites.insert(s.id.clone(), s);
            }
            Err(e) => errors.push(e),
        }
    }
    let mut tasks = BTreeMap::new();
    for p in sorted_entries(&dir.join("tasks")) {
        let file = p.join("task.yaml");
        if !file.is_file() {
            continue;
        }
        match read_yaml::<EconomicsTask>(root, &file) {
            Ok(t) => {
                tasks.insert(t.id.clone(), t);
            }
            Err(e) => errors.push(e),
        }
    }
    for s in suites.values() {
        for t in &s.tasks {
            if !tasks.contains_key(t) {
                errors.push(format!(
                    "suite {} names task {t}, which is not declared",
                    s.id
                ));
            }
        }
    }
    match (methodology, errors.is_empty()) {
        (Some(methodology), true) => Ok(Some(Declarations {
            methodology,
            suites,
            tasks,
        })),
        _ => Err(errors),
    }
}

/// A digest of the tracked files under `pathspecs`: each file's path and the SHA-256 of its
/// content as it is on disk now, in git's index order. Recorded with every run, and
/// recomputed on every read, it is what makes evidence current or stale.
///
/// ```
/// use majordomus_cli::economics::inputs_digest;
/// use std::process::Command;
/// let dir = tempfile::tempdir().unwrap();
/// let git = |a: &[&str]| Command::new("git").arg("-C").arg(dir.path()).args(a).output().unwrap();
/// git(&["init", "-q"]);
/// std::fs::write(dir.path().join("a.txt"), "one").unwrap();
/// git(&["add", "-A"]);
/// let before = inputs_digest(dir.path(), &["a.txt".to_string()]).unwrap();
/// std::fs::write(dir.path().join("a.txt"), "two").unwrap();
/// assert_ne!(before, inputs_digest(dir.path(), &["a.txt".to_string()]).unwrap());
/// ```
pub fn inputs_digest(root: &Path, pathspecs: &[String]) -> Result<String, String> {
    let specs: Vec<&str> = pathspecs.iter().map(String::as_str).collect();
    let files = crate::git::ls_files_any(root, &specs).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    for f in &files {
        let bytes = std::fs::read(root.join(f)).unwrap_or_default();
        h.update(f.as_bytes());
        h.update([0]);
        h.update(crate::policy::sha256_bytes_hex(&bytes).as_bytes());
        h.update(b"\n");
    }
    Ok(format!("{:x}", h.finalize()))
}

/// The revision evidence is recorded at: HEAD, and whether the working tree differs from it
/// anywhere outside the evidence directory. The records a suite writes are changes to the
/// working tree too, and a measurement must not call its own output a dirty tree.
///
/// ```
/// use majordomus_cli::economics::{revision, DIR};
/// let dir = tempfile::tempdir().unwrap();
/// # let git = |a: &[&str]| {
/// #     let out = std::process::Command::new("git")
/// #         .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
/// #         .arg("-C").arg(dir.path()).args(a).output().unwrap();
/// #     assert!(out.status.success(), "git {a:?}");
/// #     String::from_utf8(out.stdout).unwrap()
/// # };
/// git(&["init", "-q"]);
/// assert!(revision(dir.path()).is_err(), "no commit yet, so no HEAD to record against");
/// std::fs::write(dir.path().join("a.txt"), "one").unwrap();
/// git(&["add", "-A"]);
/// git(&["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false",
///     "commit", "-qm", "one"]);
/// let clean = revision(dir.path()).unwrap();
/// assert_eq!((clean.commit.as_str(), clean.dirty), (git(&["rev-parse", "HEAD"]).trim(), false));
///
/// std::fs::create_dir_all(dir.path().join(DIR).join("runs")).unwrap();
/// std::fs::write(dir.path().join(DIR).join("runs/r1.json"), "{}").unwrap();
/// assert!(!revision(dir.path()).unwrap().dirty, "its own records do not dirty the tree");
/// std::fs::write(dir.path().join("a.txt"), "two").unwrap();
/// assert!(revision(dir.path()).unwrap().dirty);
/// ```
pub fn revision(root: &Path) -> Result<EconomicsRevision, String> {
    let head = crate::git::read_only(root)
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !head.status.success() {
        return Err("git: no HEAD to record evidence against".into());
    }
    let status = crate::git::read_only(root)
        .args(["status", "--porcelain", "--untracked-files=all", "--", "."])
        .arg(format!(":(exclude){DIR}/runs"))
        .output()
        .map_err(|e| format!("git: {e}"))?;
    Ok(EconomicsRevision {
        commit: String::from_utf8_lossy(&head.stdout).trim().to_string(),
        dirty: !String::from_utf8_lossy(&status.stdout).trim().is_empty(),
    })
}

/// Every live run recorded under `root` for `suite`, sorted by id, with the files that
/// could not be read named.
///
/// Only `*.json` files are records. A suite with no directory yet has recorded nothing,
/// which is not an error; a record that does not parse, carries another schema, or reuses a
/// run id is named in the second list instead of being dropped.
///
/// ```
/// use majordomus_cli::economics::{load_runs, DIR};
/// let dir = tempfile::tempdir().unwrap();
/// let (runs, errors) = load_runs(dir.path(), "pilot");
/// assert!(runs.is_empty() && errors.is_empty(), "nothing recorded is not an error");
///
/// let recorded = dir.path().join(DIR).join("runs/pilot");
/// std::fs::create_dir_all(&recorded).unwrap();
/// std::fs::write(recorded.join("notes.txt"), "not a record").unwrap();
/// std::fs::write(recorded.join("r1.json"), "{").unwrap();
/// let (runs, errors) = load_runs(dir.path(), "pilot");
/// assert!(runs.is_empty());
/// assert_eq!(errors.len(), 1, "the broken record is named, the text file is ignored");
/// assert!(errors[0].starts_with(&format!("{DIR}/runs/pilot/r1.json: ")));
/// ```
pub fn load_runs(root: &Path, suite: &str) -> (Vec<(String, EconomicsRun)>, Vec<String>) {
    load_runs_from(root, suite, Source::Directory)
}

/// Where run records are read from. A person asking for a summary wants every record on
/// disk, the ones not yet committed included; a generated artifact must be the same on
/// every checkout of one commit, so it reads only the records git tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Every file in the suite's run directory.
    Directory,
    /// Only the files of that directory git tracks, read from the working tree.
    Tracked,
}

/// The files of `runs/<suite>/` the source admits, sorted by path. Only the directory's
/// own entries, never a nested one, in both sources, so the two differ only in what git
/// tracks.
fn record_files(root: &Path, suite: &str, source: Source) -> Result<Vec<PathBuf>, String> {
    let dir = root.join(DIR).join("runs").join(suite);
    match source {
        Source::Directory => Ok(sorted_entries(&dir)),
        Source::Tracked => {
            let rel = format!("{DIR}/runs/{suite}");
            let tracked = crate::git::ls_files_any(root, &[rel.as_str()])
                .map_err(|e| format!("{rel}: the tracked records could not be listed: {e}"))?;
            let mut out: Vec<PathBuf> = tracked
                .iter()
                .filter(|f| Path::new(f.as_str()).parent() == Some(Path::new(&rel)))
                .map(|f| root.join(f))
                .collect();
            out.sort();
            Ok(out)
        }
    }
}

fn load_runs_from(
    root: &Path,
    suite: &str,
    source: Source,
) -> (Vec<(String, EconomicsRun)>, Vec<String>) {
    let mut runs = Vec::new();
    let mut errors = Vec::new();
    let files = match record_files(root, suite, source) {
        Ok(files) => files,
        Err(e) => return (runs, vec![e]),
    };
    for p in files {
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<EconomicsRun>(&t).map_err(|e| e.to_string()))
        {
            Ok(r) if r.schema == RUN_SCHEMA => runs.push((rel, r)),
            Ok(r) => errors.push(format!("{rel}: schema {} is not {RUN_SCHEMA}", r.schema)),
            Err(e) => errors.push(format!("{rel}: {e}")),
        }
    }
    runs.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    let mut seen = BTreeSet::new();
    for (rel, r) in &runs {
        if !seen.insert(r.id.clone()) {
            errors.push(format!("{rel}: run id {} is recorded twice", r.id));
        }
    }
    (runs, errors)
}

/// Every context measurement recorded under `root` for `suite`, oldest first.
///
/// The order is by `measured_at`, then commit, never by file name, so the last entry is
/// the measurement the summary reports. A record under another schema is named in the
/// second list rather than read as if it were this one.
///
/// ```
/// use majordomus_cli::economics::{load_context_runs, CONTEXT_RUN_SCHEMA, DIR};
/// let dir = tempfile::tempdir().unwrap();
/// let recorded = dir.path().join(DIR).join("runs/context");
/// std::fs::create_dir_all(&recorded).unwrap();
/// # let record = |schema: &str, at: &str| serde_json::json!({
/// #     "schema": schema, "suite": "context", "suite_version": 1, "methodology": 1,
/// #     "repository": {"commit": "abc", "dirty": false}, "majordomus_version": "0.0.0",
/// #     "tokenizer": {"encoding": "o200k_base", "implementation": "tiktoken"},
/// #     "inputs_digest": "d", "budget_tokens": 8000, "measured_at": at, "seeds": []
/// # }).to_string();
/// std::fs::write(recorded.join("a.json"), record(CONTEXT_RUN_SCHEMA, "2026-09-24")).unwrap();
/// std::fs::write(recorded.join("b.json"), record(CONTEXT_RUN_SCHEMA, "2026-09-23")).unwrap();
/// std::fs::write(recorded.join("c.json"), record("economics-context-run/v0", "2026-09-22"))
///     .unwrap();
/// let (runs, errors) = load_context_runs(dir.path(), "context");
/// let at: Vec<&str> = runs.iter().map(|r| r.measured_at.as_str()).collect();
/// assert_eq!(at, ["2026-09-23", "2026-09-24"], "oldest first, whatever the file names");
/// assert_eq!(errors.len(), 1, "the record of another schema is named, not read");
/// ```
pub fn load_context_runs(root: &Path, suite: &str) -> (Vec<EconomicsContextRun>, Vec<String>) {
    load_context_runs_from(root, suite, Source::Directory)
}

fn load_context_runs_from(
    root: &Path,
    suite: &str,
    source: Source,
) -> (Vec<EconomicsContextRun>, Vec<String>) {
    let mut runs = Vec::new();
    let mut errors = Vec::new();
    let files = match record_files(root, suite, source) {
        Ok(files) => files,
        Err(e) => return (runs, vec![e]),
    };
    for p in files {
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| {
                serde_json::from_str::<EconomicsContextRun>(&t).map_err(|e| e.to_string())
            }) {
            Ok(r) if r.schema == CONTEXT_RUN_SCHEMA => runs.push(r),
            Ok(r) => errors.push(format!(
                "{rel}: schema {} is not {CONTEXT_RUN_SCHEMA}",
                r.schema
            )),
            Err(e) => errors.push(format!("{rel}: {e}")),
        }
    }
    runs.sort_by(|a, b| {
        a.measured_at
            .cmp(&b.measured_at)
            .then(a.repository.commit.cmp(&b.repository.commit))
    });
    (runs, errors)
}

// ---------------------------------------------------------------- pairing

/// Judge two runs as the methodology's pairing rule does, the outlier rule aside: a missing
/// side, then comparability, then the success gates, then usage.
fn judge(
    control: Option<&EconomicsRun>,
    treatment: Option<&EconomicsRun>,
) -> (EconomicsPairStatus, Vec<String>) {
    let (Some(c), Some(t)) = (control, treatment) else {
        let side = if control.is_none() {
            "control"
        } else {
            "treatment"
        };
        return (
            EconomicsPairStatus::Missing,
            vec![format!("the {side} run is not recorded")],
        );
    };
    let mut differ = Vec::new();
    let mut check = |what: &str, a: &str, b: &str| {
        if a != b {
            differ.push(format!("{what} differs: {a} against {b}"));
        }
    };
    check(
        "suite version",
        &c.suite_version.to_string(),
        &t.suite_version.to_string(),
    );
    check("requested model", &c.model_requested, &t.model_requested);
    // the model asked for is not always the model that answered: a harness may route part
    // of a session elsewhere, and two runs answered by different models are not one task
    check(
        "reported models",
        &c.models_reported.join(", "),
        &t.models_reported.join(", "),
    );
    check("fixture digest", &c.fixture_digest, &t.fixture_digest);
    check("harness version", &c.harness.version, &t.harness.version);
    check(
        "configuration digest",
        &c.configuration_digest,
        &t.configuration_digest,
    );
    check(
        "repository revision",
        &c.repository.commit,
        &t.repository.commit,
    );
    check(
        "session count",
        &c.sessions.len().to_string(),
        &t.sessions.len().to_string(),
    );
    if !differ.is_empty() {
        return (EconomicsPairStatus::Incomparable, differ);
    }
    let failed = |r: &EconomicsRun| -> Vec<String> {
        r.outcome
            .checks
            .iter()
            .filter(|k| !k.passed)
            .map(|k| format!("{} {}: {}", r.variant, k.id, k.detail))
            .collect()
    };
    match (c.outcome.completed, t.outcome.completed) {
        (false, false) => {
            let mut r = failed(c);
            r.extend(failed(t));
            return (EconomicsPairStatus::BothFailed, r);
        }
        (false, true) => return (EconomicsPairStatus::ControlFailed, failed(c)),
        (true, false) => return (EconomicsPairStatus::TreatmentFailed, failed(t)),
        (true, true) => {}
    }
    let mut missing = Vec::new();
    for r in [c, t] {
        if usage::totals(r).is_none() {
            missing.push(format!("{} reported no usage for a session", r.id));
        }
    }
    if !missing.is_empty() {
        return (EconomicsPairStatus::UsageUnavailable, missing);
    }
    (EconomicsPairStatus::Valid, Vec::new())
}

/// The pair's status, its reasons, and the status it would have without the outlier rule.
/// A pair with both sides recorded and a side the methodology excludes is `Excluded`, with
/// the exclusion's reasons first and whatever else was wrong with it after them.
fn pair_status(
    methodology: &EconomicsMethodology,
    control: Option<&EconomicsRun>,
    treatment: Option<&EconomicsRun>,
) -> (EconomicsPairStatus, Vec<String>, EconomicsPairStatus) {
    let (underlying, reasons) = judge(control, treatment);
    if underlying == EconomicsPairStatus::Missing {
        return (underlying, reasons, underlying);
    }
    let ids: Vec<&str> = [control, treatment]
        .into_iter()
        .flatten()
        .map(|r| r.id.as_str())
        .collect();
    let mut excluded: Vec<String> = methodology
        .outliers
        .excluded
        .iter()
        .filter(|x| ids.contains(&x.run.as_str()))
        .map(|x| format!("{} excluded: {}", x.run, x.reason))
        .collect();
    if excluded.is_empty() {
        return (underlying, reasons, underlying);
    }
    excluded.extend(reasons);
    (EconomicsPairStatus::Excluded, excluded, underlying)
}

fn last_session_tokens(r: &EconomicsRun) -> Option<u64> {
    r.sessions
        .last()
        .and_then(usage::session_totals)
        .map(|t| t.total)
}

/// A pair as the calculator holds it: the pair, the status beneath an exclusion, and the
/// model its runs asked for, which a query narrows by.
struct Judged {
    pair: EconomicsPair,
    /// What the pair would be without the outlier rule: `Valid` for an excluded pair that
    /// would otherwise count, which is what the including-excluded figure is computed over.
    underlying: EconomicsPairStatus,
    model: String,
}

/// Form every pair a live suite declares (each task, each repetition) from the runs
/// recorded for it, and judge each. A declared pair nobody ran is `missing`, not absent.
///
/// Runs recorded under another methodology version or for another suite are ignored, and a
/// suite that does not name both a control and a treatment arm forms no pair at all.
///
/// ```
/// use majordomus_cli::economics::{load, pairs};
/// use majordomus_cli::economics::model::EconomicsPairStatus;
/// let dir = tempfile::tempdir().unwrap();
/// # use majordomus_cli::economics::DIR;
/// # let decl_dir = dir.path().join(DIR);
/// # std::fs::create_dir_all(decl_dir.join("suites")).unwrap();
/// # std::fs::write(decl_dir.join("methodology.yaml"), [
/// #     "schema: economics-methodology/v1", "version: 1", "title: t", "question: q",
/// #     "unit: tokens", "primary_metric: effective_token_reduction",
/// #     "classes: []", "variants: []", "success: []",
/// #     "pairing:", "  key: []", "  comparable: []", "  valid: v",
/// #     "statistics:", "  per_pair: p", "  location: median", "  interval: bootstrap",
/// #     "  confidence_bp: 9500", "  resamples: 100", "  seed: 1", "  min_pairs_for_interval: 5",
/// #     "publication:", "  min_valid_pairs: 30", "  min_categories: 4",
/// #     "  min_pairs_per_category: 5", "  min_repetitions: 3", "  min_valid_pair_rate_bp: 8000",
/// #     "  max_interval_width_bp: 2000", "  require_current: true",
/// #     "outliers:", "  rule: none", "",
/// # ].join("\n")).unwrap();
/// # std::fs::write(decl_dir.join("suites/pilot.yaml"), [
/// #     "schema: economics-suite/v1", "id: pilot", "version: 1", "kind: live", "title: Pilot",
/// #     "control: baseline", "treatment: majordomus", "freshness_inputs: [methodology.yaml]", "",
/// # ].join("\n")).unwrap();
/// // a methodology and one live suite `pilot` with a control and a treatment arm
/// let decl = load(dir.path()).unwrap().unwrap();
/// let mut suite = decl.suites["pilot"].clone();
/// suite.tasks = vec!["vat-rounding".into()];
/// suite.repetitions = Some(2);
/// let declared = pairs(&decl, &suite, &[]);
/// assert_eq!(declared.len(), 2, "each task times each repetition, run or not");
/// assert!(declared.iter().all(|p| p.status == EconomicsPairStatus::Missing));
/// suite.control = None;
/// assert!(pairs(&decl, &suite, &[]).is_empty(), "without both arms nothing is paired");
/// ```
pub fn pairs(
    decl: &Declarations,
    suite: &EconomicsSuite,
    runs: &[EconomicsRun],
) -> Vec<EconomicsPair> {
    judged_pairs(decl, suite, runs)
        .into_iter()
        .map(|j| j.pair)
        .collect()
}

fn judged_pairs(decl: &Declarations, suite: &EconomicsSuite, runs: &[EconomicsRun]) -> Vec<Judged> {
    let (Some(control), Some(treatment)) = (&suite.control, &suite.treatment) else {
        return Vec::new();
    };
    let current: Vec<&EconomicsRun> = runs
        .iter()
        .filter(|r| r.methodology == decl.methodology.version && r.suite == suite.id)
        .collect();
    let mut keys: BTreeSet<(String, u32)> = BTreeSet::new();
    for t in &suite.tasks {
        for rep in 1..=suite.repetitions.unwrap_or(1) {
            keys.insert((t.clone(), rep));
        }
    }
    for r in &current {
        keys.insert((r.task.clone(), r.repetition));
    }
    let find = |task: &str, variant: &str, rep: u32| {
        current
            .iter()
            .copied()
            .find(|r| r.task == task && r.variant == variant && r.repetition == rep)
    };
    keys.into_iter()
        .map(|(task, rep)| {
            let c = find(&task, control, rep);
            let t = find(&task, treatment, rep);
            let (status, reasons, underlying) = pair_status(&decl.methodology, c, t);
            let declared = decl.tasks.get(&task);
            let cu = c.and_then(usage::totals);
            let tu = t.and_then(usage::totals);
            // an excluded pair that would otherwise be valid keeps its reductions, so the
            // exclusion can be shown both ways; every metric but the including-excluded one
            // reads valid pairs only
            let computable = underlying == EconomicsPairStatus::Valid;
            let both = |f: &dyn Fn(&EconomicsUsage) -> Option<u64>| -> Option<f64> {
                if !computable {
                    return None;
                }
                reduction(f(cu.as_ref()?)?, f(tu.as_ref()?)?)
            };
            let multi = declared.map(|d| d.sessions.len()).unwrap_or(1) > 1;
            let continuation = if computable && multi {
                c.zip(t)
                    .and_then(|(c, t)| reduction(last_session_tokens(c)?, last_session_tokens(t)?))
            } else {
                None
            };
            let model = c
                .or(t)
                .map(|r| r.model_requested.clone())
                .or_else(|| suite.model.clone())
                .unwrap_or_default();
            let pair = EconomicsPair {
                suite: suite.id.clone(),
                task: task.clone(),
                category: declared
                    .map(|d| d.category.clone())
                    .unwrap_or_else(|| "undeclared".into()),
                sessions: declared.map(|d| d.sessions.len()).unwrap_or(1),
                repetition: rep,
                control: c.map(|r| r.id.clone()),
                treatment: t.map(|r| r.id.clone()),
                status,
                reasons,
                token_reduction: both(&|u| Some(u.total)),
                cost_reduction: both(&|u| u.cost_microusd),
                tool_call_reduction: both(&|u| Some(u.tool_calls)),
                continuation_reduction: continuation,
                first_request_overhead: if computable {
                    let a = c
                        .and_then(|r| r.sessions.first())
                        .and_then(usage::first_request_input);
                    let b = t
                        .and_then(|r| r.sessions.first())
                        .and_then(usage::first_request_input);
                    a.zip(b).map(|(a, b)| b as i64 - a as i64)
                } else {
                    None
                },
                control_usage: cu,
                treatment_usage: tu,
            };
            Judged {
                pair,
                underlying,
                model,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- freshness

fn freshness(
    root: &Path,
    decl: &Declarations,
    suite: &EconomicsSuite,
    recorded: &[(u32, String)],
) -> (EconomicsFreshness, Option<String>) {
    if recorded.is_empty() {
        return (EconomicsFreshness::NoEvidence, None);
    }
    let other: Vec<u32> = recorded
        .iter()
        .map(|(m, _)| *m)
        .filter(|m| *m != decl.methodology.version)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if other.len() == recorded.len() || recorded.iter().all(|(m, _)| *m != decl.methodology.version)
    {
        return (
            EconomicsFreshness::Incompatible,
            Some(format!(
                "recorded under methodology {:?}; the current methodology is {}",
                other, decl.methodology.version
            )),
        );
    }
    let now = match inputs_digest(root, &suite.freshness_inputs) {
        Ok(d) => d,
        Err(e) => {
            return (
                EconomicsFreshness::Stale,
                Some(format!("the inputs could not be read: {e}")),
            )
        }
    };
    let stale = recorded
        .iter()
        .filter(|(m, d)| *m == decl.methodology.version && *d != now)
        .count();
    if stale > 0 {
        return (
            EconomicsFreshness::Stale,
            Some(format!(
                "{stale} record(s) were measured against inputs that have changed since: {}",
                suite.freshness_inputs.join(", ")
            )),
        );
    }
    (EconomicsFreshness::Current, None)
}

// ---------------------------------------------------------------- metrics

struct MetricSpec<'a> {
    id: &'a str,
    title: &'a str,
    class: EconomicsClass,
    inputs: Option<EconomicsClass>,
    unit: &'a str,
    formula: &'a str,
    not: Option<&'a str>,
}

fn metric(spec: MetricSpec, suite: &str) -> EconomicsMetric {
    EconomicsMetric {
        id: spec.id.into(),
        title: spec.title.into(),
        class: spec.class,
        inputs: spec.inputs,
        unit: spec.unit.into(),
        formula: spec.formula.into(),
        not: spec.not.map(str::to_string),
        value: None,
        status: EconomicsMetricStatus::NotMeasured,
        n: 0,
        distribution: None,
        interval: None,
        suite: suite.into(),
        evidence: Vec::new(),
        warnings: Vec::new(),
    }
}

/// The metric every headline is about.
pub const EFFECTIVE_TOKEN_REDUCTION: &str = "effective_token_reduction";
/// The primary metric with the outlier rule's exclusions put back: reported beside it when
/// the methodology excludes a run, never instead of it, and never verified.
pub const EFFECTIVE_TOKEN_REDUCTION_INCLUDING_EXCLUDED: &str =
    "effective_token_reduction.including_excluded";
/// Context selection, counted: never total savings.
pub const CONTEXT_REDUCTION: &str = "context_reduction_ratio";
/// The warning the primary metric carries when a query narrowed it.
pub const NARROWED: &str =
    "narrowed by the query: the publication rule is evaluated over all evidence, not over this slice";

/// The warning of a metric the publication rule was never evaluated for.
const SECONDARY: &str = "preliminary: the publication rule is evaluated for effective_token_reduction only, so this metric is a result, never a verified claim";

/// The metrics whose evidence is runs rather than pairs.
const RUN_BASED: [&str; 3] = [
    "completion_rate.",
    "tokens_per_completed_task.",
    "transcript_resume_avoided.",
];

/// One sampled value: the evidence key it is listed under, and the cluster (the suite and
/// task) whose other repetitions it is not independent of.
struct Sample {
    key: String,
    cluster: String,
    value: f64,
}

/// How an interval resamples its values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resampling {
    /// Whole tasks, because the repetitions of one task share its fixture and difficulty.
    Tasks,
    /// Each value on its own: the context suite's seeds, each a separate compilation.
    Independent,
}

fn cluster_of(p: &EconomicsPair) -> String {
    format!("{}/{}", p.suite, p.task)
}

/// The cluster-bootstrap interval of the median over samples, clustered by task.
fn task_interval(
    samples: &[Sample],
    policy: &EconomicsStatisticsPolicy,
) -> Option<EconomicsInterval> {
    let mut clusters: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for s in samples {
        clusters
            .entry(s.cluster.as_str())
            .or_default()
            .push(s.value);
    }
    let clusters: Vec<Vec<f64>> = clusters.into_values().collect();
    bootstrap_median_clustered(
        &clusters,
        policy.confidence_bp,
        policy.resamples,
        policy.seed,
        policy.min_pairs_for_interval,
    )
}

/// Why an interval over `n` values of `tasks` distinct tasks is absent, in the words a
/// statement uses.
fn no_interval(n: usize, policy: &EconomicsStatisticsPolicy) -> &'static str {
    if n < policy.min_pairs_for_interval.max(2) {
        "no interval: too few pairs"
    } else {
        "no interval: too few tasks, and the repetitions of one task are not independent"
    }
}

/// A metric's value, distribution and interval from its samples, with its status: none
/// without a value, `verified` only when the caller says the publication rule was evaluated
/// for exactly this metric and met, `preliminary` otherwise.
fn sampled(
    mut m: EconomicsMetric,
    samples: &[Sample],
    policy: &EconomicsStatisticsPolicy,
    resampling: Resampling,
    verified: bool,
) -> EconomicsMetric {
    let v: Vec<f64> = samples.iter().map(|s| s.value).collect();
    m.n = v.len();
    m.evidence = samples.iter().map(|s| s.key.clone()).collect();
    m.distribution = distribution(&v);
    m.value = m.distribution.as_ref().map(|d| d.median);
    m.interval = match resampling {
        Resampling::Tasks => task_interval(samples, policy),
        Resampling::Independent => bootstrap_median(
            &v,
            policy.confidence_bp,
            policy.resamples,
            policy.seed,
            policy.min_pairs_for_interval,
        ),
    };
    m.status = match (m.n, verified) {
        (0, _) => EconomicsMetricStatus::NotMeasured,
        (_, true) => EconomicsMetricStatus::Verified,
        _ => EconomicsMetricStatus::Preliminary,
    };
    if m.n > 0 && m.interval.is_none() {
        let tasks = samples
            .iter()
            .map(|s| s.cluster.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        if resampling == Resampling::Tasks && m.n >= policy.min_pairs_for_interval.max(2) {
            m.warnings.push(format!(
                "{} value(s) from {tasks} task(s): too few tasks for an interval, because the repetitions of one task are not independent",
                m.n
            ));
        } else {
            m.warnings.push(format!(
                "{} value(s): too few for an interval (the methodology computes one from {})",
                m.n, policy.min_pairs_for_interval
            ));
        }
    }
    m
}

/// A metric the publication rule was never evaluated for says so whenever it has a value.
fn secondary(mut m: EconomicsMetric) -> EconomicsMetric {
    if m.n > 0 {
        m.warnings.push(SECONDARY.into());
    }
    m
}

fn pair_key(p: &EconomicsPair) -> String {
    format!("{}/{}/r{}", p.suite, p.task, p.repetition)
}

/// A non-negative quantity (a width) as a percentage, unsigned.
fn width_text(x: f64) -> String {
    percent_text(x).trim_start_matches(['+', '-']).to_string()
}

/// An interval bound as a percentage: a minus sign when it is negative, no plus sign.
fn bound_text(x: f64) -> String {
    percent_text(x).trim_start_matches('+').to_string()
}

/// How the median moved, in a sentence's words. The verb follows the sign as it prints,
/// so a median that rounds to zero is `unchanged`, never `reduced by 0.0%`.
fn change_words(median: f64) -> String {
    let shown = percent_text(median);
    if let Some(x) = shown.strip_prefix('+') {
        format!("reduced median total token consumption by {x}")
    } else if let Some(x) = shown.strip_prefix('-') {
        format!("increased median total token consumption by {x}")
    } else {
        "left median total token consumption unchanged".into()
    }
}

/// The change and its interval, the interval stated in the same direction as the words: a
/// median increase is followed by an interval of increases, never by negative reductions.
fn change_with_interval(median: f64, interval: Option<&EconomicsInterval>, none: &str) -> String {
    let words = change_words(median);
    let Some(i) = interval else {
        return format!("{words} ({none})");
    };
    let range = if percent_text(median).starts_with('-') {
        format!(
            "an increase of {} to {}",
            bound_text(-i.high),
            bound_text(-i.low)
        )
    } else {
        format!(
            "a reduction of {} to {}",
            bound_text(i.low),
            bound_text(i.high)
        )
    };
    format!("{words} ({} CI: {range})", stats::level_text(i.level_bp))
}

/// The least-repeated declared task and variant of every live suite.
struct RepetitionFloor {
    reps: u32,
    of: String,
}

/// Every threshold of the publication rule the live evidence does not meet. Empty means a
/// quantitative total-token claim may be made.
fn unmet(
    policy: &EconomicsPublicationPolicy,
    valid: &[&EconomicsPair],
    attempted: usize,
    floor: Option<&RepetitionFloor>,
    interval: Option<&EconomicsInterval>,
    all_current: bool,
    dirty: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    if valid.len() < policy.min_valid_pairs {
        out.push(format!(
            "{} valid pair(s); the rule asks for {}",
            valid.len(),
            policy.min_valid_pairs
        ));
    }
    let mut per_category: BTreeMap<&str, usize> = BTreeMap::new();
    for p in valid {
        *per_category.entry(p.category.as_str()).or_insert(0) += 1;
    }
    let categories = per_category
        .values()
        .filter(|n| **n >= policy.min_pairs_per_category)
        .count();
    if categories < policy.min_categories {
        out.push(format!(
            "{categories} categor(ies) with at least {} valid pairs; the rule asks for {}",
            policy.min_pairs_per_category, policy.min_categories
        ));
    }
    match floor {
        None => out.push(format!(
            "no declared task and variant to repeat; the rule asks for {} repetition(s) of each",
            policy.min_repetitions
        )),
        Some(f) if f.reps < policy.min_repetitions => out.push(format!(
            "{} repetition(s) of {}, the least repeated; the rule asks for {} of every task and variant",
            f.reps, f.of, policy.min_repetitions
        )),
        Some(_) => {}
    }
    if let Some(rate) = (valid.len() * 10_000).checked_div(attempted) {
        if rate < policy.min_valid_pair_rate_bp as usize {
            out.push(format!(
                "{} of {attempted} attempted pairs are valid; the rule asks for {} in 10000",
                valid.len(),
                policy.min_valid_pair_rate_bp
            ));
        }
    }
    match interval {
        None => out.push("no interval: too few valid pairs, or too few tasks among them".into()),
        Some(i) if (i.high - i.low) * 10_000.0 > policy.max_interval_width_bp as f64 => {
            out.push(format!(
                "the interval is {} wide; the rule allows {} in 10000",
                width_text(i.high - i.low),
                policy.max_interval_width_bp
            ))
        }
        _ => {}
    }
    if policy.require_current && !all_current {
        out.push(
            "the evidence is not current: a measured mechanism changed since it was recorded"
                .into(),
        );
    }
    if dirty > 0 {
        out.push(format!(
            "{dirty} run(s) were recorded from a working tree with uncommitted changes"
        ));
    }
    out
}

fn matches(q: &EconomicsQuery, p: &EconomicsPair, model: &str) -> bool {
    q.suite.as_ref().is_none_or(|s| &p.suite == s)
        && q.category.as_ref().is_none_or(|c| &p.category == c)
        && q.task.as_ref().is_none_or(|t| &p.task == t)
        && q.model.as_ref().is_none_or(|m| m == model)
}

/// The runs of `variant` among the sides of `pairs`, each with its pair.
fn runs_of<'a>(
    pairs: &[&'a EconomicsPair],
    run_of: &BTreeMap<(&str, &str), &'a EconomicsRun>,
    variant: &str,
) -> Vec<(&'a EconomicsPair, &'a EconomicsRun)> {
    let mut out = Vec::new();
    for p in pairs {
        for id in [&p.control, &p.treatment].into_iter().flatten() {
            if let Some(r) = run_of.get(&(p.suite.as_str(), id.as_str())) {
                if r.variant == variant {
                    out.push((*p, *r));
                }
            }
        }
    }
    out
}

fn empty(statement: &str, diagnostics: Vec<String>) -> EconomicsSummary {
    EconomicsSummary {
        present: false,
        methodology: None,
        question: None,
        primary_metric: None,
        verdict: EconomicsVerdict {
            publishable: false,
            statement: statement.into(),
            unmet: Vec::new(),
        },
        metrics: Vec::new(),
        suites: Vec::new(),
        pairs: Vec::new(),
        segments: Vec::new(),
        context: None,
        history: Vec::new(),
        variants: Vec::new(),
        hypotheses: Vec::new(),
        publication: None,
        diagnostics,
    }
}

/// The statement made when there is nothing to make one from.
pub const NO_CLAIM: &str = "No verified total-token-savings claim is available";

/// The string a value serialises to, as every JSON surface spells it:
/// `EconomicsMetricStatus::NotMeasured` is `not_measured`. A renderer that prints an enum
/// uses this rather than `Debug`, so that a page and the API never spell one state two
/// ways. A value that does not serialise to a string is given as its JSON text.
///
/// ```
/// use majordomus_cli::economics::wire;
/// use majordomus_cli::economics::model::{EconomicsFreshness, EconomicsMetricStatus};
/// assert_eq!(wire(&EconomicsMetricStatus::NotMeasured), "not_measured");
/// assert_eq!(wire(&EconomicsFreshness::NoEvidence), "no_evidence");
/// assert_eq!(wire(&3), "3");
/// ```
pub fn wire<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(e) => format!("(not serialisable: {e})"),
    }
}

/// A context suite and what it recorded.
struct ContextEvidence<'a> {
    suite: &'a EconomicsSuite,
    runs: Vec<EconomicsContextRun>,
    freshness: EconomicsFreshness,
}

/// The one calculator. Reads the declarations and every recorded run under `root`, forms
/// and judges the pairs, derives every metric with its class, distribution and interval,
/// and states the strongest thing the publication rule allows. Never fails: what cannot be
/// read is a diagnostic, and a repository with no methodology gets `present: false`.
///
/// Every record file on disk is read, the ones not yet committed included, so that a
/// person sees what they just recorded; [`summarize_tracked`] is the same calculation over
/// the committed records only.
///
/// ```
/// use majordomus_cli::economics::{summarize, model::EconomicsQuery, DIR, NO_CLAIM};
/// let dir = tempfile::tempdir().unwrap();
/// let none = summarize(dir.path(), &EconomicsQuery::default());
/// assert!(!none.present && none.diagnostics.is_empty(), "not benchmarking is not a fault");
///
/// std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
/// std::fs::write(dir.path().join(DIR).join("methodology.yaml"), "schema: x\n").unwrap();
/// let broken = summarize(dir.path(), &EconomicsQuery::default());
/// assert!(!broken.present && !broken.verdict.publishable);
/// assert!(broken.verdict.statement.starts_with(NO_CLAIM));
/// assert_eq!(broken.diagnostics.len(), 1, "what cannot be read is reported, not raised");
/// ```
pub fn summarize(root: &Path, query: &EconomicsQuery) -> EconomicsSummary {
    summarize_from(root, query, Source::Directory)
}

/// [`summarize`] over the run records git tracks and nothing else: what `majordomus
/// generate` publishes. A generated artifact must be the same on every checkout of one
/// commit, and a record someone ran locally and never committed would make it depend on
/// whose checkout generated it. Tracked records are read from the working tree; a record
/// that is on disk and untracked is not read at all, not even to report it.
///
/// ```
/// use majordomus_cli::economics::{summarize, summarize_tracked, DIR};
/// let dir = tempfile::tempdir().unwrap();
/// # let decl_dir = dir.path().join(DIR);
/// # std::fs::create_dir_all(decl_dir.join("suites")).unwrap();
/// # std::fs::write(decl_dir.join("methodology.yaml"), [
/// #     "schema: economics-methodology/v1", "version: 1", "title: t", "question: q",
/// #     "unit: tokens", "primary_metric: effective_token_reduction",
/// #     "classes: []", "variants: []", "success: []",
/// #     "pairing:", "  key: []", "  comparable: []", "  valid: v",
/// #     "statistics:", "  per_pair: p", "  location: median", "  interval: bootstrap",
/// #     "  confidence_bp: 9500", "  resamples: 100", "  seed: 1", "  min_pairs_for_interval: 5",
/// #     "publication:", "  min_valid_pairs: 30", "  min_categories: 4",
/// #     "  min_pairs_per_category: 5", "  min_repetitions: 3", "  min_valid_pair_rate_bp: 8000",
/// #     "  max_interval_width_bp: 2000", "  require_current: true",
/// #     "outliers:", "  rule: none", "",
/// # ].join("\n")).unwrap();
/// # std::fs::write(decl_dir.join("suites/pilot.yaml"), [
/// #     "schema: economics-suite/v1", "id: pilot", "version: 1", "kind: live", "title: Pilot",
/// #     "control: baseline", "treatment: majordomus", "freshness_inputs: [methodology.yaml]", "",
/// # ].join("\n")).unwrap();
/// # let status = std::process::Command::new("git")
/// #     .env_remove("GIT_DIR").env_remove("GIT_WORK_TREE").env_remove("GIT_INDEX_FILE")
/// #     .arg("-C").arg(dir.path()).args(["init", "-q"]).status().unwrap();
/// # assert!(status.success());
/// // a git repository declaring one live suite `pilot`; one record, never committed
/// let recorded = dir.path().join(DIR).join("runs/pilot");
/// std::fs::create_dir_all(&recorded).unwrap();
/// std::fs::write(recorded.join("r1.json"), "{").unwrap();
/// let q = Default::default();
/// assert_eq!(summarize(dir.path(), &q).diagnostics.len(), 1, "a person sees the local record");
/// assert!(summarize_tracked(dir.path(), &q).diagnostics.is_empty(), "generation does not");
/// ```
pub fn summarize_tracked(root: &Path, query: &EconomicsQuery) -> EconomicsSummary {
    summarize_from(root, query, Source::Tracked)
}

fn summarize_from(root: &Path, query: &EconomicsQuery, source: Source) -> EconomicsSummary {
    let decl = match load(root) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return empty(
                &format!("{NO_CLAIM}: no benchmark methodology is declared in this repository."),
                Vec::new(),
            )
        }
        Err(errors) => {
            return empty(
                &format!("{NO_CLAIM}: the benchmark declarations could not be read."),
                errors,
            )
        }
    };
    let m = &decl.methodology;
    let mut diagnostics = Vec::new();
    let mut suite_views = Vec::new();
    let mut all_pairs: Vec<Judged> = Vec::new();
    let mut live_runs: Vec<EconomicsRun> = Vec::new();
    let mut context_runs: Vec<ContextEvidence> = Vec::new();
    let mut all_current = true;
    let mut floor: Option<RepetitionFloor> = None;
    let mut history = Vec::new();

    for suite in decl.suites.values() {
        match suite.kind.as_str() {
            "live" => {
                let (runs, errors) = load_runs_from(root, &suite.id, source);
                diagnostics.extend(errors);
                let runs: Vec<EconomicsRun> = runs.into_iter().map(|(_, r)| r).collect();
                let recorded: Vec<(u32, String)> = runs
                    .iter()
                    .map(|r| (r.methodology, r.inputs_digest.clone()))
                    .collect();
                let (fresh, detail) = freshness(root, &decl, suite, &recorded);
                if !matches!(
                    fresh,
                    EconomicsFreshness::Current | EconomicsFreshness::NoEvidence
                ) {
                    all_current = false;
                }
                let judged = judged_pairs(&decl, suite, &runs);
                let mut counts = EconomicsPairCounts {
                    declared: suite.tasks.len() * suite.repetitions.unwrap_or(1) as usize,
                    ..Default::default()
                };
                for p in judged.iter().map(|j| &j.pair) {
                    if p.control.is_some() || p.treatment.is_some() {
                        counts.attempted += 1;
                    }
                    match p.status {
                        EconomicsPairStatus::Valid => counts.valid += 1,
                        EconomicsPairStatus::ControlFailed => counts.control_failed += 1,
                        EconomicsPairStatus::TreatmentFailed => counts.treatment_failed += 1,
                        EconomicsPairStatus::BothFailed => counts.both_failed += 1,
                        _ if p.control.is_some() || p.treatment.is_some() => counts.other += 1,
                        _ => {}
                    }
                }
                // the rule asks for repetitions of every task and variant: count, for each,
                // the distinct repetitions that ran under this methodology (a failed run
                // ran), and keep the least; tasks in id order, so that a tie names the same
                // task however the suite lists them
                for task in suite.tasks.iter().collect::<BTreeSet<_>>() {
                    for variant in [&suite.control, &suite.treatment].into_iter().flatten() {
                        let reps = runs
                            .iter()
                            .filter(|r| r.methodology == m.version && r.suite == suite.id)
                            .filter(|r| &r.task == task && &r.variant == variant)
                            .map(|r| r.repetition)
                            .collect::<BTreeSet<_>>()
                            .len() as u32;
                        if floor.as_ref().is_none_or(|f| reps < f.reps) {
                            floor = Some(RepetitionFloor {
                                reps,
                                of: format!("{task} ({variant}) in suite {}", suite.id),
                            });
                        }
                    }
                }
                // one history point per revision the suite's valid pairs were recorded at
                let mut by_rev: BTreeMap<String, Vec<f64>> = BTreeMap::new();
                for p in judged
                    .iter()
                    .map(|j| &j.pair)
                    .filter(|p| p.status == EconomicsPairStatus::Valid)
                {
                    if let Some(r) = runs.iter().find(|r| Some(&r.id) == p.control.as_ref()) {
                        if let Some(x) = p.token_reduction {
                            by_rev
                                .entry(r.repository.commit.clone())
                                .or_default()
                                .push(x);
                        }
                    }
                }
                for (rev, v) in by_rev {
                    let at = runs
                        .iter()
                        .filter(|r| r.repository.commit == rev)
                        .map(|r| r.finished_at.clone())
                        .max()
                        .unwrap_or_default();
                    if let Some(d) = distribution(&v) {
                        history.push(EconomicsHistoryPoint {
                            suite: suite.id.clone(),
                            methodology: m.version,
                            revision: rev,
                            at,
                            metric: EFFECTIVE_TOKEN_REDUCTION.into(),
                            value: d.median,
                            n: d.n,
                        });
                    }
                }
                suite_views.push(EconomicsSuiteView {
                    id: suite.id.clone(),
                    kind: suite.kind.clone(),
                    version: suite.version,
                    title: suite.title.clone(),
                    model: suite.model.clone(),
                    freshness: fresh,
                    freshness_detail: detail,
                    runs: runs.len(),
                    pairs: counts,
                    revisions: runs
                        .iter()
                        .map(|r| r.repository.commit.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    harnesses: runs
                        .iter()
                        .map(|r| format!("{} {}", r.harness.name, r.harness.version))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    models_reported: runs
                        .iter()
                        .flat_map(|r| r.models_reported.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                });
                all_pairs.extend(judged);
                live_runs.extend(runs);
            }
            "context" => {
                let (runs, errors) = load_context_runs_from(root, &suite.id, source);
                diagnostics.extend(errors);
                let recorded: Vec<(u32, String)> = runs
                    .iter()
                    .map(|r| (r.methodology, r.inputs_digest.clone()))
                    .collect();
                let latest_only: Vec<(u32, String)> =
                    recorded.last().cloned().into_iter().collect();
                let (fresh, detail) = freshness(root, &decl, suite, &latest_only);
                for r in &runs {
                    let v: Vec<f64> = r
                        .seeds
                        .iter()
                        .filter_map(|s| reduction(s.candidate_tokens, s.selected_tokens))
                        .collect();
                    if let Some(d) = distribution(&v) {
                        history.push(EconomicsHistoryPoint {
                            suite: suite.id.clone(),
                            methodology: r.methodology,
                            revision: r.repository.commit.clone(),
                            at: r.measured_at.clone(),
                            metric: CONTEXT_REDUCTION.into(),
                            value: d.median,
                            n: d.n,
                        });
                    }
                }
                suite_views.push(EconomicsSuiteView {
                    id: suite.id.clone(),
                    kind: suite.kind.clone(),
                    version: suite.version,
                    title: suite.title.clone(),
                    model: None,
                    freshness: fresh,
                    freshness_detail: detail,
                    runs: runs.len(),
                    pairs: EconomicsPairCounts::default(),
                    revisions: runs
                        .iter()
                        .map(|r| r.repository.commit.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    harnesses: Vec::new(),
                    models_reported: Vec::new(),
                });
                context_runs.push(ContextEvidence {
                    suite,
                    runs,
                    freshness: fresh,
                });
            }
            other => diagnostics.push(format!("suite {}: unknown kind {other:?}", suite.id)),
        }
    }

    // a run is found by its suite and id: ids are unique within a suite, not across them
    let mut run_of: BTreeMap<(&str, &str), &EconomicsRun> = BTreeMap::new();
    for r in &live_runs {
        run_of.entry((r.suite.as_str(), r.id.as_str())).or_insert(r);
    }
    let token_samples = |ps: &[&EconomicsPair]| -> Vec<Sample> {
        ps.iter()
            .filter_map(|p| {
                p.token_reduction.map(|x| Sample {
                    key: pair_key(p),
                    cluster: cluster_of(p),
                    value: x,
                })
            })
            .collect()
    };

    // ------------------------------------------------ the verdict, over all live evidence
    let valid_all: Vec<&EconomicsPair> = all_pairs
        .iter()
        .map(|j| &j.pair)
        .filter(|p| p.status == EconomicsPairStatus::Valid)
        .collect();
    let attempted = all_pairs
        .iter()
        .filter(|j| j.pair.control.is_some() || j.pair.treatment.is_some())
        .count();
    let samples_all = token_samples(&valid_all);
    let reductions_all: Vec<f64> = samples_all.iter().map(|s| s.value).collect();
    let interval_all = task_interval(&samples_all, &m.statistics);
    let dirty = live_runs.iter().filter(|r| r.repository.dirty).count();
    let unmet_list = unmet(
        &m.publication,
        &valid_all,
        attempted,
        floor.as_ref(),
        interval_all.as_ref(),
        all_current,
        dirty,
    );
    let publishable = unmet_list.is_empty() && !valid_all.is_empty();
    // what the statement names is what the figure rests on: the runs of valid pairs
    let valid_runs: Vec<&EconomicsRun> = valid_all
        .iter()
        .flat_map(|p| {
            [&p.control, &p.treatment]
                .into_iter()
                .flatten()
                .map(move |id| (p.suite.as_str(), id.as_str()))
        })
        .filter_map(|k| run_of.get(&k).copied())
        .collect();
    let models: Vec<String> = valid_runs
        .iter()
        .flat_map(|r| r.models_reported.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let models_text = if models.is_empty() {
        "a model the harness did not name".to_string()
    } else {
        models.join(", ")
    };
    let revisions: BTreeSet<&str> = valid_runs
        .iter()
        .map(|r| r.repository.commit.as_str())
        .collect();
    let tasks: BTreeSet<(&str, &str)> = valid_all
        .iter()
        .map(|p| (p.suite.as_str(), p.task.as_str()))
        .collect();
    let categories: BTreeSet<&str> = valid_all.iter().map(|p| p.category.as_str()).collect();
    let none = no_interval(reductions_all.len(), &m.statistics);
    let mut statement = match (distribution(&reductions_all), publishable) {
        (None, _) => format!(
            "{NO_CLAIM}: no valid matched pair of live runs is recorded under methodology {}.",
            m.version
        ),
        (Some(d), true) => format!(
            "Across methodology {}, {} valid matched pairs across {} tasks in {} categories, at revision(s) {}, using {}, Majordomus {}, with every run passing the same acceptance tests.",
            m.version,
            d.n,
            tasks.len(),
            categories.len(),
            revisions.iter().map(|r| &r[..r.len().min(12)]).collect::<Vec<_>>().join(", "),
            models_text,
            change_with_interval(d.median, interval_all.as_ref(), none),
        ),
        (Some(d), false) => format!(
            "{NO_CLAIM}. Preliminary observation, below the publication rule: over {} valid matched pair(s) across {} task(s) in {} categor(ies), using {}, Majordomus {}.",
            d.n,
            tasks.len(),
            categories.len(),
            models_text,
            change_with_interval(d.median, interval_all.as_ref(), none),
        ),
    };
    let excluded_ids: BTreeSet<&str> = m.outliers.excluded.iter().map(|x| x.run.as_str()).collect();
    let excluded_runs: BTreeSet<(&str, &str)> = all_pairs
        .iter()
        .flat_map(|j| {
            [&j.pair.control, &j.pair.treatment]
                .into_iter()
                .flatten()
                .map(move |id| (j.pair.suite.as_str(), id.as_str()))
        })
        .filter(|(_, id)| excluded_ids.contains(id))
        .collect();
    if !excluded_runs.is_empty() {
        statement.push_str(&format!(
            " {} run(s) excluded by the methodology's outlier rule are left out",
            excluded_runs.len()
        ));
        let with_excluded: Vec<&EconomicsPair> = all_pairs
            .iter()
            .filter(|j| j.underlying == EconomicsPairStatus::Valid)
            .map(|j| &j.pair)
            .collect();
        let v: Vec<f64> = token_samples(&with_excluded)
            .into_iter()
            .map(|s| s.value)
            .collect();
        match distribution(&v) {
            Some(d) => statement.push_str(&format!(
                "; with them included, over {} pair(s), Majordomus {} (preliminary).",
                d.n,
                change_words(d.median)
            )),
            None => statement.push('.'),
        }
    }

    // ------------------------------------------------ narrowed pairs and metrics
    let narrowed = query.suite.is_some()
        || query.category.is_some()
        || query.task.is_some()
        || query.model.is_some();
    let pairs_q: Vec<&Judged> = all_pairs
        .iter()
        .filter(|j| matches(query, &j.pair, &j.model))
        .collect();
    let valid: Vec<&EconomicsPair> = pairs_q
        .iter()
        .map(|j| &j.pair)
        .filter(|p| p.status == EconomicsPairStatus::Valid)
        .collect();
    let values = |f: &dyn Fn(&EconomicsPair) -> Option<f64>| -> Vec<Sample> {
        valid
            .iter()
            .filter_map(|p| {
                f(p).map(|x| Sample {
                    key: pair_key(p),
                    cluster: cluster_of(p),
                    value: x,
                })
            })
            .collect()
    };
    let live_suite = decl
        .suites
        .values()
        .filter(|s| s.kind == "live")
        .map(|s| s.id.clone())
        .collect::<Vec<_>>()
        .join(",");
    let mut metrics = Vec::new();
    // the publication rule is evaluated for this metric over all the evidence, and for
    // nothing else: it is verified only unnarrowed and only when the rule is met
    let mut primary = sampled(
        metric(
            MetricSpec {
                id: EFFECTIVE_TOKEN_REDUCTION,
                title: "Effective total-token reduction",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Observed),
                unit: "ratio",
                formula: "median over valid pairs of 1 - treatment_total_tokens / control_total_tokens, where total tokens are all input (uncached, cache write, cache read) and all output the provider reported for every session of the run",
                not: Some("not context reduction: this is everything the provider reported for the whole task, not what one mechanism selected"),
            },
            &live_suite,
        ),
        &values(&|p| p.token_reduction),
        &m.statistics,
        Resampling::Tasks,
        publishable && !narrowed,
    );
    if primary.n > 0 && !publishable {
        primary
            .warnings
            .push("preliminary: the evidence is below the publication rule".into());
    }
    if narrowed {
        primary.warnings.push(NARROWED.into());
    }
    if !valid.is_empty() {
        primary.warnings.push(
            "cache reads are counted in total tokens at full weight; the cost reduction weighs them by price".into(),
        );
    }
    metrics.push(primary);
    if !m.outliers.excluded.is_empty() {
        let put_back: Vec<&EconomicsPair> = pairs_q
            .iter()
            .filter(|j| j.underlying == EconomicsPairStatus::Valid)
            .map(|j| &j.pair)
            .collect();
        let excluded_pairs = put_back
            .iter()
            .filter(|p| p.status == EconomicsPairStatus::Excluded)
            .count();
        let mut including = secondary(sampled(
            metric(
                MetricSpec {
                    id: EFFECTIVE_TOKEN_REDUCTION_INCLUDING_EXCLUDED,
                    title: "Effective total-token reduction, excluded runs included",
                    class: EconomicsClass::Derived,
                    inputs: Some(EconomicsClass::Observed),
                    unit: "ratio",
                    formula: "the formula of effective_token_reduction, over the valid pairs and the pairs the outlier rule excluded that would otherwise be valid",
                    not: Some("not the primary metric: the methodology excludes these runs, and this figure is reported beside the primary one so that the exclusion's effect is visible"),
                },
                &live_suite,
            ),
            &token_samples(&put_back),
            &m.statistics,
            Resampling::Tasks,
            false,
        ));
        including.warnings.push(format!(
            "includes {excluded_pairs} pair(s) the outlier rule excluded; every other metric leaves them out"
        ));
        metrics.push(including);
    }
    let mut cost = secondary(sampled(
        metric(
            MetricSpec {
                id: "cost_reduction",
                title: "Cost reduction, as the harness priced it",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Observed),
                unit: "ratio",
                formula: "median over valid pairs of 1 - treatment_cost / control_cost, where cost is the harness's own total_cost_usd at run time",
                not: Some("not a bill: the harness prices usage from its own table at run time, and historical runs are never re-priced"),
            },
            &live_suite,
        ),
        &values(&|p| p.cost_reduction),
        &m.statistics,
        Resampling::Tasks,
        false,
    ));
    // the harnesses that priced the pairs this figure rests on, and no other
    let harnesses: BTreeSet<String> = valid
        .iter()
        .flat_map(|p| {
            [&p.control, &p.treatment]
                .into_iter()
                .flatten()
                .map(move |id| (p.suite.as_str(), id.as_str()))
        })
        .filter_map(|k| run_of.get(&k))
        .map(|r| format!("{} {}", r.harness.name, r.harness.version))
        .collect();
    if !harnesses.is_empty() {
        cost.warnings.push(format!(
            "priced by {}",
            harnesses.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    metrics.push(cost);
    metrics.push(secondary(sampled(
        metric(
            MetricSpec {
                id: "tool_call_reduction",
                title: "Tool-call reduction",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Observed),
                unit: "ratio",
                formula: "median over valid pairs of 1 - treatment_tool_calls / control_tool_calls",
                not: Some("not a token metric"),
            },
            &live_suite,
        ),
        &values(&|p| p.tool_call_reduction),
        &m.statistics,
        Resampling::Tasks,
        false,
    )));
    metrics.push(secondary(sampled(
        metric(
            MetricSpec {
                id: "continuation_reduction",
                title: "Continuation-session token reduction",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Observed),
                unit: "ratio",
                formula: "median over valid multi-session pairs of 1 - the treatment's last-session total tokens / the control's",
                not: Some("only the session that continued the work, not the whole task"),
            },
            &live_suite,
        ),
        &values(&|p| p.continuation_reduction),
        &m.statistics,
        Resampling::Tasks,
        false,
    )));
    metrics.push(secondary(sampled(
        metric(
            MetricSpec {
                id: "first_request_overhead",
                title: "What the treatment adds before the model acts",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Observed),
                unit: "tokens",
                formula: "median over valid pairs of the treatment's first-request input tokens minus the control's: the instruction files and hook output a session is given before it has done anything",
                not: Some("not the whole overhead: what the treatment reads later because its instructions say to is in the total, not here"),
            },
            &live_suite,
        ),
        &values(&|p| p.first_request_overhead.map(|x| x as f64)),
        &m.statistics,
        Resampling::Tasks,
        false,
    )));
    // the run-based metrics read the same population as the pairs: the query's slice, and
    // only runs whose pair can be compared at all
    let comparable: Vec<&EconomicsPair> = pairs_q
        .iter()
        .map(|j| &j.pair)
        .filter(|p| {
            !matches!(
                p.status,
                EconomicsPairStatus::Missing
                    | EconomicsPairStatus::Incomparable
                    | EconomicsPairStatus::Excluded
            )
        })
        .collect();
    for variant in &m.variants {
        let recorded = runs_of(&comparable, &run_of, &variant.id);
        let completed = recorded.iter().filter(|(_, r)| r.outcome.completed).count();
        let mut rate = metric(
            MetricSpec {
                id: &format!("completion_rate.{}", variant.id),
                title: &format!("Completion rate, {}", variant.title),
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Observed),
                unit: "ratio",
                formula: "runs that passed every success gate / runs recorded, over the runs of pairs whose two sides were both recorded, are comparable and are not excluded; a failed run counts, which is the point",
                not: Some("not a quality score: a run passes the gates or it does not"),
            },
            &live_suite,
        );
        if !recorded.is_empty() {
            rate.n = recorded.len();
            rate.value = Some(stats::round(completed as f64 / recorded.len() as f64));
            rate.status = EconomicsMetricStatus::Preliminary;
            rate.evidence = recorded.iter().map(|(_, r)| r.id.clone()).collect();
        }
        metrics.push(secondary(rate));
        let in_valid = runs_of(&valid, &run_of, &variant.id);
        let per_task: Vec<Sample> = in_valid
            .iter()
            .filter_map(|(p, r)| {
                usage::totals(r).map(|u| Sample {
                    key: r.id.clone(),
                    cluster: cluster_of(p),
                    value: u.total as f64,
                })
            })
            .collect();
        metrics.push(secondary(sampled(
            metric(
                MetricSpec {
                    id: &format!("tokens_per_completed_task.{}", variant.id),
                    title: &format!("Total tokens per completed task, {}", variant.title),
                    class: EconomicsClass::Observed,
                    inputs: None,
                    unit: "tokens",
                    formula: "median over the runs of valid pairs of the total tokens the provider reported for the run, so that both arms describe the same tasks",
                    not: None,
                },
                &live_suite,
            ),
            &per_task,
            &m.statistics,
            Resampling::Tasks,
            false,
        )));
        let resume: Vec<Sample> = in_valid
            .iter()
            .filter(|(_, r)| r.sessions.len() > 1)
            .filter_map(|(p, r)| {
                let prev = r.sessions.get(r.sessions.len() - 2)?;
                let last_input = prev.requests.last().map(|q| {
                    q.input_tokens + q.cache_creation_input_tokens + q.cache_read_input_tokens
                })?;
                let first = usage::first_request_input(r.sessions.last()?)?;
                Some(Sample {
                    key: r.id.clone(),
                    cluster: cluster_of(p),
                    value: last_input as f64 - first as f64,
                })
            })
            .collect();
        let mut cf = sampled(
            metric(
                MetricSpec {
                    id: &format!("transcript_resume_avoided.{}", variant.id),
                    title: &format!("Context not re-sent by starting the next session fresh, {}", variant.title),
                    class: EconomicsClass::Counterfactual,
                    inputs: Some(EconomicsClass::Observed),
                    unit: "tokens",
                    formula: "over the runs of valid pairs, the previous session's last-request input minus the next session's first-request input: what resuming the previous transcript would have re-sent at the start, against what a fresh session was given",
                    not: Some("not a saving of Majordomus: both arms start the next session fresh; this is the modelled cost of resuming a transcript instead"),
                },
                &live_suite,
            ),
            &resume,
            &m.statistics,
            Resampling::Tasks,
            false,
        );
        cf.warnings
            .push("counterfactual: modelled from observed values, never a measured saving".into());
        metrics.push(cf);
    }

    // ------------------------------------------------ the context suite
    let mut context_view = None;
    for evidence in &context_runs {
        let suite = evidence.suite;
        let mut ratio = metric(
            MetricSpec {
                id: CONTEXT_REDUCTION,
                title: "Context reduction by the context compiler",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Counted),
                unit: "ratio",
                formula: "median over seeds of 1 - selected_tokens / candidate_tokens, where candidates are the distinct files the compiler judged relevant to the work (selected, or left out only for budget) and selected are those it put in the budget, counted with the named tokenizer; files it judged irrelevant are reported, never divided by",
                not: Some("not total token savings: it says what the compiler selected from what it found relevant, not what a session consumed, and a session remains free to read anything"),
            },
            &suite.id,
        );
        let Some(latest) = evidence.runs.last() else {
            metrics.push(ratio);
            continue;
        };
        // a record under another methodology measured something else: it is shown as a
        // record, and no value is taken from it
        let compatible = latest.methodology == m.version;
        let mut standing: Vec<String> = Vec::new();
        if compatible {
            if evidence.freshness == EconomicsFreshness::Stale {
                standing.push("stale: a measured mechanism changed since this record".into());
            }
            if latest.repository.dirty {
                standing.push(
                    "dirty: this record was measured from a working tree with uncommitted changes"
                        .into(),
                );
            }
        }
        let incompatible = format!(
            "incompatible: the latest record was measured under methodology {}; the current methodology is {}",
            latest.methodology, m.version
        );
        let refused = (!latest.refused.is_empty()).then(|| {
            format!(
                "{} seed(s) refused by the compiler were not measured",
                latest.refused.len()
            )
        });
        let per_seed: Vec<Sample> = if compatible {
            latest
                .seeds
                .iter()
                .filter_map(|s| {
                    reduction(s.candidate_tokens, s.selected_tokens).map(|r| Sample {
                        key: s.seed.clone(),
                        cluster: s.seed.clone(),
                        value: r,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        ratio = sampled(
            ratio,
            &per_seed,
            &m.statistics,
            Resampling::Independent,
            false,
        );
        if ratio.n > 0 {
            ratio.status = EconomicsMetricStatus::Measured;
            ratio.warnings.push(format!(
                "tokens counted with {} ({}), which is not the tokenizer of every model",
                latest.tokenizer.encoding, latest.tokenizer.implementation
            ));
            ratio.warnings.push(
                "the candidates are what the compiler's own graph walk judged relevant; a worker without the compiler would not necessarily have read them, so this ratio describes the selection, not a session's saving".into(),
            );
            ratio.warnings.extend(standing.iter().cloned());
        } else if compatible {
            ratio
                .warnings
                .push("not measured: no seed of the latest record yields a ratio".into());
        } else {
            ratio.warnings.push(incompatible.clone());
        }
        ratio.warnings.extend(refused.iter().cloned());
        metrics.push(ratio);
        let sum =
            |f: &dyn Fn(&EconomicsContextSeed) -> u64| latest.seeds.iter().map(f).sum::<u64>();
        let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
        for s in &latest.seeds {
            for (k, v) in &s.excluded_tokens {
                *excluded.entry(k.clone()).or_insert(0) += v;
            }
        }
        let selected = sum(&|s| s.selected_tokens);
        let estimated = sum(&|s| s.estimated_selected_tokens);
        let mut model_error = metric(
            MetricSpec {
                id: "context_cost_model_error",
                title: "Error of the compiler's bytes-over-four cost model",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Counted),
                unit: "ratio",
                formula: "counted selected tokens / the compiler's own estimate of them - 1, over all seeds",
                not: Some("not a saving: it says how far the budget's unit is from counted tokens"),
            },
            &suite.id,
        );
        if compatible && estimated > 0 {
            model_error.value = Some(stats::round(selected as f64 / estimated as f64 - 1.0));
            model_error.n = latest.seeds.len();
            model_error.status = EconomicsMetricStatus::Measured;
            model_error.evidence = latest.seeds.iter().map(|s| s.seed.clone()).collect();
            model_error.warnings.extend(standing.iter().cloned());
        } else if compatible {
            model_error
                .warnings
                .push("not measured: the latest record estimates no selected tokens".into());
        } else {
            model_error.warnings.push(incompatible);
        }
        model_error.warnings.extend(refused);
        metrics.push(model_error);
        context_view = Some(EconomicsContextView {
            revision: latest.repository.commit.clone(),
            measured_at: latest.measured_at.clone(),
            tokenizer: latest.tokenizer.clone(),
            seeds: latest.seeds.len(),
            candidate_tokens: sum(&|s| s.candidate_tokens),
            considered_tokens: sum(&|s| s.considered_tokens),
            selected_tokens: selected,
            estimated_selected_tokens: estimated,
            excluded_tokens: excluded,
            seeds_over_budget_counted: latest
                .seeds
                .iter()
                .filter(|s| s.selected_tokens > latest.budget_tokens)
                .count(),
            budget_tokens: latest.budget_tokens,
        });
    }
    if context_runs.is_empty() {
        metrics.push(metric(
            MetricSpec {
                id: CONTEXT_REDUCTION,
                title: "Context reduction by the context compiler",
                class: EconomicsClass::Derived,
                inputs: Some(EconomicsClass::Counted),
                unit: "ratio",
                formula: "median over seeds of 1 - selected_tokens / candidate_tokens",
                not: Some("not total token savings"),
            },
            "context",
        ));
    }

    // ------------------------------------------------ segments
    let mut segments = Vec::new();
    for (dimension, key) in [
        (
            "category",
            &(|p: &EconomicsPair| p.category.clone()) as &dyn Fn(&EconomicsPair) -> String,
        ),
        ("sessions", &|p: &EconomicsPair| {
            format!("{} session(s)", p.sessions)
        }),
        ("complexity", &|p: &EconomicsPair| {
            decl.tasks
                .get(&p.task)
                .map(|t| t.complexity.clone())
                .unwrap_or_default()
        }),
    ] {
        let mut groups: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for p in &valid {
            if let Some(x) = p.token_reduction {
                groups.entry(key(p)).or_default().push(x);
            }
        }
        for (value, v) in groups {
            segments.push(EconomicsSegment {
                dimension: dimension.into(),
                value,
                n: v.len(),
                token_reduction: distribution(&v),
            });
        }
    }
    history.sort_by(|a, b| {
        (a.suite.as_str(), a.at.as_str(), a.revision.as_str()).cmp(&(
            b.suite.as_str(),
            b.at.as_str(),
            b.revision.as_str(),
        ))
    });

    EconomicsSummary {
        present: true,
        methodology: Some(m.version),
        question: Some(m.question.clone()),
        primary_metric: Some(m.primary_metric.clone()),
        verdict: EconomicsVerdict {
            publishable,
            statement,
            unmet: unmet_list,
        },
        metrics,
        suites: suite_views,
        pairs: pairs_q.iter().map(|j| j.pair.clone()).collect(),
        segments,
        context: context_view,
        history,
        variants: m.variants.clone(),
        hypotheses: m.hypotheses.clone(),
        publication: Some(m.publication.clone()),
        diagnostics,
    }
}

/// Explain one metric: its value, formula and class, and every pair, run, suite and
/// exclusion it rests on, with the commands that reproduce it. `Err` names the metrics that
/// exist when `metric` is not one of them.
///
/// A metric over pairs lists the valid pairs it rests on and the invalid pairs of the same
/// suites, so that what was left out is in view; a metric over runs (completion rate,
/// tokens per completed task, transcript resume) lists every pair one of its runs belongs
/// to, failed ones included; a metric over context seeds lists no pair.
///
/// ```
/// use majordomus_cli::economics::{explain, EFFECTIVE_TOKEN_REDUCTION};
/// use majordomus_cli::economics::model::EconomicsMetricStatus;
/// let dir = tempfile::tempdir().unwrap();
/// # use majordomus_cli::economics::DIR;
/// # let decl_dir = dir.path().join(DIR);
/// # std::fs::create_dir_all(decl_dir.join("suites")).unwrap();
/// # std::fs::write(decl_dir.join("methodology.yaml"), [
/// #     "schema: economics-methodology/v1", "version: 1", "title: t", "question: q",
/// #     "unit: tokens", "primary_metric: effective_token_reduction",
/// #     "classes: []", "variants: []", "success: []",
/// #     "pairing:", "  key: []", "  comparable: []", "  valid: v",
/// #     "statistics:", "  per_pair: p", "  location: median", "  interval: bootstrap",
/// #     "  confidence_bp: 9500", "  resamples: 100", "  seed: 1", "  min_pairs_for_interval: 5",
/// #     "publication:", "  min_valid_pairs: 30", "  min_categories: 4",
/// #     "  min_pairs_per_category: 5", "  min_repetitions: 3", "  min_valid_pair_rate_bp: 8000",
/// #     "  max_interval_width_bp: 2000", "  require_current: true",
/// #     "outliers:", "  rule: none", "",
/// # ].join("\n")).unwrap();
/// # std::fs::write(decl_dir.join("suites/pilot.yaml"), [
/// #     "schema: economics-suite/v1", "id: pilot", "version: 1", "kind: live", "title: Pilot",
/// #     "control: baseline", "treatment: majordomus", "freshness_inputs: [methodology.yaml]", "",
/// # ].join("\n")).unwrap();
/// // a methodology and one live suite `pilot`, with no run recorded
/// let e = explain(dir.path(), EFFECTIVE_TOKEN_REDUCTION).unwrap();
/// assert_eq!(e.metric.status, EconomicsMetricStatus::NotMeasured);
/// assert!(e.reproduce.iter().any(|c| c.starts_with("majordomus economics run --suite pilot")));
/// let refused = explain(dir.path(), "tokens_saved").unwrap_err();
/// assert!(refused.contains(EFFECTIVE_TOKEN_REDUCTION), "the refusal lists what exists");
/// ```
pub fn explain(root: &Path, metric: &str) -> Result<EconomicsExplanation, String> {
    let summary = summarize(root, &EconomicsQuery::default());
    let Some(found) = summary.metrics.iter().find(|x| x.id == metric).cloned() else {
        let known: Vec<&str> = summary.metrics.iter().map(|x| x.id.as_str()).collect();
        return Err(format!(
            "no metric {metric:?}; the metrics are: {}",
            known.join(", ")
        ));
    };
    let decl = load(root).ok().flatten();
    let class_meaning = decl
        .as_ref()
        .and_then(|d| d.methodology.classes.iter().find(|c| c.id == found.class))
        .map(|c| c.meaning.clone())
        .unwrap_or_default();
    let evidence: BTreeSet<&str> = found.evidence.iter().map(String::as_str).collect();
    let of_suite = |s: &str| found.suite.split(',').any(|x| x == s);
    let pairs: Vec<EconomicsPair> = if found.class == EconomicsClass::Derived
        && found.inputs == Some(EconomicsClass::Counted)
    {
        Vec::new()
    } else if RUN_BASED.iter().any(|p| found.id.starts_with(p)) {
        summary
            .pairs
            .iter()
            .filter(|p| of_suite(&p.suite))
            .filter(|p| {
                [&p.control, &p.treatment]
                    .into_iter()
                    .flatten()
                    .any(|id| evidence.contains(id.as_str()))
            })
            .cloned()
            .collect()
    } else {
        summary
            .pairs
            .iter()
            .filter(|p| {
                evidence.contains(pair_key(p).as_str())
                    || (p.status != EconomicsPairStatus::Valid && of_suite(&p.suite))
            })
            .cloned()
            .collect()
    };
    let suites: Vec<EconomicsSuiteView> = summary
        .suites
        .iter()
        .filter(|s| of_suite(&s.id))
        .cloned()
        .collect();
    let reproduce = suites
        .iter()
        .map(|s| match s.kind.as_str() {
            "live" => format!("majordomus economics run --suite {}   # live: needs a provider credential and spends usage", s.id),
            _ => format!("majordomus economics measure --suite {}   # deterministic: no model is called", s.id),
        })
        .chain(std::iter::once(format!("majordomus economics explain {metric}")))
        .collect();
    Ok(EconomicsExplanation {
        class_meaning,
        methodology: summary.methodology.unwrap_or(0),
        suites,
        pairs,
        excluded: decl
            .map(|d| d.methodology.outliers.excluded)
            .unwrap_or_default(),
        variants: summary.variants.clone(),
        reproduce,
        metric: found,
    })
}

/// Every recorded live run, narrowed by the query, with its derived totals: the raw facts
/// behind every number, one call away.
///
/// Only live suites are read, and a query naming one suite reads no other suite's records,
/// so a broken record elsewhere is not reported against it.
///
/// ```
/// use majordomus_cli::economics::{runs, model::EconomicsRunsQuery, DIR};
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(runs(dir.path(), &EconomicsRunsQuery::default()).count, 0);
/// # let decl_dir = dir.path().join(DIR);
/// # std::fs::create_dir_all(decl_dir.join("suites")).unwrap();
/// # std::fs::write(decl_dir.join("methodology.yaml"), [
/// #     "schema: economics-methodology/v1", "version: 1", "title: t", "question: q",
/// #     "unit: tokens", "primary_metric: effective_token_reduction",
/// #     "classes: []", "variants: []", "success: []",
/// #     "pairing:", "  key: []", "  comparable: []", "  valid: v",
/// #     "statistics:", "  per_pair: p", "  location: median", "  interval: bootstrap",
/// #     "  confidence_bp: 9500", "  resamples: 100", "  seed: 1", "  min_pairs_for_interval: 5",
/// #     "publication:", "  min_valid_pairs: 30", "  min_categories: 4",
/// #     "  min_pairs_per_category: 5", "  min_repetitions: 3", "  min_valid_pair_rate_bp: 8000",
/// #     "  max_interval_width_bp: 2000", "  require_current: true",
/// #     "outliers:", "  rule: none", "",
/// # ].join("\n")).unwrap();
/// # std::fs::write(decl_dir.join("suites/pilot.yaml"), [
/// #     "schema: economics-suite/v1", "id: pilot", "version: 1", "kind: live", "title: Pilot",
/// #     "control: baseline", "treatment: majordomus", "freshness_inputs: [methodology.yaml]", "",
/// # ].join("\n")).unwrap();
/// // a methodology and one live suite `pilot`, whose only record is unreadable
/// let recorded = dir.path().join(DIR).join("runs/pilot");
/// std::fs::create_dir_all(&recorded).unwrap();
/// std::fs::write(recorded.join("r1.json"), "{").unwrap();
/// let all = runs(dir.path(), &EconomicsRunsQuery::default());
/// assert_eq!((all.count, all.diagnostics.len()), (0, 1));
/// let other = EconomicsRunsQuery { suite: Some("other".into()), ..Default::default() };
/// assert!(runs(dir.path(), &other).diagnostics.is_empty());
/// ```
pub fn runs(root: &Path, q: &EconomicsRunsQuery) -> EconomicsRunList {
    let decl = match load(root) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return EconomicsRunList {
                count: 0,
                runs: Vec::new(),
                diagnostics: Vec::new(),
            }
        }
        Err(e) => {
            return EconomicsRunList {
                count: 0,
                runs: Vec::new(),
                diagnostics: e,
            }
        }
    };
    let mut out = Vec::new();
    let mut diagnostics = Vec::new();
    for suite in decl.suites.values().filter(|s| s.kind == "live") {
        if q.suite.as_ref().is_some_and(|x| x != &suite.id) {
            continue;
        }
        let (runs, errors) = load_runs(root, &suite.id);
        diagnostics.extend(errors);
        for (path, r) in runs {
            if q.task.as_ref().is_some_and(|t| t != &r.task)
                || q.variant.as_ref().is_some_and(|v| v != &r.variant)
            {
                continue;
            }
            out.push(EconomicsRunView {
                usage: usage::totals(&r),
                id: r.id,
                suite: r.suite,
                task: r.task,
                variant: r.variant,
                repetition: r.repetition,
                model: r.model_requested,
                revision: r.repository.commit,
                completed: r.outcome.completed,
                checks: r.outcome.checks,
                sessions: r.sessions.len(),
                path,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    EconomicsRunList {
        count: out.len(),
        runs: out,
        diagnostics,
    }
}
