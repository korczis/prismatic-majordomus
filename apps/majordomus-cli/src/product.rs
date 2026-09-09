//! The product model: what this repository's product does for a person, as the features
//! under the layer's features section declare it, with every fact a surface may say about
//! a feature derived from the registries that own those facts.
//!
//! Nothing here discovers or parses a file. A feature is an ordinary declarative object of
//! the layer — kind `feature` in `share/kinds.yaml`, discovered by the source class in the
//! repository's `sources.yaml`, validated against its JSON Schema by the time the index
//! holds it. This module projects that validated metadata into typed records, resolves
//! every reference the records make against the registry that owns it, and derives what
//! nobody authored: which interfaces expose a feature, the tools, routes and command-line
//! paths behind it, how many objects of its kinds the layer holds, which operational
//! moments it answers, and what is guaranteed about it.
//!
//! Built once, when a [`crate::capability::Context`] is composed, and shared by every
//! projection: the command line, the HTTP routes, the MCP tools, the derived graph and the
//! site's dataset are readers of this one value.
//!
//! Two directions are kept apart on purpose, exactly as the Why catalogue keeps them.
//! **Authored** is what a file says: the headline, the summary, the order, whether the
//! homepage features it, and the typed references to what the feature is made of.
//! **Derived** is everything else, and none of it may be written into a source file: the
//! schema refuses a `surfaces` or a `route` key, and this module is where those come from.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::capability::{Capability, CapabilityRegistry, Provenance};
use crate::cockpit::nav;
use crate::index::Index;
use crate::model::{Object, Severity};
use crate::web::Topology;
use crate::why::Catalogue;

/// The kind of a product feature.
pub const FEATURE: &str = "feature";
/// The section the features are published under.
pub const ROUTE: &str = "/features/";
/// A feature that is complete and public.
pub const STABLE: &str = "stable";
/// The identities the section's own routes use; a feature called one of these would claim
/// a route the section already owns.
pub const RESERVED: &[&str] = &["matrix", "providers", "index"];

// ---------------------------------------------------------------- the authored record

/// One product feature, as its file declares it plus the facts the file cannot hold: where
/// it came from and where it is published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Feature {
    /// The identity, the slug and the file name.
    pub id: String,
    /// The feature as a heading.
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Two or three words for a card or a matrix row.
    pub short_title: Option<String>,
    /// The promise a visitor reads first.
    pub headline: String,
    /// One line: what it does.
    pub summary: String,
    /// `stable`, `draft` or `deprecated`.
    pub status: String,
    #[serde(default)]
    /// Presentation order, lowest first.
    pub weight: u32,
    #[serde(default)]
    /// Whether the homepage shows it as a chapter.
    pub featured: bool,
    #[serde(default)]
    /// The operational areas of the why catalogue it serves.
    pub areas: Vec<String>,
    #[serde(default)]
    /// The audiences of the why catalogue it is written for.
    pub audiences: Vec<String>,
    #[serde(default)]
    /// Capability modules of the executable it is made of.
    pub modules: Vec<String>,
    #[serde(default)]
    /// Public commands of the shell tool it is made of.
    pub commands: Vec<String>,
    #[serde(default)]
    /// Object kinds of the layer it is made of.
    pub kinds: Vec<String>,
    #[serde(default)]
    /// Rules of the effective set that govern it, by id without the version.
    pub rules: Vec<String>,
    #[serde(default)]
    /// The documents that explain it, by repository-relative path.
    pub docs: Vec<String>,
    #[serde(default)]
    /// The decisions behind it, by declared id.
    pub adrs: Vec<String>,
    #[serde(default)]
    /// Claims that say what is guaranteed here.
    pub claims: Vec<String>,
    #[serde(default)]
    /// Use cases that show it in use.
    pub use_cases: Vec<String>,
    #[serde(default)]
    /// Areas of the Cockpit that show it.
    pub cockpit: Vec<String>,
    #[serde(default)]
    /// Web surfaces of the topology it is offered through.
    pub web: Vec<String>,
    #[serde(default)]
    /// Features explicitly related to this one. The reverse is derived.
    pub related: Vec<String>,
    #[serde(default)]
    /// Free tags.
    pub tags: Vec<String>,

    #[serde(default)]
    /// Derived: `/features/<id>/`. Never authored; the schema refuses a `route` key.
    pub route: String,
    #[serde(default)]
    /// Derived: the repository-relative file the record came from.
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// The Markdown body, without its front matter.
    pub body: String,
}

impl Feature {
    /// The name a narrow column shows.
    pub fn label(&self) -> &str {
        self.short_title.as_deref().unwrap_or(&self.title)
    }
}

// ---------------------------------------------------------------- the derived views

/// The interfaces a feature is exposed through, decided from what its references project
/// and from nothing a file says. Each is a fact with a reason a reader can check: `cli` is
/// true when a module of the feature has a capability with a command-line path or the
/// feature names a shell command; `api` when a module has an HTTP route; `mcp` when a module
/// has an MCP tool or resource, or the feature names a kind, since every object of the layer
/// is an MCP resource; `cockpit` when the feature names a Cockpit area or a module, since
/// every capability has a Cockpit page; `docs` when it names a document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Surfaces {
    /// The command line: the executable's own, or the shell tool's.
    pub cli: bool,
    /// The HTTP routes under `/api/v1/`, and the OpenAPI document over them.
    pub api: bool,
    /// MCP: a tool, or a resource.
    pub mcp: bool,
    /// The Cockpit.
    pub cockpit: bool,
    /// The documentation.
    pub docs: bool,
}

impl Surfaces {
    /// The surface ids that are true, in the order the vocabulary lists them.
    pub fn present(&self) -> Vec<&'static str> {
        SURFACES
            .iter()
            .filter(|(id, _, _)| self.has(id))
            .map(|(id, _, _)| *id)
            .collect()
    }

    /// Is this surface exposed?
    pub fn has(&self, id: &str) -> bool {
        match id {
            "cli" => self.cli,
            "api" => self.api,
            "mcp" => self.mcp,
            "cockpit" => self.cockpit,
            "docs" => self.docs,
            _ => false,
        }
    }
}

/// The vocabulary of surfaces a feature can be exposed through: id, title, and the route
/// on the website where that surface is documented. Five, because the executable has five
/// ways to be reached, and nothing else is a surface.
pub const SURFACES: &[(&str, &str, &str)] = &[
    ("cli", "Command line", "/docs/cli/"),
    ("api", "HTTP API", "/docs/api/"),
    ("mcp", "MCP", "/registry/mcp/"),
    ("cockpit", "Cockpit", "/docs/cockpit/"),
    ("docs", "Documentation", "/docs/"),
];

