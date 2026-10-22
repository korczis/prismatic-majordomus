//! Composition: the static surfaces of a topology, copied into one publishable tree.
//!
//! The producers own separate directories so that a rebuild of one cannot delete another's
//! output; publication needs one tree, and this is where the two meet. The mapping is the
//! resolved mount and nothing else, so a surface that appears in the topology appears in the
//! publication without a copy step being written anywhere.
//!
//! Two guarantees the tests hold this to: nothing is written outside the destination, and
//! the same topology over the same inputs produces the same tree.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::model::Topology;
use crate::error::{Error, Result};

/// Where publication is composed, repository-relative.
pub const PUBLISH_ROOT: &str = "target/site";

/// What a composition did: which surface put which files where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Composition {
    /// The destination, repository-relative when it is inside the repository.
    pub destination: String,
    /// Files written per surface id, in id order.
    pub files: BTreeMap<String, usize>,
}

impl Composition {
    /// The total number of files written.
    pub fn total(&self) -> usize {
        self.files.values().sum()
    }
}

/// Copy every published surface of `topology` into `destination`, under its mount.
///
/// The destination is emptied first — it is derived state, and a stale file from a surface
/// that no longer exists is exactly the "stale registration" this architecture refuses.
/// Source directories are never touched.
pub fn compose(topology: &Topology, root: &Path, destination: &Path) -> Result<Composition> {
    if destination.exists() {
        std::fs::remove_dir_all(destination).map_err(|e| Error::Io {
            path: destination.into(),
            source: e,
        })?;
    }
    std::fs::create_dir_all(destination).map_err(|e| Error::Io {
        path: destination.into(),
        source: e,
    })?;
    let canonical_destination = destination.canonicalize().map_err(|e| Error::Io {
        path: destination.into(),
        source: e,
    })?;

    let mut files = BTreeMap::new();
    let mut ordered: Vec<_> = topology.surfaces.iter().filter(|s| s.publishes()).collect();
    crate::order::canonical(&mut ordered);
    for surface in ordered {
        let Some(artifact) = surface.artifact.as_ref() else {
            continue;
        };
        let source = root.join(artifact);
        if !source.is_dir() {
            return Err(Error::InvalidSurface {
                surface: surface.id.clone(),
                reason: format!(
                    "{} does not exist; run its producer ({}) before composing",
                    artifact.display(),
                    surface.producer
                ),
            });
        }
        let target = if surface.mount.is_root() {
            canonical_destination.clone()
        } else {
            canonical_destination.join(surface.mount.relative())
        };
        let written = copy_tree(&source, &target, &canonical_destination, &surface.id)?;
        files.insert(surface.id.clone(), written);
    }
    Ok(Composition {
        destination: destination.to_string_lossy().to_string(),
        files,
    })
}

/// Copy a directory tree, refusing to write anything outside `boundary`.
fn copy_tree(source: &Path, target: &Path, boundary: &Path, surface: &str) -> Result<usize> {
    std::fs::create_dir_all(target).map_err(|e| Error::Io {
        path: target.into(),
        source: e,
    })?;
    let mut written = 0;
    let mut entries: Vec<_> = std::fs::read_dir(source)
        .map_err(|e| Error::Io {
            path: source.into(),
            source: e,
        })?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let from = entry.path();
        let name = entry.file_name();
        let to = target.join(&name);
        // The boundary is the promise this function makes: a symbolic link in a producer's
        // output, or a name that walks upward, may not put a file outside the destination.
        if !to.starts_with(boundary) {
            return Err(Error::InvalidSurface {
                surface: surface.into(),
                reason: format!("{} would be written outside the destination", to.display()),
            });
        }
        let kind = entry.file_type().map_err(|e| Error::Io {
            path: from.clone(),
            source: e,
        })?;
        if kind.is_symlink() {
            return Err(Error::InvalidSurface {
                surface: surface.into(),
                reason: format!(
                    "{} is a symbolic link; a published tree is copied literally",
                    from.display()
                ),
            });
        }
        if kind.is_dir() {
            written += copy_tree(&from, &to, boundary, surface)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| Error::Io {
                path: to.clone(),
                source: e,
            })?;
            written += 1;
        }
    }
    Ok(written)
}

