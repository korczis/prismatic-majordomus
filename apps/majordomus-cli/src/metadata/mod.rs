//! Metadata: the kind schema (`kinds.yaml`) and one JSON Schema per kind, both read at run
//! time from the tool distribution's share directory and, when a repository adds its own,
//! from `.ai/repo/knowledge/`; and the YAML helpers shared by every reader. The contract
//! is documented in `docs/CAPABILITIES.md` and in `share/kinds.yaml` itself.

pub mod frontmatter;
pub mod yaml;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::error::{Error, Result};

/// The one schema version this executable reads.
pub const KINDS_SCHEMA: &str = "majordomus-kinds/v1";

/// The file, under the manifest's `knowledge` section, in which a repository adds kinds.
pub const REPO_KINDS_FILE: &str = "kinds.yaml";

/// The directory, under the manifest's `knowledge` section, in which a repository adds
/// schemas.
pub const REPO_SCHEMAS_DIR: &str = "schemas";

/// How a file of a kind is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Markdown,
    Yaml,
    /// Read as UTF-8 text with no metadata: source files, test cases.
    Text,
}

/// Whether a Markdown kind must, may, or does not carry front matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FrontMatterRule {
    Required,
    Optional,
    #[default]
    None,
}

/// A field whose value must be one of the listed ones: an integer (`version: 1`) or a
/// string (`schema: context/v1`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaVersion {
    pub field: String,
    pub supported: Vec<serde_json::Value>,
}

/// One entry of `kinds.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindSpec {
    pub format: Format,
    #[serde(default)]
    pub front_matter: FrontMatterRule,
    /// For a markdown kind with optional or required front matter: the kind a file that
    /// carries none is read as instead (the rules tree's own README is a document).
    #[serde(default)]
    pub without_front_matter: Option<String>,
    /// The JSON Schema the metadata must satisfy, by name (`rule` for
    /// `schemas/rule.schema.json`); absent means no contract.
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub identity: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub schema_version: Option<SchemaVersion>,
    /// For a yaml kind: the file's list that holds the objects, one object per item
    /// (`claims` for `docs/CLAIMS.yaml`). Identity, title and description are then read
    /// from each item; `schema_version` still applies to the file.
    #[serde(default)]
    pub members: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KindsFile {
    schema: String,
    /// Markdown kinds a file may declare through its front matter `kind:` whatever class
    /// discovered it: a context document inside the rules tree is a context document.
    #[serde(default)]
    declared: Vec<String>,
    kinds: BTreeMap<String, KindSpec>,
}

/// The kind schema with every JSON Schema it names compiled.
#[derive(Debug)]
pub struct KindSchema {
    kinds: BTreeMap<String, KindSpec>,
    declared: Vec<String>,
    schemas: SchemaSet,
    sources: Vec<String>,
}

/// One kinds document and where it came from.
struct Source {
    path: String,
    text: String,
}

fn rel_or_abs(path: &std::path::Path, root: &std::path::Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

/// One JSON Schema, compiled, with where it came from.
pub struct Schema {
    pub name: String,
    /// Directory the file was read from, repository-relative when inside the repository.
    pub source: String,
    pub json: Value,
    validator: jsonschema::Validator,
}

impl std::fmt::Debug for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schema")
            .field("name", &self.name)
            .field("source", &self.source)
            .finish()
    }
}

/// One failed constraint of a validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Dotted key path of the offending value; empty for the document itself.
    pub path: String,
    pub message: String,
    /// Set when the violation is a key the schema does not allow.
    pub unknown_keys: Vec<String>,
}

impl Schema {
    fn compile(name: &str, source: &str, json: Value) -> Result<Self> {
        let validator = jsonschema::validator_for(&json).map_err(|e| Error::KindSchema {
            reason: format!(
                "{source}/{name}{}: not a valid JSON Schema: {e}",
                crate::share::SCHEMA_SUFFIX
            ),
        })?;
        Ok(Schema {
            name: name.to_string(),
            source: source.to_string(),
            json,
            validator,
        })
    }

    /// Every violation of `instance`, in document order; empty when it conforms.
    pub fn validate(&self, instance: &Value) -> Vec<Violation> {
        self.validator
            .iter_errors(instance)
            .map(|e| {
                let path = e
                    .instance_path()
                    .as_str()
                    .trim_start_matches('/')
                    .replace('/', ".");
                let unknown_keys = match e.kind() {
                    jsonschema::error::ValidationErrorKind::AdditionalProperties { unexpected } => {
                        unexpected
                            .iter()
                            .map(|k| {
                                if path.is_empty() {
                                    k.clone()
                                } else {
                                    format!("{path}.{k}")
                                }
                            })
                            .collect()
                    }
                    _ => Vec::new(),
                };
                Violation {
                    path,
                    message: e.to_string(),
                    unknown_keys,
                }
            })
            .collect()
    }
}

