//! The boundary between the layer's two halves, checked (I2871, I1102).
//!
//! I2870 lets a directory's contract declare its layout — the lifecycle of what lives there
//! and what must never appear — and the manifest's `local:` block declares the local half's.
//! This module reads those declarations and refuses what contradicts them, naming the path
//! and the contract. It judges only what is declared: a contract without a layout says
//! nothing, and nothing is refused under it.
//!
//! What a file is, is read rather than guessed from its name:
//!
//! - a tracked file under the tracked half that git's own ignore rules match is state that
//!   does not travel — `cache` or `runtime` — committed by mistake;
//! - a file under the local half whose bytes are a tracked file of the tracked half, or
//!   whose front matter declares the schema of a kind `share/kinds.yaml` calls canonical,
//!   is a copy of authored knowledge.
//!
//! ```
//! use majordomus_cli::layout::{LayoutFinding, LayoutReport};
//! let report = LayoutReport::of(true, None, 3, Vec::new());
//! assert!(report.passes, "measured and nothing found");
//! let found = LayoutReport::of(true, None, 3, vec![LayoutFinding {
//!     path: ".ai/local/rules/copied.md".into(),
//!     contract: "manifest local".into(),
//!     excludes: "canonical".into(),
//!     reason: "a copy of .ai/repo/rules/copied.md".into(),
//! }]);
//! assert!(!found.passes);
//! assert!(!LayoutReport::of(false, Some("no git".into()), 0, Vec::new()).passes);
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::index::Index;
use crate::metadata::KindSchema;
use crate::repository::{Lifecycle, Repository};

/// The largest local file whose head is read for a declared schema; larger files are
/// compared by their bytes only.
const HEAD_BYTES: u64 = 256 * 1024;

/// One path a contract excludes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutFinding {
    /// The path, repository-relative.
    pub path: String,
    /// The contract it violates: a context document's id, or `manifest local`.
    pub contract: String,
    /// What the contract excludes that this path is.
    pub excludes: String,
    /// What the path was found to be, and how.
    pub reason: String,
}

/// Findings are listed by path, the contract breaking the tie, as everywhere else.
impl crate::order::Ordered for LayoutFinding {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.path, &self.contract)
    }
}

/// The answer of the layout check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayoutReport {
    /// Whether the halves could be read at all. `false` is not a pass.
    pub measured: bool,
    /// Why they could not, when they could not.
    pub reason: Option<String>,
    /// How many paths were judged.
    pub checked: usize,
    /// Every path a contract excludes, in canonical order. Empty is the passing answer.
    pub findings: Vec<LayoutFinding>,
    /// `true` when the halves were read and nothing was found.
    pub passes: bool,
}

impl LayoutReport {
    /// A report, its verdict derived from what was measured and found.
    pub fn of(
        measured: bool,
        reason: Option<String>,
        checked: usize,
        mut findings: Vec<LayoutFinding>,
    ) -> Self {
        crate::order::canonical(&mut findings);
        let passes = measured && findings.is_empty();
        LayoutReport {
            measured,
            reason,
            checked,
            findings,
            passes,
        }
    }

    fn unmeasured(reason: String) -> Self {
        Self::of(false, Some(reason), 0, Vec::new())
    }
}

/// A declared layout: the contract that declares it and what it excludes.
struct Declared {
    contract: String,
    excludes: Vec<String>,
}

