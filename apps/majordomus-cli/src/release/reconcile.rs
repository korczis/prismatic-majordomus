//! The version line, merged without a conflict: the git merge driver for the two files the
//! one writer edits (`merge=version` on `apps/majordomus-cli/Cargo.toml` and `Cargo.lock`).
//!
//! Two branches that each advanced the version from the same trunk always conflict on one
//! line — `version = "1.11.0"` against `version = "1.11.0"` merges cleanly, but `1.11.0`
//! against `1.12.0`, or a branch's `2.0.0` against the trunk's `1.11.0`, is a textual conflict
//! about a number that neither side should be choosing by hand. The number a merge result
//! must carry is not a merge question at all: it is the version obligation of the merged tree
//! against the trunk (ADR 0106), which `release advance` decides and writes through the one
//! writer once the merge is done.
//!
//! So the driver takes the version line out of the merge. It reads the version each side
//! declares, rewrites the base and both sides to the greater of the two with the writer's own
//! rewrite ([`version::rewrite_manifest`], [`version::rewrite_lock`]), and three-way merges
//! what remains with `git merge-file`. Every other line merges exactly as git would merge it:
//! a dependency added on one side is kept, and two sides editing the same dependency still
//! conflict and still stop the merge. Taking the greater version is never the final answer,
//! only a starting point that cannot be below either side: the advance that follows raises it
//! to the obligation, so a major one side owed survives and a minor both sides claimed is
//! raised past the trunk.
//!
//! ```
//! use majordomus_cli::release::reconcile::{merge_version, VersionFile};
//! let toml = |v: &str, dep: &str| format!("[package]\nname = \"majordomus-cli\"\nversion = \"{v}\"\n\n[dependencies]\n{dep}");
//! let base = toml("1.10.0", "a = \"1\"\n");
//! let ours = toml("1.11.0", "a = \"1\"\nb = \"2\"\n");   // our advance, and a new dependency
//! let theirs = toml("1.12.0", "a = \"1\"\n");            // the trunk advanced twice
//! let m = merge_version(VersionFile::Manifest, &base, &ours, &theirs).unwrap();
//! assert!(m.clean);
//! assert!(m.text.contains("version = \"1.12.0\""));       // the greater, for advance to raise
//! assert!(m.text.contains("b = \"2\""));                  // our dependency survives
//! ```

use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::version::{self, Version};

/// Which of the two version files a merge is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionFile {
    /// `apps/majordomus-cli/Cargo.toml`.
    Manifest,
    /// `apps/majordomus-cli/Cargo.lock`.
    Lock,
}

impl VersionFile {
    /// The file a repository-relative path names, when it is one of the two.
    ///
    /// ```
    /// use majordomus_cli::release::reconcile::VersionFile;
    /// assert_eq!(VersionFile::of("apps/majordomus-cli/Cargo.lock"), Some(VersionFile::Lock));
    /// assert_eq!(VersionFile::of("Cargo.toml"), None);
    /// ```
    pub fn of(path: &str) -> Option<VersionFile> {
        if path == version::MANIFEST {
            Some(VersionFile::Manifest)
        } else if path == version::LOCK {
            Some(VersionFile::Lock)
        } else {
            None
        }
    }

    fn declared(self, text: &str) -> Option<Version> {
        let v = match self {
            VersionFile::Manifest => version::declared_in(text),
            VersionFile::Lock => version::locked_in(text),
        }?;
        Version::parse(&v)
    }

    fn rewrite(self, text: &str, to: &str) -> String {
        match self {
            VersionFile::Manifest => version::rewrite_manifest(text, to),
            VersionFile::Lock => version::rewrite_lock(text, to),
        }
    }
}

/// The result of one version-neutral merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionMerge {
    /// The merged text, with conflict markers when `clean` is false.
    pub text: String,
    /// Whether every hunk merged.
    pub clean: bool,
    /// The version the result carries before `release advance` raises it.
    pub version: Option<String>,
}

