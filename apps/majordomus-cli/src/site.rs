//! The site's registry dataset: what GitHub Pages renders about the registry, derived
//! from the registry and the index and nothing else. The site has no content model of its
//! own for this; its templates read `site/data/registry/registry.json` and lay it out.
//!
//! Deterministic: same repository tree and same executable, same bytes. No timestamps, no
//! absolute paths, no git state (the site's `source.json` carries the commit; it is written
//! at build time by the site generator and is not compared by `generate --check`).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::capability::builtin::CapabilitySummary;
use crate::capability::registry::Summary;
use crate::capability::{CapabilityRegistry, Provenance};
use crate::index::Index;
use crate::metadata::KindSchema;
use crate::policy::LoadedPolicy;

/// The dataset's own format version, distinct from the crate's.
pub const SCHEMA: &str = "majordomus-site-registry/v1";

/// The whole dataset.
#[derive(Debug, Clone, Serialize)]
pub struct SiteRegistry {
    /// [`SCHEMA`].
    pub schema: &'static str,
    /// Who wrote it.
    pub generator: Generator,
    /// The capability registry, fingerprinted and counted, with the builtin entries.
    pub registry: RegistryView,
    /// The index of the layer: fingerprint, counts and every object.
    pub index: IndexView,
    /// The kinds the executable reads, with the schema each is validated against.
    pub kinds: Vec<KindView>,
    /// The provider projections the policy declares.
    pub projections: Vec<ProjectionView>,
}

/// The generator's identity.
#[derive(Debug, Clone, Serialize)]
pub struct Generator {
    /// `majordomus-cli`.
    pub id: &'static str,
    /// The crate version.
    pub version: &'static str,
}

/// The capability registry as the site shows it.
#[derive(Debug, Clone, Serialize)]
pub struct RegistryView {
    /// The registry's fingerprint: the index's plus every descriptor.
    pub fingerprint: String,
    /// The counts.
    pub summary: Summary,
    /// The builtin capabilities, by id. The declarative ones are the index's objects, one
    /// resource each; listing them twice would say nothing new.
    pub builtin: Vec<CapabilitySummary>,
    /// The modules, by id, with how many capabilities each composes.
    pub modules: Vec<ModuleView>,
}

/// One module.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleView {
    /// The module id.
    pub id: String,
    /// Capabilities it composes.
    pub capabilities: usize,
}

/// The index as the site shows it.
#[derive(Debug, Clone, Serialize)]
pub struct IndexView {
    /// Hash of every object's path and content.
    pub fingerprint: String,
    /// `ok`, `degraded`, ... as the index reports itself.
    pub state: String,
    /// Layer schema, e.g. `ai-repository/v1`.
    pub layer_schema: String,
    /// Manifest sections, name to repository-relative path.
    pub sections: BTreeMap<String, String>,
    /// How many objects of each kind.
    pub by_kind: BTreeMap<String, usize>,
    /// Diagnostics counted by severity.
    pub diagnostics: BTreeMap<String, usize>,
    /// Every object, by URI.
    pub objects: Vec<ObjectView>,
}

/// One object of the index, without its content.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectView {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
    /// The kind.
    pub kind: String,
    /// The identity within the kind.
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The title, when the kind has one.
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The one-line description, when the kind has one.
    pub description: Option<String>,
    /// Repository-relative source path.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The manifest section, when the path falls under one.
    pub section: Option<String>,
    /// The `sources.yaml` class that discovered it.
    pub source_class: String,
    /// Size in bytes.
    pub bytes: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    /// Declared tags.
    pub tags: Vec<String>,
}

/// One kind.
#[derive(Debug, Clone, Serialize)]
pub struct KindView {
    /// The kind's name.
    pub name: String,
    /// `markdown`, `yaml` or `text`.
    pub format: String,
    /// `required`, `optional` or `none`.
    pub front_matter: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The JSON Schema it is validated against, when it declares one.
    pub schema: Option<String>,
    /// The identity fields, joined with `@`; empty means the path.
    pub identity: Vec<String>,
    /// Objects of this kind in the index.
    pub objects: usize,
}

