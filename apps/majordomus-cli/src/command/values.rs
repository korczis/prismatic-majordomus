//! The value index: the identifiers this repository's registries own, in one compact
//! file, so that completing an argument costs a file read rather than an index build.
//!
//! Building the capability registry over this repository takes seconds; a tab completion
//! has a hundred milliseconds. The two are reconciled the way every other hot path in
//! this executable is: the expensive answer is computed once, written atomically, keyed
//! by the fingerprints of what it was derived from, and read cheaply until one of them
//! changes.
//!
//! Nothing is enumerated here. Every set comes from the registry that owns it — the
//! capability registry, the kind schema, the Why catalogue, the distribution model, the
//! graph itself — and a registry that grows an entry grows this index on the next build
//! with no edit anywhere.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{Sensitivity, ValueRegistry};

/// The schema of the index file.
pub const SCHEMA: &str = "majordomus/completion-values/v1";

/// Where the index lives, relative to the repository root: under the checkout-local half
/// of the layer, which is ignored by git and never normative. Entering a repository, or
/// completing in one, therefore leaves the working tree exactly as it was.
pub const PATH: &str = ".ai/local/state/command/values.json";

/// One candidate value with the line a shell shows beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The value as it would be typed.
    pub value: String,
    /// What it is, in one line; absent when the registry holds no description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Candidate {
    /// A candidate with no description.
    pub fn bare(value: impl Into<String>) -> Self {
        Candidate {
            value: value.into(),
            description: None,
        }
    }

    /// A candidate with one.
    pub fn described(value: impl Into<String>, description: impl Into<String>) -> Self {
        Candidate {
            value: value.into(),
            description: Some(description.into()),
        }
    }
}

/// The identifiers of every registry, as one document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValueIndex {
    /// [`SCHEMA`].
    #[serde(default)]
    pub schema: String,
    /// The executable that wrote it.
    #[serde(default)]
    pub version: String,
    /// The command graph's structure fingerprint when it was written.
    #[serde(default)]
    pub structure: String,
    /// The capability registry's fingerprint when it was written; empty when the index
    /// was built without one.
    #[serde(default)]
    pub registry: String,
    /// The candidates, by [`ValueRegistry::key`].
    #[serde(default)]
    pub values: BTreeMap<String, Vec<Candidate>>,
}

impl ValueIndex {
    /// The path of the index inside a checkout.
    pub fn path(root: &Path) -> PathBuf {
        root.join(PATH)
    }

    /// Read the index of a checkout. A file that is missing, truncated, half-written or
    /// written by another version is not an error: it yields an empty index, completion
    /// offers what it can without it, and the next explicit build replaces it. A corrupt
    /// cache must never be able to break a shell.
    pub fn read(root: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(Self::path(root)) else {
            return ValueIndex::default();
        };
        match serde_json::from_str::<ValueIndex>(&text) {
            Ok(index) if index.schema == SCHEMA && index.version == crate::VERSION => index,
            _ => ValueIndex::default(),
        }
    }

    /// Is this index the one the given graph would produce values for?
    pub fn is_current(&self, structure: &str) -> bool {
        !self.values.is_empty() && self.structure == structure
    }

