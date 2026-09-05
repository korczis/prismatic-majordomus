//! The index: every declarative object the repository declares, read, validated and
//! identified, plus every diagnostic about what could not be read. Built once per
//! invocation from a discovery pass; the MCP surface is a projection of it.
//!
//! Failure policy: a file that cannot become an object is excluded and reported with an
//! error diagnostic, and the index is `Degraded`. The manifest and `sources.yaml` are
//! different: without them nothing can be discovered, so they are errors, not diagnostics.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::discovery::{self, DiscoveredFile, DiscoverySource, Sources};
use crate::error::Result;
use crate::git::GitState;
use crate::metadata::{frontmatter, yaml, Format, FrontMatterRule, KindSchema, KindSpec};
use crate::model::{uri_for, Diagnostic, Object, Provenance, Severity};
use crate::repository::Repository;

/// A file larger than this is refused rather than read.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Whether every discovered file became an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Ok,
    Degraded,
}

/// What the index knows about the repository it was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryInfo {
    pub root: String,
    pub layer_schema: String,
    /// Manifest section name to repository-relative path.
    pub sections: BTreeMap<String, String>,
    pub git: GitState,
    /// `vcs` or `filesystem`.
    pub discovery: String,
    /// Source class id to kind, in declared order.
    pub source_classes: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct Index {
    pub repository: RepositoryInfo,
    /// Sorted by URI; unique by URI.
    pub objects: Vec<Object>,
    pub diagnostics: Vec<Diagnostic>,
    pub state: State,
}

impl Index {
    /// Discover, read, validate. `source` decides how files are enumerated; `git` is
    /// whatever `git::inspect` said and is carried, never consulted here.
    pub fn build(
        repo: &Repository,
        sources: &Sources,
        schema: &KindSchema,
        source: &dyn DiscoverySource,
        git: GitState,
    ) -> Result<Self> {
        let (files, mut diagnostics) = discovery::discover(repo, sources, source)?;
        let mut objects = Vec::with_capacity(files.len());
        for file in &files {
            match read_objects(repo, schema, file) {
                Ok((mut read, mut member_diagnostics)) => {
                    objects.append(&mut read);
                    diagnostics.append(&mut member_diagnostics);
                }
                Err(d) => diagnostics.push(d),
            }
        }
        dedupe(&mut objects, &mut diagnostics);
        objects.sort_by(|a, b| a.uri.cmp(&b.uri));
        let state = if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            State::Degraded
        } else {
            State::Ok
        };
        let repository = RepositoryInfo {
            root: repo.root().display().to_string(),
            layer_schema: repo.manifest().schema.clone(),
            sections: repo
                .manifest()
                .sections
                .keys()
                .filter_map(|k| repo.section_path(k).map(|p| (k.clone(), p)))
                .collect(),
            git,
            discovery: source.name().to_string(),
            source_classes: sources
                .sources
                .iter()
                .map(|c| (c.id.clone(), c.kind.clone()))
                .collect(),
        };
        tracing::info!(
            objects = objects.len(),
            diagnostics = diagnostics.len(),
            state = ?state,
            "index built"
        );
        Ok(Index {
            repository,
            objects,
            diagnostics,
            state,
        })
    }

    pub fn get(&self, uri: &str) -> Option<&Object> {
        self.objects
            .binary_search_by(|o| o.uri.as_str().cmp(uri))
            .ok()
            .map(|i| &self.objects[i])
    }

    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// The kinds present, with how many objects each has.
    pub fn kinds(&self) -> BTreeMap<&str, usize> {
        let mut m = BTreeMap::new();
        for o in &self.objects {
            *m.entry(o.kind.as_str()).or_insert(0) += 1;
        }
        m
    }
}

/// Two objects of one kind claiming one identity are both excluded.
fn dedupe(objects: &mut Vec<Object>, diagnostics: &mut Vec<Diagnostic>) {
    let mut by_uri: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for o in objects.iter() {
        by_uri
            .entry(o.uri.clone())
            .or_default()
            .push(o.provenance.path.clone());
    }
    let duplicates: BTreeMap<&String, &Vec<String>> =
        by_uri.iter().filter(|(_, paths)| paths.len() > 1).collect();
    if duplicates.is_empty() {
        return;
    }
    for (uri, paths) in &duplicates {
        for path in paths.iter() {
            diagnostics.push(Diagnostic::error(
                "duplicate_identity",
                Some(path.clone()),
                format!(
                    "{uri} is claimed by {}; every claimant is excluded",
                    paths.join(" and ")
                ),
            ));
        }
    }
    objects.retain(|o| !duplicates.contains_key(&o.uri));
}

