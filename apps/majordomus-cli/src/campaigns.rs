//! The prompt packs staged before they are kept (ADR 0236, I4715).
//!
//! A pack in progress lives in the primary checkout's git-ignored `tmp/owner-packs/<pack>/`
//! (the legacy place is `tmp/prompt-packs/<pack>/`). The primary checkout is derived from the
//! repository's common git directory, so a worker in any linked worktree reads the same
//! staging place. Each pack's state is derived from the tracked tree of the checkout asking:
//! whether `campaigns/MANIFEST.sha256` records its files with the same bytes, and whether a
//! record under `.ai/repo/project/` or `.ai/repo/adrs/` cites it.
//!
//! Nothing here writes, and nothing in a pack is executed or read beyond its first heading:
//! a pack is untrusted input (ADR 0124 point 6).
//!
//! ```
//! use majordomus_cli::campaigns::{staged, PackState};
//! use std::time::{Duration, UNIX_EPOCH};
//!
//! let tree = tempfile::tempdir().unwrap();
//! let pack = tree.path().join("tmp/owner-packs/alpha");
//! std::fs::create_dir_all(&pack).unwrap();
//! std::fs::write(pack.join("README.md"), "# Alpha brief\n").unwrap();
//!
//! let now = UNIX_EPOCH + Duration::from_secs(4_000_000_000);
//! let report = staged(tree.path(), Ok(tree.path().to_path_buf()), now, Some(7));
//! assert_eq!(report.packs.len(), 1);
//! assert_eq!(report.packs[0].state, PackState::Staged);
//! assert_eq!(report.packs[0].first_heading.as_deref(), Some("Alpha brief"));
//! assert!(report.packs[0].stale, "written long before `now`, and never kept");
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The staging places ADR 0236 declares, in the order they are read: the current one, then
/// the legacy one. Paths are relative to the primary checkout.
pub const STAGING_PLACES: [(&str, bool); 2] =
    [("tmp/owner-packs", false), ("tmp/prompt-packs", true)];

/// Directories of a staging place that hold something other than a pack: `drafts/`, which
/// ADR 0236 names, and the holding areas sessions made beside the packs (measured in the
/// primary checkout on 2026-11-24): records waiting to land, per-issue prompts, and archives.
pub const HOLDING: [&str; 6] = ["archives", "drafts", "duplicates", "pending", "prompts", "zips"];

/// The tracked manifest of kept packs, relative to the repository root.
const MANIFEST: &str = "campaigns/MANIFEST.sha256";
/// The kept packs no record cites, each with its reason.
const UNPLANNED: &str = ".ai/repo/ci/campaigns-unplanned.txt";
/// Where the records that cite a pack live.
const RECORDS: [&str; 2] = [".ai/repo/project", ".ai/repo/adrs"];
/// The most of a file read for its first heading.
const HEADING_BYTES: u64 = 64 * 1024;

/// What became of a staged pack, derived from the tracked tree.
///
/// ```
/// use majordomus_cli::campaigns::PackState;
/// assert_eq!(serde_json::to_value(PackState::Planned).unwrap(), "planned");
/// assert_eq!(PackState::Diverged.as_str(), "diverged");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackState {
    /// Not kept: `campaigns/MANIFEST.sha256` records no file of it.
    Staged,
    /// Kept: every staged file has the bytes the manifest records, and nothing says why no
    /// record cites it.
    Kept,
    /// Kept, and a staged file differs from what the manifest records or is not in it.
    Diverged,
    /// Kept and cited as `campaigns/<pack>` by a record under `.ai/repo/project/` or an ADR.
    Planned,
    /// Kept and named in `.ai/repo/ci/campaigns-unplanned.txt`.
    Unplanned,
}

impl PackState {
    /// The state's name, as the JSON answer spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Kept => "kept",
            Self::Diverged => "diverged",
            Self::Planned => "planned",
            Self::Unplanned => "unplanned",
        }
    }
}

/// One staging place, and whether it could be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StagingPlace {
    /// The place, relative to the primary checkout: `tmp/owner-packs`, `tmp/prompt-packs`.
    pub path: String,
    /// Whether it is the legacy place.
    pub legacy: bool,
    /// Whether it exists as a directory.
    pub present: bool,
    /// Why it was not read, when it was not.
    pub reason: Option<String>,
    /// The directories in it that are not packs (see [`HOLDING`]).
    pub holding: Vec<String>,
}

