//! Which repository and which runtime a mesh participant is — independently of any path.
//!
//! A repository's mesh identity must be the same on every machine that holds a clone of
//! it and different for every other repository. The path of the git directory (what
//! `server.status` digests to tell checkouts apart) is neither: a macOS checkout path and
//! the Linux checkout path of the same clone are one repository with two spellings. So the
//! mesh identity is derived from
//! the repository's **root commits** — content every full clone shares and no unrelated
//! repository has — or, when a person declares one, from the declared identity
//! (`cooperation.repository`), which is what a shallow clone needs and what separates a
//! fork that should not cooperate with its origin.
//!
//! A runtime is one server of one node: one per checkout. Its id is a digest of the
//! checkout id, so a restarted server of the same checkout is the same runtime (a new
//! instance of it), and two worktrees of one machine are two runtimes.
//!
//! ```
//! use majordomus_cli::mesh::repository::{declared, of_root_commits, runtime_id};
//!
//! let a = of_root_commits(&["b".repeat(40), "a".repeat(40)]);
//! let b = of_root_commits(&["a".repeat(40), "b".repeat(40)]);
//! assert_eq!(a.id, b.id, "the order git lists roots in does not matter");
//! assert_ne!(a.id, declared("majordomus").unwrap().id);
//! assert_eq!(runtime_id("checkout-1").len(), 16);
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::MeshError;

const REPOSITORY_DOMAIN: &str = "majordomus-mesh-repository/v1\n";
const RUNTIME_DOMAIN: &str = "majordomus-mesh-runtime/v1\n";

/// What the repository identity was derived from, which a reader needs in order to judge
/// it: an identity taken from the root commits is a fact about the history two clones
/// share, while a declared one is a decision somebody made, and only the second can be
/// changed by editing a file.
///
/// ```
/// use majordomus_cli::mesh::repository::{declared, of_root_commits, MeshRepositoryBasis};
///
/// assert_eq!(declared("majordomus").unwrap().basis, MeshRepositoryBasis::Declared);
/// assert_eq!(
///     of_root_commits(&["a".repeat(40)]).basis,
///     MeshRepositoryBasis::RootCommits,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeshRepositoryBasis {
    /// `cooperation.repository` in the mesh declaration.
    Declared,
    /// The repository's root commits.
    RootCommits,
}

/// A repository's mesh identity, carried by every advertisement, link and event, together
/// with the account of where it came from. The `detail` exists so that a person told two
/// checkouts are not the same repository can see what each one was derived from instead of
/// comparing two opaque digests.
///
/// ```
/// use majordomus_cli::mesh::repository::MeshRepositoryIdentity;
/// use majordomus_cli::mesh::repository::of_root_commits;
///
/// let identity: MeshRepositoryIdentity = of_root_commits(&["a".repeat(40)]);
/// assert_eq!(identity.id.len(), 32, "32 hex characters, on every machine");
/// assert_eq!(identity.detail, "aaaaaaaaaaaa", "the evidence, short enough to read");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MeshRepositoryIdentity {
    /// 32 hex: what advertisements, links and events carry.
    pub id: String,
    /// What it was derived from.
    pub basis: MeshRepositoryBasis,
    /// The evidence, human-readable: the declared value, or the root commits.
    pub detail: String,
}

fn digest(text: &str, hex_chars: usize) -> String {
    let hash = Sha256::digest(text.as_bytes());
    hash.iter()
        .take(hex_chars / 2)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The identity a declared value yields: what a shallow clone, which cannot see its own
/// root commit, must use, and what separates a fork that should not cooperate with the
/// repository it came from. The value is a name chosen by the people who run the mesh, not
/// a digest, so it is bounded and must not be empty — an empty declaration would put every
/// repository that forgot to fill it in on one mesh.
///
/// ```
/// use majordomus_cli::mesh::repository::declared;
///
/// let a = declared("majordomus").unwrap();
/// assert_eq!(a.id, declared("  majordomus  ").unwrap().id, "surrounding space is noise");
/// assert_ne!(a.id, declared("majordomus-fork").unwrap().id);
/// assert!(declared("").is_err(), "an empty declaration would merge unrelated repositories");
/// ```
pub fn declared(value: &str) -> Result<MeshRepositoryIdentity, MeshError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(MeshError::Config(
            "a declared repository identity is 1 to 128 characters".into(),
        ));
    }
    Ok(MeshRepositoryIdentity {
        id: digest(&format!("{REPOSITORY_DOMAIN}declared\n{value}"), 32),
        basis: MeshRepositoryBasis::Declared,
        detail: value.into(),
    })
}

