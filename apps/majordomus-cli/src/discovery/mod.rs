//! Discovery: which files of the repository are declarative sources, and of which kind.
//!
//! Nothing here walks `.ai/` looking for interesting files. The repository declares its
//! source classes in `sources.yaml` under the manifest's `knowledge` section, each class a
//! pathspec and a kind, and discovery enumerates exactly those. Two implementations of the
//! enumeration exist: the version-control index, which the contract prescribes, and a
//! filesystem walk with the same glob semantics for a checkout git cannot describe.

pub mod glob;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::git;
use crate::model::Diagnostic;
use crate::repository::Repository;

/// The file, under the `knowledge` section, that declares the source classes.
pub const SOURCES_FILE: &str = "sources.yaml";

/// The `version:` of `sources.yaml` this executable reads.
pub const SOURCES_VERSION: u64 = 1;

/// The prefix every pathspec carries: `*` never crosses a directory separator.
pub const GLOB_PREFIX: &str = ":(glob)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryKind {
    Vcs,
}

/// One class of `sources.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceClass {
    pub id: String,
    pub kind: String,
    pub discovery: DiscoveryKind,
    pub pathspec: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub version: u64,
    pub sources: Vec<SourceClass>,
}

impl Sources {
    /// Read `sources.yaml` from the repository's `knowledge` section.
    pub fn load(repo: &Repository) -> Result<Self> {
        let section = repo
            .section_path("knowledge")
            .ok_or_else(|| Error::InvalidManifest {
                path: repo.root().join(crate::repository::MANIFEST),
                reason: "no 'knowledge' section, so no sources can be declared".into(),
            })?;
        let path = repo.root().join(&section).join(SOURCES_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        Self::parse(&path, &text)
    }

    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let sources: Sources =
            crate::metadata::yaml::parse_into(text).map_err(|reason| Error::InvalidSources {
                path: path.to_path_buf(),
                reason,
            })?;
        if sources.version != SOURCES_VERSION {
            return Err(Error::InvalidSources {
                path: path.to_path_buf(),
                reason: format!("version {} is not {SOURCES_VERSION}", sources.version),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for class in &sources.sources {
            if !seen.insert(class.id.as_str()) {
                return Err(Error::InvalidSources {
                    path: path.to_path_buf(),
                    reason: format!("class '{}' is declared twice", class.id),
                });
            }
            if !class.pathspec.starts_with(GLOB_PREFIX) {
                return Err(Error::InvalidSources {
                    path: path.to_path_buf(),
                    reason: format!(
                        "class '{}': pathspec must start with {GLOB_PREFIX}",
                        class.id
                    ),
                });
            }
        }
        Ok(sources)
    }
}

/// A file discovered through a class, before it is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    pub class: String,
    pub kind: String,
    /// Repository-relative, forward slashes.
    pub rel_path: String,
}

/// How the files matching a pathspec are enumerated. The trait exists because two
/// enumerations are shipped and the caller chooses one explicitly.
pub trait DiscoverySource {
    /// A stable name for diagnostics: `vcs` or `filesystem`.
    fn name(&self) -> &'static str;
    /// Repository-relative paths matching `pathspec` (with its `:(glob)` prefix).
    fn enumerate(&self, root: &Path, pathspec: &str) -> Result<Vec<String>>;
}

/// The version-control index: tracked files only, as the contract prescribes.
pub struct VcsIndex;

impl DiscoverySource for VcsIndex {
    fn name(&self) -> &'static str {
        "vcs"
    }
    fn enumerate(&self, root: &Path, pathspec: &str) -> Result<Vec<String>> {
        git::ls_files(root, pathspec)
    }
}

/// A walk of the work tree with `:(glob)` semantics: for a checkout that is not a git work
/// tree, or when untracked files are wanted deliberately. Symlinks are never followed and
/// the checkout-local half of the layer is never a source.
pub struct FileSystem {
    /// Repository-relative directory prefixes never entered (`.git`, the local half).
    pub excluded: Vec<String>,
}

impl DiscoverySource for FileSystem {
    fn name(&self) -> &'static str {
        "filesystem"
    }
    fn enumerate(&self, root: &Path, pathspec: &str) -> Result<Vec<String>> {
        let pattern = pathspec.strip_prefix(GLOB_PREFIX).unwrap_or(pathspec);
        let matcher = glob::Glob::new(pattern);
        let mut out = Vec::new();
        walk(root, root, &matcher, &self.excluded, &mut out)?;
        out.sort();
        Ok(out)
    }
}

