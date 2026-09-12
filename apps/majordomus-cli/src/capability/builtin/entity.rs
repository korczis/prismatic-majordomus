//! The `entity` module: every object of the layer as an addressable, cross-linked node.
//!
//! `objects.get` answers what a file *is*. This module answers the three further questions
//! a governance object is actually read for, and answers them for every kind at once
//! because none of them is asked of a kind by name:
//!
//! - **Where does it live?** Its route, derived from its kind and identity by
//!   [`crate::entity::slug`], the same function the Cockpit resolves a request with.
//! - **What is it joined to?** Its outgoing references and — the half nothing declared —
//!   the references that resolve to it, both read from the one relation table the graph
//!   and the dangling-reference check already share.
//! - **What can be said about its enforcement?** Which of the executable artefacts it
//!   names are in the tree, stated as a state a page may not round up, and a pointer to the
//!   capability that can prove more where the registry holds one.
//!
//! Nothing here enumerates kinds, entities or routes. A file added under `.ai/` becomes an
//! object, an object has a route, and this capability answers for it — which is the whole
//! of `project.entities-are-routable`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::entity::{self, Collision, Edge, Surface};
use crate::model::Object;
use crate::{capability, module};

use super::{get, mcp, ObjectSummary};

// ---------------------------------------------------------------- input

/// Which entity: by URI, or by the address it is served at.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityInput {
    /// `majordomus://<kind>/<identity>`. Either this, or `kind` and `slug` together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// The kind, when addressing by route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The last segment of the entity's route, when addressing by route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
}

