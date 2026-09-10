//! The `objects` module: the declarative objects of the repository's layer, listed, read
//! and searched, and the one resolution of a `majordomus://` URI that `objects.get` and
//! the MCP resource read share.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, Capability, CapabilityKind, Exposure, McpResource, Provenance, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::model::Object;
use crate::{capability, module};

use super::repository::REPOSITORY_URI;
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

// ---------------------------------------------------------------- resolution

/// What a `majordomus://` URI names, resolved through the registry. This is the one rule
/// `objects.get` and the MCP `resources/read` share, so that the tool, the HTTP route and
/// the resource cannot answer one URI differently.
#[derive(Debug)]
pub enum Resolved<'a> {
    /// A declarative object of the layer, as the index holds it.
    Object(&'a Object),
    /// A query with a resource exposure ([`REPOSITORY_URI`]), executed through the
    /// executor like any other call.
    Answer {
        /// The capability that answered.
        capability: &'a Capability,
        /// The resource exposure the URI matched.
        resource: &'a McpResource,
        /// The answer, as the capability's output schema describes it.
        value: Value,
    },
}

impl Resolved<'_> {
    /// IANA media type of [`Resolved::text`]: the object's, or `application/json`.
    pub fn media_type(&self) -> String {
        match self {
            Resolved::Object(o) => o.media_type.to_string(),
            Resolved::Answer { .. } => "application/json".to_string(),
        }
    }

    /// The content as `resources/read` returns it: the file as read, or the answer
    /// pretty-printed.
    pub fn text(&self) -> String {
        match self {
            Resolved::Object(o) => o.content.clone(),
            Resolved::Answer { value, .. } => pretty_json(value),
        }
    }
}

/// A JSON value as the text a resource read returns.
fn pretty_json(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// Resolve a URI: the capability the registry projects at it, then the object behind a
/// resource or the answer of a query. An unknown URI is `NotFound`; a registry naming an
/// object the index does not hold is `Internal`; a query that refuses passes its refusal
/// through, and any other failure of the query is `Internal`, naming the capability.
///
/// ```
/// use majordomus_cli::capability::builtin::{resolve, Resolved, REPOSITORY_URI};
/// use majordomus_cli::capability::{builtin, CapabilityError, CapabilityRegistry, Context};
/// use majordomus_cli::git::GitState;
/// use majordomus_cli::index::{Index, RepositoryInfo, State};
/// use std::sync::Arc;
/// // an index with no objects: only the repository report answers
/// let index = Index {
///     repository: RepositoryInfo {
///         root: "/tmp/doc".into(), layer_schema: "ai-repository/v1".into(),
///         sections: Default::default(), git: GitState::Unavailable { reason: "doc".into() },
///         discovery: "filesystem".into(), source_classes: vec![], kind_sources: vec![],
///         scope_origin: majordomus_cli::scope::Origin::Distribution, scope_path: String::new(),
///     },
///     objects: vec![], diagnostics: vec![], state: State::Ok, fingerprint: String::new(),
///     scoped: Default::default(), distribution: None, providers: Default::default(),
///     share: None,
/// };
/// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).with_index(&index).build().unwrap();
/// let ctx = Context::new(Arc::new(index), Arc::new(registry));
/// match resolve(&ctx, REPOSITORY_URI).unwrap() {
///     Resolved::Answer { capability, resource, value } => {
///         assert_eq!(capability.id.as_str(), "repository.info");
///         assert_eq!(resource.name, "repository");
///         assert_eq!(value["objects"], 0);
///     }
///     other => panic!("{other:?}"),
/// }
/// assert!(matches!(resolve(&ctx, "majordomus://rule/none@1"), Err(CapabilityError::NotFound(_))));
/// ```
pub fn resolve<'a>(ctx: &'a Context, uri: &str) -> Result<Resolved<'a>, CapabilityError> {
    let (capability, resource) = ctx
        .registry
        .by_mcp_uri(uri)
        .ok_or_else(|| CapabilityError::NotFound(format!("unknown resource: {uri}")))?;
    if !capability.kind.is_executable() {
        return ctx.index.get(uri).map(Resolved::Object).ok_or_else(|| {
            CapabilityError::Internal(format!(
                "{uri}: the registry names {} and the index holds no such object",
                capability.id
            ))
        });
    }
    let value = ctx
        .execute(capability.id.as_str(), json!({}))
        .map_err(|e| match e {
            CapabilityError::Refused(reason) => CapabilityError::Refused(reason),
            other => CapabilityError::Internal(format!(
                "{uri}: {} did not answer: {other}",
                capability.id
            )),
        })?;
    Ok(Resolved::Answer {
        capability,
        resource,
        value,
    })
}