/// One capability of a module the feature names, with the projections the registry
/// declares for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityRef {
    /// The canonical id.
    pub id: String,
    /// The title.
    pub title: String,
    /// `query`, `command` or `resource`.
    pub kind: String,
    /// Where it stands.
    pub stability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The MCP tool name, when exposed as one.
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The MCP resource URI, when exposed as one.
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// `METHOD /path`, when exposed over HTTP.
    pub route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The words after `majordomus`, when exposed on the command line.
    pub cli: Option<String>,
}

/// One capability module the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleRef {
    /// The module id.
    pub id: String,
    /// The short name.
    pub title: String,
    /// One paragraph.
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where the module stands, when it declared it.
    pub stability: Option<String>,
    /// Repository-relative path of the file its descriptors were composed in.
    pub source_path: String,
    /// The builtin capabilities it composes, in id order.
    pub capabilities: Vec<CapabilityRef>,
}

/// One public command of the shell tool the feature names, as `share/commands.yaml`
/// declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandRef {
    /// The command.
    pub id: String,
    /// One line.
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The lifecycle stage it belongs to.
    pub stage: Option<String>,
    /// Whether it writes nothing.
    pub read_only: bool,
}

/// One object kind the feature names, with how many objects of it the layer holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KindRef {
    /// The kind.
    pub name: String,
    /// Objects of this kind in the index.
    pub objects: usize,
}

/// One rule the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleRef {
    /// The rule id, without the version.
    pub id: String,
    /// The identity the index holds, with the version.
    pub identity: String,
    /// The title.
    pub title: String,
    /// `blocking` or `advisory`.
    pub class: String,
    /// Whether the tool enforces it: the rule carries an `x-majordomus` block.
    pub enforced: bool,
    /// Repository-relative path.
    pub path: String,
}

/// One document the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocRef {
    /// Repository-relative path.
    pub path: String,
    /// The title, when the document has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// One architecture decision the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdrRef {
    /// The declared id.
    pub id: String,
    /// The title.
    pub title: String,
    /// `proposed`, `accepted`, `superseded` or `rejected`.
    pub status: String,
    /// Repository-relative path.
    pub path: String,
}

/// One claim the feature names, with the status the matrix gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimRef {
    /// The claim id.
    pub id: String,
    /// The sentence.
    pub claim: String,
    /// `guaranteed`, `advisory`, `planned` or `rejected`.
    pub status: String,
}

/// One use case the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UseCaseRef {
    /// The use case id.
    pub id: String,
    /// The title.
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The category it is filed under.
    pub category: Option<String>,
}

/// One area of the Cockpit the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CockpitAreaRef {
    /// The area id.
    pub id: String,
    /// The label the Cockpit shows.
    pub title: String,
    /// The route under the running server.
    pub route: String,
}

/// One web surface of the topology the feature names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SurfaceRef {
    /// The surface id.
    pub id: String,
    /// The title.
    pub title: String,
    /// Where it is mounted.
    pub mount: String,
    /// What it is for.
    pub category: String,
}

// `weight` is the ranking a feature declares; the canonical order takes it as the rank and
// ends on the identity, so two features of equal weight keep one order everywhere.
impl crate::order::Ordered for Feature {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id).ranked(i64::from(self.weight))
    }
}

/// One operational moment the feature answers: derived from the moments that name any of
/// the feature's commands, capabilities, claims or rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MomentRef {
    /// The moment id.
    pub id: String,
    /// The title.
    pub title: String,
    /// The first-person line an index shows.
    pub hook: String,
    /// `/why/<id>/`.
    pub route: String,
}

/// How much stands behind a feature, every number derived.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureCounts {
    /// Builtin capabilities of the modules it names.
    pub capabilities: usize,
    /// MCP tools among them.
    pub mcp_tools: usize,
    /// MCP resources among them, plus one per object of the kinds it names.
    pub mcp_resources: usize,
    /// HTTP routes among them.
    pub http_routes: usize,
    /// Command-line paths of the executable among them.
    pub cli_paths: usize,
    /// Public commands of the shell tool it names.
    pub commands: usize,
    /// Objects of the kinds it names.
    pub objects: usize,
    /// Rules it names.
    pub rules: usize,
    /// Rules the tool enforces among them.
    pub enforced_rules: usize,
    /// Documents it names.
    pub docs: usize,
    /// Decisions it names.
    pub adrs: usize,
    /// Claims it names.
    pub claims: usize,
    /// Use cases it names.
    pub use_cases: usize,
    /// Moments it answers.
    pub moments: usize,
}

/// What is guaranteed about a feature, counted from the claims it names: never a word a
/// person wrote about the feature's maturity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureEvidence {
    /// Claims by status: `guaranteed`, `advisory`, `planned`, `rejected`.
    pub claims: BTreeMap<String, usize>,
    /// The modules it names by stability.
    pub modules: BTreeMap<String, usize>,
}

/// One feature, resolved: the record as its file declares it, and everything derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedRefs {
    /// The record as its file declares it, plus its route and source.
    #[serde(flatten)]
    pub feature: Feature,
    /// Derived: the interfaces it is exposed through.
    pub surfaces: Surfaces,
    /// Derived: the modules it names, each with its capabilities and their projections.
    pub module_refs: Vec<ModuleRef>,
    /// Derived: the shell commands it names, as the command registry declares them.
    pub command_refs: Vec<CommandRef>,
    /// Derived: the kinds it names, each with its object count.
    pub kind_refs: Vec<KindRef>,
    /// Derived: the rules it names, each with its class and whether it is enforced.
    pub rule_refs: Vec<RuleRef>,
    /// Derived: the documents it names, with their titles.
    pub doc_refs: Vec<DocRef>,
    /// Derived: the decisions it names, with their status.
    pub adr_refs: Vec<AdrRef>,
    /// Derived: the claims it names, with their status.
    pub claim_refs: Vec<ClaimRef>,
    /// Derived: the use cases it names.
    pub use_case_refs: Vec<UseCaseRef>,
    /// Derived: the Cockpit areas it names, with their routes.
    pub cockpit_refs: Vec<CockpitAreaRef>,
    /// Derived: the web surfaces it names, with their mounts.
    pub web_refs: Vec<SurfaceRef>,
    /// Derived: the moments that name any command, capability, claim or rule of this feature.
    pub moments: Vec<MomentRef>,
    /// Derived: the features that name this one in their `related`.
    pub backlinks: Vec<String>,
    /// Derived: how much stands behind it.
    pub counts: FeatureCounts,
    /// Derived: what is guaranteed.
    pub evidence: FeatureEvidence,
}

