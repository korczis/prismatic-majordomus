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

/// One misplaced artifact: a path found in a half whose declared layout excludes it, with the
/// contract that excludes it and why, so a refusal names what to move and where it belongs.
///
/// It travels as the JSON of the `layout.check` capability, so its member names are the
/// wire contract every projection reads:
///
/// ```
/// use majordomus_cli::layout::LayoutFinding;
/// let finding = LayoutFinding {
///     path: ".ai/local/notes/rule.md".into(),
///     contract: "manifest local".into(),
///     excludes: "canonical".into(),
///     reason: "a copy of .ai/repo/rules/rule.md".into(),
/// };
/// let json = serde_json::to_value(&finding).unwrap();
/// assert_eq!(json["contract"], "manifest local");
/// assert_eq!(json["excludes"], "canonical");
/// let back: LayoutFinding = serde_json::from_value(json).unwrap();
/// assert_eq!(back, finding);
/// ```
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

/// The answer of the layout check: whether the halves were read, how many paths were
/// judged, and every path a contract excludes. A report that could not read the halves
/// never passes, so silence about a tree nobody measured cannot read as a clean tree.
///
/// ```
/// use majordomus_cli::layout::LayoutReport;
/// let clean = LayoutReport::of(true, None, 12, Vec::new());
/// assert!(clean.passes);
/// assert_eq!(clean.checked, 12);
/// let unread = LayoutReport::of(false, Some("git could not start".into()), 0, Vec::new());
/// assert!(!unread.passes, "nothing found because nothing was read");
/// assert_eq!(unread.reason.as_deref(), Some("git could not start"));
/// ```
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
    /// A report, its verdict derived from what was measured and found. The findings are
    /// put in canonical order — by path, the contract breaking the tie — whatever order the
    /// walk found them in, so two runs over one tree answer the same bytes.
    ///
    /// ```
    /// use majordomus_cli::layout::{LayoutFinding, LayoutReport};
    /// let at = |path: &str| LayoutFinding {
    ///     path: path.into(),
    ///     contract: "manifest local".into(),
    ///     excludes: "canonical".into(),
    ///     reason: "a copy".into(),
    /// };
    /// let found = vec![at(".ai/local/z.md"), at(".ai/local/a.md")];
    /// let report = LayoutReport::of(true, None, 5, found);
    /// assert!(!report.passes, "measured, but something was found");
    /// let paths: Vec<&str> = report.findings.iter().map(|f| f.path.as_str()).collect();
    /// assert_eq!(paths, [".ai/local/a.md", ".ai/local/z.md"]);
    /// ```
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
///
/// Reads git at the repository's root — the tracked half's index entries and the ones its
/// ignore rules match — and the local half's Markdown and YAML files. A root git cannot
/// read is answered unmeasured rather than passing; a local half the manifest does not
/// declare `excludes: [canonical]` for is counted but not judged.
///
/// ```
/// use majordomus_cli::layout;
/// use majordomus_cli::metadata::KindSchema;
/// use majordomus_cli::repository::Repository;
/// use majordomus_cli::share::Share;
/// use majordomus_cli::synthetic::{crate_share, SyntheticRepository};
///
/// let repo = SyntheticRepository::small().unwrap();
/// let root = repo.root();
/// // the local half declares that authored knowledge never lives there
/// let manifest = root.join(".ai/manifest.yaml");
/// let text = std::fs::read_to_string(&manifest).unwrap().replace(
///     "  implicit_context: false\n",
///     "  implicit_context: false\n  lifecycle: runtime\n  excludes: [canonical]\n",
/// );
/// std::fs::write(&manifest, text).unwrap();
/// let git = |args: &[&str]| {
///     let mut git = std::process::Command::new("git");
///     assert!(git.arg("-C").arg(root).args(args).status().unwrap().success());
/// };
/// git(&["init", "-q"]);
/// git(&["add", ".ai"]);
/// // a byte copy of a tracked rule, left in the local half
/// let rule = std::fs::read(root.join(".ai/repo/rules/project/rule-0.v1.md")).unwrap();
/// std::fs::create_dir_all(root.join(".ai/local/notes")).unwrap();
/// std::fs::write(root.join(".ai/local/notes/rule-0.md"), rule).unwrap();
///
/// let repository = Repository::discover(root).unwrap();
/// let share = Share::locate(Some(&crate_share()), root).unwrap();
/// let kinds = KindSchema::load(&share, &repository).unwrap();
/// let report = layout::check(&repository, &repo.index().unwrap(), &kinds);
/// assert!(report.measured && !report.passes);
/// assert_eq!(report.findings.len(), 1);
/// assert_eq!(report.findings[0].path, ".ai/local/notes/rule-0.md");
/// assert!(report.findings[0].reason.contains(".ai/repo/rules/project/rule-0.v1.md"));
/// ```
pub fn check(repository: &Repository, index: &Index, kinds: &KindSchema) -> LayoutReport {
    let root = repository.root();
    let manifest = repository.manifest();
    let tracked_half = format!(".ai/{}", manifest.repo.path.trim_end_matches('/'));
    let local_half = format!(".ai/{}", manifest.local.path.trim_end_matches('/'));

    // the tracked half with its blob ids, then which of it git's ignore rules match: both
    // listings or neither, so a half-read tree is never judged
    let listed = git_lines(root, &["ls-files", "-s", "--", &tracked_half]).and_then(|tracked| {
        git_lines(
            root,
            &[
                "ls-files",
                "-c",
                "-i",
                "--exclude-standard",
                "--",
                &tracked_half,
            ],
        )
        .map(|ignored| (tracked, ignored))
    });
    let (tracked, ignored) = match listed {
        Ok(lists) => lists,
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
        let blobs: BTreeMap<String, String> =
            tracked.iter().filter_map(|l| staged_blob(l)).collect();
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
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t)
                    if t.is_file()
                        && p.extension()
                            .is_some_and(|x| x == "md" || x == "yaml" || x == "yml")
                        && e.metadata().is_ok_and(|m| m.len() > 0) =>
                {
                    out.push(p)
                }
                // another kind of file, or one whose type cannot be read: not a record
                _ => {}
            }
        }
    }
    out
}