// ---------------------------------------------------------------- objects.get

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `objects.get`: which object.
pub struct GetInput {
    /// `majordomus://<kind>/<identity>`, or a URI a query projects
    /// (`majordomus://repository`).
    pub uri: String,
}

impl BenchmarkCases for GetInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let mut cases: Vec<NamedCase<Self>> = ctx
            .index
            .objects
            .first()
            .map(|o| {
                vec![NamedCase::new(
                    "first-object",
                    GetInput { uri: o.uri.clone() },
                )]
            })
            .unwrap_or_default();
        cases.push(NamedCase::new(
            "repository",
            GetInput {
                uri: REPOSITORY_URI.into(),
            },
        ));
        cases
    }
}

/// A URI a query projects (`majordomus://repository`), answered: the same fields a client
/// reads on an [`ObjectView`] where they apply, the answer itself as data, and the text
/// `resources/read` returns for the URI.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnswerView {
    /// The URI as given.
    pub uri: String,
    /// The capability that answered (`repository.info`).
    pub id: String,
    /// Its kind: `query`.
    pub kind: CapabilityKind,
    /// The resource name a client lists (`repository`).
    pub identity: String,
    /// The capability's title.
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The capability's description, when it has one.
    pub description: Option<String>,
    /// The answer, as the capability's output schema describes it (`capabilities.describe`
    /// carries that schema).
    pub answer: Value,
    /// Where the capability comes from: the Rust module it is composed in.
    pub provenance: Provenance,
    /// `application/json`.
    pub media_type: String,
    /// The answer as text: byte for byte what `resources/read` returns for the URI.
    pub content: String,
}

/// The answer of `objects.get`: what the URI resolved to, tagged by `source` the way a
/// capability's provenance is.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum ResourceView {
    /// A file of the layer, read as it is.
    Declarative(ObjectView),
    /// A query with a resource exposure, executed and rendered as a JSON document.
    Builtin(AnswerView),
}

