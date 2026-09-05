//! The executable capabilities this executable ships, composed explicitly in [`all`].
//! Each one is one typed function; MCP, HTTP and the command line all call the same one.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability;
use crate::index::{RepositoryInfo, State};
use crate::model::{Diagnostic, Object, Provenance as ObjectProvenance};

use super::handler::{CapabilityError, Context, Executable};
use super::model::{
    Capability, CliExposure, Exposure, HttpExposure, HttpMethod, McpExposure, McpResource,
    Stability,
};
use super::registry::Summary;

// ---------------------------------------------------------------- shared views

/// One declarative object of the repository's layer, as a client reads it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObjectView {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
    /// The capability id, `<kind>.<identity>`.
    pub id: String,
    pub kind: String,
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The parsed front matter or YAML, keys in the file's order.
    pub metadata: Value,
    pub provenance: ObjectProvenance,
    pub media_type: String,
    /// The file as read.
    pub content: String,
}

impl ObjectView {
    fn of(o: &Object) -> Self {
        ObjectView {
            uri: o.uri.clone(),
            id: format!("{}.{}", o.kind, o.identity),
            kind: o.kind.clone(),
            identity: o.identity.clone(),
            title: o.title.clone(),
            description: o.description.clone(),
            metadata: o.metadata.clone(),
            provenance: o.provenance.clone(),
            media_type: o.media_type.to_string(),
            content: o.content.clone(),
        }
    }
}

/// One object, summarised for a listing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObjectSummary {
    pub uri: String,
    pub id: String,
    pub kind: String,
    pub identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Repository-relative source path.
    pub path: String,
}

impl ObjectSummary {
    fn of(o: &Object) -> Self {
        ObjectSummary {
            uri: o.uri.clone(),
            id: format!("{}.{}", o.kind, o.identity),
            kind: o.kind.clone(),
            identity: o.identity.clone(),
            title: o.title.clone(),
            description: o.description.clone(),
            path: o.provenance.path.clone(),
        }
    }
}

// ---------------------------------------------------------------- repository.info

/// No input.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

/// The repository, its layer, its git state, and the state of this process's index.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryReport {
    pub repository: RepositoryInfo,
    /// `ok` when every discovered file became an object, `degraded` otherwise.
    pub state: State,
    pub objects: usize,
    /// Objects per kind.
    pub kinds: std::collections::BTreeMap<String, usize>,
    /// Every diagnostic the index produced.
    pub diagnostics: Vec<Diagnostic>,
    /// The capability registry, counted.
    pub capabilities: Summary,
}

fn repository_info(ctx: &Context, _: Empty) -> Result<RepositoryReport, CapabilityError> {
    let index = &ctx.index;
    Ok(RepositoryReport {
        repository: index.repository.clone(),
        state: index.state,
        objects: index.objects.len(),
        kinds: index
            .kinds()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        diagnostics: index.diagnostics.clone(),
        capabilities: ctx.registry.summary(),
    })
}

// ---------------------------------------------------------------- objects.list

/// Filters; both optional.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
    /// Only objects of this kind; the kinds present are listed by `repository.info`. A kind
    /// the repository does not have is an invalid input, not an empty answer.
    #[serde(default)]
    pub kind: Option<String>,
    /// Only objects whose metadata tags include this tag.
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ObjectList {
    pub count: usize,
    pub objects: Vec<ObjectSummary>,
}

/// A kind filter must name a kind the index holds.
fn known_kind(ctx: &Context, kind: Option<&str>) -> Result<(), CapabilityError> {
    let Some(kind) = kind else { return Ok(()) };
    let kinds = ctx.index.kinds();
    if kinds.contains_key(kind) {
        Ok(())
    } else {
        Err(CapabilityError::InvalidInput(format!(
            "kind '{kind}' is not among the kinds this repository has: {}",
            kinds.keys().copied().collect::<Vec<_>>().join(", ")
        )))
    }
}

fn objects_list(ctx: &Context, input: ListInput) -> Result<ObjectList, CapabilityError> {
    known_kind(ctx, input.kind.as_deref())?;
    let objects: Vec<ObjectSummary> = ctx
        .index
        .objects
        .iter()
        .filter(|o| input.kind.as_deref().is_none_or(|k| o.kind == k))
        .filter(|o| input.tag.as_deref().is_none_or(|t| o.tags().contains(&t)))
        .map(ObjectSummary::of)
        .collect();
    Ok(ObjectList {
        count: objects.len(),
        objects,
    })
}

