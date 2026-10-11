//! The version a landing allocates, applied where the integration executor already
//! refreshes a pull request (ADR 0134, slice B).
//!
//! Two seams and nothing else. Classification ([`super::relation`]) asks
//! [`version_only_conflict`]: under `release.allocation: landing` a conflict on the manifest
//! that vanishes once the head carries master's version is not a person's to resolve, because
//! the head's own number is no longer an author's statement. The refresh ([`super::drain`])
//! calls [`resolve_version_conflict`] when the merge of master stops, and [`allocate_in`]
//! after the merge and before the derive: master's version raised by what
//! [`crate::release::landing::next_for`] decides for the head. The head the required check
//! judges is then the head that lands, carrying the number it lands with.
//!
//! Every answer is computed from master as it is at that refresh. A retry, a second refresh
//! after master moved, or a recovery after a crash recomputes from the new master and
//! overwrites the head's number, so no integration raises a raised version again.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::release::landing::{self, Allocation, LandingDecision, ReleasePolicy};
use crate::release::version::{self, MANIFEST};

/// The trailer a refresh commit carries when it allocated a version.
pub const TRAILER: &str = "Version-Allocated";

/// The release policy master commits, at `master` in the clone at `root`.
///
/// Read from the commit, never from a working tree: the relation of a head to master is
/// cached by the pair of commit ids as a fact forever ([`super::relation`]), so whatever it
/// depends on must be a fact of those commits; and the trunk's policy is the one a landing
/// onto it obeys, whatever the executor's own checkout says. A directory with no
/// `.ai/manifest.yaml`, and a master that commits no policy, allocate nothing: the default,
/// `off`. A committed policy that cannot be read is an error, never `off`, because a refresh
/// made without its allocation lands a head that owes a number.
///
/// ```text
/// policy_at(Path::new("/tmp"), "master")  ->  Ok(ReleasePolicy { allocation: Off, .. })
/// ```
pub fn policy_at(root: &Path, master: &str) -> Result<ReleasePolicy, String> {
    if !root.join(".ai/manifest.yaml").is_file() {
        return Ok(ReleasePolicy::default());
    }
    let repo = crate::repository::Repository::open(root)
        .map_err(|e| format!("the repository could not be opened: {e}"))?;
    let rel = repo
        .section_path(crate::policy::POLICY_SECTION)
        .ok_or_else(|| "the manifest names no policy section".to_string())?;
    let Some(text) = show(root, master, &rel) else {
        return Ok(ReleasePolicy::default());
    };
    crate::metadata::yaml::parse_into::<crate::policy::Policy>(&text)
        .map(|p| p.release)
        .map_err(|e| format!("{master}:{rel} could not be read: {e}"))
}

/// Whether master allocates at landing. An unreadable policy answers no, which is the
/// stricter answer for a classification: the version conflict stays a person's.
pub fn allocates_at(root: &Path, master: &str) -> bool {
    policy_at(root, master).is_ok_and(|p| p.allocation == Allocation::Landing)
}

/// A directory of its own for one three-way merge, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An answer that may have failed, with what it was about: the one error sentence this
/// module writes for the file system, the version writer and git alike.
fn about<T, E: std::fmt::Display>(
    r: Result<T, E>,
    what: impl std::fmt::Display,
) -> Result<T, String> {
    r.map_err(|e| format!("{what}: {e}"))
}

fn scratch() -> Result<Scratch, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "majordomus-allocate-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let manifest = dir.join(MANIFEST);
    let parent = manifest.parent().unwrap_or(&dir);
    about(std::fs::create_dir_all(parent), parent.display())?;
    Ok(Scratch(dir))
}

/// `manifest` with its package version set to `to`, by the one writer of that line.
fn with_version(s: &Scratch, manifest: &str, to: &str) -> Result<String, String> {
    let path = s.0.join(MANIFEST);
    about(std::fs::write(&path, manifest), path.display())?;
    about(version::write(&s.0, to), "the version could not be written")?;
    about(std::fs::read_to_string(&path), path.display())
}

/// The manifest a three-way merge of `branch` and `trunk` over `base` gives once the branch
/// carries the trunk's version, or `None` when it still conflicts: something besides the
/// version line conflicts, and that stays a person's.
///
/// ```text
/// let base = "[package]\nversion = \"0.1.0\"\n\n[dependencies]\na = \"1\"\n";
/// let branch = "[package]\nversion = \"0.2.0\"\n\n[dependencies]\na = \"1\"\nb = \"1\"\n";
/// let trunk = "[package]\nversion = \"0.3.0\"\n\n[dependencies]\na = \"1\"\n";
/// merged_on_trunk_version(base, branch, trunk)  ->  Ok(Some(version 0.3.0, with b = "1"))
/// ```
pub fn merged_on_trunk_version(
    base: &str,
    branch: &str,
    trunk: &str,
) -> Result<Option<String>, String> {
    let Some(to) = version::declared_in(trunk) else {
        return Ok(None);
    };
    let s = scratch()?;
    let current = with_version(&s, branch, &to)?;
    let files = [
        ("current", current.as_str()),
        ("base", base),
        ("other", trunk),
    ];
    for (name, text) in files {
        about(std::fs::write(s.0.join(name), text), name)?;
    }
    let out = about(
        Command::new("git")
            .current_dir(&s.0)
            .args(["merge-file", "-p", "-q", "current", "base", "other"])
            .output(),
        "git merge-file could not run",
    )?;
    merge_verdict(out.status.code(), &out.stdout, &out.stderr)
}

