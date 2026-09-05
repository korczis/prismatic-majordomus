//! Metadata: the embedded kind schema, the key allow-lists, and the YAML helpers shared by
//! every reader. See `schema/README.md` for the contract.

pub mod frontmatter;
pub mod yaml;

use std::collections::BTreeMap;

use regex_lite::Regex;
use serde::Deserialize;

use crate::error::{Error, Result};

/// The embedded kind schema, verbatim.
pub const KINDS_YAML: &str = include_str!("../../schema/kinds.yaml");

/// The one schema version this executable reads.
pub const KINDS_SCHEMA: &str = "majordomus-kinds/v1";

// The key allow-lists, embedded from the repository's `share/allow/` by build.rs so that
// this executable and the shell tool validate the same keys from the same files.
include!(concat!(env!("OUT_DIR"), "/allow_lists.rs"));

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
    #[serde(default)]
    pub allow: Option<String>,
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

/// The kind schema with its allow-lists compiled.
#[derive(Debug)]
pub struct KindSchema {
    kinds: BTreeMap<String, KindSpec>,
    declared: Vec<String>,
    allow: BTreeMap<String, AllowList>,
}

impl KindSchema {
    /// Parse the embedded schema. Failing here is a build defect, not a repository one.
    pub fn embedded() -> Result<Self> {
        Self::parse(KINDS_YAML)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let file: KindsFile =
            yaml::parse_into(text).map_err(|reason| Error::KindSchema { reason })?;
        if file.schema != KINDS_SCHEMA {
            return Err(Error::KindSchema {
                reason: format!("schema '{}' is not {KINDS_SCHEMA}", file.schema),
            });
        }
        let mut allow = BTreeMap::new();
        for d in &file.declared {
            match file.kinds.get(d) {
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
        for (name, spec) in &file.kinds {
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
                && (spec.allow.is_some() || !spec.identity.is_empty() || spec.members.is_some())
            {
                return Err(Error::KindSchema {
                    reason: format!("kind '{name}': a text kind has no metadata, so no allow-list, identity fields or members"),
                });
            }
            if let Some(fallback) = &spec.without_front_matter {
                match file.kinds.get(fallback) {
                    Some(f) if f.format == Format::Markdown && f.front_matter != FrontMatterRule::Required => {}
                    _ => {
                        return Err(Error::KindSchema {
                            reason: format!("kind '{name}': without_front_matter names '{fallback}', which is not a markdown kind that accepts a file without front matter"),
                        })
                    }
                }
            }
            if let Some(list) = &spec.allow {
                if !allow.contains_key(list) {
                    allow.insert(list.clone(), AllowList::embedded(list)?);
                }
            }
        }
        Ok(KindSchema {
            kinds: file.kinds,
            declared: file.declared,
            allow,
        })
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

    /// The allow-list a kind names, if it names one.
    pub fn allow_for(&self, spec: &KindSpec) -> Option<&AllowList> {
        spec.allow.as_deref().and_then(|n| self.allow.get(n))
    }
}

/// A compiled `share/allow/<name>.txt`: one anchored pattern per line, a key path is
/// allowed when any pattern matches it whole.
#[derive(Debug)]
pub struct AllowList {
    name: String,
    patterns: Vec<Regex>,
}

impl AllowList {
    /// Look an embedded allow-list up by name.
    pub fn embedded(name: &str) -> Result<Self> {
        let text = ALLOW_LISTS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, t)| *t)
            .ok_or_else(|| Error::KindSchema {
                reason: format!("allow-list '{name}' is not embedded (share/allow/{name}.txt)"),
            })?;
        Self::parse(name, text)
    }

    pub fn parse(name: &str, text: &str) -> Result<Self> {
        let mut patterns = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let re = Regex::new(line).map_err(|e| Error::KindSchema {
                reason: format!("allow-list '{name}': bad pattern '{line}': {e}"),
            })?;
            patterns.push(re);
        }
        Ok(AllowList {
            name: name.to_string(),
            patterns,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn allows(&self, key_path: &str) -> bool {
        self.patterns.iter().any(|re| re.is_match(key_path))
    }

    /// The key paths of `keys` this list does not allow, in input order.
    pub fn unknown<'a>(&self, keys: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        keys.into_iter()
            .filter(|k| !self.allows(k))
            .map(str::to_string)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_schema_parses_and_names_known_lists() {
        let schema = KindSchema::embedded().expect("embedded schema");
        let rule = schema.kind("rule").expect("rule kind");
        assert_eq!(rule.format, Format::Markdown);
        assert_eq!(rule.identity, vec!["id", "version"]);
        assert!(schema.allow_for(rule).is_some());
        assert!(schema.kind("document").is_some());
        assert!(schema.is_declared_kind("context") && !schema.is_declared_kind("rule"));
        assert_eq!(
            schema.kind("claim").unwrap().members.as_deref(),
            Some("claims")
        );
        assert_eq!(schema.kind("test").unwrap().format, Format::Text);
    }

    #[test]
    fn declared_kinds_members_and_text_kinds_are_validated() {
        let bad = KINDS_YAML.replace("declared: [context]", "declared: [profile]");
        assert!(matches!(
            KindSchema::parse(&bad),
            Err(Error::KindSchema { .. })
        ));
        let bad = KINDS_YAML.replace(
            "    format: text\n    identity: []\n",
            "    format: text\n    identity: [id]\n",
        );
        assert!(matches!(
            KindSchema::parse(&bad),
            Err(Error::KindSchema { .. })
        ));
    }

    #[test]
    fn every_embedded_allow_list_comes_from_share_allow() {
        let names: Vec<&str> = ALLOW_LISTS.iter().map(|(n, _)| *n).collect();
        for expected in [
            "manifest",
            "rule",
            "prompt",
            "profile",
            "policy",
            "milestone",
            "issue",
            "context",
        ] {
            assert!(
                names.contains(&expected),
                "{expected} missing from {names:?}"
            );
        }
        assert!(
            names.windows(2).all(|w| w[0] < w[1]),
            "sorted by name: {names:?}"
        );
        assert!(AllowList::embedded("context")
            .unwrap()
            .allows("composition"));
    }

    #[test]
    fn allow_list_matches_whole_key_paths() {
        let list = AllowList::embedded("rule").unwrap();
        assert!(list.allows("id"));
        assert!(list.allows("depends_on.0"));
        assert!(list.allows("x-majordomus.enforced_by.1"));
        assert!(!list.allows("owner"));
        assert!(!list.allows("identity"));
        assert_eq!(
            list.unknown(["id", "owner", "tags.0", "extra.k"]),
            vec!["owner", "extra.k"]
        );
    }

    #[test]
    fn schema_version_mismatch_is_refused() {
        let bad = KINDS_YAML.replace("majordomus-kinds/v1", "majordomus-kinds/v9");
        assert!(matches!(
            KindSchema::parse(&bad),
            Err(Error::KindSchema { .. })
        ));
    }

    #[test]
    fn unknown_field_in_schema_is_refused() {
        let bad = KINDS_YAML.replace(
            "    identity: [id, version]",
            "    identity: [id, version]\n    colour: red",
        );
        assert!(matches!(
            KindSchema::parse(&bad),
            Err(Error::KindSchema { .. })
        ));
    }

    #[test]
    fn fallback_kind_must_accept_files_without_front_matter() {
        let bad = KINDS_YAML.replace(
            "without_front_matter: document",
            "without_front_matter: prompt",
        );
        assert!(matches!(
            KindSchema::parse(&bad),
            Err(Error::KindSchema { .. })
        ));
        let rule = KindSchema::embedded().unwrap();
        assert_eq!(
            rule.kind("rule").unwrap().without_front_matter.as_deref(),
            Some("document")
        );
    }
}
