//! What a run measured about its own checkout when it ended.
//!
//! A recorder that stamps a report with the tree *it* sees vouches for a tree it did not
//! see: CI records in a fresh checkout, and a local run may be recorded after its checkout
//! changed. So the run measures and the recorder carries. [`stamp`] is the one measurement a
//! runner takes at the end of its run: the commit, the tree through
//! [`crate::git::working_tree_excluding`] (the evidence ledger ignored, the run's own
//! untracked outputs excluded, a tracked change never hidden), the producer, the toolchain,
//! the recorder's version, the report's digest, the host and the CI run. The result is an
//! [`EvidenceProvenance`], which `evidence record --provenance` attaches to that report's
//! executions.
//!
//! A stamp measures the checkout when it is taken, after the report was written, and not
//! the tree while the run executed: a run on a dirty tree that is cleaned before the stamp
//! reads clean.
//!
//! # A stamp, taken and read back
//!
//! ```
//! use majordomus_cli::evidence::{stamp, EvidenceProducer, EvidenceProvenance, StampRequest, TreeState};
//! use std::process::Command;
//!
//! let root = tempfile::tempdir().unwrap();
//! let git = |args: &[&str]| {
//!     Command::new("git").arg("-C").arg(root.path()).args(args).output().unwrap()
//! };
//! git(&["init", "-q"]);
//! git(&["config", "user.email", "t@example.com"]);
//! git(&["config", "user.name", "t"]);
//! std::fs::write(root.path().join("a.md"), "a").unwrap();
//! git(&["add", "-A"]);
//! git(&["commit", "-qm", "init"]);
//!
//! // the run wrote its report into the checkout: it is one of the run's own outputs
//! std::fs::write(root.path().join("suite.tsv"), "01_a\tok\t1\tparallel\n").unwrap();
//! let req = StampRequest {
//!     producer: Some(EvidenceProducer::Suite),
//!     report: Some(root.path().join("suite.tsv")),
//!     exclude: vec![],
//!     run: None,
//! };
//! let measured = stamp(root.path(), &req).unwrap();
//! assert_eq!(measured.working_tree, TreeState::Clean);
//! assert_eq!(measured.excluded, ["suite.tsv"]);
//! assert_eq!(measured.report.as_ref().unwrap().path, "suite.tsv");
//!
//! let out = tempfile::tempdir().unwrap();
//! let file = out.path().join("p.json");
//! std::fs::write(&file, serde_json::to_string(&measured).unwrap()).unwrap();
//! assert_eq!(EvidenceProvenance::read(&file).unwrap(), measured);
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::freshness::TreeState;
use super::run::EvidenceProducer;
use super::{digest_of, RunRef, LEDGER_PATH};
use crate::error::{Error, Result};

/// How a toolchain's version was known: asked of the executable, or read from the pin the
/// repository declares.
///
/// ```
/// use majordomus_cli::evidence::EvidenceToolchainSource;
/// assert_eq!(serde_json::to_value(EvidenceToolchainSource::Pinned).unwrap(), "pinned");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceToolchainSource {
    /// The executable answered `--version`.
    Measured,
    /// The repository pins it (`rust-toolchain.toml`), and the pin is what ran.
    Pinned,
}

/// The toolchain the run's producer used.
///
/// ```
/// use majordomus_cli::evidence::EvidenceToolchain;
/// let t: EvidenceToolchain =
///     serde_json::from_str(r#"{"name":"rustc","version":"1.2.3","source":"pinned"}"#).unwrap();
/// assert_eq!(t.version, "1.2.3");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceToolchain {
    /// The executable: `rustc`, `bash`.
    pub name: String,
    /// Its version.
    pub version: String,
    /// How the version was known.
    pub source: EvidenceToolchainSource,
}

/// The report a measurement belongs to: where it was and what its bytes were.
///
/// ```
/// use majordomus_cli::evidence::EvidenceReportArtifact;
/// let r: EvidenceReportArtifact =
///     serde_json::from_str(r#"{"path":"suite.tsv","digest":"sha256:00","bytes":3}"#).unwrap();
/// assert_eq!(r.bytes, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReportArtifact {
    /// Repository-relative when the report is inside the checkout, else its file name alone.
    /// Never a machine path.
    pub path: String,
    /// `sha256:<hex>` of the report's bytes.
    pub digest: String,
    /// How many bytes it held.
    pub bytes: u64,
}