/// Schemas gathered from the distribution and the repository, each name once.
#[derive(Debug, Default)]
pub struct SchemaSet {
    schemas: BTreeMap<String, Schema>,
}

impl SchemaSet {
    /// Add every `(name, json)` read from one directory; a name already present is an
    /// error naming both directories.
    pub fn extend(&mut self, schemas: Vec<(String, Value)>, from: &str) -> Result<()> {
        for (name, json) in schemas {
            if let Some(first) = self.schemas.get(&name) {
                return Err(Error::KindSchema {
                    reason: format!("schema '{name}' exists in both {} and {from}; a repository adds schemas, it does not redefine them", first.source),
                });
            }
            self.schemas
                .insert(name.clone(), Schema::compile(&name, from, json)?);
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Schema> {
        self.schemas.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

impl KindSchema {
    /// Load the distribution's kinds and schemas, then the repository's additions. A kind
    /// or a schema declared by both is an error naming both files; nothing overrides.
    pub fn load(share: &crate::share::Share, repo: &crate::repository::Repository) -> Result<Self> {
        let dist_path = share.kinds_path();
        let dist_text =
            std::fs::read_to_string(&dist_path).map_err(|e| Error::io(&dist_path, e))?;
        let mut schemas = SchemaSet::default();
        let dist_schemas = share.schemas_dir();
        schemas.extend(
            crate::share::read_schema_dir(&dist_schemas)?,
            &rel_or_abs(&dist_schemas, repo.root()),
        )?;
        let mut sources = vec![Source {
            path: rel_or_abs(&dist_path, repo.root()),
            text: dist_text,
        }];
        if let Some(knowledge) = repo.section_path("knowledge") {
            let dir = repo.root().join(&knowledge);
            let repo_kinds = dir.join(REPO_KINDS_FILE);
            if repo_kinds.is_file() {
                let text =
                    std::fs::read_to_string(&repo_kinds).map_err(|e| Error::io(&repo_kinds, e))?;
                sources.push(Source {
                    path: format!("{knowledge}/{REPO_KINDS_FILE}"),
                    text,
                });
            }
            let repo_schemas = dir.join(REPO_SCHEMAS_DIR);
            schemas.extend(
                crate::share::read_schema_dir(&repo_schemas)?,
                &format!("{knowledge}/{REPO_SCHEMAS_DIR}"),
            )?;
        }
        Self::parse_all(&sources, schemas)
    }

    /// Parse one kinds document with the given schemas (tests, and the distribution alone).
    pub fn parse(text: &str, schemas: SchemaSet) -> Result<Self> {
        Self::parse_all(
            &[Source {
                path: "kinds.yaml".into(),
                text: text.to_string(),
            }],
            schemas,
        )
    }

    fn parse_all(sources: &[Source], schemas: SchemaSet) -> Result<Self> {
        let mut kinds: BTreeMap<String, KindSpec> = BTreeMap::new();
        let mut owner: BTreeMap<String, String> = BTreeMap::new();
        let mut declared: Vec<String> = Vec::new();
        let mut loaded = Vec::new();
        for source in sources {
            let file: KindsFile =
                yaml::parse_into(&source.text).map_err(|reason| Error::KindSchema {
                    reason: format!("{}: {reason}", source.path),
                })?;
            if file.schema != KINDS_SCHEMA {
                return Err(Error::KindSchema {
                    reason: format!(
                        "{}: schema '{}' is not {KINDS_SCHEMA}",
                        source.path, file.schema
                    ),
                });
            }
            for (name, spec) in file.kinds {
                if let Some(first) = owner.get(&name) {
                    return Err(Error::KindSchema {
                        reason: format!("kind '{name}' is declared by both {first} and {}; a repository adds kinds, it does not redefine them", source.path),
                    });
                }
                owner.insert(name.clone(), source.path.clone());
                kinds.insert(name, spec);
            }
            for d in file.declared {
                if !declared.contains(&d) {
                    declared.push(d);
                }
            }
            loaded.push(source.path.clone());
        }
        for d in &declared {
            match kinds.get(d) {
                Some(k)
                    if k.format == Format::Markdown
                        && k.front_matter == FrontMatterRule::Required => {}
                _ => {
                    return Err(Error::KindSchema {
                        reason: format!(
                            "declared kind '{d}' is not a markdown kind that requires front matter"
                        ),
                    })
                }
            }
        }
        for (name, spec) in &kinds {
            if spec.format != Format::Markdown && spec.front_matter != FrontMatterRule::None {
                return Err(Error::KindSchema {
                    reason: format!("kind '{name}': only a markdown kind has a front matter rule"),
                });
            }
            if spec.members.is_some() && spec.format != Format::Yaml {
                return Err(Error::KindSchema {
                    reason: format!("kind '{name}': only a yaml kind has members"),
                });
            }
            if spec.format == Format::Text
                && (spec.schema.is_some() || !spec.identity.is_empty() || spec.members.is_some())
            {
                return Err(Error::KindSchema {
                    reason: format!("kind '{name}': a text kind has no metadata, so no allow-list, identity fields or members"),
                });
            }
            if let Some(fallback) = &spec.without_front_matter {
                match kinds.get(fallback) {
                    Some(f) if f.format == Format::Markdown && f.front_matter != FrontMatterRule::Required => {}
                    _ => {
                        return Err(Error::KindSchema {
                            reason: format!("kind '{name}': without_front_matter names '{fallback}', which is not a markdown kind that accepts a file without front matter"),
                        })
                    }
                }
            }
            if let Some(schema) = &spec.schema {
                if !schemas.schemas.contains_key(schema) {
                    return Err(Error::KindSchema {
                        reason: format!("kind '{name}' names schema '{schema}', and no schemas/{schema}.schema.json exists in the distribution or the repository"),
                    });
                }
            }
        }
        Ok(KindSchema {
            kinds,
            declared,
            schemas,
            sources: loaded,
        })
    }

    /// Every schema, by name, distribution first then repository, each name once.
    pub fn schemas(&self) -> impl Iterator<Item = (&String, &Schema)> {
        self.schemas.schemas.iter()
    }

    /// The kinds files this schema was read from, in order, repository-relative when inside
    /// the repository.
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    pub fn kind(&self, name: &str) -> Option<&KindSpec> {
        self.kinds.get(name)
    }

    /// Is `name` a kind a markdown file may declare for itself?
    pub fn is_declared_kind(&self, name: &str) -> bool {
        self.declared.iter().any(|d| d == name)
    }

    pub fn kinds(&self) -> impl Iterator<Item = (&String, &KindSpec)> {
        self.kinds.iter()
    }

    /// The schema a kind names, if it names one.
    pub fn schema_for(&self, spec: &KindSpec) -> Option<&Schema> {
        spec.schema.as_deref().and_then(|n| self.schemas.get(n))
    }

    /// A schema by name, from either source.
    pub fn schema(&self, name: &str) -> Option<&Schema> {
        self.schemas.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The distribution beside this crate: `../../share`.
    fn dist() -> (String, SchemaSet) {
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let text = std::fs::read_to_string(share.join("kinds.yaml")).unwrap();
        let mut schemas = SchemaSet::default();
        schemas
            .extend(
                crate::share::read_schema_dir(&share.join("schemas")).unwrap(),
                "share/schemas",
            )
            .unwrap();
        (text, schemas)
    }

    #[test]
    fn the_distribution_schema_parses_and_names_known_schemas() {
        let (text, schemas) = dist();
        let schema = KindSchema::parse(&text, schemas).expect("distribution schema");
        let rule = schema.kind("rule").expect("rule kind");
        assert_eq!(rule.format, Format::Markdown);
        assert_eq!(rule.identity, vec!["id", "version"]);
        assert_eq!(
            schema.schema_for(rule).map(|s| s.name.as_str()),
            Some("rule")
        );
        assert!(schema.kind("document").is_some());
        assert!(schema.is_declared_kind("context") && !schema.is_declared_kind("rule"));
        assert_eq!(
            schema.kind("claim").unwrap().members.as_deref(),
            Some("claims")
        );
        assert_eq!(schema.kind("test").unwrap().format, Format::Text);
        assert_eq!(
            schema.kind("rule").unwrap().without_front_matter.as_deref(),
            Some("document")
        );
        assert!(
            schema.schema("manifest").is_some(),
            "schemas that no kind names are still loaded"
        );
    }

    #[test]
    fn a_schema_validates_and_names_unknown_keys_and_wrong_types() {
        let (_, schemas) = dist();
        let rule = schemas.get("rule").unwrap();
        let ok: Value = serde_json::json!({ "id": "p.x", "version": 1, "kind": "rule", "title": "T", "description": "D", "statement": "S", "status": "active", "class": "advisory", "depends_on": [], "tags": ["a"] });
        assert!(rule.validate(&ok).is_empty());
        let mut bad = ok.clone();
        bad["owner"] = serde_json::json!("me");
        bad["x-majordomus"] = serde_json::json!({ "colour": "red" });
        let v = rule.validate(&bad);
        let unknown: Vec<String> = v.iter().flat_map(|v| v.unknown_keys.clone()).collect();
        assert!(
            unknown.contains(&"owner".to_string())
                && unknown.contains(&"x-majordomus.colour".to_string()),
            "{v:?}"
        );
        let mut typed = ok.clone();
        typed["version"] = serde_json::json!("one");
        typed["class"] = serde_json::json!("fatal");
        let v = typed_violations(rule, &typed);
        assert!(
            v.iter().any(|x| x.path == "version") && v.iter().any(|x| x.path == "class"),
            "{v:?}"
        );
        let mut redefine = SchemaSet::default();
        redefine
            .extend(
                vec![("rule".into(), serde_json::json!({ "type": "object" }))],
                "a",
            )
            .unwrap();
        let err = redefine
            .extend(
                vec![("rule".into(), serde_json::json!({ "type": "object" }))],
                "b",
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("schema 'rule' exists in both a and b"),
            "{err}"
        );
    }

    fn typed_violations(schema: &Schema, v: &Value) -> Vec<Violation> {
        schema
            .validate(v)
            .into_iter()
            .filter(|x| x.unknown_keys.is_empty())
            .collect()
    }

    #[test]
    fn schema_refusals_name_the_reason() {
        let (text, _) = dist();
        let bad = |t: &str| {
            KindSchema::parse(t, dist().1)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
        };
        assert!(
            bad(&text.replace("majordomus-kinds/v1", "majordomus-kinds/v9"))
                .contains("is not majordomus-kinds/v1")
        );
        assert!(bad(&text.replace(
            "    identity: [id, version]",
            "    identity: [id, version]\n    colour: red"
        ))
        .contains("colour"));
        assert!(bad(&text.replace(
            "without_front_matter: document",
            "without_front_matter: prompt"
        ))
        .contains("without_front_matter"));
        assert!(
            bad(&text.replace("declared: [context]", "declared: [profile]"))
                .contains("declared kind 'profile'")
        );
        assert!(bad(&text.replace(
            "    format: text\n    identity: []\n",
            "    format: text\n    identity: [id]\n"
        ))
        .contains("text kind"));
        assert!(
            bad(&text.replace("    schema: rule\n", "    schema: nothing\n"))
                .contains("schema 'nothing'")
        );
        let mut invalid = SchemaSet::default();
        let err = invalid
            .extend(
                vec![("x".into(), serde_json::json!({ "type": "nonsense" }))],
                "d",
            )
            .unwrap_err();
        assert!(err.to_string().contains("not a valid JSON Schema"), "{err}");
    }

    #[test]
    fn a_repository_adds_kinds_and_may_not_redefine_one() {
        let (text, schemas) = dist();
        let extra = "schema: majordomus-kinds/v1\nkinds:\n  note:\n    format: markdown\n    front_matter: required\n    identity: [id]\n    title: title\n";
        let sources = [
            Source {
                path: "share/kinds.yaml".into(),
                text: text.clone(),
            },
            Source {
                path: ".ai/repo/knowledge/kinds.yaml".into(),
                text: extra.into(),
            },
        ];
        let schema = KindSchema::parse_all(&sources, schemas).unwrap();
        assert!(schema.kind("note").is_some());
        assert_eq!(
            schema.sources(),
            ["share/kinds.yaml", ".ai/repo/knowledge/kinds.yaml"]
        );
        let redefine = "schema: majordomus-kinds/v1\nkinds:\n  rule:\n    format: yaml\n";
        let sources = [
            Source {
                path: "share/kinds.yaml".into(),
                text,
            },
            Source {
                path: ".ai/repo/knowledge/kinds.yaml".into(),
                text: redefine.into(),
            },
        ];
        let err = KindSchema::parse_all(&sources, dist().1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("kind 'rule' is declared by both share/kinds.yaml and .ai/repo/knowledge/kinds.yaml"), "{err}");
    }
}