// ---------------------------------------------------------------- providers

/// One provider the tool has an adapter for, discovered from the templates the distribution
/// ships and decorated with what this repository does with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProductProvider {
    /// The provider id: the template's file stem.
    pub id: String,
    /// The name a person knows it by.
    pub title: String,
    /// The bootstraps this repository's policy renders through it: target, mode, whether
    /// every worker loads it.
    pub bootstraps: Vec<ProviderBootstrap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The client configuration at the repository root that starts the shared MCP server
    /// for this provider, when the repository carries it.
    pub client_config: Option<String>,
    /// The enforcement entries of the policy wired by this provider's hooks.
    pub hooks: Vec<String>,
    /// The scratch roots this provider creates checkouts of its own under, as declared:
    /// the worktree topology reports a checkout there as a session's scratch checkout and
    /// never moves it (ADR 0024).
    #[serde(default)]
    pub scratch_roots: Vec<String>,
}

/// One bootstrap a provider renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderBootstrap {
    /// The target file, repository-relative.
    pub target: String,
    /// `file` or `region`.
    pub mode: String,
    /// Loaded by every worker without asking.
    pub always_loaded: bool,
}

// ---------------------------------------------------------------- findings

/// One thing wrong with the product model, named where it is, with the nearest candidate
/// when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProductFinding {
    /// `error` or `warning`.
    pub severity: Severity,
    /// `unknown_reference`, `duplicate_identity`, `missing_content`, `uncovered`, ...
    pub code: String,
    /// The repository-relative file the finding is in, or the registry it is about.
    pub path: String,
    /// The record's identity, when one record owns the finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The front-matter key the finding is about, when one key owns it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// What is wrong, in one sentence.
    pub message: String,
    /// The nearest existing name, when the value looks like a typo of one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did_you_mean: Option<String>,
}

// ---------------------------------------------------------------- coverage

/// One thing of the product — a module, a public command, a kind — and the features that
/// name it. Empty means the product page says nothing about it, which is a gap the
/// validation reports and the matrix shows rather than hides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProductCoverage {
    /// The id.
    pub id: String,
    /// The title.
    pub title: String,
    /// The stable features that name it.
    pub features: Vec<String>,
}

// ---------------------------------------------------------------- the model

/// The validated product model and every index derived from it. Immutable: built once from
/// an index and a registry, read many times.
#[derive(Debug, Clone, Default)]
pub struct ProductModel {
    features: Vec<ResolvedRefs>,
    providers: Vec<ProductProvider>,
    findings: Vec<ProductFinding>,
    fingerprint: String,
    by_id: BTreeMap<String, usize>,
    modules: Vec<ProductCoverage>,
    commands: Vec<ProductCoverage>,
    kinds: Vec<ProductCoverage>,
    module_areas: BTreeMap<String, String>,
}

/// What claims a module: the features that name it, each with the areas it serves.
type Claims = BTreeMap<String, Vec<(String, Vec<String>)>>;

/// Resolve one area per module, and name the modules whose claimants disagree.
///
/// The rule is the catalogue's own ranking: among the areas the claiming features serve,
/// the one with the lowest `weight`, ties broken by id. The areas are already ordered by
/// what the operator cares about most, and a second ranking invented here would be a second
/// opinion about a question the layer has already answered.
///
/// Two features that name one module and share no area disagree about what the module is
/// for. The resolution stays deterministic and the disagreement is still returned, so that
/// it is settled in the feature files where the semantics live rather than by a tiebreak in
/// this function.
fn resolve_module_areas(
    claims: &Claims,
    weight: &BTreeMap<&str, u32>,
) -> (BTreeMap<String, String>, Vec<(String, String)>) {
    let mut areas: BTreeMap<String, String> = BTreeMap::new();
    let mut contested: Vec<(String, String)> = Vec::new();

    for (module, claimants) in claims {
        // A set, not a sort and a dedup: unique and ordered is what a BTreeSet is, and the
        // canonical-order gate is right to ask why a comparator appeared here.
        let union: std::collections::BTreeSet<&str> = claimants
            .iter()
            .flat_map(|(_, areas)| areas.iter().map(String::as_str))
            .collect();

        let disjoint = claimants.len() > 1
            && claimants.iter().any(|(_, a)| {
                claimants
                    .iter()
                    .any(|(_, b)| !a.iter().any(|x| b.contains(x)))
            });
        if disjoint {
            let who: Vec<String> = claimants
                .iter()
                .map(|(id, a)| format!("{id} ({})", a.join(", ")))
                .collect();
            contested.push((module.clone(), who.join(" / ")));
        }

        if let Some(best) = union.iter().min_by(|a, b| {
            weight
                .get(*a)
                .unwrap_or(&u32::MAX)
                .cmp(weight.get(*b).unwrap_or(&u32::MAX))
                .then_with(|| a.cmp(b))
        }) {
            areas.insert(module.clone(), (*best).to_string());
        }
    }
    (areas, contested)
}

