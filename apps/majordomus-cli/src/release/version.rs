//! The version: what the tree declares, what the commits imply, and the one writer that
//! raises it.
//!
//! The version is stated in two files and that stays true — `scripts/release-version` gives
//! the reason and it is a real one. What this adds is the missing half: something that
//! *writes* both, so that the check which proves they agree is proving the work of one
//! writer rather than the memory of one person.

use std::path::Path;

use super::model::{Change, ChangeKind, VersionReport};
use crate::model::Object;

/// The crate manifest: the authority for the version.
pub const MANIFEST: &str = "apps/majordomus-cli/Cargo.toml";
/// The shell tool, which prints its own version and cannot read the manifest at run time.
pub const ENTRY: &str = "bin/majordomus";
/// The lock file, which records the crate's own version alongside every dependency's.
///
/// Not a third *declaration* — cargo writes it from the manifest, and nobody chooses the
/// value. It is a third *site*, because the lock is tracked and the build runs `--locked`:
/// a bump that raised the manifest and left the lock behind made the next build fail with
/// `cannot update the lock file ... because --locked was passed`, which reads as a toolchain
/// fault rather than as a half-applied bump. The writer owns it for the same reason it owns
/// the other two.
pub const LOCK: &str = "apps/majordomus-cli/Cargo.lock";

/// How much a set of changes raises a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bump {
    /// Nothing since the last release.
    None,
    /// A fix, or anything that is not a feature.
    Patch,
    /// At least one feature.
    Minor,
    /// At least one breaking change.
    Major,
}

impl Bump {
    /// The word the reports use.
    pub fn as_str(self) -> &'static str {
        match self {
            Bump::None => "none",
            Bump::Patch => "patch",
            Bump::Minor => "minor",
            Bump::Major => "major",
        }
    }

    /// The bump named by a word, for `release bump --minor` and friends.
    pub fn parse(word: &str) -> Option<Bump> {
        match word {
            "major" => Some(Bump::Major),
            "minor" => Some(Bump::Minor),
            "patch" => Some(Bump::Patch),
            "none" => Some(Bump::None),
            _ => None,
        }
    }
}

/// What a set of conventional commits *says about itself*.
///
/// The rule is the conventional-commit rule and nothing more: a breaking change is major, a
/// feature is minor, anything else is patch, and no commits at all is none.
///
/// # This is evidence, not authority
///
/// It used to decide what `release bump` wrote, and that was the defect this subsystem
/// exists to remove. A commit message is a label a person chose, and a capability deleted
/// under a `refactor:` heading was a patch to this function and a broken caller to everyone
/// else. The compatibility level is now measured from the public contract by
/// [`crate::release::compat::analyze`]; this answer is carried beside it as
/// [`crate::release::compat::CommitEvidence`], and when the two disagree the plan says which
/// one understated the change. Nothing decides a version from this alone.
pub fn bump_of(changes: &[Change]) -> Bump {
    if changes.is_empty() {
        return Bump::None;
    }
    if changes.iter().any(|c| c.breaking) {
        return Bump::Major;
    }
    if changes.iter().any(|c| c.kind == ChangeKind::Feat) {
        return Bump::Minor;
    }
    Bump::Patch
}

/// A semantic version, only as much as this needs.
///
/// Ordered by the three numbers, which is what lets the writer refuse a version smaller than
/// the one the contract requires rather than only a *bump* smaller than required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// Major.
    pub major: u64,
    /// Minor.
    pub minor: u64,
    /// Patch.
    pub patch: u64,
}

impl Version {
    /// Read `1.2.3`; anything else is `None`.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.trim().split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Version {
            major,
            minor,
            patch,
        })
    }

    /// This version raised by `bump`.
    ///
    /// Arithmetic and nothing else: zero-major is not special-cased here, because whether a
    /// breaking change below 1.0 costs a major or a minor is a *policy* question and this
    /// repository answers it in exactly one place, [`crate::release::compat::Policy`]. A
    /// second answer here is the shape this subsystem was built to remove.
    pub fn raised(self, bump: Bump) -> Version {
        match bump {
            Bump::None => self,
            Bump::Patch => Version {
                patch: self.patch + 1,
                ..self
            },
            Bump::Minor => Version {
                minor: self.minor + 1,
                patch: 0,
                ..self
            },
            Bump::Major => Version {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            },
        }
    }
}

