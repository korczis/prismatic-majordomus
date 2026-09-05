//! The tool distribution's data directory: `share/kinds.yaml` and
//! `share/schemas/<kind>.schema.json`, read at run time. Nothing about kinds or keys is
//! compiled into the executable; the directory is located per invocation, explicitly or
//! by convention, and named in every error when it is not.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The environment variable that names the share directory.
pub const SHARE_ENV: &str = "MAJORDOMUS_SHARE";

/// The kinds file inside the share directory.
pub const KINDS_FILE: &str = "kinds.yaml";

/// The JSON Schema directory inside the share directory.
pub const SCHEMAS_DIR: &str = "schemas";

/// The directory of the shell tool's allow-lists, which `generate allow` derives from the
/// schemas.
pub const ALLOW_DIR: &str = "allow";

/// The suffix of a schema file: `<name>.schema.json`.
pub const SCHEMA_SUFFIX: &str = ".schema.json";

/// A located distribution directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Share {
    dir: PathBuf,
    /// How it was found, for diagnostics: `--share`, the environment, the repository, or
    /// the executable's own installation.
    pub origin: &'static str,
}

impl Share {
    /// Locate the share directory. In order: `explicit` (from `--share`), `MAJORDOMUS_SHARE`,
    /// `<repository root>/share` when it holds a kinds file (this repository supervising
    /// itself), and `<directory of the executable>/../share` (an installation laid out as
    /// `bin/` beside `share/`). A candidate that exists but holds no kinds file is skipped
    /// only when it came from a convention; an explicit path that lacks one is an error.
    pub fn locate(explicit: Option<&Path>, repo_root: &Path) -> Result<Self> {
        let mut tried = Vec::new();
        if let Some(p) = explicit {
            return Self::open(p, "--share");
        }
        if let Some(p) = std::env::var_os(SHARE_ENV) {
            return Self::open(Path::new(&p), SHARE_ENV);
        }
        let in_repo = repo_root.join("share");
        if in_repo.join(KINDS_FILE).is_file() {
            return Self::open(&in_repo, "repository");
        }
        tried.push(in_repo);
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let beside = dir.join("../share");
                if beside.join(KINDS_FILE).is_file() {
                    return Self::open(&beside, "installation");
                }
                tried.push(beside);
            }
        }
        Err(Error::ShareNotFound { tried })
    }

    fn open(dir: &Path, origin: &'static str) -> Result<Self> {
        let kinds = dir.join(KINDS_FILE);
        if !kinds.is_file() {
            return Err(Error::ShareNotFound {
                tried: vec![dir.to_path_buf()],
            });
        }
        let dir = dir.canonicalize().map_err(|e| Error::io(dir, e))?;
        Ok(Share { dir, origin })
    }

    /// The directory, canonical.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `<share>/kinds.yaml`.
    pub fn kinds_path(&self) -> PathBuf {
        self.dir.join(KINDS_FILE)
    }

    /// `<share>/schemas`.
    pub fn schemas_dir(&self) -> PathBuf {
        self.dir.join(SCHEMAS_DIR)
    }

    /// `<share>/allow`.
    pub fn allow_dir(&self) -> PathBuf {
        self.dir.join(ALLOW_DIR)
    }
}

/// Every `<name>.schema.json` under a schema directory, parsed, sorted by name. A directory
/// that does not exist yields nothing; that is the repository's case when it adds none.
pub fn read_schema_dir(dir: &Path) -> Result<Vec<(String, serde_json::Value)>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut schemas = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let Some(name) = file.strip_suffix(SCHEMA_SUFFIX) else {
            continue;
        };
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::KindSchema {
                reason: format!("{}: not JSON: {e}", path.display()),
            })?;
        schemas.push((name.to_string(), json));
    }
    schemas.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(schemas)
}