/// The machine the run measured on.
///
/// ```
/// use majordomus_cli::evidence::EvidenceHost;
/// let h = EvidenceHost { os: std::env::consts::OS.into(), arch: std::env::consts::ARCH.into() };
/// assert!(!h.os.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceHost {
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
}

/// What a run measured about its own checkout when it ended.
///
/// Only `commit` and `working_tree` are required: a producing job's tree file
/// (`{"commit","working_tree","excluded"}`) is a valid, minimal measurement. It names no
/// report, so it is accepted only for a CI recording.
///
/// ```
/// use majordomus_cli::evidence::{EvidenceProvenance, TreeState};
/// let head = "0123456789abcdef0123456789abcdef01234567";
/// let p: EvidenceProvenance = serde_json::from_value(serde_json::json!({
///     "commit": head, "working_tree": "clean", "excluded": ["suite.tsv"]
/// })).unwrap();
/// assert_eq!(p.working_tree, TreeState::Clean);
/// assert!(p.report.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceProvenance {
    /// Which report this measures, when named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<EvidenceProducer>,
    /// The full commit the checkout was at.
    pub commit: String,
    /// The tree, the evidence ledger ignored and the run's own untracked outputs excluded.
    pub working_tree: TreeState,
    /// The run's own untracked outputs that were excluded, the report among them when it is
    /// inside the checkout. The ledger is always ignored and not listed.
    #[serde(default)]
    pub excluded: Vec<String>,
    /// When it was measured, RFC 3339 UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_at: Option<String>,
    /// The producer's toolchain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<EvidenceToolchain>,
    /// The version of the executable that measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorder: Option<String>,
    /// The report measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<EvidenceReportArtifact>,
    /// The machine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<EvidenceHost>,
    /// The CI run, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunRef>,
}

/// What to measure: which producer's run, its report, its own untracked outputs, its CI run.
///
/// ```
/// use majordomus_cli::evidence::StampRequest;
/// let req = StampRequest { producer: None, report: None, exclude: vec!["dist".into()], run: None };
/// assert_eq!(req.exclude, ["dist"]);
/// ```
#[derive(Debug, Clone, Default)]
pub struct StampRequest {
    /// The producer whose run this is.
    pub producer: Option<EvidenceProducer>,
    /// The report the run wrote.
    pub report: Option<PathBuf>,
    /// The run's own untracked outputs, repository-relative paths (files or directories).
    pub exclude: Vec<String>,
    /// The CI run, when there is one.
    pub run: Option<RunRef>,
}

fn refused(reason: impl Into<String>) -> Error {
    Error::InvalidSurface {
        surface: "evidence".into(),
        reason: reason.into(),
    }
}

/// Measure the checkout at `root` as the run `req` describes left it.
///
/// Refused, before anything is measured, when there is no commit, when an exclude is not a
/// repository path, is a pattern, names the whole checkout or holds tracked files (an
/// exclude names only the run's own untracked outputs), and when the report is unreadable.
///
/// ```
/// use majordomus_cli::evidence::{stamp, StampRequest};
/// let empty = tempfile::tempdir().unwrap();
/// let err = stamp(empty.path(), &StampRequest::default()).unwrap_err().to_string();
/// assert!(err.contains("no commit to measure"), "{err}");
/// ```
pub fn stamp(root: &Path, req: &StampRequest) -> Result<EvidenceProvenance> {
    let commit = match crate::git::inspect(root) {
        crate::git::GitState::Available(i) => i.head,
        crate::git::GitState::Unavailable { .. } => None,
    }
    .ok_or_else(|| {
        refused("there is no commit to measure: a run measured here would carry no provenance")
    })?;

    let mut outputs = Vec::new();
    for e in &req.exclude {
        outputs.push(checked_exclude(root, e)?);
    }

    let report = match &req.report {
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| {
                refused(format!("the report {} cannot be read: {e}", path.display()))
            })?;
            let (name, inside) = report_path(root, path);
            if inside {
                outputs.push(name.clone());
            }
            Some(EvidenceReportArtifact {
                path: name,
                digest: digest_of(&bytes),
                bytes: bytes.len() as u64,
            })
        }
        None => None,
    };
    outputs.sort();
    outputs.dedup();

    let refs: Vec<&str> = outputs.iter().map(String::as_str).collect();
    let working_tree = TreeState::parse(&crate::git::working_tree_excluding(
        root,
        &[LEDGER_PATH],
        &refs,
    ));

    Ok(EvidenceProvenance {
        producer: req.producer,
        commit,
        working_tree,
        excluded: outputs,
        measured_at: Some(super::record::now_rfc3339()),
        toolchain: req.producer.and_then(|p| toolchain(root, p)),
        recorder: Some(crate::VERSION.to_string()),
        report,
        host: Some(EvidenceHost {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
        }),
        run: req.run.clone(),
    })
}