/// The identity a set of root commits yields. The roots are treated as a set rather than a
/// list, because `git rev-list` makes no promise about the order it prints them in and two
/// machines that disagreed about that order would decide they held different repositories;
/// for the same reason the same root named twice is one root.
///
/// ```
/// use majordomus_cli::mesh::repository::of_root_commits;
///
/// let a = of_root_commits(&["b".repeat(40), "a".repeat(40)]);
/// let b = of_root_commits(&["a".repeat(40), "b".repeat(40), "a".repeat(40)]);
/// assert_eq!(a.id, b.id, "a set: order and repetition are not information");
/// assert_ne!(a.id, of_root_commits(&["a".repeat(40)]).id);
/// ```
pub fn of_root_commits(roots: &[String]) -> MeshRepositoryIdentity {
    // A set, so the digest reads the same on every machine whatever order the commits were
    // listed in, and the same root named twice is one root.
    let sorted: Vec<&str> = roots
        .iter()
        .map(|r| r.trim())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let joined = sorted.join("\n");
    MeshRepositoryIdentity {
        id: digest(&format!("{REPOSITORY_DOMAIN}roots\n{joined}"), 32),
        basis: MeshRepositoryBasis::RootCommits,
        detail: sorted
            .iter()
            .map(|r| r.chars().take(12).collect::<String>())
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Resolve the mesh identity of the repository at `root`: the declared value when there
/// is one; otherwise its root commits, refusing a shallow clone (whose "root" is only
/// where its history was cut) and a repository with no commit yet.
///
/// The declaration is consulted first and answers on its own, so a caller that has one
/// never runs git — which is what lets a shallow clone, a worktree of one, or a directory
/// that is not a repository at all still take part in a mesh.
///
/// ```
/// use majordomus_cli::mesh::repository::{declared, resolve};
///
/// let nowhere = tempfile::tempdir().unwrap();
/// let stated = resolve(nowhere.path(), Some("majordomus")).unwrap();
/// assert_eq!(stated.id, declared("majordomus").unwrap().id, "git was never asked");
///
/// // without one, the history has to answer, and a directory holding no history cannot
/// assert!(resolve(nowhere.path(), None).is_err());
/// ```
pub fn resolve(
    root: &Path,
    declared_value: Option<&str>,
) -> Result<MeshRepositoryIdentity, MeshError> {
    if let Some(value) = declared_value {
        return declared(value);
    }
    let git = |args: &[&str]| -> Result<String, MeshError> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|e| MeshError::Config(format!("git: {e}")))?;
        if !out.status.success() {
            return Err(MeshError::Config(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    if git(&["rev-parse", "--is-shallow-repository"])? == "true" {
        return Err(MeshError::Config(
            "a shallow clone's root commit is where its history was cut, not the repository's: declare cooperation.repository in the mesh declaration".into(),
        ));
    }
    let roots: Vec<String> = git(&["rev-list", "--max-parents=0", "HEAD"])
        .map_err(|_| {
            MeshError::Config("the repository has no commit yet: nothing identifies it".into())
        })?
        .lines()
        .map(str::to_string)
        .collect();
    if roots.is_empty() {
        return Err(MeshError::Config(
            "the repository has no root commit: nothing identifies it".into(),
        ));
    }
    Ok(of_root_commits(&roots))
}

/// The runtime slot of a checkout: 16 hex of a digest of its checkout id. A runtime is one
/// server of one node, so this is what makes a restarted server the same runtime as the
/// one it replaced — a new instance of it rather than a new participant — while two
/// worktrees of one machine stay two runtimes that can claim against each other.
///
/// ```
/// use majordomus_cli::mesh::repository::runtime_id;
///
/// assert_eq!(runtime_id("checkout-1"), runtime_id("checkout-1"), "a restart is not a new runtime");
/// assert_ne!(runtime_id("checkout-1"), runtime_id("checkout-2"));
/// assert_eq!(runtime_id("checkout-1").len(), 16);
/// ```
pub fn runtime_id(checkout_id: &str) -> String {
    digest(&format!("{RUNTIME_DOMAIN}{checkout_id}"), 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("git runs");
        assert!(
            status.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn repo(dir: &Path, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q"]);
        std::fs::write(dir.join("f"), content).unwrap();
        git(dir, &["add", "f"]);
        git(dir, &["commit", "-q", "-m", "root"]);
    }

    #[test]
    fn two_clones_at_different_paths_are_one_repository_and_another_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("machine-a/repo");
        repo(&origin, "one");
        let clone = tmp.path().join("machine-b/elsewhere/repo");
        std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
        git(
            tmp.path(),
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        let other = tmp.path().join("machine-a/other");
        repo(&other, "two");

        let a = resolve(&origin, None).unwrap();
        let b = resolve(&clone, None).unwrap();
        let c = resolve(&other, None).unwrap();
        assert_eq!(a.id, b.id, "same history, different paths: one repository");
        assert_ne!(a.id, c.id, "different history: different repositories");
        assert_eq!(a.basis, MeshRepositoryBasis::RootCommits);
        // A declaration overrides the history, both ways.
        assert_eq!(
            resolve(&origin, Some("x")).unwrap().id,
            resolve(&other, Some("x")).unwrap().id
        );
        assert_ne!(resolve(&origin, Some("x")).unwrap().id, a.id);
    }

    #[test]
    fn a_shallow_clone_and_an_empty_repository_are_refused_with_the_remedy() {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin");
        repo(&origin, "one");
        std::fs::write(origin.join("f"), "two").unwrap();
        git(&origin, &["commit", "-q", "-am", "second"]);
        let shallow = tmp.path().join("shallow");
        git(
            tmp.path(),
            &[
                "clone",
                "-q",
                "--depth",
                "1",
                &format!("file://{}", origin.display()),
                shallow.to_str().unwrap(),
            ],
        );
        let error = resolve(&shallow, None).unwrap_err().to_string();
        assert!(error.contains("cooperation.repository"), "{error}");
        assert!(resolve(&shallow, Some("declared")).is_ok());

        let empty = tmp.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        git(&empty, &["init", "-q"]);
        assert!(resolve(&empty, None).is_err());
    }

    #[test]
    fn a_runtime_is_stable_per_checkout_and_distinct_across_checkouts() {
        assert_eq!(runtime_id("c1"), runtime_id("c1"));
        assert_ne!(runtime_id("c1"), runtime_id("c2"));
        assert!(runtime_id("c1").bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(declared("").is_err());
    }
}
