//! Repository discovery: the nearest ancestor of the start directory that carries
//! `.ai/manifest.yaml` is the root, and the manifest is the only thing read to find it.
//!
//! `.git` is not a marker: an arbitrary git repository is not a Majordomus repository. A
//! `.majordomus/` directory is not a marker either; it is an optional installation of the
//! tool, or the pre-`.ai` layout, which is refused by name.

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