/// An exclude as a repository path, or why it is not one the stamp accepts.
fn checked_exclude(root: &Path, raw: &str) -> Result<String> {
    let mut e = raw;
    while let Some(rest) = e.strip_prefix("./") {
        e = rest;
    }
    let e = e.trim_end_matches('/');
    if e.is_empty() || e == "." {
        return Err(refused(format!("`{raw}` names the whole checkout")));
    }
    let not_a_path = Path::new(e).is_absolute()
        || e.split('/').any(|s| s == "..")
        || e.starts_with(':')
        || e.starts_with('-')
        || e.contains('\0');
    if not_a_path {
        return Err(refused(format!("`{raw}` is not a repository path")));
    }
    if e.contains(['*', '?', '[', '\\']) {
        return Err(refused(format!(
            "`{raw}`: an exclude is a path, not a pattern"
        )));
    }
    match crate::git::ls_files(root, &format!(":(literal){e}")) {
        Ok(tracked) if tracked.is_empty() => Ok(e.to_string()),
        Ok(_) => Err(refused(format!(
            "`{raw}` holds tracked files: an exclude names only the run's own untracked \
             outputs, and a change to a tracked file is always a change"
        ))),
        Err(err) => Err(refused(format!(
            "`{raw}` could not be checked against the index: {err}"
        ))),
    }
}

/// The report's name as a measurement carries it, and whether it is inside the checkout:
/// repository-relative when it is, else its file name alone, never a machine path.
fn report_path(root: &Path, report: &Path) -> (String, bool) {
    let canonical = |p: &Path| std::fs::canonicalize(p).ok();
    if let (Some(r), Some(root)) = (canonical(report), canonical(root)) {
        if let Ok(rel) = r.strip_prefix(&root) {
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            return (parts.join("/"), true);
        }
    }
    let name = report.file_name().map(|n| n.to_string_lossy().into_owned());
    (name.unwrap_or_default(), false)
}

