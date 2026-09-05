//! The canonical policy, typed: `.ai/repo/policy.yaml` and the profiles beside it. The
//! policy is the one canonical input of the provider projections; the shell tool reads the
//! same file with the same subset reader, so the two agree byte for byte on what a
//! projection is and which hash it carries.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::metadata::yaml;
use crate::repository::Repository;

/// The manifest section that holds the policy file.
pub const POLICY_SECTION: &str = "policy";
/// The manifest section that holds the profiles directory.
pub const PROFILES_SECTION: &str = "profiles";

/// How a projection owns its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMode {
    /// The whole file is generated; its first line is the stamp.
    #[default]
    File,
    /// Only the text between `majordomus:begin` and `majordomus:end` markers is generated.
    Region,
}

/// One declared provider projection: `projections[]` in the policy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    /// The provider whose template renders the target: `agents`, `claude-code`, ...
    pub provider: String,
    /// Repository-relative target path, e.g. `AGENTS.md`.
    pub target: String,
    /// Whether every worker loads this target without asking; bounded by the budget.
    #[serde(default)]
    pub always_loaded: bool,
    /// Whole file or a marked region.
    #[serde(default)]
    pub mode: ProjectionMode,
}

/// `context:` of the policy, the part the projections need.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ContextPolicy {
    /// Hard cap, in lines, on an `always_loaded` projection.
    #[serde(default)]
    pub always_loaded_budget_lines: Option<u64>,
}

/// `profiles:` of the policy, the part the projections need.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ProfilesPolicy {
    /// The profile a worker starts with.
    #[serde(default)]
    pub default: Option<String>,
    /// The checkpoint interval the default profile sets.
    #[serde(default)]
    pub checkpoint_interval_default: Option<String>,
}

/// The policy, typed to what the projections consume. Every other key is carried through
/// unread: the policy schema under `share/schemas/policy.schema.json` owns the full shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Policy {
    /// `version:`.
    #[serde(default)]
    pub version: Option<u64>,
    /// `context:`.
    #[serde(default)]
    pub context: ContextPolicy,
    /// `profiles:`.
    #[serde(default)]
    pub profiles: ProfilesPolicy,
    /// `projections:`.
    #[serde(default)]
    pub projections: Vec<Projection>,
}

/// The policy as loaded: the typed value, where it came from, and the hash the stamps
/// carry.
#[derive(Debug, Clone)]
pub struct LoadedPolicy {
    /// The typed policy.
    pub policy: Policy,
    /// Repository-relative path of the policy file.
    pub path: String,
    /// SHA-256 (hex) of the policy file followed by every profile file, in name order,
    /// concatenated: exactly what the shell tool's `mj_policy_cat` hashes.
    pub sha256: String,
}

impl LoadedPolicy {
    /// Read the policy and profiles of a repository.
    pub fn load(repository: &Repository) -> Result<Self> {
        let rel =
            repository
                .section_path(POLICY_SECTION)
                .ok_or_else(|| Error::InvalidManifest {
                    path: repository.root().join(crate::repository::MANIFEST),
                    reason: format!("the manifest names no `{POLICY_SECTION}` section"),
                })?;
        let path = repository.root().join(&rel);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let policy: Policy = yaml::parse_into(&text).map_err(|reason| Error::InvalidPolicy {
            path: path.clone(),
            reason,
        })?;
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        for profile in profile_files(repository)? {
            let bytes = std::fs::read(&profile).map_err(|e| Error::io(&profile, e))?;
            hasher.update(&bytes);
        }
        Ok(LoadedPolicy {
            policy,
            path: rel,
            sha256: format!("{:x}", hasher.finalize()),
        })
    }
}

/// The profile files, `<profiles>/*.yaml`, sorted by name. An absent directory is no
/// profile, not an error.
pub fn profile_files(repository: &Repository) -> Result<Vec<PathBuf>> {
    let Some(rel) = repository.section_path(PROFILES_SECTION) else {
        return Ok(Vec::new());
    };
    let dir = repository.root().join(rel);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(&dir, e)),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file())
        .collect();
    files.sort();
    Ok(files)
}

/// The repository-relative directory where a repository may override a provider template.
pub fn repository_providers_dir(repository: &Repository) -> PathBuf {
    repository
        .ai_dir()
        .join(&repository.manifest().repo.path)
        .join("providers")
}

/// Hex SHA-256 of a text.
pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

/// A path is inside `root` after lexical normalisation: no absolute path, no `..`
/// component, nothing empty.
pub fn is_safe_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !p.is_absolute()
        && p.components().all(|c| {
            matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_defaults_are_file_mode_and_not_always_loaded() {
        let p: Policy = yaml::parse_into(
            "version: 1\nprojections:\n  - provider: agents\n    target: AGENTS.md\n",
        )
        .unwrap();
        assert_eq!(p.projections.len(), 1);
        assert_eq!(p.projections[0].mode, ProjectionMode::File);
        assert!(!p.projections[0].always_loaded);
        assert_eq!(p.profiles.default, None);
    }

    #[test]
    fn unknown_projection_keys_are_refused() {
        let r: std::result::Result<Policy, String> = yaml::parse_into(
            "projections:\n  - provider: agents\n    target: AGENTS.md\n    colour: red\n",
        );
        assert!(r.unwrap_err().contains("colour"));
    }

    #[test]
    fn safe_relative_paths() {
        assert!(is_safe_relative("AGENTS.md"));
        assert!(is_safe_relative("docs/x/y.md"));
        assert!(is_safe_relative("./AGENTS.md"));
        assert!(!is_safe_relative(""));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("../x"));
        assert!(!is_safe_relative("a/../../x"));
    }
}