impl BenchmarkCases for EntityInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .objects
            .first()
            .map(|o| {
                vec![NamedCase::new(
                    "first-object",
                    EntityInput {
                        uri: Some(o.uri.clone()),
                        ..Default::default()
                    },
                )]
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------- evidence

/// What this repository can say about the executable artefacts an object names, and only
/// that. The word is chosen so that no reader can mistake it for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "EntityEvidenceState")]
pub enum EvidenceState {
    /// The object names no executable artefact. Most kinds name none, and that is not a
    /// defect: a prompt is not enforced by anything.
    Unclaimed,
    /// It names artefacts and at least one of them is not in the tree. A claim that points
    /// at nothing is worse than no claim, because it reads as enforcement.
    Dangling,
    /// Every artefact it names is in the tree. This says the claim resolves — never that it
    /// passed. Whether it ever ran is a different question, and `proof` is where it is
    /// asked.
    Resolved,
}

/// One executable artefact an object names, and whether the tree holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimedArtifact {
    /// The front matter key it was named under (`x-majordomus.tests`, `test`, ...).
    pub field: String,
    /// The repository-relative path as named.
    pub path: String,
    /// Whether the tree holds it.
    pub present: bool,
}

/// Where a fuller answer about this entity's enforcement lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EntityProofRef")]
pub struct ProofRef {
    /// The capability that answers it.
    pub capability: String,
    /// Its title.
    pub title: String,
    /// How to ask it here.
    pub address: String,
}

/// What can be said about an object's enforcement without running anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EntityEvidence")]
pub struct Evidence {
    /// The state, which is never rounded up.
    pub state: EvidenceState,
    /// That state in one sentence, for a reader.
    pub meaning: String,
    /// Every artefact named, with whether the tree holds it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ClaimedArtifact>,
    /// The capability that can say more, when this kind has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<ProofRef>,
}

/// The front matter keys that name an executable artefact. Not a list of kinds: a key, on
/// whatever kind carries it. `x-majordomus` is the extension block the layer's schemas
/// reserve for exactly this, and the others are the keys the claim and rule schemas already
/// use for the same purpose.
const ARTIFACT_FIELDS: &[&str] = &["tests", "test", "implementation", "validator"];

/// Collect the paths an object names as executable artefacts, from its front matter and
/// from its `x-majordomus` extension block.
fn claimed(metadata: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut take = |prefix: &str, source: &Value| {
        for field in ARTIFACT_FIELDS {
            let key = if prefix.is_empty() {
                (*field).to_string()
            } else {
                format!("{prefix}.{field}")
            };
            match source.get(field) {
                Some(Value::Array(a)) => out.extend(
                    a.iter()
                        .filter_map(Value::as_str)
                        .filter(|s| looks_like_a_path(s))
                        .map(|s| (key.clone(), s.to_string())),
                ),
                Some(Value::String(s)) if looks_like_a_path(s) => {
                    out.push((key.clone(), s.clone()))
                }
                _ => {}
            }
        }
    };
    take("", metadata);
    if let Some(x) = metadata.get("x-majordomus") {
        take("x-majordomus", x);
    }
    out
}

/// A value that names a file rather than a symbol. `-` is the layer's written absence, and
/// a validator is named by its function rather than its path; neither is a path to look for.
fn looks_like_a_path(s: &str) -> bool {
    s.contains('/') && s != "-"
}

fn evidence(ctx: &Context, o: &Object) -> Evidence {
    let root = &ctx.index.repository.root;
    let artifacts: Vec<ClaimedArtifact> = claimed(&o.metadata)
        .into_iter()
        .map(|(field, path)| ClaimedArtifact {
            present: std::path::Path::new(root).join(&path).exists(),
            field,
            path,
        })
        .collect();
    // one formula, no table: a kind whose module answers `<kind>s.show` can prove more
    let proof = ctx
        .registry
        .get(&format!("{}s.show", o.kind))
        .map(|c| ProofRef {
            capability: c.id.to_string(),
            title: c.title.clone(),
            address: match &c.exposure.http {
                Some(h) => format!("{} {}", h.method.as_str(), h.path),
                None => c.id.to_string(),
            },
        });
    let missing = artifacts.iter().filter(|a| !a.present).count();
    let (state, meaning) = if artifacts.is_empty() {
        (
            EvidenceState::Unclaimed,
            "This object names no executable artefact, so nothing here claims it is enforced."
                .to_string(),
        )
    } else if missing > 0 {
        (
            EvidenceState::Dangling,
            format!(
                "{missing} of the {} artefact(s) this object names are not in the tree: it reads as enforced and is not.",
                artifacts.len()
            ),
        )
    } else {
        (
            EvidenceState::Resolved,
            format!(
                "Every one of the {} artefact(s) this object names is in the tree. That they resolve is not that they ran.",
                artifacts.len()
            ),
        )
    };
    Evidence {
        state,
        meaning,
        artifacts,
        proof,
    }
}

// ---------------------------------------------------------------- the view

/// One object of the layer as an addressable node: what it is, where it is served, what it
/// is joined to, and what can be said about its enforcement.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityView {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
    /// The capability id, `<kind>.<identity>`.
    pub id: String,
    /// The kind.
    pub kind: String,
    /// The identity within the kind.
    pub identity: String,
    /// The last segment of its route, derived from the identity.
    pub slug: String,
    /// Its route in the Cockpit.
    pub route: String,
    /// The route of its kind's index.
    pub kind_route: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The title, when the kind's title rule found one.
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The one-line description, when the kind holds one.
    pub description: Option<String>,
    /// Where it came from.
    pub provenance: crate::model::Provenance,
    /// The parsed front matter or YAML, keys in the file's order.
    pub metadata: Value,
    /// IANA media type of `content`.
    pub media_type: String,
    /// The file as read.
    pub content: String,
    /// The references it declares, and the references that resolve to it.
    pub relations: Vec<Edge>,
    /// Every surface that answers for it.
    pub surfaces: Vec<Surface>,
    /// What can be said about its enforcement without running anything.
    pub evidence: Evidence,
}

fn subject<'a>(ctx: &'a Context, input: &EntityInput) -> Result<&'a Object, CapabilityError> {
    match (&input.uri, &input.kind, &input.slug) {
        (Some(uri), None, None) => ctx.index.get(uri).ok_or_else(|| {
            CapabilityError::NotFound(format!("no object of this layer has the URI {uri}"))
        }),
        (None, Some(kind), Some(slug)) => {
            entity::find(&ctx.index.objects, kind, slug).ok_or_else(|| {
                CapabilityError::NotFound(format!(
                    "no object of kind '{kind}' is served at {}",
                    entity::route(kind, slug)
                ))
            })
        }
        _ => Err(CapabilityError::InvalidInput(
            "name the entity either by 'uri' or by 'kind' and 'slug' together".into(),
        )),
    }
}

fn objects_entity(ctx: &Context, input: EntityInput) -> Result<EntityView, CapabilityError> {
    let o = subject(ctx, &input)?;
    Ok(EntityView {
        uri: o.uri.clone(),
        id: format!("{}.{}", o.kind, o.identity),
        kind: o.kind.clone(),
        identity: o.identity.clone(),
        slug: entity::slug(&o.identity),
        route: entity::object_route(o),
        kind_route: entity::kind_route(&o.kind),
        title: o.title.clone(),
        description: o.description.clone(),
        provenance: o.provenance.clone(),
        metadata: o.metadata.clone(),
        media_type: o.media_type.to_string(),
        content: o.content.clone(),
        relations: entity::edges(&ctx.registry, &ctx.index.objects, o),
        surfaces: entity::surfaces(&ctx.registry, o),
        evidence: evidence(ctx, o),
    })
}