/// Levenshtein distance, for the nearest-candidate hint on an unresolved reference.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The nearest candidate to `value`, when one is close enough to be a plausible typo.
fn nearest<'a>(value: &str, candidates: impl Iterator<Item = &'a str>) -> Option<String> {
    let limit = (value.chars().count() / 3).max(2);
    candidates
        .map(|c| (distance(value, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, c)| c.to_string())
}

fn meta_str(o: &Object, key: &str) -> Option<String> {
    o.metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn enum_word<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// What the layer holds, read once into the lookups every reference needs.
struct Lookups<'a> {
    commands: BTreeMap<String, &'a Object>,
    rules_by_stem: BTreeMap<String, &'a Object>,
    docs: BTreeMap<String, &'a Object>,
    adrs_by_id: BTreeMap<String, &'a Object>,
    claims: BTreeMap<String, &'a Object>,
    use_cases: BTreeMap<String, &'a Object>,
    kinds: BTreeMap<String, usize>,
    areas: BTreeSet<String>,
    audiences: BTreeSet<String>,
}

impl<'a> Lookups<'a> {
    fn new(index: &'a Index) -> Self {
        let mut l = Lookups {
            commands: BTreeMap::new(),
            rules_by_stem: BTreeMap::new(),
            docs: BTreeMap::new(),
            adrs_by_id: BTreeMap::new(),
            claims: BTreeMap::new(),
            use_cases: BTreeMap::new(),
            kinds: BTreeMap::new(),
            areas: BTreeSet::new(),
            audiences: BTreeSet::new(),
        };
        for o in &index.objects {
            *l.kinds.entry(o.kind.clone()).or_default() += 1;
            match o.kind.as_str() {
                // a public command only: an internal one is dispatched and absent from the
                // help text, and a feature may not present it
                "command" => {
                    if meta_str(o, "visibility").as_deref() == Some("public") {
                        l.commands.insert(o.identity.clone(), o);
                    }
                }
                "rule" => {
                    let stem = o
                        .identity
                        .split_once('@')
                        .map(|(s, _)| s.to_string())
                        .unwrap_or_else(|| o.identity.clone());
                    l.rules_by_stem.entry(stem).or_insert(o);
                }
                "document" => {
                    l.docs.insert(o.provenance.path.clone(), o);
                }
                "adr" => {
                    if let Some(id) = meta_str(o, "id") {
                        l.adrs_by_id.insert(id, o);
                    }
                }
                "claim" => {
                    l.claims.insert(o.identity.clone(), o);
                }
                "use-case" => {
                    l.use_cases.insert(o.identity.clone(), o);
                }
                crate::why::AREA => {
                    l.areas.insert(o.identity.clone());
                }
                crate::why::AUDIENCE => {
                    l.audiences.insert(o.identity.clone());
                }
                _ => {}
            }
        }
        l
    }
}

impl ProductModel {
    /// Build the model from an index, the registry its module references resolve against,
    /// the catalogue whose moments it answers, and the topology whose surfaces it names.
    /// Deterministic: the same objects produce the same value, and no derivation reads a
    /// clock.
    pub fn build(
        index: &Index,
        registry: &CapabilityRegistry,
        why: &Catalogue,
        web: &Topology,
    ) -> Self {
        let mut m = ProductModel::default();
        let mut features: Vec<Feature> = Vec::new();
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for o in index.objects.iter().filter(|o| o.kind == FEATURE) {
            let path = o.provenance.path.clone();
            if let Some(first) = seen.insert(o.identity.clone(), path.clone()) {
                m.findings.push(ProductFinding {
                    severity: Severity::Error,
                    code: "duplicate_identity".into(),
                    path: path.clone(),
                    id: Some(o.identity.clone()),
                    field: Some("id".into()),
                    message: format!(
                        "a second feature claims the identity '{}' (first: {first})",
                        o.identity
                    ),
                    did_you_mean: None,
                });
            }
            let stem = path
                .rsplit('/')
                .next()
                .and_then(|f| f.strip_suffix(".md"))
                .unwrap_or_default();
            if stem != o.identity {
                m.findings.push(ProductFinding {
                    severity: Severity::Error,
                    code: "filename_mismatch".into(),
                    path: path.clone(),
                    id: Some(o.identity.clone()),
                    field: Some("id".into()),
                    message: format!(
                        "the file is named '{stem}.md' and the record's id is '{}'; the id is the file name and the route",
                        o.identity
                    ),
                    did_you_mean: Some(format!("{}.md", o.identity)),
                });
            }
            if let Ok(mut f) = serde_json::from_value::<Feature>(o.metadata.clone()) {
                f.route = format!("{ROUTE}{}/", f.id);
                f.source = path;
                f.body = o.body.clone();
                features.push(f);
            }
        }
        crate::order::canonical(&mut features);

        m.adopt_diagnostics(index);
        let lookups = Lookups::new(index);
        let backlinks = backlinks(&features);
        let areas = nav::areas();

        let mut resolved = Vec::with_capacity(features.len());
        for f in &features {
            let (r, findings) = resolve(
                f, registry, index, why, web, &lookups, areas, &backlinks, &features,
            );
            m.findings.extend(findings);
            resolved.push(r);
        }
        m.features = resolved;
        for (i, r) in m.features.iter().enumerate() {
            m.by_id.insert(r.feature.id.clone(), i);
        }
        m.providers = providers(index);
        for decl in &index.providers.providers {
            if !decl.template {
                m.findings.push(ProductFinding {
                    severity: Severity::Error,
                    code: "provider_without_template".into(),
                    path: "share/providers.yaml".into(),
                    id: Some(decl.id.clone()),
                    field: Some("providers".into()),
                    message: format!(
                        "provider '{}' is declared and the distribution ships no share/providers/{}.tmpl; a provider with no bootstrap is a name and nothing else",
                        decl.id, decl.id
                    ),
                    did_you_mean: None,
                });
            } else if !decl.declared {
                m.findings.push(ProductFinding {
                    severity: Severity::Warning,
                    code: "provider_undeclared".into(),
                    path: format!("share/providers/{}.tmpl", decl.id),
                    id: Some(decl.id.clone()),
                    field: None,
                    message: format!(
                        "provider '{}' has a template and no entry in share/providers.yaml; it is named by its file and declares no title, client configuration or scratch root",
                        decl.id
                    ),
                    did_you_mean: None,
                });
            }
        }
        m.coverage(registry, &lookups);
        let area_weights: BTreeMap<&str, u32> = why
            .areas()
            .iter()
            .map(|a| (a.id.as_str(), a.weight))
            .collect();
        m.derive_module_areas(&area_weights);
        m.findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then(a.path.cmp(&b.path))
                .then(a.code.cmp(&b.code))
                .then(a.message.cmp(&b.message))
        });
        m.fingerprint = m.compute_fingerprint();
        m
    }

    /// The area each capability module serves, derived rather than declared.
    ///
    /// A module has no area of its own and is given none: the features that name it in
    /// `modules:` already say which areas they serve, so the module's area is theirs,
    /// resolved by [`resolve_module_areas`]. A feature that declares no area claims
    /// nothing; a module no feature claims is shown under no heading and ordered last.
    fn derive_module_areas(&mut self, weight: &BTreeMap<&str, u32>) {
        let mut claims: Claims = BTreeMap::new();
        for r in &self.features {
            if r.feature.areas.is_empty() {
                continue;
            }
            for module in &r.feature.modules {
                claims
                    .entry(module.clone())
                    .or_default()
                    .push((r.feature.id.clone(), r.feature.areas.clone()));
            }
        }
        let (areas, contested) = resolve_module_areas(&claims, weight);
        self.module_areas = areas;
        for (module, who) in contested {
            self.findings.push(ProductFinding {
                severity: Severity::Warning,
                code: "contested_area".into(),
                path: ".ai/repo/features".into(),
                id: Some(module.clone()),
                field: Some("areas".into()),
                message: format!(
                    "the module '{module}' is claimed by features that share no area: {who}; \
                     the lowest-weight area wins until a feature file settles it"
                ),
                did_you_mean: None,
            });
        }
    }

    /// The area a capability module serves, or `None` for a module no feature with an area
    /// names.
    ///
    /// Derived when the model is built, from the features that name the module, resolved by
    /// the areas' own weight; the Cockpit's sidebar groups by it. The derivation is private
    /// and this text does not link to it: rustdoc refuses a public link to a private item,
    /// and the public fact is the answer rather than the route to it.
    pub fn module_area(&self, module: &str) -> Option<&str> {
        self.module_areas.get(module).map(String::as_str)
    }

    /// Every diagnostic the index raised about a file of the features section: an object the
    /// index refused never reaches the model, and without this the model would report
    /// itself valid over the objects excluded from under it.
    fn adopt_diagnostics(&mut self, index: &Index) {
        let Some(section) = index.repository.sections.get("features") else {
            return;
        };
        for d in index
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
        {
            let Some(path) = d.path.as_deref() else {
                continue;
            };
            if !path.starts_with(section.as_str()) {
                continue;
            }
            self.findings.push(ProductFinding {
                severity: Severity::Error,
                code: d.code.clone(),
                path: path.to_string(),
                id: None,
                field: None,
                message: d.message.clone(),
                did_you_mean: None,
            });
        }
    }

    /// Every module of the executable, every public command and every kind of the layer,
    /// each with the stable features that name it. A thing no feature names is a warning:
    /// the product page says nothing about something the product does.
    fn coverage(&mut self, registry: &CapabilityRegistry, lookups: &Lookups<'_>) {
        let stable: Vec<&ResolvedRefs> = self
            .features
            .iter()
            .filter(|r| r.feature.status == STABLE)
            .collect();
        let naming = |pick: &dyn Fn(&Feature) -> &Vec<String>, id: &str| -> Vec<String> {
            stable
                .iter()
                .filter(|r| pick(&r.feature).iter().any(|x| x == id))
                .map(|r| r.feature.id.clone())
                .collect()
        };
        self.modules = registry
            .modules()
            .filter(|m| m.source != crate::capability::registry::ModuleSource::Declarative)
            .map(|m| ProductCoverage {
                id: m.id.to_string(),
                title: m.title.clone(),
                features: naming(&|f| &f.modules, m.id.as_str()),
            })
            .collect();
        self.commands = lookups
            .commands
            .iter()
            .map(|(id, o)| ProductCoverage {
                id: id.clone(),
                title: o.title.clone().unwrap_or_else(|| id.clone()),
                features: naming(&|f| &f.commands, id),
            })
            .collect();
        // the kinds a feature can present: those holding an object of the layer under the
        // manifest's sections; the implementation and test classes index source files so
        // that a claim's chain resolves, and are not things the product does
        self.kinds = lookups
            .kinds
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "implementation" | "test" | "document"))
            .map(|(k, n)| ProductCoverage {
                id: k.clone(),
                title: format!("{k} ({n})"),
                features: naming(&|f| &f.kinds, k),
            })
            .collect();
        let mut uncovered = Vec::new();
        for (what, list) in [
            ("module", &self.modules),
            ("command", &self.commands),
            ("kind", &self.kinds),
        ] {
            for c in list.iter().filter(|c| c.features.is_empty()) {
                uncovered.push(ProductFinding {
                    severity: Severity::Warning,
                    code: "uncovered".into(),
                    path: match what {
                        "module" => crate::capability::model::CRATE_DIR.to_string(),
                        "command" => "share/commands.yaml".to_string(),
                        _ => "share/kinds.yaml".to_string(),
                    },
                    id: Some(c.id.clone()),
                    field: None,
                    message: format!(
                        "the {what} '{}' is named by no stable feature; the product page says nothing about it",
                        c.id
                    ),
                    did_you_mean: None,
                });
            }
        }
        self.findings.extend(uncovered);
    }

    /// A hash of the model's own sources, in identity order: the feature files, and the
    /// facts derived from the registries, so a registry change that moves a feature's
    /// surfaces moves the fingerprint too.
    fn compute_fingerprint(&self) -> String {
        let mut h = Sha256::new();
        for r in &self.features {
            h.update(r.feature.id.as_bytes());
            h.update([0]);
            h.update(r.feature.source.as_bytes());
            h.update([0]);
            h.update(r.feature.body.as_bytes());
            h.update([0]);
            h.update(
                serde_json::to_string(&r.surfaces)
                    .unwrap_or_default()
                    .as_bytes(),
            );
            h.update(
                serde_json::to_string(&r.counts)
                    .unwrap_or_default()
                    .as_bytes(),
            );
            h.update(b"\n");
        }
        format!("{:x}", h.finalize())
    }

    /// Every feature, resolved, in presentation order, whatever its status.
    pub fn all(&self) -> &[ResolvedRefs] {
        &self.features
    }
    /// One feature by id.
    pub fn feature(&self, id: &str) -> Option<&ResolvedRefs> {
        self.by_id.get(id).map(|i| &self.features[*i])
    }
    /// The stable features, in presentation order.
    pub fn public(&self) -> Vec<&ResolvedRefs> {
        self.features
            .iter()
            .filter(|r| r.feature.status == STABLE)
            .collect()
    }
    /// The stable features the homepage shows, in presentation order.
    pub fn featured(&self) -> Vec<&ResolvedRefs> {
        self.public()
            .into_iter()
            .filter(|r| r.feature.featured)
            .collect()
    }
    /// The providers the tool has an adapter for.
    pub fn providers(&self) -> &[ProductProvider] {
        &self.providers
    }
    /// Every finding.
    pub fn findings(&self) -> &[ProductFinding] {
        &self.findings
    }
    /// How many findings are errors.
    pub fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }
    /// The hash of the model's sources and derived facts.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
    /// Every module of the executable with the stable features that name it.
    pub fn module_coverage(&self) -> &[ProductCoverage] {
        &self.modules
    }
    /// Every public shell command with the stable features that name it.
    pub fn command_coverage(&self) -> &[ProductCoverage] {
        &self.commands
    }
    /// Every kind of the layer with the stable features that name it.
    pub fn kind_coverage(&self) -> &[ProductCoverage] {
        &self.kinds
    }
}

