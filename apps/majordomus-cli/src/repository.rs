//! Repository discovery: the nearest ancestor of the start directory that carries
//! `.ai/manifest.yaml` is the root, and the manifest is the only thing read to find it.
//!
//! `.git` is not a marker: an arbitrary git repository is not a Majordomus repository. A
//! `.majordomus/` directory is not a marker either; it is an optional installation of the
//! tool, or the pre-`.ai` layout, which is refused by name.
//!
//! # Why the marker is the manifest
//!
//! Every other candidate marker is ambiguous. `.git` says a directory is version
//! controlled, which almost every directory a person runs this in will be; a `.ai`
//! directory alone says nothing about whether it is *this* layer. The manifest declares its
//! own schema, so discovery either finds a layer this executable can read or it says which
//! schema it found and which it reads — never a half-open repository.
//!
//! # Errors
//!
//! [`crate::Error::RepositoryNotFound`] when no ancestor carries the manifest, naming where
//! the search began; [`crate::Error::LegacyLayout`] when it finds the pre-`.ai` layout,
//! naming the migration; and the manifest's own parse and schema errors, each naming the
//! file. None of them is a diagnostic: without a manifest nothing can be discovered at all.
//!
//! ```
//! use majordomus_cli::Repository;
//!
//! // a directory that is not a repository is an error that says where it looked
//! let empty = tempfile::tempdir().unwrap();
//! let err = Repository::discover(empty.path()).unwrap_err();
//! assert!(matches!(err, majordomus_cli::Error::RepositoryNotFound { .. }));
//! assert_eq!(err.exit_code(), 12, "the code for something that is not there");
//!
//! // and a git repository is not one either: the marker is the manifest and nothing else
//! std::fs::create_dir(empty.path().join(".git")).unwrap();
//! assert!(Repository::discover(empty.path()).is_err());
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::metadata::yaml;

/// The file whose presence makes a directory the root of a Majordomus repository.
pub const MANIFEST: &str = ".ai/manifest.yaml";

/// The layer schema this executable reads.
pub const LAYER_SCHEMA: &str = "ai-repository/v1";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
/// The tracked half of the layer.
pub struct RepoHalf {
    /// Relative to `.ai/`.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
/// The checkout-local half of the layer, never a source.
pub struct LocalHalf {
    /// Relative to `.ai/`.
    pub path: String,
    /// Tracked in git? The contract says `false`.
    pub tracked: bool,
    /// Loaded into a worker's context implicitly? The contract says `false`.
    pub implicit_context: bool,
}

/// The scoped context documents: file names that must carry the context contract
/// (`schema: context/v1`) wherever they appear under the layer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ContextConventions {
    #[serde(default)]
    /// File names, e.g. `README.md`.
    pub documents: Vec<String>,
}

/// `.ai/manifest.yaml`, typed. Unknown keys are refused through `share/allow/manifest.txt`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The layer schema, [`LAYER_SCHEMA`].
    pub schema: String,
    /// Where the tracked half is.
    pub repo: RepoHalf,
    /// Where the checkout-local half is.
    pub local: LocalHalf,
    /// Section name to path, relative to `.ai/`. Sorted by name.
    pub sections: BTreeMap<String, String>,
    /// Optional: absent in a layer written before context documents existed.
    #[serde(default)]
    pub context: Option<ContextConventions>,
}

impl Manifest {
    /// Parse the manifest. The typed struct refuses a key it does not read and names it;
    /// the distribution's `manifest` schema is applied by the application once the share
    /// directory is located, for the constraints a type cannot express.
    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let map = yaml::parse_mapping(text).map_err(|reason| Error::InvalidManifest {
            path: path.to_path_buf(),
            reason,
        })?;
        let schema = map
            .get("schema")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if schema != LAYER_SCHEMA {
            return Err(Error::UnsupportedSchema {
                path: path.to_path_buf(),
                found: schema,
                supported: LAYER_SCHEMA.to_string(),
            });
        }
        serde_json::from_value(serde_json::Value::Object(map)).map_err(|e| {
            let reason = e.to_string();
            match reason.strip_prefix("unknown field `") {
                Some(rest) => Error::UnknownKeys {
                    path: path.to_path_buf(),
                    keys: vec![rest.split('`').next().unwrap_or(rest).to_string()],
                },
                None => Error::InvalidManifest {
                    path: path.to_path_buf(),
                    reason,
                },
            }
        })
    }
}