fn walk(
    root: &Path,
    dir: &Path,
    matcher: &glob::Glob,
    excluded: &[String],
    out: &mut Vec<String>,
) -> Result<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(dir, e))?;
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let Some(rel) = rel.to_str() else { continue };
        let rel = rel.replace('\\', "/");
        if excluded
            .iter()
            .any(|x| rel == *x || rel.starts_with(&format!("{x}/")))
        {
            continue;
        }
        let meta = std::fs::symlink_metadata(&path).map_err(|e| Error::io(&path, e))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if matcher.could_match_under(&rel) {
                walk(root, &path, matcher, excluded, out)?;
            }
        } else if meta.is_file() && matcher.matches(&rel) {
            out.push(rel);
        }
    }
    Ok(())
}

/// Run every class of `sources` through `source`, in declared class order, each class's
/// files sorted by path. A file two classes both match is kept under the first and reported.
pub fn discover(
    repo: &Repository,
    sources: &Sources,
    source: &dyn DiscoverySource,
) -> Result<(Vec<DiscoveredFile>, Vec<Diagnostic>)> {
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    let mut claimed = std::collections::BTreeMap::<String, String>::new();
    let local = repo.local_path();
    for class in &sources.sources {
        let mut paths = source.enumerate(repo.root(), &class.pathspec)?;
        paths.sort();
        paths.dedup();
        let mut count = 0;
        for rel in paths {
            if rel == local || rel.starts_with(&format!("{local}/")) {
                continue;
            }
            if let Some(first) = claimed.get(&rel) {
                diagnostics.push(Diagnostic::warning(
                    "claimed_twice",
                    Some(rel.clone()),
                    format!(
                        "matched by class '{}' after class '{first}'; kept under '{first}'",
                        class.id
                    ),
                ));
                continue;
            }
            claimed.insert(rel.clone(), class.id.clone());
            files.push(DiscoveredFile {
                class: class.id.clone(),
                kind: class.kind.clone(),
                rel_path: rel,
            });
            count += 1;
        }
        if class.required && count == 0 {
            diagnostics.push(Diagnostic::error(
                "required_source_empty",
                None,
                format!(
                    "required class '{}' ({}) discovered nothing",
                    class.id, class.pathspec
                ),
            ));
        }
        tracing::debug!(class = %class.id, kind = %class.kind, files = count, "discovered");
    }
    Ok((files, diagnostics))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_parse_and_refuse_bad_shapes() {
        let ok = "version: 1\nsources:\n  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/rules/**/*.md'\n    required: true\n";
        let s = Sources::parse(Path::new("s"), ok).unwrap();
        assert_eq!(s.sources[0].kind, "rule");
        let dup = format!("{ok}  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob)x'\n    required: false\n");
        assert!(matches!(
            Sources::parse(Path::new("s"), &dup),
            Err(Error::InvalidSources { .. })
        ));
        let v2 = ok.replace("version: 1", "version: 2");
        assert!(matches!(
            Sources::parse(Path::new("s"), &v2),
            Err(Error::InvalidSources { .. })
        ));
        let noglob = ok.replace(":(glob)", "");
        assert!(matches!(
            Sources::parse(Path::new("s"), &noglob),
            Err(Error::InvalidSources { .. })
        ));
        let unknown = format!("{ok}    colour: red\n");
        assert!(matches!(
            Sources::parse(Path::new("s"), &unknown),
            Err(Error::InvalidSources { .. })
        ));
    }
}