/// The features that name each feature in their `related`.
fn backlinks(features: &[Feature]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in features {
        for r in &f.related {
            out.entry(r.clone()).or_default().push(f.id.clone());
        }
    }
    for v in out.values_mut() {
        v.sort();
        v.dedup();
    }
    out
}

/// One capability as the feature's view shows it.
fn capability_ref(c: &Capability) -> CapabilityRef {
    CapabilityRef {
        id: c.id.to_string(),
        title: c.title.clone(),
        kind: enum_word(&c.kind),
        stability: enum_word(&c.stability),
        tool: c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
        resource: c
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref())
            .map(|r| r.uri.clone()),
        route: c
            .exposure
            .http
            .as_ref()
            .map(|h| format!("{} {}", h.method.as_str(), h.path)),
        cli: c
            .exposure
            .cli
            .as_ref()
            .map(|x| format!("majordomus {}", x.path.join(" "))),
    }
}

/// Resolve one feature: every reference against the thing it names, with a finding for
/// each that resolves to nothing, then everything derived.
#[allow(clippy::too_many_arguments)]
fn resolve(
    f: &Feature,
    registry: &CapabilityRegistry,
    index: &Index,
    why: &Catalogue,
    web: &Topology,
    lookups: &Lookups<'_>,
    areas: &[nav::AreaInfo],
    backlinks: &BTreeMap<String, Vec<String>>,
    all: &[Feature],
) -> (ResolvedRefs, Vec<ProductFinding>) {
    let mut findings = Vec::new();
    // the unresolved references, collected apart so the closure holds nothing the other
    // findings need, and merged once every reference has been looked at
    let mut unknowns = Vec::new();
    let mut unknown = |field: &str, value: &str, what: &str, candidates: Vec<&str>| {
        unknowns.push(ProductFinding {
            severity: Severity::Error,
            code: "unknown_reference".into(),
            path: f.source.clone(),
            id: Some(f.id.clone()),
            field: Some(field.into()),
            message: format!("unknown {what} reference: \"{value}\""),
            did_you_mean: nearest(value, candidates.into_iter()),
        });
    };

    // modules: the builtin ones of the registry, with every capability they compose
    let mut module_refs = Vec::new();
    for id in &f.modules {
        match registry
            .module(id)
            .filter(|m| m.source != crate::capability::registry::ModuleSource::Declarative)
        {
            Some(m) => {
                let capabilities: Vec<&Capability> = registry
                    .iter()
                    .filter(|c| {
                        c.module.as_str() == m.id.as_str()
                            && matches!(c.provenance, Provenance::Builtin { .. })
                    })
                    .collect();
                module_refs.push(ModuleRef {
                    id: m.id.to_string(),
                    title: m.title.clone(),
                    description: m.description.clone(),
                    stability: m.stability.map(|s| enum_word(&s)),
                    source_path: capabilities
                        .first()
                        .map(|c| c.provenance.source_path())
                        .unwrap_or_default(),
                    capabilities: capabilities.iter().map(|c| capability_ref(c)).collect(),
                });
            }
            None => unknown(
                "modules",
                id,
                "module",
                registry
                    .modules()
                    .filter(|m| m.source != crate::capability::registry::ModuleSource::Declarative)
                    .map(|m| m.id.as_str())
                    .collect(),
            ),
        }
    }

    let mut command_refs = Vec::new();
    for id in &f.commands {
        match lookups.commands.get(id) {
            Some(o) => command_refs.push(CommandRef {
                id: id.clone(),
                summary: o.description.clone().unwrap_or_default(),
                stage: meta_str(o, "stage"),
                read_only: meta_str(o, "class").as_deref() == Some("read-only"),
            }),
            None => unknown(
                "commands",
                id,
                "command",
                lookups.commands.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut kind_refs = Vec::new();
    for k in &f.kinds {
        match lookups.kinds.get(k) {
            Some(n) => kind_refs.push(KindRef {
                name: k.clone(),
                objects: *n,
            }),
            None => unknown(
                "kinds",
                k,
                "kind",
                lookups.kinds.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut rule_refs = Vec::new();
    for id in &f.rules {
        match lookups.rules_by_stem.get(id) {
            Some(o) => rule_refs.push(RuleRef {
                id: id.clone(),
                identity: o.identity.clone(),
                title: o.title.clone().unwrap_or_else(|| id.clone()),
                class: meta_str(o, "class").unwrap_or_default(),
                enforced: o.metadata.get("x-majordomus").is_some(),
                path: o.provenance.path.clone(),
            }),
            None => unknown(
                "rules",
                id,
                "rule",
                lookups.rules_by_stem.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut doc_refs = Vec::new();
    for p in &f.docs {
        match lookups.docs.get(p) {
            Some(o) => doc_refs.push(DocRef {
                path: p.clone(),
                title: o.title.clone(),
            }),
            None => unknown(
                "docs",
                p,
                "document",
                lookups.docs.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut adr_refs = Vec::new();
    for id in &f.adrs {
        match lookups.adrs_by_id.get(id) {
            Some(o) => adr_refs.push(AdrRef {
                id: id.clone(),
                title: o.title.clone().unwrap_or_else(|| id.clone()),
                status: meta_str(o, "status").unwrap_or_default(),
                path: o.provenance.path.clone(),
            }),
            None => unknown(
                "adrs",
                id,
                "decision",
                lookups.adrs_by_id.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut claim_refs = Vec::new();
    for id in &f.claims {
        match lookups.claims.get(id) {
            Some(o) => claim_refs.push(ClaimRef {
                id: id.clone(),
                claim: o.title.clone().unwrap_or_else(|| id.clone()),
                status: meta_str(o, "status").unwrap_or_default(),
            }),
            None => unknown(
                "claims",
                id,
                "claim",
                lookups.claims.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut use_case_refs = Vec::new();
    for id in &f.use_cases {
        match lookups.use_cases.get(id) {
            Some(o) => use_case_refs.push(UseCaseRef {
                id: id.clone(),
                title: o.title.clone().unwrap_or_else(|| id.clone()),
                category: meta_str(o, "category"),
            }),
            None => unknown(
                "use_cases",
                id,
                "use case",
                lookups.use_cases.keys().map(String::as_str).collect(),
            ),
        }
    }

    let mut cockpit_refs = Vec::new();
    for id in &f.cockpit {
        match areas.iter().find(|a| a.id == id) {
            Some(a) => cockpit_refs.push(CockpitAreaRef {
                id: id.clone(),
                title: a.label.to_string(),
                route: a.href.to_string(),
            }),
            None => unknown(
                "cockpit",
                id,
                "Cockpit area",
                areas.iter().map(|a| a.id).collect(),
            ),
        }
    }

    let mut web_refs = Vec::new();
    for id in &f.web {
        match web.get(id) {
            Some(s) => web_refs.push(SurfaceRef {
                id: id.clone(),
                title: s.title.clone(),
                mount: s.mount.as_str().to_string(),
                category: s.category.to_string(),
            }),
            None => unknown("web", id, "web surface", web.ids()),
        }
    }

    for a in &f.areas {
        if !lookups.areas.contains(a) {
            unknown(
                "areas",
                a,
                "area",
                lookups.areas.iter().map(String::as_str).collect(),
            );
        }
    }
    for a in &f.audiences {
        if !lookups.audiences.contains(a) {
            unknown(
                "audiences",
                a,
                "audience",
                lookups.audiences.iter().map(String::as_str).collect(),
            );
        }
    }
    for r in &f.related {
        if r == &f.id {
            findings.push(ProductFinding {
                severity: Severity::Error,
                code: "self_reference".into(),
                path: f.source.clone(),
                id: Some(f.id.clone()),
                field: Some("related".into()),
                message: "a feature may not be related to itself".into(),
                did_you_mean: None,
            });
        } else if !all.iter().any(|x| &x.id == r) {
            unknown(
                "related",
                r,
                "feature",
                all.iter().map(|x| x.id.as_str()).collect(),
            );
        }
    }
    findings.extend(unknowns);
    if RESERVED.contains(&f.id.as_str()) {
        findings.push(ProductFinding {
            severity: Severity::Error,
            code: "reserved_identity".into(),
            path: f.source.clone(),
            id: Some(f.id.clone()),
            field: Some("id".into()),
            message: format!(
                "'{}' is a route this section owns; a feature may not claim it (reserved: {})",
                f.id,
                RESERVED.join(", ")
            ),
            did_you_mean: None,
        });
    }
    if f.featured && f.status != STABLE {
        findings.push(ProductFinding {
            severity: Severity::Error,
            code: "featured_draft".into(),
            path: f.source.clone(),
            id: Some(f.id.clone()),
            field: Some("featured".into()),
            message: format!(
                "a {} feature may not be featured; the homepage shows stable features only",
                f.status
            ),
            did_you_mean: None,
        });
    }

    // the floors a stable feature is held to
    if f.status == STABLE {
        let mut floor = |field: &str, message: &str| {
            findings.push(ProductFinding {
                severity: Severity::Warning,
                code: "missing_content".into(),
                path: f.source.clone(),
                id: Some(f.id.clone()),
                field: Some(field.into()),
                message: message.into(),
                did_you_mean: None,
            });
        };
        if module_refs.is_empty() && command_refs.is_empty() && kind_refs.is_empty() {
            floor(
                "modules",
                "a stable feature names at least one mechanism: a module, a command or a kind",
            );
        }
        if doc_refs.is_empty() {
            floor(
                "docs",
                "a stable feature names at least one document that explains it",
            );
        }
        for heading in ["## What it does", "## What it does not do"] {
            if !f.body.lines().any(|l| l.trim_end() == heading) {
                floor(
                    "body",
                    &format!("a stable feature's body carries `{heading}`"),
                );
            }
        }
    }

    // the surfaces, decided from what the references project
    let caps: Vec<&CapabilityRef> = module_refs.iter().flat_map(|m| &m.capabilities).collect();
    let surfaces = Surfaces {
        cli: caps.iter().any(|c| c.cli.is_some()) || !command_refs.is_empty(),
        api: caps.iter().any(|c| c.route.is_some()),
        mcp: caps
            .iter()
            .any(|c| c.tool.is_some() || c.resource.is_some())
            || !kind_refs.is_empty(),
        cockpit: !cockpit_refs.is_empty() || !module_refs.is_empty(),
        docs: !doc_refs.is_empty(),
    };

    // the moments this feature answers: the moments that name any of its mechanisms.
    // Read through the catalogue's own reverse index, so the relation is the one the why
    // pages already show from the other side.
    let mut moment_ids: Vec<String> = Vec::new();
    let mut add = |ms: Vec<&crate::why::Moment>| {
        for m in ms {
            if m.status == crate::why::STABLE && !moment_ids.contains(&m.id) {
                moment_ids.push(m.id.clone());
            }
        }
    };
    for c in &command_refs {
        add(why.naming("command", &c.id));
    }
    for c in &caps {
        add(why.naming("capability", &c.id));
    }
    for c in &claim_refs {
        add(why.naming("claim", &c.id));
    }
    for r in &rule_refs {
        add(why.naming("doctrine", &r.id));
    }
    let mut moments: Vec<MomentRef> = moment_ids
        .iter()
        .filter_map(|id| why.moment(id))
        .map(|m| MomentRef {
            id: m.id.clone(),
            title: m.title.clone(),
            hook: m.hook.clone(),
            route: m.route.clone(),
        })
        .collect();
    moments.sort_by(|a, b| {
        let wa = why.moment(&a.id).map(|m| m.weight).unwrap_or_default();
        let wb = why.moment(&b.id).map(|m| m.weight).unwrap_or_default();
        wa.cmp(&wb).then(a.id.cmp(&b.id))
    });

    let counts = FeatureCounts {
        capabilities: caps.len(),
        mcp_tools: caps.iter().filter(|c| c.tool.is_some()).count(),
        mcp_resources: caps.iter().filter(|c| c.resource.is_some()).count()
            + kind_refs.iter().map(|k| k.objects).sum::<usize>(),
        http_routes: caps.iter().filter(|c| c.route.is_some()).count(),
        cli_paths: caps.iter().filter(|c| c.cli.is_some()).count(),
        commands: command_refs.len(),
        objects: kind_refs.iter().map(|k| k.objects).sum(),
        rules: rule_refs.len(),
        enforced_rules: rule_refs.iter().filter(|r| r.enforced).count(),
        docs: doc_refs.len(),
        adrs: adr_refs.len(),
        claims: claim_refs.len(),
        use_cases: use_case_refs.len(),
        moments: moments.len(),
    };
    let mut evidence = FeatureEvidence::default();
    for c in &claim_refs {
        *evidence.claims.entry(c.status.clone()).or_default() += 1;
    }
    for m in &module_refs {
        *evidence
            .modules
            .entry(m.stability.clone().unwrap_or_else(|| "unknown".into()))
            .or_default() += 1;
    }
    let _ = index;

    (
        ResolvedRefs {
            feature: f.clone(),
            surfaces,
            module_refs,
            command_refs,
            kind_refs,
            rule_refs,
            doc_refs,
            adr_refs,
            claim_refs,
            use_case_refs,
            cockpit_refs,
            web_refs,
            moments,
            backlinks: backlinks.get(&f.id).cloned().unwrap_or_default(),
            counts,
            evidence,
        },
        findings,
    )
}

/// The providers, derived: one per template the distribution ships or declaration it
/// carries, decorated with what this repository's policy renders through it, the client
/// configuration it carries and the hooks the policy wires. The title, the client
/// configuration's name and the scratch roots are the declaration's (`share/providers.yaml`);
/// whether the configuration is present is a fact of the tree. Reads the policy as the index holds it — an object of kind
/// `policy` whose metadata is the whole file — and the repository root for the client
/// configurations, which are tracked files and therefore facts of the tree.
fn providers(index: &Index) -> Vec<ProductProvider> {
    let policy = index.objects.iter().find(|o| o.kind == "policy");
    let projections: Vec<(String, String, String, bool)> = policy
        .and_then(|p| p.metadata.get("projections"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some((
                        p.get("provider")?.as_str()?.to_string(),
                        p.get("target")?.as_str()?.to_string(),
                        p.get("mode")
                            .and_then(Value::as_str)
                            .unwrap_or("file")
                            .to_string(),
                        p.get("always_loaded")
                            .map(|v| v == "true" || v == &Value::Bool(true))
                            .unwrap_or(false),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let hooks: Vec<(String, String)> = policy
        .and_then(|p| p.metadata.get("enforcement"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    let wired = e.get("wired_by")?.as_str()?;
                    let provider = wired.strip_prefix("provider-hook:")?;
                    let provider = provider.split(':').next().unwrap_or(provider);
                    Some((provider.to_string(), e.get("name")?.as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    let root = Path::new(&index.repository.root);
    let mut out: Vec<ProductProvider> = index
        .providers
        .providers
        .iter()
        .map(|decl| {
            let id = &decl.id;
            let client_config = decl
                .client_config
                .as_deref()
                .filter(|c| root.join(c).is_file())
                .map(str::to_string);
            ProductProvider {
                id: id.clone(),
                title: decl.title.clone(),
                bootstraps: projections
                    .iter()
                    .filter(|(p, _, _, _)| p == id)
                    .map(|(_, target, mode, always)| ProviderBootstrap {
                        target: target.clone(),
                        mode: mode.clone(),
                        always_loaded: *always,
                    })
                    .collect(),
                client_config,
                hooks: hooks
                    .iter()
                    .filter(|(p, _)| p == id)
                    .map(|(_, n)| n.clone())
                    .collect(),
                scratch_roots: decl.scratch_roots.clone(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Areas as weights: the ranking the resolution reads, and the whole of what it reads.
    /// The catalogue's own numbers, five of the nine.
    fn areas() -> BTreeMap<&'static str, u32> {
        [
            ("context", 10),
            ("coordination", 20),
            ("governance", 30),
            ("verification", 60),
            ("documentation", 90),
        ]
        .into_iter()
        .collect()
    }

    /// One module's claimants in a fixture: the feature's id and the areas it serves.
    type Claimant<'a> = (&'a str, &'a [&'a str]);

    /// Claims as the model builds them: module -> the features that name it, with the areas
    /// each of those features serves.
    fn claims(of: &[(&str, &[Claimant<'_>])]) -> Claims {
        of.iter()
            .map(|(module, claimants)| {
                (
                    (*module).to_string(),
                    claimants
                        .iter()
                        .map(|(feature, areas)| {
                            (
                                (*feature).to_string(),
                                areas.iter().map(|a| (*a).to_string()).collect(),
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn a_modules_area_is_the_lowest_weight_area_of_the_features_that_name_it() {
        let (areas_by_module, contested) = resolve_module_areas(
            &claims(&[
                (
                    "continuity",
                    &[("continuity", &["context", "coordination"])],
                ),
                (
                    "distribution",
                    &[("install", &["verification", "documentation"])],
                ),
            ]),
            &areas(),
        );
        assert_eq!(
            areas_by_module.get("continuity").map(String::as_str),
            Some("context")
        );
        assert_eq!(
            areas_by_module.get("distribution").map(String::as_str),
            Some("verification")
        );
        assert_eq!(areas_by_module.get("nothing-names-me"), None);
        assert!(
            contested.is_empty(),
            "one claimant per module is not a disagreement"
        );
    }

    #[test]
    fn a_module_two_features_place_differently_is_reported_and_still_resolved() {
        let (areas_by_module, contested) = resolve_module_areas(
            &claims(&[(
                "health",
                &[
                    ("cockpit", &["documentation"]),
                    ("doctrine", &["governance", "verification"]),
                ],
            )]),
            &areas(),
        );
        // Deterministic despite the disagreement: the catalogue's own ranking decides.
        assert_eq!(
            areas_by_module.get("health").map(String::as_str),
            Some("governance")
        );
        assert_eq!(contested.len(), 1, "the disagreement is reported once");
        assert_eq!(contested[0].0, "health");
        assert!(contested[0].1.contains("cockpit"));
        assert!(contested[0].1.contains("doctrine"));
    }

    #[test]
    fn features_that_overlap_in_one_area_are_not_a_disagreement() {
        let (areas_by_module, contested) = resolve_module_areas(
            &claims(&[(
                "worktree",
                &[
                    ("coordination", &["coordination"]),
                    ("worktrees", &["coordination", "context"]),
                ],
            )]),
            &areas(),
        );
        assert_eq!(
            areas_by_module.get("worktree").map(String::as_str),
            Some("context")
        );
        assert!(
            contested.is_empty(),
            "a shared area is agreement, whatever else each feature also serves"
        );
    }

    #[test]
    fn an_area_the_catalogue_does_not_rank_loses_to_one_it_does() {
        let (areas_by_module, _) = resolve_module_areas(
            &claims(&[("m", &[("f", &["not-an-area", "documentation"])])]),
            &areas(),
        );
        assert_eq!(
            areas_by_module.get("m").map(String::as_str),
            Some("documentation")
        );
    }

    #[test]
    fn the_surface_vocabulary_is_closed_and_read_back_in_order() {
        let all = Surfaces {
            cli: true,
            api: true,
            mcp: true,
            cockpit: true,
            docs: true,
        };
        assert_eq!(all.present(), ["cli", "api", "mcp", "cockpit", "docs"]);
        let none = Surfaces::default();
        assert!(none.present().is_empty());
        assert!(!none.has("nowhere"));
        assert_eq!(SURFACES.len(), 5);
    }

    #[test]
    fn nearest_offers_a_plausible_typo_and_nothing_else() {
        assert_eq!(
            nearest("worktre", ["worktree", "why"].into_iter()),
            Some("worktree".into())
        );
        assert_eq!(nearest("zzzzzzzz", ["worktree", "why"].into_iter()), None);
    }

    #[test]
    fn a_feature_labels_itself_by_its_short_title_when_it_has_one() {
        let f: Feature = serde_json::from_value(serde_json::json!({
            "id": "x", "title": "A long title", "short_title": "Short",
            "headline": "h", "summary": "s", "status": "stable"
        }))
        .unwrap();
        assert_eq!(f.label(), "Short");
        assert!(
            f.route.is_empty(),
            "the route is derived when the model is built"
        );
    }
}
