//! The generated manifest: the resolved topology, written down for whoever needs it without
//! recomputing it.
//!
//! It is derived state with the same status as every other generated tree in this
//! repository — reproducible, disposable, never read as truth that could not be recomputed.
//! A publisher or a CI job may consume it; nothing may edit it, and nothing depends on it
//! existing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::discover::GENERATED_ROOT;
use super::model::Topology;
use super::validate::Finding;
use crate::error::{Error, Result};

/// Where the manifest is written, repository-relative.
pub const MANIFEST_PATH: &str = "target/web/manifest.json";

/// The manifest's contract.
pub const MANIFEST_SCHEMA: &str = "web-topology/v1";

/// The resolved topology as a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The contract this document follows.
    pub schema: String,
    /// The executable that resolved it.
    pub version: String,
    /// The generated root every static producer writes into.
    pub generated_root: String,
    /// The surfaces, in route-precedence order.
    #[serde(flatten)]
    pub topology: Topology,
    /// What validation said about them when the manifest was written.
    pub findings: Vec<Finding>,
}

impl Manifest {
    /// Build a manifest from a resolved topology and its findings.
    pub fn new(topology: Topology, findings: Vec<Finding>, version: &str) -> Self {
        Manifest {
            schema: MANIFEST_SCHEMA.into(),
            version: version.into(),
            generated_root: GENERATED_ROOT.into(),
            topology,
            findings,
        }
    }

    /// Write it under the generated root, creating the directory when it is not there.
    ///
    /// Returns the path written, repository-relative, so a caller can name it without
    /// knowing where it goes.
    pub fn write(&self, root: &Path) -> Result<PathBuf> {
        let path = root.join(MANIFEST_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Io {
                path: parent.into(),
                source: e,
            })?;
        }
        let mut body = serde_json::to_string_pretty(self).map_err(|e| Error::InvalidSurface {
            surface: "manifest".into(),
            reason: format!("cannot be serialised: {e}"),
        })?;
        body.push('\n');
        std::fs::write(&path, body).map_err(|e| Error::Io {
            path: path.clone(),
            source: e,
        })?;
        Ok(PathBuf::from(MANIFEST_PATH))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::model::{Availability, Mount, Surface, SurfaceKind};
    use std::collections::BTreeMap;

    #[test]
    fn a_manifest_round_trips_and_is_deterministic() {
        let topology = Topology::new(vec![Surface {
            id: "tests".into(),
            title: "Test report".into(),
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::parse("/tests").unwrap(),
            producer: "quality".into(),
            artifact: Some("target/web/tests".into()),
            index: Some("index.html".into()),
            availability: Availability::Both,
            provenance: BTreeMap::new(),
        }]);
        let manifest = Manifest::new(topology, Vec::new(), "0.0.0");
        let once = serde_json::to_string(&manifest).unwrap();
        let twice = serde_json::to_string(&manifest).unwrap();
        assert_eq!(once, twice);
        let back: Manifest = serde_json::from_str(&once).unwrap();
        assert_eq!(back, manifest);
    }

    #[test]
    fn writing_creates_the_generated_root() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = Manifest::new(Topology::new(Vec::new()), Vec::new(), "0.0.0");
        let written = manifest.write(tmp.path()).unwrap();
        assert_eq!(written, PathBuf::from(MANIFEST_PATH));
        assert!(tmp.path().join(MANIFEST_PATH).is_file());
    }
}