impl Version {
    /// This version raised by a measured compatibility impact.
    ///
    /// The bridge between the engine's verdict and the arithmetic: the policy has already
    /// decided what a `0.x` breaking change costs, so this only has to apply it.
    ///
    /// ```
    /// use majordomus_cli::release::compat::Impact;
    /// use majordomus_cli::release::version::Version;
    /// let v = Version::parse("0.5.0").unwrap();
    /// assert_eq!(v.raised_to(Impact::Minor).to_string(), "0.6.0");
    /// assert_eq!(v.raised_to(Impact::Major).to_string(), "1.0.0");
    /// assert_eq!(v.raised_to(Impact::None).to_string(), "0.5.0");
    /// ```
    pub fn raised_to(self, impact: crate::release::compat::Impact) -> Version {
        use crate::release::compat::Impact;
        self.raised(match impact {
            Impact::None => Bump::None,
            Impact::Patch => Bump::Patch,
            Impact::Minor => Bump::Minor,
            Impact::Major => Bump::Major,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The version the crate manifest declares.
pub fn declared(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(MANIFEST)).ok()?;
    // The first `version = "..."` inside `[package]`, which is where the manifest puts it;
    // a dependency's version further down must not be mistaken for the crate's.
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("version") {
                if let Some(v) = rest.split('"').nth(1) {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// The version the lock file records for this crate.
///
/// Only the entry whose `name` is this crate's: a dependency that happens to be at the same
/// version is a different package and is never read or written here.
///
/// ```
/// use majordomus_cli::release::version::locked;
/// let dir = tempfile::tempdir().unwrap();
/// // No lock file at all is `None`, never a guess.
/// assert_eq!(locked(dir.path()), None);
/// ```
pub fn locked(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(LOCK)).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_package = false;
            continue;
        }
        if line == "name = \"majordomus-cli\"" {
            in_package = true;
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("version") {
                return rest.split('"').nth(1).map(|v| v.to_string());
            }
        }
    }
    None
}

/// The version the shell tool prints.
pub fn tool(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(ENTRY)).ok()?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("MJ_VERSION="))
        .and_then(|v| v.split('"').nth(1).map(|v| v.to_string()))
}

/// Write `to` into both places, and say which files changed.
///
/// Byte-exact and narrow: the manifest's `version` line inside `[package]`, and the tool's
/// `MJ_VERSION=` line. Nothing else in either file is read or rewritten, so a manifest with
/// a dependency at the same version, or a tool with the string in a comment, is untouched.
pub fn write(root: &Path, to: &str) -> std::io::Result<Vec<String>> {
    let mut written = Vec::new();

    let manifest = root.join(MANIFEST);
    let text = std::fs::read_to_string(&manifest)?;
    let mut out = String::with_capacity(text.len());
    let mut in_package = false;
    let mut done = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
        }
        if in_package && !done && trimmed.starts_with("version") && trimmed.contains('"') {
            out.push_str(&format!("version = \"{to}\"\n"));
            done = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if out != text {
        std::fs::write(&manifest, out)?;
        written.push(MANIFEST.to_string());
    }

    // The lock's own entry, and only it: the `version` line that follows
    // `name = "majordomus-cli"`. Every other `version` in the file belongs to a dependency.
    let lock = root.join(LOCK);
    if let Ok(text) = std::fs::read_to_string(&lock) {
        let mut out = String::with_capacity(text.len());
        let mut here = false;
        let mut done = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "[[package]]" {
                here = false;
            } else if trimmed == "name = \"majordomus-cli\"" {
                here = true;
            } else if here && !done && trimmed.starts_with("version") && trimmed.contains('"') {
                out.push_str(&format!("version = \"{to}\"\n"));
                here = false;
                done = true;
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        if out != text {
            std::fs::write(&lock, out)?;
            written.push(LOCK.to_string());
        }
    }

    let entry = root.join(ENTRY);
    let text = std::fs::read_to_string(&entry)?;
    let out: String = text
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("MJ_VERSION=") {
                format!("MJ_VERSION=\"{to}\"")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    if out != text {
        std::fs::write(&entry, out)?;
        written.push(ENTRY.to_string());
    }

    Ok(written)
}

/// The whole version answer: what is declared, whether the two writers agree, and what the
/// commits since the last release imply.
///
/// The last release is the newest record the layer holds, which is the same source the
/// changelog uses — so the two never disagree about where "since" begins.
pub fn report(root: &Path, objects: &[Object]) -> VersionReport {
    let declared = declared(root).unwrap_or_else(|| "unknown".into());
    let tool_version = tool(root).unwrap_or_else(|| "unknown".into());

    let last = objects
        .iter()
        .filter(|o| o.kind == super::changelog::RELEASE_KIND)
        .filter_map(|o| {
            let v = o.metadata.get("version")?.as_str()?.to_string();
            let c = o.metadata.get("commit")?.as_str()?.to_string();
            let d = o
                .metadata
                .get("published_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            Some((d, v, c))
        })
        .max_by(|a, b| a.0.cmp(&b.0));

    let changes = match &last {
        Some((_, _, commit)) => super::commits::in_range(root, &format!("{commit}..HEAD"), objects),
        None => super::commits::in_range(root, "HEAD", objects),
    };
    let bump = bump_of(&changes);
    let next = Version::parse(&declared).map(|v| v.raised(bump).to_string());

    VersionReport {
        agree: declared == tool_version,
        declared,
        tool: tool_version,
        last_release: last.map(|(_, v, _)| v),
        bump: bump.as_str().to_string(),
        next,
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(kind: ChangeKind, breaking: bool) -> Change {
        Change {
            kind,
            scope: None,
            subject: "x".into(),
            breaking,
            commit: "abc1234".into(),
            url: None,
            references: Vec::new(),
        }
    }

    #[test]
    fn the_bump_is_a_total_function_of_the_changes() {
        assert_eq!(bump_of(&[]), Bump::None);
        assert_eq!(bump_of(&[change(ChangeKind::Chore, false)]), Bump::Patch);
        assert_eq!(bump_of(&[change(ChangeKind::Fix, false)]), Bump::Patch);
        assert_eq!(
            bump_of(&[
                change(ChangeKind::Fix, false),
                change(ChangeKind::Feat, false)
            ]),
            Bump::Minor
        );
        assert_eq!(
            bump_of(&[
                change(ChangeKind::Feat, false),
                change(ChangeKind::Fix, true)
            ]),
            Bump::Major
        );
    }

    #[test]
    fn raising_zeroes_what_the_bump_supersedes() {
        let v = Version::parse("0.3.1").unwrap();
        assert_eq!(v.raised(Bump::Patch).to_string(), "0.3.2");
        assert_eq!(v.raised(Bump::Minor).to_string(), "0.4.0");
        assert_eq!(v.raised(Bump::Major).to_string(), "1.0.0");
        assert_eq!(v.raised(Bump::None).to_string(), "0.3.1");
    }

    #[test]
    fn a_version_that_is_not_three_numbers_is_refused() {
        assert!(Version::parse("0.3").is_none());
        assert!(Version::parse("0.3.1.4").is_none());
        assert!(Version::parse("v0.3.1").is_none());
    }

    #[test]
    fn writing_touches_only_the_lines_that_state_the_version() {
        let dir = std::env::temp_dir().join(format!("mj-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("apps/majordomus-cli")).unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        // A dependency at the same version, and the string in a comment: neither may move.
        std::fs::write(
            dir.join(MANIFEST),
            "[package]\nname = \"majordomus-cli\"\nversion = \"0.3.1\"\n\n[dependencies]\nserde = { version = \"0.3.1\" }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join(ENTRY),
            "#!/usr/bin/env bash\n# was 0.3.1 once\nMJ_VERSION=\"0.3.1\"\necho hi\n",
        )
        .unwrap();
        // The lock: this crate's entry, and a dependency that happens to be at the same
        // version, which must not move.
        std::fs::write(
            dir.join(LOCK),
            "[[package]]\nname = \"serde\"\nversion = \"0.3.1\"\n\n[[package]]\nname = \"majordomus-cli\"\nversion = \"0.3.1\"\ndependencies = [\n \"serde\",\n]\n",
        )
        .unwrap();

        let written = write(&dir, "0.4.0").unwrap();
        assert_eq!(written.len(), 3, "all three sites, once each");
        assert_eq!(declared(&dir).as_deref(), Some("0.4.0"));
        assert_eq!(tool(&dir).as_deref(), Some("0.4.0"));
        assert_eq!(locked(&dir).as_deref(), Some("0.4.0"));

        let lock = std::fs::read_to_string(dir.join(LOCK)).unwrap();
        assert!(
            lock.contains("name = \"serde\"\nversion = \"0.3.1\""),
            "a dependency at the same version was rewritten: {lock}"
        );

        let manifest = std::fs::read_to_string(dir.join(MANIFEST)).unwrap();
        assert!(
            manifest.contains("serde = { version = \"0.3.1\" }"),
            "a dependency was rewritten: {manifest}"
        );
        let entry = std::fs::read_to_string(dir.join(ENTRY)).unwrap();
        assert!(
            entry.contains("# was 0.3.1 once"),
            "a comment was rewritten: {entry}"
        );

        // Writing the same version again changes nothing, so a bump is safe to repeat.
        assert!(write(&dir, "0.4.0").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