/// The toolchain `producer`'s run used: the repository's Rust pin for a crate or coverage
/// run, else what `rustc` answers; `bash` for the suite.
fn toolchain(root: &Path, producer: EvidenceProducer) -> Option<EvidenceToolchain> {
    use crate::environment::{probe::bounded_output, toolchain as tc};
    let ask = |exe: &str| {
        let out = bounded_output(
            std::process::Command::new(exe).arg("--version"),
            tc::VERSION_TIMEOUT,
        )?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let measured = |name: &str, version: String| EvidenceToolchain {
        name: name.into(),
        version,
        source: EvidenceToolchainSource::Measured,
    };
    match producer {
        EvidenceProducer::Crate | EvidenceProducer::Coverage => {
            let pinned = tc::declared(root).into_iter().find(|t| {
                t.id == "rust"
                    && matches!(
                        t.declared_by.as_str(),
                        "rust-toolchain.toml" | "rust-toolchain"
                    )
            });
            if let Some(version) = pinned.and_then(|t| t.declared) {
                return Some(EvidenceToolchain {
                    name: "rustc".into(),
                    version,
                    source: EvidenceToolchainSource::Pinned,
                });
            }
            let text = ask("rustc")?;
            Some(measured(
                "rustc",
                text.split_whitespace().nth(1)?.to_string(),
            ))
        }
        EvidenceProducer::Suite => {
            let text = ask("bash")?;
            let first = text.lines().next()?;
            let (_, rest) = first.split_once("version ")?;
            Some(measured(
                "bash",
                rest.split_whitespace().next()?.to_string(),
            ))
        }
    }
}

impl EvidenceProvenance {
    /// The measurement a file holds, or why it holds none: an unreadable file, bad JSON, an
    /// unknown field or a tree word that is not one, each naming the file.
    ///
    /// ```
    /// use majordomus_cli::evidence::EvidenceProvenance;
    /// let dir = tempfile::tempdir().unwrap();
    /// let f = dir.path().join("p.json");
    /// std::fs::write(&f, r#"{"commit":"c","working_tree":"clean","extra":1}"#).unwrap();
    /// assert!(EvidenceProvenance::read(&f).unwrap_err().contains("p.json"));
    /// ```
    pub fn read(path: &Path) -> std::result::Result<EvidenceProvenance, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("the measurement {} cannot be read: {e}", path.display()))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("the measurement {} is not JSON: {e}", path.display()))?;
        // TreeState reads any word as `unknown`; a measurement that says something else is
        // a measurement nobody can read, and is refused rather than weakened in silence.
        let tree = value.get("working_tree").and_then(|t| t.as_str());
        if let Some(word) = tree {
            if !matches!(word, "clean" | "dirty" | "unknown") {
                return Err(format!(
                    "the measurement {} states the tree `{word}`, which is not clean, dirty \
                     or unknown",
                    path.display()
                ));
            }
        }
        serde_json::from_value(value)
            .map_err(|e| format!("the measurement {} is not one: {e}", path.display()))
    }

    /// The measurement of each report, from `--provenance` values (`<file>` or
    /// `<producer>=<file>`) and the producers whose reports were given.
    ///
    /// A value's producer is its key, else the file's own, else the only report given;
    /// refused when none of those names one, when a key and the file disagree, and when a
    /// producer is given two measurements.
    ///
    /// ```
    /// use majordomus_cli::evidence::{EvidenceProducer, EvidenceProvenance};
    /// let dir = tempfile::tempdir().unwrap();
    /// let f = dir.path().join("p.json");
    /// std::fs::write(&f, r#"{"commit":"c","working_tree":"clean"}"#).unwrap();
    /// let v = vec![f.display().to_string()];
    /// let got = EvidenceProvenance::resolve(&v, &[EvidenceProducer::Suite]).unwrap();
    /// assert_eq!(got[&EvidenceProducer::Suite].producer, Some(EvidenceProducer::Suite));
    /// let two = [EvidenceProducer::Suite, EvidenceProducer::Crate];
    /// assert!(EvidenceProvenance::resolve(&v, &two).unwrap_err().contains("<producer>=<file>"));
    /// ```
    pub fn resolve(
        values: &[String],
        given: &[EvidenceProducer],
    ) -> std::result::Result<BTreeMap<EvidenceProducer, EvidenceProvenance>, String> {
        let mut out = BTreeMap::new();
        for value in values {
            let (key, file) = match value.split_once('=') {
                Some((k, f)) => match EvidenceProducer::parse(k) {
                    Some(p) => (Some(p), f),
                    None => (None, value.as_str()),
                },
                None => (None, value.as_str()),
            };
            if file.is_empty() {
                return Err(format!("`{value}` names no measurement file"));
            }
            let mut prov = EvidenceProvenance::read(Path::new(file))?;
            let producer = match (key, prov.producer, given) {
                (Some(k), _, _) => k,
                (None, Some(own), _) => own,
                (None, None, [only]) => *only,
                (None, None, _) => {
                    return Err(format!(
                        "`{file}` names no producer and more than one report was given: give \
                         it as `<producer>=<file>`"
                    ))
                }
            };
            if let Some(own) = prov.producer.filter(|own| *own != producer) {
                return Err(format!(
                    "`{file}` measures the {} report, not the {} one",
                    own.as_str(),
                    producer.as_str()
                ));
            }
            prov.producer = Some(producer);
            if out.insert(producer, prov).is_some() {
                return Err(format!(
                    "the {} report was given two measurements",
                    producer.as_str()
                ));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn repo(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        git(dir.path(), &["config", "user.name", "t"]);
        for f in files {
            write(dir.path(), f, "committed");
        }
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-qm", "init"]);
        dir
    }

    fn write(root: &Path, path: &str, text: &str) {
        let p = root.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn excluding(exclude: &[&str]) -> StampRequest {
        StampRequest {
            exclude: exclude.iter().map(|e| e.to_string()).collect(),
            ..StampRequest::default()
        }
    }

    fn refusal(root: &Path, req: &StampRequest) -> String {
        stamp(root, req).unwrap_err().to_string()
    }

    #[test]
    fn a_stamp_on_a_committed_tree_is_clean_and_names_its_commit() {
        let dir = repo(&["a.md"]);
        let p = stamp(dir.path(), &StampRequest::default()).unwrap();
        assert_eq!(p.commit, git(dir.path(), &["rev-parse", "HEAD"]));
        assert_eq!(p.working_tree, TreeState::Clean);
        assert!(p.excluded.is_empty() && p.report.is_none() && p.producer.is_none());
        assert_eq!(p.recorder.as_deref(), Some(crate::VERSION));
        assert_eq!(p.host.as_ref().unwrap().os, std::env::consts::OS);
        assert!(p.measured_at.as_ref().unwrap().ends_with('Z'));
    }

    #[test]
    fn the_ledgers_own_working_copy_does_not_make_a_stamp_dirty() {
        let dir = repo(&["a.md", LEDGER_PATH]);
        write(dir.path(), LEDGER_PATH, "{\"changed\": true}");
        assert_eq!(
            stamp(dir.path(), &StampRequest::default())
                .unwrap()
                .working_tree,
            TreeState::Clean
        );
        write(dir.path(), "a.md", "edited");
        assert_eq!(
            stamp(dir.path(), &StampRequest::default())
                .unwrap()
                .working_tree,
            TreeState::Dirty
        );
    }

    #[test]
    fn an_excluded_directory_is_excluded_with_everything_under_it_and_nothing_else_is() {
        let dir = repo(&["a.md"]);
        write(dir.path(), "dist/out.bin", "x");
        write(dir.path(), "dist/deep/more.bin", "x");
        write(dir.path(), "timings.tsv", "x");
        let got = stamp(dir.path(), &excluding(&["./dist/", "timings.tsv"])).unwrap();
        assert_eq!(got.working_tree, TreeState::Clean);
        assert_eq!(got.excluded, ["dist", "timings.tsv"], "normalised, sorted");
        let got = stamp(dir.path(), &excluding(&["timings.tsv"])).unwrap();
        assert_eq!(got.working_tree, TreeState::Dirty, "dist is not excluded");
        write(dir.path(), "distx/y", "x");
        let got = stamp(dir.path(), &excluding(&["dist", "timings.tsv"])).unwrap();
        assert_eq!(
            got.working_tree,
            TreeState::Dirty,
            "distx is not under dist"
        );
    }

    #[test]
    fn an_exclude_that_holds_tracked_files_is_refused() {
        let dir = repo(&["a.md", "test/cases/01_a.sh"]);
        write(dir.path(), "test/cases/01_a.sh", "edited");
        for e in [".", "./", "test", "test/", "test/cases/01_a.sh"] {
            let err = refusal(dir.path(), &excluding(&[e]));
            assert!(err.contains(&format!("`{e}`")), "{e}: {err}");
            assert!(
                err.contains("whole checkout") || err.contains("tracked files"),
                "{e}: {err}"
            );
        }
    }

    #[test]
    fn an_exclude_that_is_a_pattern_or_not_a_repository_path_is_refused() {
        let dir = repo(&["a.md"]);
        for e in ["*", "a?", "[ab]", "a\\b"] {
            let err = refusal(dir.path(), &excluding(&[e]));
            assert!(err.contains("a path, not a pattern"), "{e}: {err}");
        }
        for e in ["/tmp", "../x", "a/../b", ":x", "-x", "a\0b"] {
            let err = refusal(dir.path(), &excluding(&[e]));
            assert!(err.contains("is not a repository path"), "{e}: {err}");
        }
        let err = refusal(dir.path(), &excluding(&[""]));
        assert!(err.contains("whole checkout"), "{err}");
    }

    #[test]
    fn an_exclude_that_cannot_be_checked_against_the_index_is_refused() {
        // a checkout whose index git cannot read: HEAD resolves, ls-files does not
        let dir = repo(&["a.md"]);
        std::fs::write(dir.path().join(".git/index"), "not an index").unwrap();
        let err = refusal(dir.path(), &excluding(&["dist"]));
        assert!(
            err.contains("could not be checked against the index"),
            "{err}"
        );
    }

    #[test]
    fn a_report_inside_the_checkout_is_one_of_the_runs_outputs() {
        let dir = repo(&["a.md"]);
        write(dir.path(), "out/suite.tsv", "01_a\tok\t1\tparallel\n");
        let req = StampRequest {
            report: Some(dir.path().join("out/suite.tsv")),
            ..StampRequest::default()
        };
        let got = stamp(dir.path(), &req).unwrap();
        assert_eq!(got.working_tree, TreeState::Clean);
        assert_eq!(got.excluded, ["out/suite.tsv"]);
        assert_eq!(got.report.unwrap().path, "out/suite.tsv");
    }

    #[test]
    fn a_stamp_without_a_commit_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(refusal(dir.path(), &StampRequest::default()).contains("no commit to measure"));
        git(dir.path(), &["init", "-q"]);
        assert!(refusal(dir.path(), &StampRequest::default()).contains("no commit to measure"));
    }

    #[test]
    fn the_report_is_named_without_a_machine_path_and_digested() {
        let dir = repo(&["a.md"]);
        let elsewhere = tempfile::tempdir().unwrap();
        let report = elsewhere.path().join("all.tsv");
        std::fs::write(&report, "01_a\tok\t1\tparallel\n").unwrap();
        let req = StampRequest {
            report: Some(report.clone()),
            ..StampRequest::default()
        };
        let got = stamp(dir.path(), &req).unwrap();
        let artifact = got.report.unwrap();
        assert_eq!(artifact.path, "all.tsv");
        assert_eq!(artifact.digest, digest_of(&std::fs::read(&report).unwrap()));
        assert_eq!(artifact.bytes, 19);
        assert!(
            got.excluded.is_empty(),
            "a report outside the checkout is no output of it"
        );

        let missing = StampRequest {
            report: Some(elsewhere.path().join("absent.tsv")),
            ..StampRequest::default()
        };
        assert!(refusal(dir.path(), &missing).contains("absent.tsv"));
    }

    #[test]
    fn the_crate_toolchain_is_pinned_from_rust_toolchain_toml() {
        let dir = repo(&["a.md"]);
        write(
            dir.path(),
            "rust-toolchain.toml",
            "[toolchain]\nchannel = \"1.2.3\"\n",
        );
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-qm", "pin"]);
        for p in [EvidenceProducer::Crate, EvidenceProducer::Coverage] {
            let req = StampRequest {
                producer: Some(p),
                ..StampRequest::default()
            };
            let t = stamp(dir.path(), &req).unwrap().toolchain.unwrap();
            assert_eq!(
                t,
                EvidenceToolchain {
                    name: "rustc".into(),
                    version: "1.2.3".into(),
                    source: EvidenceToolchainSource::Pinned,
                }
            );
        }
    }

    #[test]
    fn an_unpinned_toolchain_is_measured() {
        let dir = repo(&["a.md"]);
        // a minimum is not a pin
        write(
            dir.path(),
            "Cargo.toml",
            "[package]\nrust-version = \"1.0\"\n",
        );
        if let Some(t) = toolchain(dir.path(), EvidenceProducer::Crate) {
            assert_eq!(
                (t.name.as_str(), t.source),
                ("rustc", EvidenceToolchainSource::Measured)
            );
            assert_ne!(t.version, "1.0");
        }
        let bash = toolchain(dir.path(), EvidenceProducer::Suite).unwrap();
        assert_eq!(
            (bash.name.as_str(), bash.source),
            ("bash", EvidenceToolchainSource::Measured)
        );
        assert!(
            bash.version.chars().next().unwrap().is_ascii_digit(),
            "{}",
            bash.version
        );
    }

    #[test]
    fn a_stamp_without_a_producer_measures_no_toolchain() {
        let dir = repo(&["a.md"]);
        write(
            dir.path(),
            "rust-toolchain.toml",
            "[toolchain]\nchannel = \"1.2.3\"\n",
        );
        assert!(stamp(dir.path(), &StampRequest::default())
            .unwrap()
            .toolchain
            .is_none());
    }

    #[test]
    fn a_minimal_measurement_reads_as_a_provenance() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("suite-tree.json");
        let head = "0123456789abcdef0123456789abcdef01234567";
        std::fs::write(
            &f,
            format!(r#"{{"commit":"{head}","working_tree":"clean","excluded":["suite.tsv"]}}"#),
        )
        .unwrap();
        let p = EvidenceProvenance::read(&f).unwrap();
        assert_eq!(
            (p.commit.as_str(), p.working_tree),
            (head, TreeState::Clean)
        );
        assert_eq!(p.excluded, ["suite.tsv"]);
        assert!(p.producer.is_none() && p.report.is_none() && p.toolchain.is_none());
    }

    #[test]
    fn a_measurement_with_an_unknown_field_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("p.json");
        for (text, why) in [
            (
                r#"{"commit":"c","working_tree":"clean","extra":1}"#,
                "is not one",
            ),
            (r#"{"commit":"c","working_tree":"spotless"}"#, "`spotless`"),
            (r#"{"working_tree":"clean"}"#, "is not one"),
            ("not json", "is not JSON"),
        ] {
            std::fs::write(&f, text).unwrap();
            let err = EvidenceProvenance::read(&f).unwrap_err();
            assert!(err.contains("p.json") && err.contains(why), "{text}: {err}");
        }
        let err = EvidenceProvenance::read(&dir.path().join("absent.json")).unwrap_err();
        assert!(err.contains("cannot be read"), "{err}");
    }

    fn measurement(dir: &Path, name: &str, producer: Option<&str>) -> String {
        let f = dir.join(name);
        let mut v = serde_json::json!({"commit": "c", "working_tree": "clean"});
        if let Some(p) = producer {
            v["producer"] = p.into();
        }
        std::fs::write(&f, v.to_string()).unwrap();
        f.display().to_string()
    }

    #[test]
    fn a_keyless_measurement_takes_its_producer_from_the_file_or_the_only_report() {
        use EvidenceProducer::*;
        let dir = tempfile::tempdir().unwrap();
        let own = measurement(dir.path(), "own.json", Some("crate"));
        let bare = measurement(dir.path(), "bare.json", None);
        let got = EvidenceProvenance::resolve(std::slice::from_ref(&own), &[Suite, Crate]).unwrap();
        assert_eq!(got.keys().copied().collect::<Vec<_>>(), [Crate]);
        let got = EvidenceProvenance::resolve(std::slice::from_ref(&bare), &[Coverage]).unwrap();
        assert_eq!(got[&Coverage].producer, Some(Coverage));
        let keyed = format!("suite={bare}");
        let got = EvidenceProvenance::resolve(&[keyed, own], &[Suite, Crate]).unwrap();
        assert_eq!(got.keys().copied().collect::<Vec<_>>(), [Suite, Crate]);
        assert!(got.values().all(|p| p.producer.is_some()));
    }

    #[test]
    fn a_keyless_measurement_without_a_producer_among_several_reports_is_refused() {
        use EvidenceProducer::*;
        let dir = tempfile::tempdir().unwrap();
        let bare = measurement(dir.path(), "bare.json", None);
        for given in [&[Suite, Crate][..], &[][..]] {
            let err = EvidenceProvenance::resolve(std::slice::from_ref(&bare), given).unwrap_err();
            assert!(err.contains("give it as `<producer>=<file>`"), "{err}");
        }
        let err = EvidenceProvenance::resolve(&["suite=".to_string()], &[Suite]).unwrap_err();
        assert!(err.contains("names no measurement file"), "{err}");
    }

    #[test]
    fn a_key_that_disagrees_with_the_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let own = measurement(dir.path(), "own.json", Some("crate"));
        let err =
            EvidenceProvenance::resolve(&[format!("suite={own}")], &[EvidenceProducer::Suite])
                .unwrap_err();
        assert!(
            err.contains("measures the crate report, not the suite one"),
            "{err}"
        );
    }

    #[test]
    fn a_producer_given_twice_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let a = measurement(dir.path(), "a.json", Some("suite"));
        let b = measurement(dir.path(), "b.json", None);
        let err =
            EvidenceProvenance::resolve(&[a, format!("suite={b}")], &[EvidenceProducer::Suite])
                .unwrap_err();
        assert!(
            err.contains("the suite report was given two measurements"),
            "{err}"
        );
    }
}