/// One declared provider projection.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectionView {
    /// The provider.
    pub provider: String,
    /// The target, repository-relative.
    pub target: String,
    /// `file` or `region`.
    pub mode: String,
    /// Loaded by every worker without asking.
    pub always_loaded: bool,
}

/// Build the dataset.
pub fn dataset(
    registry: &CapabilityRegistry,
    index: &Index,
    schema: &KindSchema,
    policy: &LoadedPolicy,
) -> SiteRegistry {
    let mut builtin: Vec<CapabilitySummary> = registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .map(CapabilitySummary::from)
        .collect();
    builtin.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let mut modules: Vec<ModuleView> = registry
        .modules()
        .map(|m| ModuleView {
            id: m.id.to_string(),
            capabilities: registry
                .iter()
                .filter(|c| c.module.as_str() == m.id.as_str())
                .count(),
        })
        .collect();
    modules.sort_by(|a, b| a.id.cmp(&b.id));

    let mut objects: Vec<ObjectView> = index
        .objects
        .iter()
        .map(|o| ObjectView {
            uri: o.uri.clone(),
            kind: o.kind.clone(),
            identity: o.identity.clone(),
            title: o.title.clone(),
            description: o.description.clone(),
            path: o.provenance.path.clone(),
            section: o.provenance.section.clone(),
            source_class: o.provenance.source_class.clone(),
            bytes: o.provenance.bytes,
            tags: o.tags().into_iter().map(str::to_string).collect(),
        })
        .collect();
    objects.sort_by(|a, b| a.uri.cmp(&b.uri));
    let by_kind: BTreeMap<String, usize> = index
        .kinds()
        .into_iter()
        .map(|(k, n)| (k.to_string(), n))
        .collect();
    let mut diagnostics: BTreeMap<String, usize> = BTreeMap::new();
    for d in &index.diagnostics {
        *diagnostics
            .entry(format!("{:?}", d.severity).to_lowercase())
            .or_default() += 1;
    }

    let mut kinds: Vec<KindView> = schema
        .kinds()
        .map(|(name, spec)| KindView {
            name: name.clone(),
            format: format!("{:?}", spec.format).to_lowercase(),
            front_matter: format!("{:?}", spec.front_matter).to_lowercase(),
            schema: spec.schema.clone(),
            identity: spec.identity.clone(),
            objects: by_kind.get(name).copied().unwrap_or(0),
        })
        .collect();
    kinds.sort_by(|a, b| a.name.cmp(&b.name));

    let projections = policy
        .policy
        .projections
        .iter()
        .map(|p| ProjectionView {
            provider: p.provider.clone(),
            target: p.target.clone(),
            mode: format!("{:?}", p.mode).to_lowercase(),
            always_loaded: p.always_loaded,
        })
        .collect();

    SiteRegistry {
        schema: SCHEMA,
        generator: Generator {
            id: "majordomus-cli",
            version: crate::VERSION,
        },
        registry: RegistryView {
            fingerprint: registry.fingerprint().to_string(),
            summary: registry.summary(),
            builtin,
            modules,
        },
        index: IndexView {
            fingerprint: index.fingerprint.clone(),
            state: format!("{:?}", index.state).to_lowercase(),
            layer_schema: index.repository.layer_schema.clone(),
            sections: index.repository.sections.clone(),
            by_kind,
            diagnostics,
            objects,
        },
        kinds,
        projections,
    }
}

/// The dataset as the committed file: pretty JSON, trailing newline.
pub fn render(dataset: &SiteRegistry) -> String {
    let mut s = serde_json::to_string_pretty(dataset).unwrap_or_default();
    s.push('\n');
    s
}