/// One pack in a staging place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StagedPack {
    /// The pack's directory name, which is also its name under `campaigns/`.
    pub name: String,
    /// The staging place it is in.
    pub place: String,
    /// What became of it.
    pub state: PackState,
    /// How many regular files it holds, hidden files and symbolic links aside.
    pub files: usize,
    /// The first `# ` heading of its README.md, INDEX.md or first Markdown file.
    pub first_heading: Option<String>,
    /// The newest modification time of its files, RFC 3339 in UTC.
    pub modified: Option<String>,
    /// Whole days since then.
    pub age_days: Option<u64>,
    /// Whether it is `staged` and older than the policy's bound.
    pub stale: bool,
    /// The staged files whose bytes the manifest does not record, for a `diverged` pack.
    pub differing: Vec<String>,
    /// The records that name it, as `campaigns/<pack>` or in a staging place, repository-relative.
    pub cited_by: Vec<String>,
}

/// Every staged pack, with what became of each: the answer of `campaigns.staged`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StagedReport {
    /// The primary checkout the staging places were read in, when it could be derived.
    pub primary: Option<String>,
    /// Why no staging place was read, when none was.
    pub reason: Option<String>,
    /// The policy's bound in days (`campaigns.staged_max_age_days`); absent judges nothing stale.
    pub stale_after_days: Option<u64>,
    /// Each staging place, current first.
    pub places: Vec<StagingPlace>,
    /// Every pack, by place and then by name.
    pub packs: Vec<StagedPack>,
    /// How many packs are in each state.
    pub tallies: BTreeMap<String, usize>,
    /// How many packs are stale.
    pub stale: usize,
}

/// The primary checkout of the repository at `root`: the work tree whose `.git` is the
/// repository's common directory, so that every linked worktree derives the same one.
///
/// ```
/// let dir = tempfile::tempdir().unwrap();
/// let err = majordomus_cli::campaigns::primary_checkout(dir.path()).unwrap_err();
/// assert!(err.contains("common directory"), "{err}");
/// ```
pub fn primary_checkout(root: &Path) -> Result<PathBuf, String> {
    let out = crate::git::read_only(root)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .map_err(|e| format!("git could not be started to find the common directory: {e}"))?;
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if !out.status.success() || common.as_os_str().is_empty() {
        return Err(format!(
            "git did not name the common directory of {}",
            root.display()
        ));
    }
    match (common.file_name(), common.parent()) {
        (Some(name), Some(parent)) if name == ".git" => Ok(parent.to_path_buf()),
        _ => Err(format!(
            "the common directory {} is not a work tree's .git, so there is no primary checkout",
            common.display()
        )),
    }
}

/// Every pack in the staging places of `primary`, judged against the tracked tree at `root`.
///
/// `primary` is [`primary_checkout`]'s answer; a refusal there is the report's `reason` and an
/// empty list, never an error. `stale_after_days` is the policy's bound.
pub fn staged(
    root: &Path,
    primary: Result<PathBuf, String>,
    now: SystemTime,
    stale_after_days: Option<u64>,
) -> StagedReport {
    let mut report = StagedReport {
        primary: None,
        reason: None,
        stale_after_days,
        places: Vec::new(),
        packs: Vec::new(),
        tallies: BTreeMap::new(),
        stale: 0,
    };
    let primary = match primary {
        Ok(p) => p,
        Err(reason) => {
            report.reason = Some(reason);
            return report;
        }
    };
    report.primary = Some(primary.display().to_string());
    let tree = Tree::read(root);
    for (path, legacy) in STAGING_PLACES {
        let (place, names) = place(&primary, path, legacy);
        report.places.push(place);
        for name in names {
            let dir = primary.join(path).join(&name);
            report.packs.push(tree.judge(&dir, path, name, now, stale_after_days));
        }
    }
    for pack in &report.packs {
        *report.tallies.entry(pack.state.as_str().to_string()).or_default() += 1;
    }
    report.stale = report.packs.iter().filter(|p| p.stale).count();
    report
}