/// Every file of a composed tree, repository-relative to it, sorted: what this module's
/// own tests compare a composition against.
///
/// Test support, and compiled only for tests: a helper that exists for the suite has no
/// business in the shipped binary, and `pub` on one is how internals leak into an API.
#[cfg(test)]
pub(crate) fn walk(root: &Path) -> Vec<std::path::PathBuf> {
    fn inner(dir: &Path, base: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = read.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                inner(&path, base, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_path_buf());
            }
        }
    }
    let mut out = Vec::new();
    inner(root, root, &mut out);
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::model::{Availability, Category, Mount, Surface, SurfaceKind, Visibility};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn static_surface(id: &str, mount: &str, artifact: &str) -> Surface {
        Surface {
            id: id.into(),
            title: id.into(),
            category: Category::Report,
            visibility: Visibility::Public,
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::parse(mount).unwrap(),
            producer: "test".into(),
            feature: None,
            artifact: Some(artifact.into()),
            index: Some("index.html".into()),
            availability: Availability::Both,
            built_from: None,
            provenance: BTreeMap::new(),
        }
    }

    fn seed(root: &Path, rel: &str, files: &[(&str, &str)]) {
        for (name, body) in files {
            let path = root.join(rel).join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
    }

    #[test]
    fn mounts_decide_the_publication_layout() {
        let tmp = tempfile::tempdir().unwrap();
        seed(
            tmp.path(),
            "site/public",
            &[("index.html", "app"), ("css/app.css", "x")],
        );
        seed(tmp.path(), "target/web/tests", &[("index.html", "tests")]);
        let topology = Topology::new(vec![
            static_surface("app", "/", "site/public"),
            static_surface("tests", "/tests", "target/web/tests"),
        ]);
        let dest = tmp.path().join("target/site");
        let composition = compose(&topology, tmp.path(), &dest).unwrap();
        assert_eq!(composition.total(), 3);
        let files = walk(&dest);
        assert!(files.contains(&PathBuf::from("index.html")));
        assert!(files.contains(&PathBuf::from("css/app.css")));
        assert!(files.contains(&PathBuf::from("tests/index.html")));
        assert_eq!(
            std::fs::read_to_string(dest.join("tests/index.html")).unwrap(),
            "tests"
        );
    }

    #[test]
    fn composition_is_deterministic_and_leaves_the_sources_alone() {
        let tmp = tempfile::tempdir().unwrap();
        seed(tmp.path(), "target/web/a", &[("index.html", "a")]);
        seed(tmp.path(), "target/web/b", &[("index.html", "b")]);
        let topology = Topology::new(vec![
            static_surface("a", "/a", "target/web/a"),
            static_surface("b", "/b", "target/web/b"),
        ]);
        let dest = tmp.path().join("target/site");
        let first = compose(&topology, tmp.path(), &dest).unwrap();
        let first_files = walk(&dest);
        let second = compose(&topology, tmp.path(), &dest).unwrap();
        assert_eq!(first, second);
        assert_eq!(first_files, walk(&dest));
        assert!(tmp.path().join("target/web/a/index.html").is_file());
    }

    #[test]
    fn a_surface_that_disappeared_leaves_nothing_behind() {
        let tmp = tempfile::tempdir().unwrap();
        seed(tmp.path(), "target/web/a", &[("index.html", "a")]);
        seed(tmp.path(), "target/web/gone", &[("index.html", "gone")]);
        let dest = tmp.path().join("target/site");
        let with_both = Topology::new(vec![
            static_surface("a", "/a", "target/web/a"),
            static_surface("gone", "/gone", "target/web/gone"),
        ]);
        compose(&with_both, tmp.path(), &dest).unwrap();
        assert!(dest.join("gone/index.html").is_file());
        let without = Topology::new(vec![static_surface("a", "/a", "target/web/a")]);
        compose(&without, tmp.path(), &dest).unwrap();
        assert!(
            !dest.join("gone").exists(),
            "a removed surface leaves no stale files"
        );
    }

    #[test]
    fn a_missing_producer_output_is_named_rather_than_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let topology = Topology::new(vec![static_surface("tests", "/tests", "target/web/tests")]);
        let err = compose(&topology, tmp.path(), &tmp.path().join("target/site"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("tests"), "{err}");
        assert!(err.contains("producer"), "{err}");
    }

    #[test]
    fn a_symbolic_link_in_a_producers_output_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        seed(tmp.path(), "target/web/a", &[("index.html", "a")]);
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc", tmp.path().join("target/web/a/escape")).unwrap();
        let topology = Topology::new(vec![static_surface("a", "/a", "target/web/a")]);
        let result = compose(&topology, tmp.path(), &tmp.path().join("target/site"));
        #[cfg(unix)]
        assert!(result.unwrap_err().to_string().contains("symbolic link"));
        #[cfg(not(unix))]
        assert!(result.is_ok());
    }
}