/// Read one discovered file into its objects: one for most kinds, one per member for a
/// collection file. A failure of the whole file is the `Err`; a failure of one member is a
/// diagnostic beside the members that were read.
fn read_objects(
    repo: &Repository,
    schema: &KindSchema,
    file: &DiscoveredFile,
) -> std::result::Result<(Vec<Object>, Vec<Diagnostic>), Diagnostic> {
    let rel = file.rel_path.as_str();
    let d = |code, msg: String| Diagnostic::error(code, Some(rel.to_string()), msg);
    let Some(mut spec) = schema.kind(&file.kind) else {
        return Err(d(
            "unknown_kind",
            format!(
                "class '{}' declares kind '{}', which this executable does not read",
                file.class, file.kind
            ),
        ));
    };
    let mut kind = file.kind.as_str();
    let abs = repo.root().join(rel);
    let content = read_text(&abs).map_err(|(code, msg)| d(code, msg))?;
    let bytes = content.len() as u64;
    let directory = match rel.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => ".".to_string(),
    };
    let provenance = |member: Option<String>| Provenance {
        path: rel.to_string(),
        directory: directory.clone(),
        source_class: file.class.clone(),
        section: repo.section_of(rel).map(str::to_string),
        bytes,
        member,
    };

    let (metadata, body, media_type): (Map<String, Value>, String, &'static str) = match spec.format
    {
        Format::Text => (Map::new(), content.clone(), "text/plain"),
        Format::Yaml => {
            let map = yaml::parse_mapping(&content).map_err(|e| d("malformed_yaml", e))?;
            (map, content.clone(), "application/yaml")
        }
        Format::Markdown => {
            let split = frontmatter::split(&content)
                .map_err(|e| d("malformed_front_matter", e.to_string()))?;
            let map = match split.front {
                Some(front) => {
                    frontmatter::parse(front).map_err(|e| d("malformed_front_matter", e))?
                }
                None => Map::new(),
            };
            if split.front.is_none() {
                if let Some(fallback) = spec.without_front_matter.as_deref() {
                    // Validated at schema load: the fallback exists and accepts the file.
                    if let Some((name, f)) = schema.kinds().find(|(k, _)| k.as_str() == fallback) {
                        spec = f;
                        kind = name.as_str();
                    }
                }
            } else if let Some(declared) = map.get("kind").and_then(Value::as_str) {
                // A file that declares a kind the schema lets files declare is that kind,
                // whatever class found it.
                if declared != kind && schema.is_declared_kind(declared) {
                    if let Some((name, f)) = schema.kinds().find(|(k, _)| k.as_str() == declared) {
                        spec = f;
                        kind = name.as_str();
                    }
                }
            }
            if split.front.is_none() && spec.front_matter == FrontMatterRule::Required {
                return Err(d(
                    "missing_front_matter",
                    "no front matter, and the kind requires it".into(),
                ));
            }
            (map, split.body.to_string(), "text/markdown")
        }
    };

    if let Some(sv) = &spec.schema_version {
        check_version(sv, &metadata).map_err(|(code, msg)| d(code, msg))?;
    }

    // a collection file: one object per member of the named list
    if let Some(list) = &spec.members {
        let Some(items) = metadata.get(list).and_then(Value::as_array) else {
            return Err(d(
                "missing_field",
                format!("no list '{list}' holding the members"),
            ));
        };
        let mut objects = Vec::new();
        let mut diagnostics = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let member = format!("{list}.{i}");
            let Value::Object(item) = item else {
                diagnostics.push(d(
                    "malformed_yaml",
                    format!("member {member} is not a mapping"),
                ));
                continue;
            };
            match object_of(
                spec,
                kind,
                item,
                "",
                &member,
                provenance(Some(member.clone())),
            ) {
                Ok(o) => objects.push(o),
                Err(msg) => diagnostics.push(d("missing_field", format!("member {member}: {msg}"))),
            }
        }
        return Ok((objects, diagnostics));
    }

    if let Some(allow) = schema.allow_for(spec) {
        let unknown = allow.unknown(yaml::key_paths(&metadata).iter().map(String::as_str));
        if !unknown.is_empty() {
            return Err(d(
                "unknown_key",
                format!(
                    "key(s) not in share/allow/{}.txt: {}",
                    allow.name(),
                    unknown.join(", ")
                ),
            ));
        }
    }
    if let Some(declared) = metadata.get("kind").and_then(Value::as_str) {
        if declared != kind {
            return Err(d(
                "kind_mismatch",
                format!(
                    "declares kind '{declared}' but was discovered by class '{}' as '{kind}'",
                    file.class
                ),
            ));
        }
    }
    let object = object_of(spec, kind, &metadata, &body, rel, provenance(None))
        .map_err(|msg| d("missing_field", msg))?;
    Ok((
        vec![Object {
            content,
            media_type,
            ..object
        }],
        Vec::new(),
    ))
}

