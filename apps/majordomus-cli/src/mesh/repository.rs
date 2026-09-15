//! Which repository and which runtime a mesh participant is — independently of any path.
//!
//! A repository's mesh identity must be the same on every machine that holds a clone of
//! it and different for every other repository. The path of the git directory (what
//! `server.status` digests to tell checkouts apart) is neither: `/Users/a/repo` and
//! `/home/a/repo` are one repository with two paths. So the mesh identity is derived from
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

/// What the repository identity was derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeshRepositoryBasis {
    /// `cooperation.repository` in the mesh declaration.
    Declared,
    /// The repository's root commits.
    RootCommits,
}

/// A repository's mesh identity and where it came from.
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

/// The identity a declared value yields.
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

/// The identity a set of root commits yields, in any order.
pub fn of_root_commits(roots: &[String]) -> MeshRepositoryIdentity {
    let mut sorted: Vec<&str> = roots.iter().map(|r| r.trim()).collect();
    sorted.sort_unstable();
    sorted.dedup();
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

/// The runtime slot of a checkout: 16 hex of a digest of its checkout id.
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
