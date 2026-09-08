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

/// The directory of the shell tool's body-section lists, which `generate documents`
/// derives from the `.proto` document schemas. The allow-list is the header half of a
/// contract and this is the body half; a shell cannot read either out of a schema.
pub const SECTIONS_DIR: &str = "sections";

/// The subdirectory of [`SCHEMAS_DIR`] holding one JSON Schema per generated document.
/// A kind's schema is projected from a `.proto` under `<vendor>/<name>/`, so nothing here
/// is ever read as a kind's schema.
pub const GENERATED_SCHEMAS_DIR: &str = "generated";

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

    /// The distribution's default scope declaration, the file `majordomus init` seeds.
    pub fn skeleton_scope(&self) -> PathBuf {
        self.dir.join(crate::scope::SKELETON_SCOPE)
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

    /// `<share>/sections`.
    pub fn sections_dir(&self) -> PathBuf {
        self.dir.join(SECTIONS_DIR)
    }

    /// `<share>/schemas/generated`: the contracts of the generated documents, matched to a
    /// document by the `const` of its `schema` member.
    pub fn generated_schemas_dir(&self) -> PathBuf {
        self.dir.join(SCHEMAS_DIR).join(GENERATED_SCHEMAS_DIR)
    }

    /// `<share>/providers`: the provider templates the tool ships, `<provider>.tmpl` each.
    pub fn providers_dir(&self) -> PathBuf {
        self.dir.join("providers")
    }

    /// The providers the tool has a template for, by id (the file stem), sorted. A
    /// distribution without the directory has none; that is not an error, it is a
    /// distribution that ships no adapter.
    pub fn provider_templates(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.providers_dir()) else {
            return Vec::new();
        };
        let mut out: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter_map(|n| n.strip_suffix(".tmpl").map(str::to_string))
            .collect();
        out.sort();
        out
    }
}

/// Every `<vendor>/<name>/<name>.v<n>.schema.json` under a schema root, parsed, keyed by
/// the schema identity its path derives (`majordomus.adr/v1`) and sorted by it. A root that
/// does not exist yields nothing; that is the repository's case when it adds no schema of
/// its own.
///
/// The identity and the path fix each other, so a schema can be found from a `kinds.yaml`
/// entry without a directory scan and a file cannot quietly answer to a name it does not
/// declare. A file whose path does not have the shape is refused by name rather than
/// skipped, because a schema nobody loads validates nothing and says so nowhere.
pub fn read_schema_dir(dir: &Path) -> Result<Vec<(String, serde_json::Value)>> {
    let mut schemas = Vec::new();
    read_schema_tree(dir, dir, &mut schemas)?;
    schemas.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(schemas)
}

fn read_schema_tree(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, serde_json::Value)>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        if path.is_dir() {
            // `<schemas>/generated/` holds the contracts of the *generated documents* —
            // the manifest, the registry, the benchmark matrix. Those are not kinds of the
            // layer, carry no `<vendor>/<name>/` identity, and are read by
            // [`read_generated_schema_dir`] instead.
            if dir == root
                && path.file_name().and_then(|f| f.to_str()) == Some(GENERATED_SCHEMAS_DIR)
            {
                continue;
            }
            read_schema_tree(root, &path, out)?;
            continue;
        }
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !file.ends_with(SCHEMA_SUFFIX) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file.to_string());
        let identity = schema_identity(&relative).ok_or_else(|| Error::KindSchema {
            reason: format!(
                "{}: a schema's path is <vendor>/<name>/<name>.v<n>{SCHEMA_SUFFIX}, which this is not",
                path.display()
            ),
        })?;
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::KindSchema {
                reason: format!("{}: not JSON: {e}", path.display()),
            })?;
        out.push((identity, json));
    }
    Ok(())
}

/// Every `<name>.schema.json` directly under a directory, parsed, keyed by that `<name>`
/// and sorted by it. A directory that does not exist yields nothing.
///
/// This is the reader for `<share>/schemas/generated/`, whose files are the contracts of
/// the *generated documents* rather than of the layer's kinds: they answer to the document
/// id they pin in `properties.schema.const`, not to a `<vendor>/<name>` path, so the shape
/// [`read_schema_dir`] insists on does not apply to them.
pub fn read_generated_schema_dir(dir: &Path) -> Result<Vec<(String, serde_json::Value)>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))? {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if path.is_dir() || !file.ends_with(SCHEMA_SUFFIX) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::KindSchema {
                reason: format!("{}: not JSON: {e}", path.display()),
            })?;
        out.push((file[..file.len() - SCHEMA_SUFFIX.len()].to_string(), json));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// The identity a schema path carries: `majordomus/adr/adr.v1.schema.json` is
/// `majordomus.adr/v1`. A path of another shape yields nothing.
///
/// ```
/// use majordomus_cli::share::schema_identity;
/// assert_eq!(
///     schema_identity("majordomus/adr/adr.v1.schema.json").as_deref(),
///     Some("majordomus.adr/v1")
/// );
/// assert_eq!(schema_identity("adr.schema.json"), None);
/// assert_eq!(schema_identity("majordomus/adr/other.v1.schema.json"), None);
/// ```
pub fn schema_identity(relative: &str) -> Option<String> {
    let stem = relative.strip_suffix(SCHEMA_SUFFIX)?;
    let mut parts = stem.split('/');
    let vendor = parts.next()?;
    let name = parts.next()?;
    let file = parts.next()?;
    if parts.next().is_some() || vendor.is_empty() || name.is_empty() {
        return None;
    }
    let version = file.strip_prefix(&format!("{name}."))?;
    if !version.starts_with('v') || version.len() < 2 {
        return None;
    }
    Some(format!("{vendor}.{name}/{version}"))
}