// ---------------------------------------------------------------- objects.get

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInput {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
}

fn objects_get(ctx: &Context, input: GetInput) -> Result<ObjectView, CapabilityError> {
    ctx.index
        .get(&input.uri)
        .map(ObjectView::of)
        .ok_or_else(|| CapabilityError::NotFound(format!("unknown resource: {}", input.uri)))
}

// ---------------------------------------------------------------- objects.search

pub const SEARCH_DEFAULT_LIMIT: u64 = 20;
pub const SEARCH_MAX_LIMIT: u64 = 200;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    /// Case-insensitive substring, matched against identity, title, description and content.
    pub query: String,
    #[serde(default)]
    pub kind: Option<String>,
    /// At most this many hits (default 20, at most 200).
    #[serde(default)]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchHit {
    #[serde(flatten)]
    pub object: ObjectSummary,
    /// The first line of content that matched, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchResult {
    pub query: String,
    pub count: usize,
    pub limit: u64,
    pub hits: Vec<SearchHit>,
}

fn objects_search(ctx: &Context, input: SearchInput) -> Result<SearchResult, CapabilityError> {
    if input.query.trim().is_empty() {
        return Err(CapabilityError::Refused(
            "argument 'query' is required and must not be blank".into(),
        ));
    }
    let limit = input
        .limit
        .map(|l| l.clamp(1, SEARCH_MAX_LIMIT))
        .unwrap_or(SEARCH_DEFAULT_LIMIT);
    let needle = input.query.to_lowercase();
    let mut hits = Vec::new();
    for o in ctx
        .index
        .objects
        .iter()
        .filter(|o| input.kind.as_deref().is_none_or(|k| o.kind == k))
    {
        let head = [
            o.identity.as_str(),
            o.title.as_deref().unwrap_or(""),
            o.description.as_deref().unwrap_or(""),
        ]
        .iter()
        .any(|s| s.to_lowercase().contains(&needle));
        let line = o
            .content
            .lines()
            .find(|l| l.to_lowercase().contains(&needle));
        if head || line.is_some() {
            hits.push(SearchHit {
                object: ObjectSummary::of(o),
                snippet: line.map(|l| l.trim().chars().take(200).collect()),
            });
            if hits.len() as u64 == limit {
                break;
            }
        }
    }
    Ok(SearchResult {
        query: input.query,
        count: hits.len(),
        limit,
        hits,
    })
}