/// Check both halves of `repository` against their declared layouts.
pub fn check(repository: &Repository, index: &Index, kinds: &KindSchema) -> LayoutReport {
    let root = repository.root();
    let manifest = repository.manifest();
    let tracked_half = format!(".ai/{}", manifest.repo.path.trim_end_matches('/'));
    let local_half = format!(".ai/{}", manifest.local.path.trim_end_matches('/'));

    let tracked = match git_lines(root, &["ls-files", "-s", "--", &tracked_half]) {
        Ok(lines) => lines,
        Err(e) => return LayoutReport::unmeasured(e),
    };
    let ignored = match git_lines(
        root,
        &[
            "ls-files",
            "-c",
            "-i",
            "--exclude-standard",
            "--",
            &tracked_half,
        ],
    ) {
        Ok(lines) => lines,
        Err(e) => return LayoutReport::unmeasured(e),
    };

    let mut findings = Vec::new();
    let contracts = declared_layouts(index);
    for path in &ignored {
        let Some(d) = nearest(&contracts, path) else {
            continue;
        };
        let Some(excluded) = d
            .excludes
            .iter()
            .find(|e| matches!(e.as_str(), "cache" | "runtime"))
        else {
            continue;
        };
        findings.push(LayoutFinding {
            path: path.clone(),
            contract: d.contract.clone(),
            excludes: excluded.clone(),
            reason: "tracked although git's ignore rules match it: state that does not travel"
                .into(),
        });
    }

    let mut checked = tracked.len();
    if manifest.local.excludes.iter().any(|e| e == "canonical") {
        // the tracked half's blobs, by id: a local file with one of these ids is that file
        let blobs: BTreeMap<String, String> = tracked
            .iter()
            .filter_map(|l| {
                let (meta, path) = l.split_once('\t')?;
                let blob = meta.split_whitespace().nth(1)?;
                Some((blob.to_string(), path.to_string()))
            })
            .collect();
        let canonical: BTreeSet<String> = kinds
            .kinds()
            .filter(|(_, k)| k.lifecycle.unwrap_or(Lifecycle::Canonical) == Lifecycle::Canonical)
            .filter_map(|(_, k)| k.schema.as_deref())
            .flat_map(|s| {
                let short = s.split_once('.').map_or(s, |(_, rest)| rest);
                [s.to_string(), short.to_string()]
            })
            .collect();
        let files = local_files(&root.join(&local_half));
        checked += files.len();
        let ids = match hash_objects(root, &files) {
            Ok(ids) => ids,
            Err(e) => return LayoutReport::unmeasured(e),
        };
        for (file, id) in files.iter().zip(ids) {
            let rel = file
                .strip_prefix(root)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            if let Some(original) = blobs.get(&id) {
                findings.push(LayoutFinding {
                    path: rel,
                    contract: "manifest local".into(),
                    excludes: "canonical".into(),
                    reason: format!(
                        "a copy of {original}: authored knowledge belongs in the tracked half"
                    ),
                });
                continue;
            }
            if let Some(schema) = declared_schema(file).filter(|s| canonical.contains(s)) {
                findings.push(LayoutFinding {
                    path: rel,
                    contract: "manifest local".into(),
                    excludes: "canonical".into(),
                    reason: format!("declares `schema: {schema}`, a canonical kind: authored knowledge belongs in the tracked half"),
                });
            }
        }
    }
    LayoutReport::of(true, None, checked, findings)
}

/// Every contract that declares a layout, by the directory it governs.
fn declared_layouts(index: &Index) -> BTreeMap<String, Declared> {
    index
        .objects
        .iter()
        .filter(|o| o.kind == "context")
        .filter_map(|o| {
            let layout = o.metadata.get("layout")?;
            let dir = Path::new(&o.provenance.path)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let excludes = layout
                .get("excludes")
                .and_then(|e| e.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            Some((
                dir,
                Declared {
                    contract: o.identity.clone(),
                    excludes,
                },
            ))
        })
        .collect()
}

/// The contract nearest above `path` that declares a layout.
fn nearest<'a>(contracts: &'a BTreeMap<String, Declared>, path: &str) -> Option<&'a Declared> {
    Path::new(path)
        .ancestors()
        .skip(1)
        .find_map(|dir| contracts.get(&dir.to_string_lossy().replace('\\', "/")))
}

/// The `schema:` a file declares in its first lines (front matter or a YAML document).
fn declared_schema(file: &Path) -> Option<String> {
    let meta = std::fs::metadata(file).ok()?;
    if meta.len() > HEAD_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(file).ok()?;
    text.lines().take(40).find_map(|l| {
        let v = l.strip_prefix("schema:")?.trim();
        Some(v.trim_matches(|c| c == '"' || c == '\'').to_string())
    })
}

/// Every regular file under `dir` that could carry knowledge (Markdown or YAML). Links are
/// not followed; the order is the walk's, and the report orders what it finds.
fn local_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let p = e.path();
            if t.is_dir() {
                stack.push(p);
            } else if t.is_file()
                && p.extension()
                    .is_some_and(|x| x == "md" || x == "yaml" || x == "yml")
                && e.metadata().is_ok_and(|m| m.len() > 0)
            {
                out.push(p);
            }
        }
    }
    out
}

/// git's blob id of each file, in order, from one `git hash-object` over all of them.
fn hash_objects(root: &Path, files: &[PathBuf]) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    use std::io::Write as _;
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["hash-object", "--no-filters", "--stdin-paths"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("git hash-object could not start: {e}"))?;
    {
        let mut stdin = child.stdin.take().ok_or("git hash-object took no input")?;
        for f in files {
            writeln!(stdin, "{}", f.display())
                .map_err(|e| format!("git hash-object refused a path: {e}"))?;
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git hash-object did not finish: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git hash-object failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let ids: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    if ids.len() != files.len() {
        return Err(format!(
            "git hash-object answered {} id(s) for {} file(s)",
            ids.len(),
            files.len()
        ));
    }
    Ok(ids)
}

/// The lines git prints for `args` at `root`.
fn git_lines(root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git could not start: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}
