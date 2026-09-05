//! The `objects` module: the declarative objects of the repository's layer, listed, read
//! and searched.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{get, mcp, ObjectSummary, ObjectView};

// ---------------------------------------------------------------- objects.list

/// Filters; both optional.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
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

impl BenchmarkCases for ListInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases = vec![NamedCase::new("all", ListInput::default())];
        if let Some((kind, _)) = ctx.index.kinds().into_iter().next() {
            cases.push(NamedCase::new(
                "first-kind",
                ListInput {
                    kind: Some(kind.to_string()),
                    tag: None,
                },
            ));
        }
        cases
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `objects.list`: the matching objects, summarised, in URI order.
pub struct ObjectList {
    /// How many objects matched.
    pub count: usize,
    /// The objects, one summary each.
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `objects.get`: which object.
pub struct GetInput {
    /// `majordomus://<kind>/<identity>`.
    pub uri: String,
}

impl BenchmarkCases for GetInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .objects
            .first()
            .map(|o| {
                vec![NamedCase::new(
                    "first-object",
                    GetInput { uri: o.uri.clone() },
                )]
            })
            .unwrap_or_default()
    }
}

fn objects_get(ctx: &Context, input: GetInput) -> Result<ObjectView, CapabilityError> {
    ctx.index
        .get(&input.uri)
        .map(ObjectView::of)
        .ok_or_else(|| CapabilityError::NotFound(format!("unknown resource: {}", input.uri)))
}

// ---------------------------------------------------------------- objects.search

/// Hits returned by `objects.search` when no limit is given.
pub const SEARCH_DEFAULT_LIMIT: u64 = 20;
/// The most hits `objects.search` returns; a larger limit is clamped to this.
pub const SEARCH_MAX_LIMIT: u64 = 200;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `objects.search`: what to look for, where, how much.
pub struct SearchInput {
    /// Case-insensitive substring, matched against identity, title, description and content.
    pub query: String,
    #[serde(default)]
    /// Only objects of this kind; a kind the repository does not have is an invalid input.
    pub kind: Option<String>,
    /// At most this many hits (default 20, at most 200).
    #[serde(default)]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<u64>,
}

impl BenchmarkCases for SearchInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "common-word",
                SearchInput {
                    query: "the".into(),
                    kind: None,
                    limit: None,
                },
            ),
            NamedCase::new(
                "no-hit",
                SearchInput {
                    query: "zqx-nothing-carries-this".into(),
                    kind: None,
                    limit: Some(5),
                },
            ),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// One search hit: the object, summarised, and the first matching line of its content.
pub struct SearchHit {
    #[serde(flatten)]
    /// The object that matched.
    pub object: ObjectSummary,
    /// The first line of content that matched, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// The answer of `objects.search`.
pub struct SearchResult {
    /// The query as given.
    pub query: String,
    /// How many hits were returned.
    pub count: usize,
    /// The limit that applied.
    pub limit: u64,
    /// The hits, in URI order, at most `limit` of them.
    pub hits: Vec<SearchHit>,
}

fn objects_search(ctx: &Context, input: SearchInput) -> Result<SearchResult, CapabilityError> {
    if input.query.trim().is_empty() {
        return Err(CapabilityError::Refused(
            "argument 'query' is required and must not be blank".into(),
        ));
    }
    known_kind(ctx, input.kind.as_deref())?;
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

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "objects",
        title: "Objects",
        description: "The declarative objects of the repository's AI layer: rules, prompts, profiles, policy, documents, milestones, issues, claims, and whatever kinds the repository adds; listed, read by URI, and searched.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
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
                cache: CachePolicy::Process { max_entries: 64, ttl_seconds: None },
                handler: objects_search,
            },
        ],
    }
}