    /// The candidates of one registry, or none.
    pub fn candidates(&self, registry: ValueRegistry) -> &[Candidate] {
        self.values
            .get(registry.key())
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Write the index atomically: a temporary file beside the target, then a rename, so
    /// that a reader either sees the previous index or the new one and never half of
    /// either. Two shells writing at once both succeed, and the last rename wins.
    pub fn write(&self, root: &Path) -> std::io::Result<PathBuf> {
        let path = Self::path(root);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        super::atomic_write(&path, serde_json::to_string(self).unwrap_or_default().as_bytes())?;
        Ok(path)
    }
}

/// Build the index from the registries that own the identifiers. Takes the whole context
/// because it is the expensive path by design: it is called once, by an explicit command,
/// and never by completion.
pub fn build(ctx: &crate::capability::Context, graph: &super::CommandGraph) -> ValueIndex {
    let mut values: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();

    values.insert(
        ValueRegistry::Capability.key().to_string(),
        ctx.registry
            .iter()
            .map(|c| Candidate::described(c.id.to_string(), c.title.clone()))
            .collect(),
    );
    values.insert(
        ValueRegistry::Module.key().to_string(),
        ctx.registry
            .modules()
            .map(|m| Candidate::described(m.id.to_string(), m.title.clone()))
            .collect(),
    );
    values.insert(
        ValueRegistry::Command.key().to_string(),
        graph
            .commands
            .iter()
            .map(|c| Candidate::described(c.id.to_string(), c.summary.clone()))
            .collect(),
    );
    values.insert(
        ValueRegistry::ObjectKind.key().to_string(),
        {
            let mut kinds: Vec<String> = ctx
                .index
                .objects
                .iter()
                .map(|o| o.kind.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            kinds.sort();
            kinds.into_iter().map(Candidate::bare).collect()
        },
    );
    values.insert(
        ValueRegistry::Moment.key().to_string(),
        ctx.why
            .all()
            .iter()
            .map(|m| Candidate::described(m.id.clone(), m.title.clone()))
            .collect(),
    );
    values.insert(
        ValueRegistry::Audience.key().to_string(),
        ctx.why
            .audiences()
            .iter()
            .map(|a| Candidate::described(a.id.clone(), a.title.clone()))
            .collect(),
    );
    values.insert(
        ValueRegistry::Area.key().to_string(),
        ctx.why
            .areas()
            .iter()
            .map(|a| Candidate::described(a.id.clone(), a.title.clone()))
            .collect(),
    );
    values.insert(
        ValueRegistry::DistributionTarget.key().to_string(),
        ctx.index
            .distribution
            .as_ref()
            .map(|m| {
                m.targets
                    .iter()
                    .map(|t| Candidate::described(t.id.clone(), t.rust_target.clone()))
                    .collect()
            })
            .unwrap_or_default(),
    );
    for registry in [ValueRegistry::Skill, ValueRegistry::Rule] {
        let kind = match registry {
            ValueRegistry::Skill => "skill",
            _ => "rule",
        };
        values.insert(
            registry.key().to_string(),
            ctx.index
                .objects
                .iter()
                .filter(|o| o.kind == kind)
                .map(|o| {
                    Candidate::described(
                        o.identity.clone(),
                        o.title.clone().unwrap_or_else(|| o.identity.clone()),
                    )
                })
                .collect(),
        );
    }

    for list in values.values_mut() {
        list.sort_by(|a, b| a.value.cmp(&b.value));
        list.dedup_by(|a, b| a.value == b.value);
    }

    ValueIndex {
        schema: SCHEMA.to_string(),
        version: crate::VERSION.to_string(),
        structure: graph.structure.clone(),
        registry: ctx.registry.fingerprint().to_string(),
        values,
    }
}

/// Is this value safe to offer at all? A value of an argument that carries a credential
/// is never completed, never cached and never rendered — the check is here, once, so that
/// no adapter has to remember it.
pub fn may_offer(sensitivity: Sensitivity) -> bool {
    matches!(sensitivity, Sensitivity::Public)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_index_is_empty_and_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let index = ValueIndex::read(dir.path());
        assert!(index.values.is_empty());
        assert!(index.candidates(ValueRegistry::Capability).is_empty());
        assert!(!index.is_current("anything"));
    }

    #[test]
    fn a_corrupt_index_is_discarded_rather_than_breaking_a_shell() {
        let dir = tempfile::tempdir().unwrap();
        let path = ValueIndex::path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for rubbish in ["", "{", "null", "{\"schema\":\"other/v1\"}", "\u{0}\u{1}"] {
            std::fs::write(&path, rubbish).unwrap();
            assert!(ValueIndex::read(dir.path()).values.is_empty(), "{rubbish:?}");
        }
    }

    #[test]
    fn an_index_written_by_another_version_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut index = ValueIndex {
            schema: SCHEMA.into(),
            version: "0.0.0-other".into(),
            structure: "s".into(),
            registry: "r".into(),
            values: BTreeMap::new(),
        };
        index.values.insert(
            ValueRegistry::Capability.key().into(),
            vec![Candidate::bare("x.y")],
        );
        index.write(dir.path()).unwrap();
        assert!(ValueIndex::read(dir.path()).values.is_empty());
    }

    #[test]
    fn writing_and_reading_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut values = BTreeMap::new();
        values.insert(
            ValueRegistry::Moment.key().to_string(),
            vec![Candidate::described("a-moment", "What it is")],
        );
        let index = ValueIndex {
            schema: SCHEMA.into(),
            version: crate::VERSION.into(),
            structure: "structure".into(),
            registry: "registry".into(),
            values,
        };
        index.write(dir.path()).unwrap();
        let back = ValueIndex::read(dir.path());
        assert_eq!(back, index);
        assert!(back.is_current("structure"));
        assert!(!back.is_current("other"));
        assert_eq!(back.candidates(ValueRegistry::Moment).len(), 1);
    }

    #[test]
    fn nothing_but_a_public_value_is_ever_offered() {
        assert!(may_offer(Sensitivity::Public));
        assert!(!may_offer(Sensitivity::Sensitive));
        assert!(!may_offer(Sensitivity::Secret));
    }
}