// ---------------------------------------------------------------- kinds

/// One kind of the layer, with its address and how many objects it holds.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KindEntry {
    /// The kind.
    pub kind: String,
    /// How many objects of it the index holds.
    pub count: usize,
    /// Where its index is served.
    pub route: String,
    /// One of its objects, so a reader has somewhere to go.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<ObjectSummary>,
}

/// Every kind of the layer, each with its route, and every route collision there is.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KindList {
    /// How many kinds.
    pub count: usize,
    /// The kinds, in the index's order.
    pub kinds: Vec<KindEntry>,
    /// How many objects have a route. Every object does, unless it is in `collisions`.
    pub routable: usize,
    /// Two objects of one kind whose identities reduce to one route, which is the only way
    /// an object can fail to be addressable. Empty is the healthy answer and the only one
    /// `scripts/ci/entity-check` accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collisions: Vec<Collision>,
}

fn objects_kinds(ctx: &Context, _: super::Empty) -> Result<KindList, CapabilityError> {
    let kinds: Vec<KindEntry> = ctx
        .index
        .kinds()
        .into_iter()
        .map(|(kind, count)| KindEntry {
            route: entity::kind_route(kind),
            example: ctx
                .index
                .objects
                .iter()
                .find(|o| o.kind == kind)
                .map(ObjectSummary::of),
            kind: kind.to_string(),
            count,
        })
        .collect();
    let collisions = entity::collisions(&ctx.index.objects);
    let colliding: usize = collisions.iter().map(|c| c.identities.len()).sum();
    Ok(KindList {
        count: kinds.len(),
        kinds,
        routable: ctx.index.objects.len().saturating_sub(colliding),
        collisions,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "entity",
        title: "Entities",
        description: "Every object of the layer as an addressable, cross-linked node: its route, its outgoing references, the references that resolve to it, the surfaces that answer for it, and what can be said about its enforcement without running anything. Nothing here enumerates kinds, entities or routes — an object of the index has a route because it is an object.",
        stability: Stability::Implemented,
        capabilities: [
            capability! {
                id: "entity.show",
                title: "Read one entity",
                description: "One object of the layer as an addressable node, by URI or by the address it is served at: identity and route, provenance, front matter and content, every reference it declares and every reference that resolves to it, the surfaces that answer for it, and the state of the executable artefacts it names.",
                input: EntityInput,
                output: EntityView,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_entity"),
                    http: get("/api/v1/entity"),
                    cli: Some(CliExposure { path: vec!["entity".into(), "show".into()] }),
                },
                tags: ["objects", "governance"],
                handler: objects_entity,
            },
            capability! {
                id: "entity.kinds",
                title: "Every kind, and whether every object of it is addressable",
                description: "The kinds the layer holds, each with the route of its index and how many objects it carries, and every route collision there is. A collision is the only way an object of the index can fail to have an address of its own; the healthy answer is none.",
                input: super::Empty,
                output: KindList,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_kinds"),
                    http: get("/api/v1/entity/kinds"),
                    cli: Some(CliExposure { path: vec!["entity".into(), "kinds".into()] }),
                },
                tags: ["objects", "governance"],
                handler: objects_kinds,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_a_path_counts_as_a_named_artefact() {
        assert!(looks_like_a_path("scripts/ci/x"));
        assert!(!looks_like_a_path("-"));
        assert!(
            !looks_like_a_path("scope_is_declared"),
            "a validator named by its function is not a file to look for"
        );
    }

    #[test]
    fn artefacts_are_collected_from_the_front_matter_and_the_extension_block() {
        let m = json!({
            "test": "test/cases/1_a.sh",
            "implementation": "-",
            "x-majordomus": { "tests": ["scripts/ci/a", "scripts/ci/b"] },
        });
        assert_eq!(
            claimed(&m),
            vec![
                ("test".to_string(), "test/cases/1_a.sh".to_string()),
                ("x-majordomus.tests".to_string(), "scripts/ci/a".to_string()),
                ("x-majordomus.tests".to_string(), "scripts/ci/b".to_string()),
            ],
            "the written absence is not an artefact, and the extension block is read too"
        );
    }

    #[test]
    fn an_object_that_names_nothing_is_unclaimed_rather_than_enforced() {
        assert!(claimed(&json!({"title": "x"})).is_empty());
        assert!(claimed(&Value::Null).is_empty());
    }
}