fn objects_get(ctx: &Context, input: GetInput) -> Result<ResourceView, CapabilityError> {
    let resolved = resolve(ctx, &input.uri)?;
    let (media_type, content) = (resolved.media_type(), resolved.text());
    Ok(match resolved {
        Resolved::Object(o) => ResourceView::Declarative(ObjectView::of(o)),
        Resolved::Answer {
            capability,
            resource,
            value,
        } => ResourceView::Builtin(AnswerView {
            uri: input.uri,
            id: capability.id.to_string(),
            kind: capability.kind,
            identity: resource.name.clone(),
            title: capability.title.clone(),
            description: (!capability.description.is_empty())
                .then(|| capability.description.clone()),
            answer: value,
            provenance: capability.provenance.clone(),
            media_type,
            content,
        }),
    })
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

// ---------------------------------------------------------------- objects.verify

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// The input of `objects.verify`.
pub struct VerifyInput {
    /// Only objects of this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// How many objects to read at most; every one of them when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl BenchmarkCases for VerifyInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // a bounded case: this reads files, and a benchmark of it should measure the
        // reading rather than the size of whichever repository it happens to run in
        vec![NamedCase::new(
            "bounded",
            VerifyInput {
                kind: None,
                limit: Some(25),
            },
        )]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// How one file stands against what the index read from it.
pub enum ObjectStanding {
    /// It is what the index read.
    Current,
    /// It is there and it has changed since the index was built.
    Changed,
    /// The file the index read is no longer there.
    Missing,
    /// It is there and could not be read.
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// How closely a file could be compared with what the index holds.
///
/// A file that is one object is compared byte for byte, because the index kept its whole
/// content. A collection file holds one object per member, and what the index kept for each
/// is that member as JSON rather than the file's text — so the strongest thing that can be
/// said without re-parsing it is whether its size is what it was. The report says which
/// comparison was made rather than implying the stronger one.
pub enum Comparison {
    /// Byte for byte against the content the index holds.
    Content,
    /// By size against the size the index recorded.
    Size,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One file of the layer that is no longer what the index read.
pub struct DriftedObject {
    /// Its repository-relative path.
    pub path: String,
    /// How it stands.
    pub standing: ObjectStanding,
    /// How it was compared.
    pub comparison: Comparison,
    /// How many objects of the index came from it.
    pub objects: usize,
    /// One of the URIs it holds, so a reader can see what is affected.
    pub example_uri: String,
    /// What is wrong, for a person.
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The answer of `objects.verify`.
pub struct VerifyReport {
    /// How many files were read.
    pub files: usize,
    /// How many of them are what the index read.
    pub current: usize,
    /// How many are not, of any kind of not.
    pub drifted: usize,
    /// How many objects of the index those files carry.
    pub objects: usize,
    /// How many were compared byte for byte rather than by size.
    pub compared_by_content: usize,
    /// The index fingerprint this process is serving.
    pub fingerprint: String,
    /// Whether the index is still a true picture of the working tree, as far as this
    /// comparison can tell.
    pub index_is_current: bool,
    /// The files that are not, with what is wrong with each.
    pub findings: Vec<DriftedObject>,
}

/// Read every file the layer was built from and compare it with what this process is
/// serving.
///
/// The index is built once, when the process starts, and kept: that is the contract, and
/// it is what makes every request cost nothing. The price of it is that a file edited
/// afterwards is served as it was, and until now nothing could say so without restarting
/// the server.
///
/// The unit of work is the file, not the object, because a collection file holds many
/// objects and is read once. Each file's [`Comparison`] says how strongly it could be
/// checked.
fn objects_verify(ctx: &Context, input: VerifyInput) -> Result<VerifyReport, CapabilityError> {
    if let Some(kind) = &input.kind {
        if !ctx.index.objects.iter().any(|o| &o.kind == kind) {
            return Err(CapabilityError::InvalidInput(format!(
                "no objects of kind '{kind}'; the kinds are listed by objects.list"
            )));
        }
    }
    // one entry per file, in the index's order, with what the index knows about it
    let mut files: Vec<FileSubject> = Vec::new();
    let mut seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for object in ctx
        .index
        .objects
        .iter()
        .filter(|o| input.kind.as_ref().is_none_or(|k| &o.kind == k))
    {
        let path = object.provenance.path.as_str();
        match seen.get(path) {
            Some(at) => files[*at].objects += 1,
            None => {
                seen.insert(path, files.len());
                files.push(FileSubject {
                    path: path.to_string(),
                    bytes: object.provenance.bytes,
                    objects: 1,
                    example_uri: object.uri.clone(),
                    // a member's content is the member as JSON, not the file's text: only a
                    // file that is one whole object can be compared byte for byte
                    content: object
                        .provenance
                        .member
                        .is_none()
                        .then(|| object.content.clone()),
                });
            }
        }
    }
    let subjects: Vec<FileSubject> = files
        .into_iter()
        .take(input.limit.unwrap_or(usize::MAX))
        .collect();

    let root = std::path::Path::new(&ctx.index.repository.root);
    let total = subjects.len() as u64;
    let p = &ctx.progress;
    p.step("read", &format!("Reading {total} file(s) of the layer"));

    let mut findings = Vec::new();
    let mut current = 0usize;
    let mut objects = 0usize;
    let mut by_content = 0usize;
    for (n, subject) in subjects.iter().enumerate() {
        if p.cancelled() {
            p.step_done(
                "read",
                false,
                Some(format!("cancelled after {n} of {total}")),
            );
            return Err(p.cancellation());
        }
        objects += subject.objects;
        let comparison = if subject.content.is_some() {
            by_content += 1;
            Comparison::Content
        } else {
            Comparison::Size
        };
        let standing = match std::fs::read_to_string(root.join(&subject.path)) {
            Ok(text) => match &subject.content {
                Some(known) if &text == known => ObjectStanding::Current,
                Some(_) => ObjectStanding::Changed,
                None if text.len() as u64 == subject.bytes => ObjectStanding::Current,
                None => ObjectStanding::Changed,
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ObjectStanding::Missing,
            Err(_) => ObjectStanding::Unreadable,
        };
        match standing {
            ObjectStanding::Current => current += 1,
            other => {
                let detail = match other {
                    ObjectStanding::Changed => match comparison {
                        Comparison::Content => {
                            "the file has changed since this process built its index".to_string()
                        }
                        Comparison::Size => format!(
                            "the file's size is no longer the {} bytes this process read",
                            subject.bytes
                        ),
                    },
                    ObjectStanding::Missing => "the file is gone".to_string(),
                    _ => "the file could not be read".to_string(),
                };
                p.diagnostic(crate::execution::ExecutionDiagnostic {
                    severity: crate::model::Severity::Warning,
                    code: format!("layer_file_{}", enum_word(&other)),
                    summary: format!("{}: {detail}", subject.path),
                    detail: Some(format!(
                        "{} object(s) of the index came from it, such as {}",
                        subject.objects, subject.example_uri
                    )),
                    suggestion: Some("restart this server to serve the layer as it is now".into()),
                });
                findings.push(DriftedObject {
                    path: subject.path.clone(),
                    standing: other,
                    comparison,
                    objects: subject.objects,
                    example_uri: subject.example_uri.clone(),
                    detail,
                });
            }
        }
        p.progress(
            n as u64 + 1,
            Some(total),
            format!("{} file(s) read; last {}", n + 1, subject.path),
        );
    }
    p.step_done(
        "read",
        findings.is_empty(),
        Some(format!("{current} current, {} drifted", findings.len())),
    );
    Ok(VerifyReport {
        files: subjects.len(),
        current,
        drifted: findings.len(),
        objects,
        compared_by_content: by_content,
        fingerprint: ctx.index.fingerprint.clone(),
        index_is_current: findings.is_empty(),
        findings,
    })
}

/// One file of the layer, and what the index knows about it.
struct FileSubject {
    path: String,
    bytes: u64,
    objects: usize,
    example_uri: String,
    /// The file's whole text, when the index kept it: a file that is one object.
    content: Option<String>,
}

/// The serialised word of a small enum, for a diagnostic code.
fn enum_word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
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
                description: "One object by URI (majordomus://<kind>/<identity>): metadata, provenance and content; a URI a query projects (majordomus://repository) answers that query as a JSON document. The same resolution serves the MCP resource read.",
                input: GetInput,
                output: ResourceView,
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
            capability! {
                id: "objects.verify",
                title: "Verify the index against the working tree",
                description: "Read every file the layer was built from and compare it with what this process is serving. The index is built once at start-up and kept, which is what makes every other request cost nothing and what makes a file edited afterwards be served as it was; this is how a running server says whether that has happened, without being restarted to find out. A file that is one object is compared byte for byte; a collection file, whose objects the index keeps as members rather than as text, is compared by size, and every finding says which comparison was made. It reads every file of the layer, so it reports its progress file by file and stops when it is asked to.",
                input: VerifyInput,
                output: VerifyReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_verify_objects"), http: get("/api/v1/objects/verify"), cli: None },
                tags: ["objects", "diagnostic"],
                handler: objects_verify,
            }
            .cancellable(),
        ],
    }
}