/// Merge three versions of one version file with the version line taken out of the merge.
///
/// A side whose version cannot be read is merged as git would merge it: the driver never
/// invents a number it could not read, and a conflict it cannot explain stays a conflict.
pub fn merge_version(
    file: VersionFile,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<VersionMerge, String> {
    let target = match (file.declared(ours), file.declared(theirs)) {
        (Some(a), Some(b)) => Some(a.max(b).to_string()),
        _ => None,
    };
    let (base, ours, theirs) = match &target {
        Some(to) => (
            file.rewrite(base, to),
            file.rewrite(ours, to),
            file.rewrite(theirs, to),
        ),
        None => (base.to_string(), ours.to_string(), theirs.to_string()),
    };
    let (text, clean) = merge_file(&base, &ours, &theirs)?;
    Ok(VersionMerge {
        text,
        clean,
        version: target,
    })
}

/// `git merge-file -p`, which is git's own three-way text merge: the same hunks, the same
/// conflict markers a merge without this driver would have produced.
fn merge_file(base: &str, ours: &str, theirs: &str) -> Result<(String, bool), String> {
    let dir = scratch_dir()?;
    let result = (|| {
        let write = |name: &str, text: &str| -> Result<std::path::PathBuf, String> {
            let p = dir.join(name);
            std::fs::File::create(&p)
                .and_then(|mut f| f.write_all(text.as_bytes()))
                .map_err(|e| format!("cannot write {name}: {e}"))?;
            Ok(p)
        };
        let (o, a, b) = (
            write("base", base)?,
            write("ours", ours)?,
            write("theirs", theirs)?,
        );
        let out = Command::new("git")
            .args([
                "merge-file",
                "-p",
                "-L",
                "ours",
                "-L",
                "base",
                "-L",
                "theirs",
            ])
            .arg(&a)
            .arg(&o)
            .arg(&b)
            .output()
            .map_err(|e| format!("cannot run git merge-file: {e}"))?;
        // the exit status is the number of conflicts, and negative on an error
        match out.status.code() {
            Some(0) => Ok((String::from_utf8_lossy(&out.stdout).into_owned(), true)),
            Some(n) if n > 0 => Ok((String::from_utf8_lossy(&out.stdout).into_owned(), false)),
            _ => Err(format!(
                "git merge-file failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )),
        }
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// A directory of this process's own under the system's temporary directory.
fn scratch_dir() -> Result<std::path::PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("mj-merge-version-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Run the driver as git invokes it: `%O %A %B %P`. The merged text is written over `%A`,
/// as git requires of a driver, and the exit status is 0 when every hunk merged and 1 when
/// conflict markers were left for a person — exactly what git's own merge would have left.
pub fn drive(base: &Path, ours: &Path, theirs: &Path, path: &str) -> Result<bool, String> {
    let read = |p: &Path| {
        std::fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()))
    };
    let (o, a, b) = (read(base)?, read(ours)?, read(theirs)?);
    let merged = match VersionFile::of(path) {
        Some(file) => merge_version(file, &o, &a, &b)?,
        // a path the attribute names but the driver does not know: merged as git would
        None => {
            let (text, clean) = merge_file(&o, &a, &b)?;
            VersionMerge {
                text,
                clean,
                version: None,
            }
        }
    };
    std::fs::write(ours, &merged.text)
        .map_err(|e| format!("cannot write {}: {e}", ours.display()))?;
    Ok(merged.clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml(v: &str, deps: &str) -> String {
        format!("[package]\nname = \"majordomus-cli\"\nversion = \"{v}\"\n\n[dependencies]\n{deps}")
    }

    fn lock(v: &str, extra: &str) -> String {
        format!("version = 4\n\n[[package]]\nname = \"majordomus-cli\"\nversion = \"{v}\"\n{extra}")
    }

    #[test]
    fn two_advances_from_one_trunk_merge_to_the_greater_without_a_conflict() {
        let m = merge_version(
            VersionFile::Manifest,
            &toml("1.10.0", "a = \"1\"\n"),
            &toml("1.11.0", "a = \"1\"\n"),
            &toml("1.12.0", "a = \"1\"\n"),
        )
        .unwrap();
        assert!(m.clean, "{}", m.text);
        assert_eq!(m.version.as_deref(), Some("1.12.0"));
        assert_eq!(m.text, toml("1.12.0", "a = \"1\"\n"));
    }

    #[test]
    fn a_major_one_side_owed_survives_the_merge() {
        let m = merge_version(
            VersionFile::Manifest,
            &toml("1.10.0", ""),
            &toml("2.0.0", ""),
            &toml("1.11.0", ""),
        )
        .unwrap();
        assert!(m.clean);
        assert!(m.text.contains("version = \"2.0.0\""), "{}", m.text);
    }

    #[test]
    fn a_dependency_edit_still_merges_and_a_competing_one_still_conflicts() {
        let kept = merge_version(
            VersionFile::Manifest,
            &toml("1.10.0", "a = \"1\"\n"),
            &toml("1.11.0", "a = \"1\"\nb = \"2\"\n"),
            &toml("1.11.0", "a = \"1\"\n"),
        )
        .unwrap();
        assert!(kept.clean);
        assert!(kept.text.contains("b = \"2\""), "{}", kept.text);

        let fought = merge_version(
            VersionFile::Manifest,
            &toml("1.10.0", "a = \"1\"\n"),
            &toml("1.11.0", "a = \"2\"\n"),
            &toml("1.12.0", "a = \"3\"\n"),
        )
        .unwrap();
        assert!(
            !fought.clean,
            "two edits of one dependency are a real conflict"
        );
        assert!(fought.text.contains("<<<<<<< ours"), "{}", fought.text);
        // the version line itself is not part of the conflict
        assert!(
            fought.text.contains("version = \"1.12.0\"\n"),
            "{}",
            fought.text
        );
    }

    #[test]
    fn the_lock_merges_the_same_way() {
        let m = merge_version(
            VersionFile::Lock,
            &lock("1.10.0", ""),
            &lock("1.11.0", ""),
            &lock("1.12.0", ""),
        )
        .unwrap();
        assert!(m.clean, "{}", m.text);
        assert_eq!(m.version.as_deref(), Some("1.12.0"));
    }

    #[test]
    fn a_side_whose_version_cannot_be_read_is_merged_as_git_would() {
        let m = merge_version(
            VersionFile::Manifest,
            &toml("1.10.0", ""),
            &toml("1.11.0", ""),
            "[package]\nname = \"majordomus-cli\"\nversion = \"not-three-numbers\"\n\n[dependencies]\n",
        )
        .unwrap();
        assert_eq!(m.version, None, "no number is invented");
        assert!(!m.clean, "the conflict it cannot explain stays a conflict");
    }

    #[test]
    fn the_driver_writes_over_ours_and_reports_conflicts_by_its_result() {
        let dir = std::env::temp_dir().join(format!("mj-drive-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (o, a, b) = (dir.join("o"), dir.join("a"), dir.join("b"));
        std::fs::write(&o, toml("1.10.0", "")).unwrap();
        std::fs::write(&a, toml("1.11.0", "")).unwrap();
        std::fs::write(&b, toml("1.11.0", "")).unwrap();
        assert!(drive(&o, &a, &b, version::MANIFEST).unwrap());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), toml("1.11.0", ""));
        assert!(drive(&o, &dir.join("absent"), &b, version::MANIFEST).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