fn check_version(
    sv: &crate::metadata::SchemaVersion,
    metadata: &Map<String, Value>,
) -> std::result::Result<(), (&'static str, String)> {
    match metadata.get(&sv.field) {
        Some(v) if sv.supported.contains(v) => Ok(()),
        Some(v) => Err((
            "unsupported_version",
            format!(
                "{} {v} is not among the supported {}",
                sv.field,
                serde_json::to_string(&sv.supported).unwrap_or_default()
            ),
        )),
        None => Err(("missing_field", format!("no '{}'", sv.field))),
    }
}

/// Build the object of a metadata mapping: identity, title, description, URI. Content and
/// media type for a whole file are filled by the caller; a member carries itself as JSON.
fn object_of(
    spec: &KindSpec,
    kind: &str,
    metadata: &Map<String, Value>,
    body: &str,
    fallback_identity: &str,
    provenance: Provenance,
) -> std::result::Result<Object, String> {
    let identity = identity_of(spec, metadata, fallback_identity)?;
    let title = title_of(spec, metadata, body);
    let description = spec
        .description
        .as_deref()
        .and_then(|f| metadata.get(f))
        .and_then(yaml::scalar_string);
    let is_member = provenance.member.is_some();
    let text = if is_member {
        serde_json::to_string_pretty(&Value::Object(metadata.clone())).unwrap_or_default()
    } else {
        String::new()
    };
    Ok(Object {
        uri: uri_for(kind, &identity),
        kind: kind.to_string(),
        identity,
        title,
        description,
        metadata: Value::Object(metadata.clone()),
        body: if is_member {
            text.clone()
        } else {
            body.to_string()
        },
        content: text,
        media_type: if is_member {
            "application/json"
        } else {
            "text/plain"
        },
        provenance,
    })
}

fn identity_of(
    spec: &KindSpec,
    metadata: &Map<String, Value>,
    rel: &str,
) -> std::result::Result<String, String> {
    if spec.identity.is_empty() {
        return Ok(rel.to_string());
    }
    let mut parts = Vec::with_capacity(spec.identity.len());
    for field in &spec.identity {
        let value = metadata
            .get(field)
            .and_then(yaml::scalar_string)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("identity field '{field}' is absent or empty"))?;
        if value.contains('/') || value.contains('@') || value.contains(char::is_whitespace) {
            return Err(format!(
                "identity field '{field}' carries '/', '@' or whitespace: '{value}'"
            ));
        }
        parts.push(value);
    }
    Ok(parts.join("@"))
}

fn title_of(spec: &KindSpec, metadata: &Map<String, Value>, body: &str) -> Option<String> {
    match spec.title.as_deref()? {
        "@heading" => body
            .lines()
            .find_map(|l| l.strip_prefix("# "))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty()),
        field => metadata.get(field).and_then(yaml::scalar_string),
    }
}

/// Read a file as UTF-8 text, refusing symlinks, oversized files and invalid encodings.
fn read_text(abs: &Path) -> std::result::Result<String, (&'static str, String)> {
    let meta = std::fs::symlink_metadata(abs).map_err(|e| ("unreadable", e.to_string()))?;
    if meta.file_type().is_symlink() {
        return Err((
            "symlink",
            "a symlink is not read; sources are regular files".into(),
        ));
    }
    if !meta.is_file() {
        return Err(("not_a_file", "not a regular file".into()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err((
            "oversized",
            format!("{} bytes, over the {MAX_FILE_BYTES} byte limit", meta.len()),
        ));
    }
    let bytes = std::fs::read(abs).map_err(|e| ("unreadable", e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| ("invalid_utf8", e.to_string()))
}