/// A located repository with its manifest read.
#[derive(Debug, Clone)]
pub struct Repository {
    root: PathBuf,
    manifest: Manifest,
}

impl Repository {
    /// Walk from `start` upward to the nearest directory carrying [`MANIFEST`]. A manifest
    /// that does not parse stops the search with its error: a nearer broken layer is never
    /// skipped in favour of a farther working one.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = start.canonicalize().map_err(|e| Error::io(start, e))?;
        for dir in start.ancestors() {
            let manifest = dir.join(MANIFEST);
            if manifest.is_file() {
                return Self::open(dir);
            }
            if is_legacy_layout(dir) {
                return Err(Error::LegacyLayout {
                    root: dir.to_path_buf(),
                });
            }
        }
        Err(Error::RepositoryNotFound { start })
    }

    /// Open a directory known to be a root.
    pub fn open(root: &Path) -> Result<Self> {
        let manifest_path = root.join(MANIFEST);
        let text =
            std::fs::read_to_string(&manifest_path).map_err(|e| Error::io(&manifest_path, e))?;
        let manifest = Manifest::parse(&manifest_path, &text)?;
        Ok(Repository {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// The root, absolute and canonical.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The manifest, typed.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The manifest as a JSON value, for schema validation.
    pub fn manifest_value(&self) -> Result<serde_json::Value> {
        let path = self.root.join(MANIFEST);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let map =
            yaml::parse_mapping(&text).map_err(|reason| Error::InvalidManifest { path, reason })?;
        Ok(serde_json::Value::Object(map))
    }

    /// `.ai/`, absolute.
    pub fn ai_dir(&self) -> PathBuf {
        self.root.join(".ai")
    }

    /// The repository-relative path of a section, e.g. `.ai/repo/rules`, or `None` when
    /// the manifest does not name it.
    pub fn section_path(&self, name: &str) -> Option<String> {
        self.manifest.sections.get(name).map(|p| format!(".ai/{p}"))
    }

    /// The repository-relative path of the checkout-local half, never a source.
    pub fn local_path(&self) -> String {
        format!(".ai/{}", self.manifest.local.path)
    }

    /// The repository-relative path of the tracked half.
    pub fn repo_path(&self) -> String {
        format!(".ai/{}", self.manifest.repo.path)
    }

    /// The manifest section a repository-relative path falls under, by longest prefix.
    pub fn section_of(&self, rel_path: &str) -> Option<&str> {
        let mut best: Option<(&str, usize)> = None;
        for (name, p) in &self.manifest.sections {
            let full = format!(".ai/{p}");
            let matches = rel_path == full || rel_path.starts_with(&format!("{full}/"));
            if matches && best.is_none_or(|(_, len)| full.len() > len) {
                best = Some((name.as_str(), full.len()));
            }
        }
        best.map(|(n, _)| n)
    }
}

/// Project data under `.majordomus/` with no `.ai/manifest.yaml` is the pre-`.ai` layout.
/// A `.majordomus/` that holds a tool distribution (`bin/majordomus`) is an installation.
fn is_legacy_layout(dir: &Path) -> bool {
    let mj = dir.join(".majordomus");
    mj.is_dir() && !mj.join("bin/majordomus").is_file()
}

/// A repository's identity over the wire: a digest of its absolute root.
///
/// A server on a loopback socket has to be able to say *which* repository it serves, so
/// that a second process can tell a live server for this repository from one for another.
/// The root itself would answer that and would also tell whoever reached the socket where
/// the checkout sits on the host, which is of no use to them and of some use to somebody
/// else. A digest answers the question and discloses nothing: a process that already knows
/// the root can compute it and compare, and a process that does not cannot invert it.
///
/// Two repositories at one path are one repository, which is exactly the identity a lease
/// needs. A repository that moves is a different one, which is also right: the lease of the
/// old path names a server that is no longer serving that path.
///
/// ```
/// use majordomus_cli::repository::identity;
/// use std::path::Path;
/// assert_eq!(identity(Path::new("/a/b")), identity(Path::new("/a/b")));
/// assert_ne!(identity(Path::new("/a/b")), identity(Path::new("/a/c")));
/// assert_eq!(identity(Path::new("/a/b")).len(), 32);
/// assert!(!identity(Path::new("/a/b")).contains('/'));
/// ```
pub fn identity(root: &Path) -> String {
    digest(root)
}

fn digest(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    format!("{:x}", hasher.finalize())[..32].to_string()
}

/// The git repository a checkout belongs to, as distinct from the checkout itself.
///
/// [`identity`] names a checkout: one server, one lease, one index per checkout root. Every
/// linked worktree of one git repository is a checkout of its own by that measure, and
/// nothing in it said that they belong together. This does: the git directory every work
/// tree shares (`git rev-parse --git-common-dir`) is the repository's identity, its digest
/// is one value for every checkout of that repository, and `linked` says whether this
/// checkout is the primary one or a work tree hanging off it. `None` where git cannot be
/// asked: a repository of the layer does not have to be version controlled.
///
/// ```
/// use majordomus_cli::repository::{git_identity, identity, GitIdentity};
/// use std::process::Command;
/// let dir = tempfile::tempdir().unwrap();
/// let root = dir.path().join("repo");
/// std::fs::create_dir_all(&root).unwrap();
/// let git = |args: &[&str]| assert!(Command::new("git").arg("-C").arg(&root).args(args).status().unwrap().success());
/// git(&["init", "-q", "."]);
/// git(&["-c", "user.email=t@example.com", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "init"]);
/// let wt = dir.path().join("repo-wt");
/// git(&["worktree", "add", "-q", "-b", "feature/x", wt.to_str().unwrap()]);
///
/// let primary: GitIdentity = git_identity(&root).expect("a work tree");
/// let linked: GitIdentity = git_identity(&wt).expect("a linked work tree");
/// assert_eq!(primary.id, linked.id, "one repository");
/// assert_ne!(identity(&root), identity(&wt), "two checkouts");
/// assert!(!primary.linked && linked.linked);
/// assert!(!primary.id.contains('/'), "a digest, never a path");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct GitIdentity {
    /// The git directory every work tree of the repository shares, canonical.
    pub common_dir: PathBuf,
    /// Its digest: the same for every checkout of one repository, different across
    /// repositories, and never a path.
    pub id: String,
    /// Whether this checkout is a linked work tree rather than the primary one.
    pub linked: bool,
}

/// Ask git which repository the checkout at `root` belongs to. One subprocess; `None`
/// when git is absent or the root is not a work tree.
///
/// ```
/// use majordomus_cli::repository::git_identity;
/// let plain = tempfile::tempdir().unwrap();
/// assert!(git_identity(plain.path()).is_none(), "a plain directory belongs to no repository");
/// ```
pub fn git_identity(root: &Path) -> Option<GitIdentity> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    // relative to the current directory when the current directory is inside the work
    // tree, absolute otherwise; both forms are answers
    let common = PathBuf::from(&raw);
    let common = if common.is_absolute() {
        common
    } else {
        root.join(common)
    };
    let common = common.canonicalize().ok()?;
    Some(GitIdentity {
        id: digest(&common),
        linked: root.join(".git").is_file(),
        common_dir: common,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST_TEXT: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  rules: repo/rules\n";

    #[test]
    fn manifest_parses_and_sections_resolve() {
        let m = Manifest::parse(Path::new("m"), MANIFEST_TEXT).unwrap();
        assert_eq!(m.schema, LAYER_SCHEMA);
        assert_eq!(m.sections["rules"], "repo/rules");
        assert_eq!(m.context, None);
        let with_context = format!("{MANIFEST_TEXT}context:\n  documents: [README.md]\n");
        let m = Manifest::parse(Path::new("m"), &with_context).unwrap();
        assert_eq!(m.context.unwrap().documents, vec!["README.md"]);
    }

    #[test]
    fn manifest_unknown_key_is_named() {
        let bad = format!("{MANIFEST_TEXT}extra: 1\n");
        match Manifest::parse(Path::new("m"), &bad) {
            Err(Error::UnknownKeys { keys, .. }) => assert_eq!(keys, vec!["extra"]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn manifest_wrong_schema_is_unsupported() {
        let bad = MANIFEST_TEXT.replace("v1", "v2");
        assert!(matches!(
            Manifest::parse(Path::new("m"), &bad),
            Err(Error::UnsupportedSchema { .. })
        ));
    }

    #[test]
    fn manifest_malformed_yaml_is_invalid() {
        assert!(matches!(
            Manifest::parse(Path::new("m"), "schema:\tai-repository/v1\n"),
            Err(Error::InvalidManifest { .. })
        ));
    }
}