/// What `git merge-file`'s exit says: the merged text, `None` for a conflict (a positive
/// status is the number of conflicts git left), and an error for anything else, a signal
/// included.
fn merge_verdict(
    code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Option<String>, String> {
    match code {
        Some(0) => Ok(Some(String::from_utf8_lossy(stdout).into_owned())),
        Some(n) if (1..=127).contains(&n) => Ok(None),
        _ => Err(format!(
            "git merge-file failed: {}",
            String::from_utf8_lossy(stderr).trim()
        )),
    }
}

fn show(root: &Path, rev: &str, path: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{rev}:{path}")])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Whether a merge of `head` into `master` conflicts on the manifest only because the two
/// declare different versions: the conflict vanishes once the head carries master's. Read
/// from git alone, as [`super::relation`] reads everything else, and `false` whenever any of
/// the three manifests cannot be read.
pub fn version_only_conflict(root: &Path, master: &str, head: &str) -> bool {
    let Some(base) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", master, head])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    else {
        return false;
    };
    match (
        show(root, &base, MANIFEST),
        show(root, master, MANIFEST),
        show(root, head, MANIFEST),
    ) {
        (Some(base), Some(trunk), Some(branch)) => {
            matches!(merged_on_trunk_version(&base, &branch, &trunk), Ok(Some(_)))
        }
        _ => false,
    }
}

/// The version a landing of `head` onto `master` allocates, when master allocates at
/// landing and the head, which contains master, does not carry it: such a head owes a
/// refresh although nothing of master is missing from it (ADR 0134, slice B2). `None` when
/// master does not allocate, when the head carries the allocated version, and when the
/// allocation cannot be decided, which the refresh would refuse in its own words.
///
/// Every input is a fact of the two commits, as [`super::relation`] requires of what it
/// caches: master's committed policy, its manifest and registry, the head's.
pub fn owed_version(root: &Path, master: &str, head: &str) -> Option<String> {
    let policy = policy_at(root, master)
        .ok()
        .filter(|p| p.allocation == Allocation::Landing)?;
    let decision = landing::next_for(root, master, head, &policy).ok()?;
    let carried = show(root, head, MANIFEST).and_then(|m| version::declared_in(&m));
    (carried.as_deref() != Some(decision.next_version.as_str())).then_some(decision.next_version)
}

/// In `dir`, where a merge of master into the head stopped: when the manifest is the only
/// path left unmerged and its conflict is the version line alone, resolve it to the merge
/// with master's version and stage it. `Ok(false)` leaves the merge as it stopped, for the
/// caller to refuse in its own words.
pub fn resolve_version_conflict(dir: &Path) -> Result<bool, String> {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let unmerged: Vec<String> = git(&["diff", "--name-only", "--diff-filter=U"])
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    if unmerged != [MANIFEST] {
        return Ok(false);
    }
    // stage 1 is the merge base, 2 the head being refreshed, 3 master
    let stage = |n: u8| git(&["show", &format!(":{n}:{MANIFEST}")]);
    let (Some(base), Some(branch), Some(trunk)) = (stage(1), stage(2), stage(3)) else {
        return Ok(false);
    };
    let Some(merged) = merged_on_trunk_version(&base, &branch, &trunk)? else {
        return Ok(false);
    };
    std::fs::write(dir.join(MANIFEST), merged).map_err(|e| format!("{MANIFEST}: {e}"))?;
    git(&["add", "--", MANIFEST])
        .map(|_| true)
        .ok_or_else(|| format!("git add {MANIFEST} failed"))
}

/// In `dir`, where master has just been merged into `head` and nothing is committed yet:
/// write the version a landing of `head` onto `master` allocates, decided by
/// [`landing::next_for`] in the clone at `root`. `None` when the policy does not allocate at
/// landing; an allocation that cannot be decided is an error, never a head left on a number
/// nobody chose.
pub fn allocate_in(
    root: &Path,
    dir: &Path,
    master: &str,
    head: &str,
    policy: &ReleasePolicy,
) -> Result<Option<LandingDecision>, String> {
    if policy.allocation != Allocation::Landing {
        return Ok(None);
    }
    let decision = landing::next_for(root, master, head, policy)
        .map_err(|e| format!("the version could not be allocated: {e}"))?;
    version::write(dir, &decision.next_version)
        .map_err(|e| format!("the allocated version could not be written: {e}"))?;
    Ok(Some(decision))
}

/// The trailer line a refresh commit carries for `decision`: the number, what selected it
/// and the trunk it was raised from, so `git log` answers which landing took which number.
///
/// ```text
/// Version-Allocated: 0.22.0 (minor from 0.21.0)
/// ```
pub fn trailer(decision: &LandingDecision) -> String {
    format!(
        "{TRAILER}: {} ({} from {})",
        decision.next_version,
        decision.selected.as_str(),
        decision.trunk_version
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str =
        "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[dependencies]\na = \"1\"\n";

    #[test]
    fn a_failure_says_what_it_was_about_and_git_merge_files_exit_is_read_in_full() {
        assert_eq!(
            about::<(), _>(Err("boom"), "the manifest"),
            Err("the manifest: boom".into())
        );
        assert_eq!(
            merge_verdict(Some(0), b"merged", b""),
            Ok(Some("merged".into()))
        );
        assert_eq!(merge_verdict(Some(2), b"", b""), Ok(None));
        assert_eq!(
            merge_verdict(Some(255), b"", b"usage"),
            Err("git merge-file failed: usage".into())
        );
        assert!(
            merge_verdict(None, b"", b"killed").is_err(),
            "a signal is no merge"
        );
    }

    fn git(root: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .expect("git")
    }

    fn ok(root: &Path, args: &[&str]) {
        let out = git(root, args);
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(path, body).expect("written");
    }

    /// A layer whose master commits `BASE` as the crate's manifest, this repository's own
    /// committed registry (so both refs publish the same surface) and `release` as its
    /// policy's release block.
    fn trunk(release: &str) -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a synthetic repository");
        let root = repo.root();
        ok(root, &["init", "-q", "-b", "master"]);
        write(root, MANIFEST, BASE);
        let registry = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(crate::release::surface::REGISTRY),
        )
        .expect("this repository's registry");
        write(root, crate::release::surface::REGISTRY, &registry);
        let policy = root.join(".ai/repo/policy.yaml");
        let text = std::fs::read_to_string(&policy).expect("the policy");
        std::fs::write(&policy, format!("{text}release:\n{release}")).expect("the policy");
        ok(root, &["add", "-A"]);
        ok(root, &["commit", "-qm", "base"]);
        repo
    }

    /// The policy is read from master's commit: allocation at landing when it says so, the
    /// default for a revision that does not exist, and every unreadable layer refused by
    /// what made it unreadable rather than taken as off.
    #[test]
    fn the_policy_is_read_from_masters_commit_and_an_unreadable_one_is_refused() {
        let repo = trunk("  allocation: landing\n  floor: minor\n");
        let root = repo.root();
        assert_eq!(
            policy_at(root, "master").expect("readable").allocation,
            Allocation::Landing
        );
        assert!(allocates_at(root, "master"));
        assert_eq!(
            policy_at(root, "no-such-rev").expect("default").allocation,
            Allocation::Off
        );

        let policy = root.join(".ai/repo/policy.yaml");
        let text = std::fs::read_to_string(&policy).unwrap();
        std::fs::write(
            &policy,
            text.replace("allocation: landing", "allocation: sideways"),
        )
        .unwrap();
        ok(root, &["commit", "-qam", "an unreadable policy"]);
        let refused = policy_at(root, "master").expect_err("refused");
        assert!(refused.contains("could not be read"), "{refused}");
        assert!(
            !allocates_at(root, "master"),
            "an unreadable policy allocates nothing"
        );

        let manifest = root.join(".ai/manifest.yaml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        std::fs::write(&manifest, text.replace("  policy: repo/policy.yaml\n", "")).unwrap();
        let refused = policy_at(root, "master").expect_err("refused");
        assert!(refused.contains("no policy section"), "{refused}");

        std::fs::write(&manifest, "schema: not-a-layer\n").unwrap();
        let refused = policy_at(root, "master").expect_err("refused");
        assert!(refused.contains("could not be opened"), "{refused}");
    }

    /// The whole seam on a real history (the head is a branch named `topic`: on a
    /// case-insensitive file system a branch named `head` is read as HEAD): a head and
    /// master that both moved the version
    /// conflict on that line alone; the version the head owes is the one a landing
    /// allocates; the refresh resolves the conflict to master's number, keeping the head's
    /// own change, and writes the allocated version over it.
    #[test]
    fn a_version_conflict_is_resolved_and_the_landing_number_written() {
        let repo = trunk("  allocation: landing\n  floor: minor\n");
        let root = repo.root();
        ok(root, &["checkout", "-q", "-b", "topic"]);
        write(
            root,
            MANIFEST,
            &BASE
                .replace("0.1.0", "0.1.1")
                .replace("a = \"1\"\n", "a = \"1\"\nb = \"2\"\n"),
        );
        ok(
            root,
            &["commit", "-qam", "the head moves its version and adds b"],
        );
        ok(root, &["checkout", "-q", "master"]);
        write(root, MANIFEST, &BASE.replace("0.1.0", "0.2.0"));
        ok(root, &["commit", "-qam", "master moves its version"]);

        assert!(version_only_conflict(root, "master", "topic"));
        assert!(!version_only_conflict(root, "master", "no-such-head"));
        let owed = owed_version(root, "master", "topic").expect("the head owes a number");
        assert_eq!(owed, "0.3.0", "master's 0.2.0 raised by the minor floor");

        ok(root, &["checkout", "-q", "topic"]);
        assert!(
            !git(root, &["merge", "-q", "master"]).status.success(),
            "the merge stops on the version"
        );
        assert_eq!(resolve_version_conflict(root), Ok(true));
        let merged = std::fs::read_to_string(root.join(MANIFEST)).unwrap();
        assert!(
            merged.contains("version = \"0.2.0\"") && merged.contains("b = \"2\""),
            "{merged}"
        );

        let landing = policy_at(root, "master").unwrap();
        let decision = allocate_in(root, root, "master", "topic", &landing)
            .expect("allocated")
            .expect("a decision under landing");
        assert_eq!(decision.next_version, "0.3.0");
        assert!(std::fs::read_to_string(root.join(MANIFEST))
            .unwrap()
            .contains("version = \"0.3.0\""));
        assert_eq!(
            allocate_in(root, root, "master", "topic", &ReleasePolicy::default()),
            Ok(None),
            "off allocates nothing"
        );
        ok(root, &["commit", "-qam", "the refresh"]);
        assert_eq!(
            resolve_version_conflict(root),
            Ok(false),
            "nothing unmerged, nothing to resolve"
        );
    }

    #[test]
    fn a_conflict_on_the_version_line_alone_resolves_to_the_trunks_number() {
        let branch = BASE.replace("0.1.0", "0.2.0");
        let trunk = BASE.replace("0.1.0", "0.3.0");
        let merged = merged_on_trunk_version(BASE, &branch, &trunk).unwrap();
        assert_eq!(merged.as_deref(), Some(trunk.as_str()));
    }

    #[test]
    fn the_branchs_own_change_beside_the_version_survives() {
        let branch = BASE
            .replace("0.1.0", "0.2.0")
            .replace("a = \"1\"\n", "a = \"1\"\nb = \"2\"\n");
        let trunk = BASE.replace("0.1.0", "0.3.0");
        let merged = merged_on_trunk_version(BASE, &branch, &trunk)
            .unwrap()
            .unwrap();
        assert!(merged.contains("version = \"0.3.0\""), "{merged}");
        assert!(merged.contains("b = \"2\""), "{merged}");
    }

    #[test]
    fn a_conflict_beside_the_version_stays_a_persons() {
        let branch = BASE
            .replace("0.1.0", "0.2.0")
            .replace("a = \"1\"", "a = \"2\"");
        let trunk = BASE
            .replace("0.1.0", "0.3.0")
            .replace("a = \"1\"", "a = \"3\"");
        assert_eq!(
            merged_on_trunk_version(BASE, &branch, &trunk).unwrap(),
            None
        );
    }

    #[test]
    fn a_trunk_that_declares_no_version_resolves_nothing() {
        let trunk = "[package]\nname = \"x\"\n";
        assert_eq!(merged_on_trunk_version(BASE, BASE, trunk).unwrap(), None);
    }

    #[test]
    fn the_trailer_names_the_number_the_selection_and_the_trunk() {
        let policy = ReleasePolicy {
            allocation: Allocation::Landing,
            floor: crate::release::compat::Impact::Minor,
            exempt: Vec::new(),
        };
        let d = landing::decide(
            version::Version::parse("0.21.0").unwrap(),
            crate::release::compat::Impact::None,
            &policy,
            &["src/a.rs".to_string()],
        );
        assert_eq!(trailer(&d), "Version-Allocated: 0.22.0 (minor from 0.21.0)");
    }

    #[test]
    fn an_unsupervised_directory_allocates_nothing() {
        let dir =
            std::env::temp_dir().join(format!("majordomus-allocate-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            policy_at(&dir, "master").unwrap().allocation,
            Allocation::Off
        );
        assert!(!allocates_at(&dir, "master"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn allocation_off_writes_nothing() {
        let dir = std::env::temp_dir();
        let got = allocate_in(&dir, &dir, "m", "h", &ReleasePolicy::default()).unwrap();
        assert!(got.is_none());
    }
}