/// How many paths one `git hash-object` is handed, so that the argument list stays far
/// below any platform's limit however many local files there are.
const HASH_CHUNK: usize = 256;

/// git's blob id of each file, in order. The paths are arguments, never lines of input: git
/// prints one id per path it is given or fails, so the ids pair with the files one to one,
/// whatever characters a file name holds.
fn hash_objects(root: &Path, files: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut ids = Vec::with_capacity(files.len());
    for chunk in files.chunks(HASH_CHUNK) {
        let paths: Vec<String> = chunk.iter().map(|f| f.display().to_string()).collect();
        let mut args = vec!["hash-object", "--no-filters", "--"];
        args.extend(paths.iter().map(String::as_str));
        ids.extend(git_lines(root, &args)?);
    }
    Ok(ids)
}

/// The tracked path and its blob id, from one line of `git ls-files -s`
/// (`<mode> <blob> <stage>\t<path>`); none from a line of another shape.
fn staged_blob(line: &str) -> Option<(String, String)> {
    let (meta, path) = line.split_once('\t')?;
    let blob = meta.split_whitespace().nth(1)?;
    Some((blob.to_string(), path.to_string()))
}

/// The lines git prints for `args` at `root`.
fn git_lines(root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    program_lines("git", root, args)
}

/// The lines `program`, a git, prints for `args` at `root`: the seam a test names a program
/// that cannot start through.
fn program_lines(program: &str, root: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    let out = Command::new(program)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::Share;
    use crate::synthetic::{crate_share, SyntheticRepository};

    const RULE: &str = ".ai/repo/rules/project/rule-0.v1.md";

    /// A synthetic layer whose tracked half is in git's index, its local half declared to
    /// exclude canonical knowledge when `excludes` says so.
    fn layer(excludes: bool) -> SyntheticRepository {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        if excludes {
            let manifest = repo.root().join(".ai/manifest.yaml");
            let text = std::fs::read_to_string(&manifest).unwrap().replace(
                "  implicit_context: false\n",
                "  implicit_context: false\n  lifecycle: runtime\n  excludes: [canonical]\n",
            );
            std::fs::write(&manifest, text).unwrap();
        }
        git(repo.root(), &["init", "-q"]);
        git(repo.root(), &["add", ".ai"]);
        repo
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .expect("git");
        assert!(status.success(), "git {}", args.join(" "));
    }

    fn local(repo: &SyntheticRepository, rel: &str, body: &[u8]) {
        let path = repo.root().join(".ai/local").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn checked(repo: &SyntheticRepository) -> LayoutReport {
        let repository = Repository::discover(repo.root()).unwrap();
        let share = Share::locate(Some(&crate_share()), repo.root()).unwrap();
        let kinds = KindSchema::load(&share, &repository).unwrap();
        check(&repository, &repo.index().unwrap(), &kinds)
    }

    #[test]
    fn a_local_copy_of_a_tracked_file_is_refused_and_names_the_original() {
        let repo = layer(true);
        local(
            &repo,
            "notes/copied.md",
            &std::fs::read(repo.root().join(RULE)).unwrap(),
        );
        local(&repo, "notes/mine.md", b"what this machine noticed today\n");
        local(&repo, "empty.md", b"");

        let report = checked(&repo);
        assert!(report.measured, "{:?}", report.reason);
        assert!(!report.passes);
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        let f = &report.findings[0];
        assert_eq!(f.path, ".ai/local/notes/copied.md");
        assert_eq!(f.contract, "manifest local");
        assert_eq!(f.excludes, "canonical");
        assert!(f.reason.contains(RULE), "{}", f.reason);
        // every tracked file of the tracked half, and the two local files with content
        let tracked = git_lines(repo.root(), &["ls-files", "--", ".ai/repo"]).unwrap();
        assert_eq!(report.checked, tracked.len() + 2);
    }

    #[test]
    fn a_local_file_declaring_a_canonical_schema_is_refused_and_a_runtime_one_is_not() {
        let repo = layer(true);
        local(
            &repo,
            "drafts/I9999.yaml",
            b"schema: majordomus.issue/v1\nid: I9999\n",
        );
        local(
            &repo,
            "sessions/now.md",
            b"---\nschema: \"majordomus.session-context/v1\"\n---\n\nruntime state\n",
        );

        let report = checked(&repo);
        let paths: Vec<&str> = report.findings.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, [".ai/local/drafts/I9999.yaml"]);
        assert!(
            report.findings[0]
                .reason
                .contains("schema: majordomus.issue/v1"),
            "{}",
            report.findings[0].reason
        );
    }

    #[test]
    fn a_local_half_that_excludes_nothing_is_not_judged() {
        let repo = layer(false);
        local(
            &repo,
            "notes/copied.md",
            &std::fs::read(repo.root().join(RULE)).unwrap(),
        );

        let report = checked(&repo);
        assert!(report.passes, "{:?}", report.findings);
        let tracked = git_lines(repo.root(), &["ls-files", "--", ".ai/repo"]).unwrap();
        assert_eq!(
            report.checked,
            tracked.len(),
            "the local half is not walked"
        );
    }

    #[test]
    fn a_root_git_cannot_read_is_unmeasured_and_does_not_pass() {
        let repo = SyntheticRepository::small().unwrap();
        if git_lines(repo.root(), &["rev-parse", "--show-toplevel"]).is_ok() {
            return; // the temporary directory lies inside a work tree; nothing to prove here
        }
        let report = checked(&repo);
        assert!(!report.measured);
        assert!(!report.passes);
        assert_eq!(report.checked, 0);
        assert!(report.reason.unwrap().contains("ls-files"));
    }

    /// Findings are listed by path and, where two contracts refuse one path, by contract,
    /// whatever order the walk found them in.
    #[test]
    fn findings_are_ordered_by_path_and_then_by_contract() {
        let at = |path: &str, contract: &str| LayoutFinding {
            path: path.into(),
            contract: contract.into(),
            excludes: "canonical".into(),
            reason: "a copy".into(),
        };
        let found = vec![
            at(".ai/local/b.md", "manifest local"),
            at(".ai/local/a.md", "ai.repo"),
            at(".ai/local/a.md", "ai.local"),
        ];
        let report = LayoutReport::of(true, None, 3, found);
        let order: Vec<(&str, &str)> = report
            .findings
            .iter()
            .map(|f| (f.path.as_str(), f.contract.as_str()))
            .collect();
        assert_eq!(
            order,
            [
                (".ai/local/a.md", "ai.local"),
                (".ai/local/a.md", "ai.repo"),
                (".ai/local/b.md", "manifest local"),
            ]
        );
    }

    /// A context document under the tracked half that declares a layout.
    fn contract(repo: &SyntheticRepository, dir: &str, id: &str, excludes: &str) {
        let path = repo.root().join(dir).join("README.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = format!(
            "---\nschema: context/v1\nid: {id}\nkind: context\ntitle: {id}\n\
             description: The {id} section.\nstatus: active\nscope: subtree\n\
             providers: [\"*\"]\naudience: [human, agent]\ncomposition: extend\n\
             order: 20\nlayout:\n  lifecycle: canonical\n  excludes: [{excludes}]\n---\n\n\
             # {id}\n"
        );
        std::fs::write(path, text).unwrap();
    }

    /// A tracked file git's ignore rules match is refused under the nearest contract that
    /// excludes cache or runtime, and only there: not under a contract that excludes
    /// something else, and not where no contract declares a layout.
    #[test]
    fn a_tracked_file_the_ignore_rules_match_is_refused_only_where_a_layout_excludes_it() {
        let repo = SyntheticRepository::small().unwrap();
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let text = std::fs::read_to_string(&sources).unwrap().replacen(
            "sources:\n",
            "sources:\n  - id: context\n    kind: context\n    discovery: vcs\n    \
             pathspec: ':(glob).ai/**/README.md'\n    required: false\n",
            1,
        );
        std::fs::write(&sources, text).unwrap();
        contract(&repo, ".ai/repo/state", "ai.state", "runtime");
        contract(&repo, ".ai/repo/profiles", "ai.profiles", "canonical");
        for rel in [
            ".ai/repo/state/deep/now.state",
            ".ai/repo/profiles/stray.state",
            ".ai/repo/stray.state",
        ] {
            std::fs::create_dir_all(repo.root().join(rel).parent().unwrap()).unwrap();
            std::fs::write(repo.root().join(rel), "pid: 42\n").unwrap();
        }
        git(repo.root(), &["init", "-q"]);
        git(repo.root(), &["add", ".ai"]);
        std::fs::write(repo.root().join(".gitignore"), "*.state\n").unwrap();

        let report = checked(&repo);
        assert!(report.measured, "{:?}", report.reason);
        assert!(!report.passes);
        assert_eq!(report.findings.len(), 1, "{:?}", report.findings);
        let f = &report.findings[0];
        assert_eq!(f.path, ".ai/repo/state/deep/now.state");
        assert_eq!(f.contract, "ai.state");
        assert_eq!(f.excludes, "runtime");
        assert!(
            f.reason.contains("git's ignore rules match it"),
            "{}",
            f.reason
        );
    }

    /// A local half that does not exist holds nothing to refuse: the check passes, having
    /// judged the tracked half alone, without asking git to hash anything.
    #[test]
    fn an_absent_local_half_passes_having_judged_the_tracked_half() {
        let repo = layer(true);
        assert!(!repo.root().join(".ai/local").exists());

        let report = checked(&repo);
        assert!(report.passes, "{:?} {:?}", report.reason, report.findings);
        let tracked = git_lines(repo.root(), &["ls-files", "--", ".ai/repo"]).unwrap();
        assert_eq!(report.checked, tracked.len());
        assert_eq!(hash_objects(repo.root(), &[]), Ok(Vec::new()));
    }

    /// A local file git cannot hash — one nobody may read — leaves the check unmeasured,
    /// never passing.
    #[cfg(unix)]
    #[test]
    fn a_local_file_git_cannot_hash_leaves_the_check_unmeasured() {
        use std::os::unix::fs::PermissionsExt;
        let repo = layer(true);
        local(&repo, "notes/sealed.md", b"a note\n");
        let sealed = repo.root().join(".ai/local/notes/sealed.md");
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000)).unwrap();

        let report = checked(&repo);
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!report.measured);
        assert!(!report.passes);
        let reason = report.reason.unwrap();
        assert!(
            reason.contains("git hash-object") && reason.contains("failed"),
            "{reason}"
        );
    }

    /// A file name holding a line break is one path, hashed as that file: the paths reach
    /// git as arguments, never as lines it would split.
    #[test]
    fn a_file_name_with_a_line_break_is_hashed_as_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let odd = dir.path().join("one\ntwo.md");
        std::fs::write(&odd, "two\n").unwrap();
        let plain = dir.path().join("two.md");
        std::fs::write(&plain, "two\n").unwrap();

        let ids = hash_objects(dir.path(), &[odd, plain]).unwrap();
        assert_eq!(ids.len(), 2, "{ids:?}");
        assert_eq!(ids[0], ids[1], "the same bytes are the same blob");
    }

    /// Only a `git ls-files -s` line of its own shape names a blob and a path.
    #[test]
    fn a_staged_line_names_its_blob_and_its_path() {
        assert_eq!(
            staged_blob("100644 e69de29bb2d1d6434b8b29ae775ad8c2e48c5391 0\t.ai/repo/a.md"),
            Some((
                "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391".to_string(),
                ".ai/repo/a.md".to_string()
            ))
        );
        assert_eq!(staged_blob("no tab here"), None);
        assert_eq!(staged_blob("100644\t.ai/repo/a.md"), None);
    }

    /// A git that cannot start is said as such, and nothing is listed.
    #[test]
    fn a_git_that_cannot_start_is_said() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-git");
        let err = program_lines(missing.to_str().unwrap(), dir.path(), &["status"]).unwrap_err();
        assert!(err.starts_with("git could not start"), "{err}");
    }

    /// A declared schema is read only from a file that exists, fits the head budget and is
    /// text; from anything else nothing is read, and the file is judged by its bytes alone.
    #[test]
    fn a_schema_is_read_only_from_a_small_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("small.yaml");
        std::fs::write(&small, "schema: 'majordomus.issue/v1'\nid: I1\n").unwrap();
        assert_eq!(
            declared_schema(&small).as_deref(),
            Some("majordomus.issue/v1")
        );

        assert_eq!(declared_schema(&dir.path().join("missing.yaml")), None);

        let large = dir.path().join("large.yaml");
        let mut body = b"schema: majordomus.issue/v1\n".to_vec();
        body.resize(HEAD_BYTES as usize + 1, b'#');
        std::fs::write(&large, body).unwrap();
        assert_eq!(declared_schema(&large), None, "larger than the head budget");

        let binary = dir.path().join("binary.yaml");
        std::fs::write(&binary, b"schema: majordomus.issue/v1\n\xff\xfe\n").unwrap();
        assert_eq!(declared_schema(&binary), None, "not UTF-8");
    }
}