// ---------------------------------------------------------------- capabilities.list

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitiesInput {
    /// Only capabilities of this kind: `query` or `resource`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Only capabilities exposed through this projection: `mcp`, `http` or `cli`.
    #[serde(default)]
    pub exposure: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityList {
    pub count: usize,
    pub summary: Summary,
    pub capabilities: Vec<Capability>,
}

fn capabilities_list(
    ctx: &Context,
    input: CapabilitiesInput,
) -> Result<CapabilityList, CapabilityError> {
    let kind = match input.kind.as_deref() {
        None => None,
        Some("query") => Some(super::model::CapabilityKind::Query),
        Some("resource") => Some(super::model::CapabilityKind::Resource),
        Some(other) => {
            return Err(CapabilityError::InvalidInput(format!(
                "kind '{other}' is not query or resource"
            )))
        }
    };
    let exposure = match input.exposure.as_deref() {
        None | Some("mcp") | Some("http") | Some("cli") => input.exposure.clone(),
        Some(other) => {
            return Err(CapabilityError::InvalidInput(format!(
                "exposure '{other}' is not mcp, http or cli"
            )))
        }
    };
    let capabilities: Vec<Capability> = ctx
        .registry
        .iter()
        .filter(|c| kind.is_none_or(|k| c.kind == k))
        .filter(|c| match exposure.as_deref() {
            None => true,
            Some("mcp") => c.exposure.mcp.is_some(),
            Some("http") => c.exposure.http.is_some(),
            Some("cli") => c.exposure.cli.is_some(),
            Some(_) => false,
        })
        .cloned()
        .collect();
    Ok(CapabilityList {
        count: capabilities.len(),
        summary: ctx.registry.summary(),
        capabilities,
    })
}

// ---------------------------------------------------------------- capabilities.describe

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DescribeInput {
    /// The canonical id, e.g. `repository.info` or `rule.majordomus.scope-integrity@1`.
    pub id: String,
}

fn capabilities_describe(
    ctx: &Context,
    input: DescribeInput,
) -> Result<Capability, CapabilityError> {
    ctx.registry
        .get(&input.id)
        .cloned()
        .ok_or_else(|| CapabilityError::NotFound(format!("unknown capability: {}", input.id)))
}

// ---------------------------------------------------------------- composition

fn mcp(tool: &str) -> Option<McpExposure> {
    Some(McpExposure {
        tool: Some(tool.into()),
        resource: None,
    })
}
fn get(path: &str) -> Option<HttpExposure> {
    Some(HttpExposure {
        method: HttpMethod::Get,
        path: path.into(),
    })
}

/// Every builtin, in one explicit list. Adding one is adding a line here and a typed
/// function above; no projection is touched.
pub fn all() -> Vec<Executable> {
    vec![
        capability! {
            id: "repository.info",
            title: "Repository and index state",
            description: "The repository root, layer sections, git state, discovery mode, kinds present, every diagnostic, and the capability registry counted.",
            input: Empty,
            output: RepositoryReport,
            stability: Stability::BehaviorallyVerified,
            exposure: Exposure {
                mcp: Some(McpExposure {
                    tool: Some("majordomus_repository".into()),
                    resource: Some(McpResource { uri: "majordomus://repository".into(), name: "repository".into() }),
                }),
                http: get("/api/v1/repository"),
                cli: None,
            },
            tags: ["repository", "introspection"],
            handler: repository_info,
        },
        capability! {
            id: "objects.list",
            title: "List objects",
            description: "List the declarative objects of the repository's AI layer, optionally by kind or tag.",
            input: ListInput,
            output: ObjectList,
            stability: Stability::BehaviorallyVerified,
            exposure: Exposure { mcp: mcp("majordomus_list"), http: get("/api/v1/objects"), cli: None },
            tags: ["objects"],
            handler: objects_list,
        },
        capability! {
            id: "objects.get",
            title: "Get one object",
            description: "One object by URI (majordomus://<kind>/<identity>): metadata, provenance and content.",
            input: GetInput,
            output: ObjectView,
            stability: Stability::BehaviorallyVerified,
            exposure: Exposure { mcp: mcp("majordomus_get"), http: get("/api/v1/object"), cli: None },
            tags: ["objects"],
            handler: objects_get,
        },
        capability! {
            id: "objects.search",
            title: "Search objects",
            description: "Case-insensitive substring search over identities, titles, descriptions and content.",
            input: SearchInput,
            output: SearchResult,
            stability: Stability::BehaviorallyVerified,
            exposure: Exposure { mcp: mcp("majordomus_search"), http: get("/api/v1/search"), cli: None },
            tags: ["objects"],
            handler: objects_search,
        },
        capability! {
            id: "capabilities.list",
            title: "List capabilities",
            description: "Every capability of this executable and this repository, with its kind, stability, provenance and the projections it declares.",
            input: CapabilitiesInput,
            output: CapabilityList,
            stability: Stability::BehaviorallyVerified,
            exposure: Exposure {
                mcp: mcp("majordomus_capabilities"),
                http: get("/api/v1/capabilities"),
                cli: Some(CliExposure { path: vec!["capabilities".into(), "list".into()] }),
            },
            tags: ["introspection"],
            handler: capabilities_list,
        },
        capability! {
            id: "capabilities.describe",
            title: "Describe one capability",
            description: "One capability by canonical id: its schemas, provenance, stability and every projection it appears in.",
            input: DescribeInput,
            output: Capability,
            stability: Stability::BehaviorallyVerified,
            exposure: Exposure {
                mcp: mcp("majordomus_capability"),
                http: get("/api/v1/capability"),
                cli: Some(CliExposure { path: vec!["capabilities".into(), "describe".into()] }),
            },
            tags: ["introspection"],
            handler: capabilities_describe,
        },
    ]
}