/// One staging place and the names of the packs in it, in canonical order.
fn place(primary: &Path, path: &str, legacy: bool) -> (StagingPlace, Vec<String>) {
    let mut place = StagingPlace {
        path: path.to_string(),
        legacy,
        present: false,
        reason: None,
        holding: Vec::new(),
    };
    let entries = match std::fs::read_dir(primary.join(path)) {
        Ok(entries) => entries,
        Err(e) => {
            place.reason = Some(format!("{path} could not be read in the primary checkout: {e}"));
            return (place, Vec::new());
        }
    };
    place.present = true;
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir || name.starts_with('.') {
            continue;
        }
        if HOLDING.contains(&name.as_str()) {
            place.holding.push(name);
        } else {
            names.push(name);
        }
    }
    crate::order::canonical_strings(&mut place.holding);
    crate::order::canonical_strings(&mut names);
    (place, names)
}

/// What the tracked tree says about kept packs and the records that cite them.
struct Tree {
    /// `<pack>/<file>` to its recorded SHA-256, from the manifest.
    manifest: BTreeMap<String, String>,
    /// The packs named in the unplanned list.
    unplanned: Vec<String>,
    /// Every record, repository-relative, with its text.
    records: Vec<(String, String)>,
}

impl Tree {
    fn read(root: &Path) -> Self {
        let manifest = std::fs::read_to_string(root.join(MANIFEST))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.split_once("  "))
            .map(|(hash, path)| (path.to_string(), hash.to_string()))
            .collect();
        let unplanned = std::fs::read_to_string(root.join(UNPLANNED))
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter_map(|line| line.split_whitespace().next().map(str::to_string))
            .collect();
        let mut records = Vec::new();
        for dir in RECORDS {
            collect_records(root, &root.join(dir), &mut records);
        }
        Self { manifest, unplanned, records }
    }

    fn judge(
        &self,
        dir: &Path,
        place: &str,
        name: String,
        now: SystemTime,
        stale_after_days: Option<u64>,
    ) -> StagedPack {
        let mut files = Vec::new();
        walk(dir, dir, &mut files);
        crate::order::canonical_strings(&mut files);
        let newest = files
            .iter()
            .filter_map(|f| std::fs::metadata(dir.join(f)).and_then(|m| m.modified()).ok())
            .max();
        let age_days = newest.map(|t| {
            now.duration_since(t).map(|d| d.as_secs() / 86_400).unwrap_or(0)
        });
        let prefix = format!("{name}/");
        let kept = self.manifest.keys().any(|k| k.starts_with(&prefix));
        let differing: Vec<String> = if kept {
            files
                .iter()
                .filter(|f| self.manifest.get(&format!("{prefix}{f}")) != sha256(&dir.join(f)).as_ref())
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let kept_at = format!("campaigns/{name}");
        let staged_at = format!("{place}/{name}");
        let cited_by: Vec<String> = self
            .records
            .iter()
            .filter(|(_, text)| names(text, &kept_at) || names(text, &staged_at))
            .map(|(path, _)| path.clone())
            .collect();
        let planned = self.records.iter().any(|(_, text)| names(text, &kept_at));
        let state = if !kept {
            PackState::Staged
        } else if !differing.is_empty() {
            PackState::Diverged
        } else if planned {
            PackState::Planned
        } else if self.unplanned.contains(&name) {
            PackState::Unplanned
        } else {
            PackState::Kept
        };
        let stale = state == PackState::Staged
            && matches!((age_days, stale_after_days), (Some(age), Some(bound)) if age >= bound);
        StagedPack {
            first_heading: first_heading(dir, &files),
            modified: newest.map(crate::peers::rfc3339),
            files: files.len(),
            name,
            place: place.to_string(),
            state,
            age_days,
            stale,
            differing,
            cited_by,
        }
    }
}

/// Whether `text` names `path` as a whole path: what follows does not continue a file name.
/// A full stop ends a sentence unless a name character follows it, as `campaigns-check`
/// reads it.
fn names(text: &str, path: &str) -> bool {
    let name = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    text.match_indices(path).any(|(at, _)| {
        let mut rest = text[at + path.len()..].chars();
        match rest.next() {
            Some('.') => !rest.next().is_some_and(name),
            Some(c) => !name(c),
            None => true,
        }
    })
}

/// Every regular file under `dir`, relative to `base`, hidden entries and links aside.
fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        match entry.file_type() {
            Ok(t) if hidden || t.is_symlink() => {}
            Ok(t) if t.is_dir() => walk(base, &path, out),
            Ok(t) if t.is_file() => {
                let rel = path.strip_prefix(base).unwrap_or(&path);
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
            _ => {}
        }
    }
}

/// Every file under `dir`, repository-relative, with its text.
fn collect_records(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let mut files = Vec::new();
    walk(root, dir, &mut files);
    crate::order::canonical_strings(&mut files);
    for file in files {
        if let Ok(text) = std::fs::read_to_string(root.join(&file)) {
            out.push((file, text));
        }
    }
}

/// The SHA-256 of a file in lowercase hex, as `sha256sum` writes it.
fn sha256(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// The first `# ` heading of the pack's README.md, else INDEX.md, else its first Markdown file.
fn first_heading(dir: &Path, files: &[String]) -> Option<String> {
    let markdown = || files.iter().find(|f| f.ends_with(".md"));
    let file = ["README.md", "INDEX.md"]
        .iter()
        .find_map(|n| files.iter().find(|f| f.as_str() == *n))
        .or_else(markdown)?;
    let mut text = String::new();
    std::fs::File::open(dir.join(file))
        .ok()?
        .take(HEADING_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|h| h.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    const NOW: u64 = 4_000_000_000;

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(NOW)
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn hash(text: &str) -> String {
        Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn pack<'a>(report: &'a StagedReport, name: &str) -> &'a StagedPack {
        report.packs.iter().find(|p| p.name == name).unwrap()
    }

    /// One tree holding a pack in every state, with the manifest, the unplanned list and the
    /// records that decide each.
    #[test]
    fn each_state_is_derived_from_the_tracked_tree() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for name in ["kept", "planned", "unplanned", "diverged", "fresh"] {
            write(root, &format!("tmp/owner-packs/{name}/a.md"), "# A\n");
        }
        write(root, "tmp/owner-packs/diverged/new.md", "added after keeping\n");
        write(root, "tmp/owner-packs/INDEX.md", "a file beside the packs is not a pack\n");
        write(root, "tmp/owner-packs/drafts/x/y.md", "a draft\n");
        write(root, "tmp/owner-packs/.hidden/z.md", "hidden\n");
        let manifest: String = ["kept", "planned", "unplanned", "diverged"]
            .iter()
            .map(|n| format!("{}  {n}/a.md\n", hash("# A\n")))
            .collect();
        write(root, MANIFEST, &manifest);
        write(root, UNPLANNED, "# comment naming kept\nunplanned  executed\n");
        write(root, ".ai/repo/project/issues/I1.yaml", "why: \"campaigns/planned, as kept\"\n");
        write(root, ".ai/repo/adrs/0001-x.md", "drafted in tmp/owner-packs/fresh.\n");
        write(root, ".ai/repo/adrs/0002-y.md", "campaigns/kept-but-longer and campaigns/kept.v2 are other packs\n");
        write(root, ".ai/repo/adrs/0003-z.md", "staged first as tmp/owner-packs/kept");

        let report = staged(root, Ok(root.to_path_buf()), now(), Some(7));

        let names: Vec<&str> = report.packs.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["diverged", "fresh", "kept", "planned", "unplanned"]);
        assert_eq!(pack(&report, "kept").state, PackState::Kept);
        assert_eq!(
            pack(&report, "kept").cited_by,
            [".ai/repo/adrs/0003-z.md"],
            "a longer name is not this pack, and a staged citation does not plan it"
        );
        assert_eq!(pack(&report, "planned").state, PackState::Planned);
        assert_eq!(pack(&report, "planned").cited_by, [".ai/repo/project/issues/I1.yaml"]);
        assert_eq!(pack(&report, "unplanned").state, PackState::Unplanned);
        assert_eq!(pack(&report, "diverged").state, PackState::Diverged);
        assert_eq!(pack(&report, "diverged").differing, ["new.md"]);
        assert_eq!(pack(&report, "diverged").files, 2);
        let fresh = pack(&report, "fresh");
        assert_eq!(fresh.state, PackState::Staged);
        assert_eq!(fresh.cited_by, [".ai/repo/adrs/0001-x.md"], "a staged citation is named");
        assert!(fresh.stale && report.stale == 5 - 4, "only a staged pack is ever stale");
        assert_eq!(report.places[0].holding, ["drafts"]);
        assert!(report.places[0].present && !report.places[1].present);
        assert!(report.places[1].reason.as_deref().unwrap().contains("tmp/prompt-packs"));
        assert_eq!(report.tallies.get("staged"), Some(&1));
        assert_eq!(report.tallies.get("kept"), Some(&1));
    }

    /// The bound decides staleness: under it, or with no bound declared, nothing is stale.
    #[test]
    fn a_pack_under_the_bound_or_without_one_is_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "tmp/prompt-packs/old/INDEX.md", "intro\n# Index heading\n");
        write(root, "tmp/prompt-packs/old/z.md", "# not first\n");
        let fresh = SystemTime::now();

        let under = staged(root, Ok(root.to_path_buf()), fresh, Some(7));
        let old = pack(&under, "old");
        assert!(!old.stale && old.age_days == Some(0));
        assert_eq!(old.first_heading.as_deref(), Some("Index heading"));
        assert_eq!(old.place, "tmp/prompt-packs");
        assert!(under.places[1].legacy && under.places[1].present);

        let unbound = staged(root, Ok(root.to_path_buf()), now(), None);
        assert!(!pack(&unbound, "old").stale, "no bound declared judges nothing stale");
        assert!(pack(&unbound, "old").age_days.unwrap() > 7);
    }

    /// A pack with no Markdown, or none with a heading, has no first heading; an empty pack
    /// has no modification time and is never stale.
    #[test]
    fn a_pack_without_a_heading_or_files_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "tmp/owner-packs/plain/notes.txt", "no markdown\n");
        write(root, "tmp/owner-packs/flat/a.md", "no heading here\n");
        std::fs::create_dir_all(root.join("tmp/owner-packs/empty")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join("tmp/owner-packs/plain/link"))
            .unwrap();

        let report = staged(root, Ok(root.to_path_buf()), now(), Some(1));
        assert_eq!(pack(&report, "plain").first_heading, None);
        assert_eq!(pack(&report, "plain").files, 1, "a link is not a file of the pack");
        assert_eq!(pack(&report, "flat").first_heading, None);
        let empty = pack(&report, "empty");
        assert_eq!((empty.files, empty.modified.as_deref(), empty.stale), (0, None, false));
    }

    /// A primary checkout that could not be derived is the report's reason, never an error.
    #[test]
    fn an_underived_primary_is_an_empty_answer_with_its_reason() {
        let dir = tempfile::tempdir().unwrap();
        let report = staged(dir.path(), Err("no git".into()), now(), Some(7));
        assert_eq!(report.reason.as_deref(), Some("no git"));
        assert!(report.primary.is_none() && report.places.is_empty() && report.packs.is_empty());
    }

    /// From a linked worktree, the primary checkout is the one whose `.git` is the common
    /// directory, not the worktree asking.
    #[test]
    fn a_linked_worktree_derives_the_primary_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("primary");
        let linked = dir.path().join("linked");
        let git = |at: &Path, args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(at)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        std::fs::create_dir_all(&primary).unwrap();
        git(&primary, &["init", "-q"]);
        git(&primary, &["commit", "-q", "--allow-empty", "-m", "root"]);
        git(&primary, &["worktree", "add", "-q", linked.to_str().unwrap()]);

        let from_linked = primary_checkout(&linked).unwrap();
        assert_eq!(from_linked.canonicalize().unwrap(), primary.canonicalize().unwrap());
        assert_eq!(primary_checkout(&primary).unwrap().canonicalize().unwrap(), primary.canonicalize().unwrap());

        let bare = dir.path().join("bare.git");
        git(dir.path(), &["init", "-q", "--bare", bare.to_str().unwrap()]);
        let err = primary_checkout(&bare).unwrap_err();
        assert!(err.contains("not a work tree's .git"), "{err}");
    }

    /// Every state has the name its JSON spells.
    #[test]
    fn every_state_names_itself_as_it_serialises() {
        for state in [
            PackState::Staged,
            PackState::Kept,
            PackState::Diverged,
            PackState::Planned,
            PackState::Unplanned,
        ] {
            assert_eq!(serde_json::to_value(state).unwrap(), state.as_str());
        }
    }
}
