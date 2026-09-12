//! The site's registry dataset: what GitHub Pages renders about the executable, derived
//! from the registry, the index, the command line, the benchmark projection and the
//! accepted baselines, and nothing else. The site has no content model of its own for
//! any of it; its templates read `site/data/registry/registry.json` and lay it out, and
//! the shell site generator reads `docs/generated/registry.json` for the ids it turns into
//! routes.
//!
//! Deterministic: same repository tree and same executable, same bytes. No timestamps of
//! its own (a baseline's `finished_at` is observed evidence, copied as committed), no
//! absolute paths, no git state (the site's `source.json` carries the commit; it is
//! written at build time by the site generator and is not compared by `generate --check`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::bench::baseline::{self, Policy};
use crate::bench::results::{CacheMode, Provenance as RunProvenance};
use crate::bench::{BenchmarkProjection, Coverage, ResultDocument, SystemTarget, TargetKind};
use crate::capability::registry::{ModuleSource, Summary};
use crate::capability::{Capability, CapabilityKind, Context, Provenance, Stability};
use crate::cli::CommandDoc;
use crate::error::{Error, Result};
use crate::http::openapi;
use crate::index::Index;
use crate::metadata::KindSchema;
use crate::policy::LoadedPolicy;
use crate::repository::Repository;

/// The dataset's own format version, distinct from the crate's.
pub const SCHEMA: &str = "majordomus-site-registry/v2";

/// The whole dataset.
#[derive(Debug, Clone, Serialize)]
pub struct SiteRegistry {
    /// [`SCHEMA`].
    pub schema: &'static str,
    /// That it is generated, in the words every other generated document uses: this file
    /// is a cache of the registry and the index, and editing it is editing a cache.
    pub generated: String,
    /// Who wrote it.
    pub generator: Generator,
    /// The capability registry, fingerprinted and counted, with the builtin entries in full.
    pub registry: RegistryView,
    /// The index of the layer: fingerprint, counts and every object.
    pub index: IndexView,
    /// The kinds the executable reads, with the schema each is validated against.
    pub kinds: Vec<KindView>,
    /// The provider projections the policy declares.
    pub projections: Vec<ProjectionView>,
    /// The command line, as clap declares it.
    pub cli: CommandDoc,
    /// The MCP surface: the tools and the resources the builtin capabilities project.
    pub mcp: McpView,
    /// The HTTP surface: the routes the capabilities project and the projection's own.
    pub http: HttpView,
    /// The benchmark system: targets, coverage, the regression policy, the accepted baselines.
    pub benchmarks: BenchmarksView,
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
    /// The builtin capabilities in full, by id. The declarative ones are the index's
    /// objects, one resource each; listing them twice would say nothing new.
    pub builtin: Vec<CapabilityView>,
    /// The modules, by id: the builtin ones with their capabilities, the declarative ones
    /// (one per kind) with their count.
    pub modules: Vec<ModuleView>,
}

/// One builtin capability: the canonical descriptor and where its source is.
#[derive(Debug, Clone, Serialize)]
pub struct CapabilityView {
    #[serde(flatten)]
    /// The descriptor, every field.
    pub capability: Capability,
    /// Repository-relative path of the Rust file the descriptor was composed in.
    pub source_path: String,
}

/// The view orders as the capability it carries: the source path it adds is provenance.
impl crate::order::Ordered for CapabilityView {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        self.capability.order_key()
    }
}

/// One module.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleView {
    /// The module id.
    pub id: String,
    /// The short name.
    pub title: String,
    /// One paragraph.
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Where the module stands, when it declared it.
    pub stability: Option<Stability>,
    /// `builtin` or `declarative`.
    pub source: String,
    /// Capabilities it composes.
    pub capabilities: usize,
    /// The ids of its builtin capabilities, in registry order; empty for a declarative module.
    pub capability_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Repository-relative path of the Rust file the module's descriptors were composed
    /// in, for a builtin module.
    pub source_path: Option<String>,
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

/// A module is presented by its id, the same string its capabilities are prefixed with.
impl crate::order::Ordered for ModuleView {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.id, &self.id)
    }
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

/// A kind is presented by its name, which is the identity the whole index files objects
/// under; there is nothing else about a kind that a reader would look it up by.
impl crate::order::Ordered for KindView {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.name, &self.name)
    }
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

/// The MCP surface as the registry projects it.
#[derive(Debug, Clone, Serialize)]
pub struct McpView {
    /// The server name `initialize` answers with.
    pub server_name: &'static str,
    /// The protocol versions accepted, newest first.
    pub protocol_versions: &'static [&'static str],
    /// The tools, in registry order.
    pub tools: Vec<ToolView>,
    /// The resources builtin capabilities answer (`majordomus://repository`, ...), in
    /// registry order. Every object of the layer is one more resource; those are the
    /// index's objects.
    pub resources: Vec<ResourceView>,
    /// The URI shape of a declarative object's resource.
    pub resource_template: &'static str,
}

/// One MCP tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolView {
    /// The tool name a client calls.
    pub name: String,
    /// The canonical id the tool projects.
    pub id: String,
    /// The title.
    pub title: String,
    /// The description.
    pub description: String,
    /// `query`, `command` or `resource`.
    pub kind: CapabilityKind,
    /// Leaves the process as it found it.
    pub read_only: bool,
    /// The input type's name.
    pub input: String,
    /// The output type's name.
    pub output: String,
}

/// One MCP resource a builtin capability answers.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceView {
    /// The URI a client reads.
    pub uri: String,
    /// The short name.
    pub name: String,
    /// The canonical id.
    pub id: String,
    /// The title.
    pub title: String,
    /// `query`, `command` or `resource`.
    pub kind: CapabilityKind,
}

/// The HTTP surface as the registry projects it.
#[derive(Debug, Clone, Serialize)]
pub struct HttpView {
    /// The capability routes, in registry order.
    pub routes: Vec<RouteView>,
    /// The projection's own routes, not capabilities: each with what it is and whether a
    /// publication can carry it, so a page can tell a link from a promise.
    pub infrastructure: Vec<crate::web::ProjectionRoute>,
    /// Where the OpenAPI document is committed, repository-relative.
    pub openapi_path: String,
}

/// One HTTP route.
#[derive(Debug, Clone, Serialize)]
pub struct RouteView {
    /// `GET` or `POST`.
    pub method: String,
    /// The path.
    pub path: String,
    /// The canonical id, the document's `operationId`.
    pub id: String,
    /// `query`, `command` or `resource`.
    pub kind: CapabilityKind,
}

/// The benchmark system as the site shows it.
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarksView {
    /// Every target of this repository, by key: what `majordomus bench` times.
    pub targets: Vec<TargetView>,
    /// The coverage: every requirement and where it stands, with the tallies.
    pub coverage: Coverage,
    /// The system targets: the transports' own operations.
    pub system_targets: Vec<SystemTargetView>,
    /// The regression policy `bench --check` applies, from
    /// `.ai/repo/benchmarks/rust/policy.yaml` or the default.
    pub policy: Policy,
    /// Where the policy lives, repository-relative.
    pub policy_path: String,
    /// The accepted baselines, one per platform, as committed.
    pub baselines: Vec<BaselineView>,
}

/// One benchmark target.
#[derive(Debug, Clone, Serialize)]
pub struct TargetView {
    /// `<id>|<transport>|<case>` or `system.<transport>.<name>`.
    pub key: String,
    /// `capability` or `system`.
    pub kind: &'static str,
    /// The transport.
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The capability id, for a capability target.
    pub id: Option<String>,
    /// The module, or `system`.
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The case name, for a capability target.
    pub case: Option<String>,
    /// Measured cold and warm as well as uncached.
    pub cached: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The MCP tool name, on the MCP transport.
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `METHOD /path`, on the HTTP transport.
    pub route: Option<String>,
}

/// One system target.
#[derive(Debug, Clone, Serialize)]
pub struct SystemTargetView {
    /// The key.
    pub key: String,
    /// The transport.
    pub transport: String,
    /// What it measures.
    pub description: String,
}

/// One accepted baseline.
#[derive(Debug, Clone, Serialize)]
pub struct BaselineView {
    /// `<os>-<arch>-<build>`, from the file name.
    pub platform: String,
    /// Repository-relative path of the file.
    pub path: String,
    /// The profile the run used.
    pub profile: String,
    /// When the run finished, RFC 3339 UTC; observed evidence, copied as committed.
    pub finished_at: String,
    /// What measured: commit, build profile, OS, architecture, host, executable version,
    /// registry fingerprint.
    pub provenance: RunProvenance,
    /// Every measurement.
    pub results: Vec<MeasurementView>,
}

/// One measurement of a baseline.
#[derive(Debug, Clone, Serialize)]
pub struct MeasurementView {
    /// The target's key.
    pub key: String,
    /// `uncached`, `cold`, `warm`, or `n/a` for a system target.
    pub cache_mode: String,
    /// How many samples.
    pub samples: usize,
    /// The median, microseconds.
    pub p50_us: f64,
    /// The 95th percentile, microseconds.
    pub p95_us: f64,
    /// The 99th percentile, microseconds.
    pub p99_us: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Handler invocations during the samples, when known.
    pub handler_invocations: Option<u64>,
}

/// Build the dataset.
pub fn dataset(
    ctx: &Context,
    schema: &KindSchema,
    policy: &LoadedPolicy,
    repo: &Repository,
) -> Result<SiteRegistry> {
    let registry = &ctx.registry;
    let index: &Index = &ctx.index;
    let mut builtin: Vec<CapabilityView> = registry
        .iter()
        .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
        .map(|c| CapabilityView {
            capability: c.clone(),
            source_path: c.provenance.source_path(),
        })
        .collect();
    crate::order::canonical(&mut builtin);
    let mut modules: Vec<ModuleView> = registry
        .modules()
        .map(|m| {
            let mine: Vec<&Capability> = registry
                .iter()
                .filter(|c| {
                    c.module.as_str() == m.id.as_str()
                        && matches!(c.provenance, Provenance::Builtin { .. })
                })
                .collect();
            ModuleView {
                id: m.id.to_string(),
                title: m.title.clone(),
                description: m.description.clone(),
                stability: m.stability,
                source: enum_name(m.source),
                capabilities: registry
                    .iter()
                    .filter(|c| c.module.as_str() == m.id.as_str())
                    .count(),
                capability_ids: mine.iter().map(|c| c.id.to_string()).collect(),
                source_path: (m.source != ModuleSource::Declarative)
                    .then(|| mine.first().map(|c| c.provenance.source_path()))
                    .flatten(),
            }
        })
        .collect();
    crate::order::canonical(&mut modules);

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
    crate::order::canonical(&mut kinds);

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

    let mcp = McpView {
        server_name: crate::mcp::protocol::SERVER_NAME,
        protocol_versions: crate::mcp::protocol::PROTOCOL_VERSIONS,
        tools: registry
            .iter()
            .filter_map(|c| {
                let name = c.exposure.mcp.as_ref()?.tool.clone()?;
                Some(ToolView {
                    name,
                    id: c.id.to_string(),
                    title: c.title.clone(),
                    description: c.description.clone(),
                    kind: c.kind,
                    read_only: c.kind.is_read_only(),
                    input: c.input.name.clone().unwrap_or_else(|| "object".into()),
                    output: c.output.name.clone().unwrap_or_else(|| "object".into()),
                })
            })
            .collect(),
        resources: registry
            .iter()
            .filter(|c| matches!(c.provenance, Provenance::Builtin { .. }))
            .filter_map(|c| {
                let r = c.exposure.mcp.as_ref()?.resource.as_ref()?;
                Some(ResourceView {
                    uri: r.uri.clone(),
                    name: r.name.clone(),
                    id: c.id.to_string(),
                    title: c.title.clone(),
                    kind: c.kind,
                })
            })
            .collect(),
        resource_template: "majordomus://<kind>/<identity>",
    };

    let http = HttpView {
        routes: registry
            .iter()
            .filter_map(|c| {
                let h = c.exposure.http.as_ref()?;
                Some(RouteView {
                    method: h.method.as_str().to_string(),
                    path: h.path.clone(),
                    id: c.id.to_string(),
                    kind: c.kind,
                })
            })
            .collect(),
        infrastructure: openapi::infrastructure_routes(),
        openapi_path: format!("{}/openapi.json", crate::generate::OUT_DIR),
    };

    let projection = BenchmarkProjection::from_context(ctx);
    let coverage = Coverage::compute(ctx, &projection);
    let targets = projection
        .targets
        .iter()
        .map(|t| match &t.kind {
            TargetKind::Capability {
                id,
                module,
                transport,
                case,
                cache,
                tool,
                route,
                ..
            } => TargetView {
                key: t.key.clone(),
                kind: "capability",
                transport: transport.name().to_string(),
                id: Some(id.clone()),
                module: module.clone(),
                case: Some(case.clone()),
                cached: !matches!(cache, crate::capability::CachePolicy::Disabled),
                tool: tool.clone(),
                route: route.as_ref().map(|(m, p)| format!("{m} {p}")),
            },
            TargetKind::System { target } => TargetView {
                key: t.key.clone(),
                kind: "system",
                transport: target.transport().name().to_string(),
                id: None,
                module: "system".into(),
                case: None,
                cached: false,
                tool: None,
                route: None,
            },
        })
        .collect();
    let policy_path = baseline::policy_path(repo);
    let policy_rel = relative(repo.root(), &policy_path);
    let bench_policy = Policy::load(repo)?;
    let baselines = load_baselines(repo, &policy_path)?;
    let benchmarks = BenchmarksView {
        targets,
        coverage,
        system_targets: SystemTarget::ALL
            .iter()
            .map(|s| SystemTargetView {
                key: s.key().to_string(),
                transport: s.transport().name().to_string(),
                description: s.description().to_string(),
            })
            .collect(),
        policy: bench_policy,
        policy_path: policy_rel,
        baselines,
    };

    Ok(SiteRegistry {
        schema: SCHEMA,
        generated: crate::generate::json_banner(
            "the capability registry and the index of this repository's layer",
        ),
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
        cli: crate::cli::tree(),
        mcp,
        http,
        benchmarks,
    })
}

/// Every `baseline.<platform>.json` beside the policy, by platform, as committed. A
/// baseline that does not parse is an error: a stale evidence file is not silently
/// dropped from the page that presents it.
fn load_baselines(repo: &Repository, policy_path: &Path) -> Result<Vec<BaselineView>> {
    let Some(dir) = policy_path.parent() else {
        return Ok(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.starts_with("baseline.") && n.ends_with(".json"))
        .collect();
    crate::order::canonical(&mut names);
    let mut out = Vec::new();
    for name in names {
        let path = dir.join(&name);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let doc: ResultDocument =
            serde_json::from_str(&text).map_err(|e| Error::InvalidManifest {
                path: path.clone(),
                reason: format!("not a benchmark result document: {e}"),
            })?;
        let platform = name
            .trim_start_matches("baseline.")
            .trim_end_matches(".json")
            .to_string();
        out.push(BaselineView {
            platform,
            path: relative(repo.root(), &path),
            profile: doc.profile.clone(),
            finished_at: doc.finished_at.clone(),
            provenance: doc.provenance.clone(),
            results: doc
                .results
                .iter()
                .map(|r| MeasurementView {
                    key: r.key.clone(),
                    cache_mode: match r.cache_mode {
                        CacheMode::Uncached => "uncached",
                        CacheMode::Cold => "cold",
                        CacheMode::Warm => "warm",
                        CacheMode::NotApplicable => "n/a",
                    }
                    .to_string(),
                    samples: r.stats.samples,
                    p50_us: r.stats.p50_us,
                    p95_us: r.stats.p95_us,
                    p99_us: r.stats.p99_us,
                    handler_invocations: r.handler_invocations,
                })
                .collect(),
        });
    }
    Ok(out)
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn enum_name<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The dataset as the committed file: pretty JSON, trailing newline.
pub fn render(dataset: &SiteRegistry) -> String {
    let mut s = serde_json::to_string_pretty(dataset).unwrap_or_default();
    s.push('\n');
    s
}

// ---------------------------------------------------------------- the Why catalogue

/// The dataset's own format version.
pub const WHY_SCHEMA: &str = "majordomus-site-why/v1";

/// Where `why.json` says it came from.
pub const WHY_SOURCE: &str =
    "the operational moments, audiences and areas of this repository's layer";

/// Where `why-graph.json` says it came from.
pub const GRAPH_SOURCE: &str = "the moments and what answers them, as the derived `why` graph";

/// The Why catalogue and its graph, as the site's templates read them:
/// `site/data/registry/why.json` and `site/data/registry/why-graph.json`.
///
/// Both are projections of [`crate::why::Catalogue`], which the context already built.
/// Nothing here re-reads a file, and nothing here is a second opinion about what a moment
/// says: the site generator writes one page per entry from this document, and the
/// templates read it for every listing, filter, count, backlink and questionnaire.
///
/// The bodies are not in it: a page's prose is read from the file the record names in
/// `source`, so the dataset stays the metadata every listing, filter, count and backlink
/// needs and never becomes a second copy of the writing.
///
/// Deterministic and index-independent: the payload depends on the catalogue's own
/// sources and on nothing else, so `scripts/derive` — which runs `generate`, then the
/// site generator, then `generate` again over the tree that left — produces the same
/// bytes in both passes.
pub fn why_artifacts(ctx: &Context) -> Result<Vec<crate::generate::Artifact>> {
    let by_cli = |path: &[&str]| -> Option<String> {
        ctx.registry
            .by_cli(&path.iter().map(|w| w.to_string()).collect::<Vec<_>>())
            .map(|c| c.id.to_string())
    };
    let run = |path: &[&str], input: serde_json::Value| -> Result<serde_json::Value> {
        let id = by_cli(path).ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
        ctx.execute(&id, input).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })
    };

    // the catalogue, then one detail per moment: the site needs the derived relations and
    // the body, and asking the capability for them is what keeps this from being a second
    // reading of the same files
    let catalogue = run(&["why", "list"], serde_json::json!({ "status": "any" }))?;
    let mut moments = Vec::new();
    for m in catalogue["moments"].as_array().into_iter().flatten() {
        let id = m["id"].as_str().unwrap_or_default();
        let mut detail = run(&["why", "show"], serde_json::json!({ "id": id }))?;
        // The prose stays where it was authored. Every page the site generator writes
        // takes the body from the file this record names in `source`, so the dataset is
        // the metadata a template reads and not a second copy of the writing.
        if let Some(o) = detail.as_object_mut() {
            o.remove("body");
        }
        moments.push(detail);
    }
    let validation = run(&["why", "validate"], serde_json::json!({}))?;

    // the same for the two taxonomies: their pages take the prose from their own files
    let strip = |v: &serde_json::Value| -> Vec<serde_json::Value> {
        v.as_array()
            .into_iter()
            .flatten()
            .map(|e| {
                let mut e = e.clone();
                if let Some(o) = e.as_object_mut() {
                    o.remove("body");
                }
                e
            })
            .collect()
    };
    let audiences = strip(&catalogue["audiences"]);
    let areas = strip(&catalogue["areas"]);

    // the questionnaire's index: every signal with the moment that owns it, flat, so a
    // template renders it without joining two lists and a script never learns a mapping
    let mut signals = Vec::new();
    for m in &moments {
        if m["status"] != "stable" {
            continue;
        }
        for s in m["signals"].as_array().into_iter().flatten() {
            signals.push(serde_json::json!({
                "id": s["id"],
                "text": s["text"],
                "moment": m["id"],
            }));
        }
    }

    let document = serde_json::json!({
        "schema": WHY_SCHEMA,
        "generated": crate::generate::json_banner(WHY_SOURCE),
        "generator": { "id": "majordomus-cli", "version": crate::VERSION },
        "fingerprint": catalogue["fingerprint"],
        "route": crate::why::ROUTE,
        "counts": catalogue["counts"],
        "facets": catalogue["facets"],
        "audiences": audiences,
        "areas": areas,
        "signals": signals,
        "moments": moments,
        "valid": validation["valid"],
    });

    let graph =
        crate::graph::derive("why", &ctx.registry, &ctx.index).ok_or_else(|| Error::Protocol {
            reason: "this executable derives no `why` graph".into(),
        })?;
    // the graph is a value of the domain and carries no provenance of its own; the artifact
    // does, in the members every generated document of this repository carries
    let mut graph_document = serde_json::to_value(&graph).unwrap_or_default();
    if let Some(o) = graph_document.as_object_mut() {
        o.insert(
            "generated".into(),
            serde_json::Value::String(crate::generate::json_banner(GRAPH_SOURCE)),
        );
        o.insert(
            "generator".into(),
            serde_json::json!({ "id": "majordomus-cli", "version": crate::VERSION }),
        );
    }

    Ok(vec![
        crate::generate::Artifact::verbatim(
            format!("{}/why.json", crate::generate::SITE_DATA_DIR),
            "site-why",
            crate::generate::ArtifactFormat::Json,
            Some(WHY_SCHEMA.to_string()),
            WHY_SOURCE,
            render_json(&document),
        ),
        crate::generate::Artifact::verbatim(
            format!("{}/why-graph.json", crate::generate::SITE_DATA_DIR),
            "site-why-graph",
            crate::generate::ArtifactFormat::Json,
            None,
            GRAPH_SOURCE,
            render_json(&graph_document),
        ),
    ])
}

fn render_json(v: &serde_json::Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap_or_default();
    s.push('\n');
    s
}

// ---------------------------------------------------------------- the product model

/// The dataset's own format version.
pub const PRODUCT_SCHEMA: &str = "majordomus-site-product/v1";

/// Where `product.json` says it came from.
pub const PRODUCT_SOURCE: &str =
    "the product features of this repository's layer, resolved against the registry, the index, the catalogue and the topology";

/// The fields of one feature the public dataset carries. An allow-list, not an exclusion:
/// a field added to the resolved view reaches the published site only when it is named
/// here, so a value that names a machine or an internal state cannot arrive by accident.
pub const PUBLIC_FEATURE_FIELDS: &[&str] = &[
    "id",
    "title",
    "short_title",
    "headline",
    "summary",
    "status",
    "weight",
    "featured",
    "areas",
    "audiences",
    "modules",
    "commands",
    "kinds",
    "rules",
    "docs",
    "adrs",
    "claims",
    "use_cases",
    "cockpit",
    "web",
    "related",
    "tags",
    "route",
    "source",
    "surfaces",
    "module_refs",
    "command_refs",
    "kind_refs",
    "rule_refs",
    "doc_refs",
    "adr_refs",
    "claim_refs",
    "use_case_refs",
    "cockpit_refs",
    "web_refs",
    "moments",
    "backlinks",
    "counts",
    "evidence",
];

/// The product model as the site's templates read it: `site/data/registry/product.json`.
///
/// A projection of [`crate::product::ProductModel`] through the `product.*` capabilities, so the
/// homepage renders what the API and MCP answer. The bodies are not in it: a feature's
/// page takes its prose from the file the record names in `source`, so the dataset stays
/// the metadata every card, chapter, matrix row and count needs and never a second copy of
/// the writing.
///
/// Every feature is copied field by field through [`PUBLIC_FEATURE_FIELDS`]: the site is
/// public, and what reaches it is what was named, not what was not excluded.
///
/// Index-independent, like the Why dataset: the payload depends on the features, the
/// registry, the catalogue and the topology and never on the index's fingerprint, so both
/// `generate` passes of `scripts/derive` produce the same bytes.
pub fn product_artifacts(ctx: &Context) -> Result<Vec<crate::generate::Artifact>> {
    let by_cli = |path: &[&str]| -> Option<String> {
        ctx.registry
            .by_cli(&path.iter().map(|w| w.to_string()).collect::<Vec<_>>())
            .map(|c| c.id.to_string())
    };
    let run = |path: &[&str], input: serde_json::Value| -> Result<serde_json::Value> {
        let id = by_cli(path).ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
        ctx.execute(&id, input).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })
    };

    let list = run(&["product", "list"], serde_json::json!({ "status": "any" }))?;
    let mut features = Vec::new();
    for f in list["features"].as_array().into_iter().flatten() {
        let id = f["id"].as_str().unwrap_or_default();
        let detail = run(&["product", "show"], serde_json::json!({ "id": id }))?;
        // the allow-list, applied: the body stays in the file, and nothing the view did not
        // name reaches the site
        let mut public = serde_json::Map::new();
        if let Some(o) = detail.as_object() {
            for (k, v) in o {
                if PUBLIC_FEATURE_FIELDS.contains(&k.as_str()) {
                    public.insert(k.clone(), v.clone());
                }
            }
        }
        features.push(serde_json::Value::Object(public));
    }
    let matrix = run(&["product", "matrix"], serde_json::json!({}))?;
    let providers = run(&["product", "providers"], serde_json::json!({}))?;
    let validation = run(&["product", "validate"], serde_json::json!({}))?;

    // the telemetry the homepage shows: every number a fact of the registry and the index,
    // counted here once so that no template counts and no template can be handed a number
    let summary = ctx.registry.summary();
    let by_kind: BTreeMap<String, usize> = ctx
        .index
        .kinds()
        .into_iter()
        .map(|(k, n)| (k.to_string(), n))
        .collect();
    let modules = ctx
        .registry
        .modules()
        .filter(|m| m.source != ModuleSource::Declarative)
        .count();
    let cli_commands = crate::cli::tree()
        .flatten()
        .iter()
        .filter(|c| c.executable)
        .count();
    let telemetry = serde_json::json!({
        "capabilities": summary.total,
        "builtin": summary.builtin,
        "declarative": summary.declarative,
        "modules": modules,
        "mcp_tools": summary.mcp_tools,
        "mcp_resources": summary.mcp_resources,
        "http_routes": summary.http_routes,
        "cli_commands": cli_commands,
        "shell_commands": list["counts"]["commands"],
        "kinds": by_kind.len(),
        "objects": ctx.index.objects.len(),
        "by_kind": by_kind,
        "providers": list["counts"]["providers"],
        "features": list["counts"]["features"],
        "web_surfaces": ctx.web.surfaces.len(),
    });

    // The doctrine the product rests on: every rule a public feature names, once, carrying the
    // class and the enforcement the rule itself declares. Deduplicated here and not in a
    // template, because which rules the product rests on is a fact of the model, and a page
    // that had to work it out would be a page inferring domain structure.
    let mut by_rule: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for f in &features {
        if f["status"].as_str() == Some("draft") {
            continue;
        }
        for r in f["rule_refs"].as_array().into_iter().flatten() {
            if let Some(id) = r["id"].as_str() {
                by_rule.entry(id.to_string()).or_insert_with(|| r.clone());
            }
        }
    }
    let rules: Vec<serde_json::Value> = by_rule.into_values().collect();

    let cockpit_areas: Vec<serde_json::Value> = crate::cockpit::nav::areas()
        .iter()
        .map(|a| serde_json::json!({ "id": a.id, "title": a.label, "route": a.href }))
        .collect();

    let document = serde_json::json!({
        "schema": PRODUCT_SCHEMA,
        "generated": crate::generate::json_banner(PRODUCT_SOURCE),
        "generator": { "id": "majordomus-cli", "version": crate::VERSION },
        "fingerprint": list["fingerprint"],
        "route": crate::product::ROUTE,
        "counts": list["counts"],
        "surfaces": list["surfaces"],
        "features": features,
        "matrix": { "rows": matrix["rows"], "modules": matrix["modules"], "commands": matrix["commands"], "kinds": matrix["kinds"] },
        "providers": providers["providers"],
        "rules": rules,
        "cockpit_areas": cockpit_areas,
        "telemetry": telemetry,
        "valid": validation["valid"],
    });

    let graph = crate::graph::derive("product", &ctx.registry, &ctx.index).ok_or_else(|| {
        Error::Protocol {
            reason: "this executable derives no `product` graph".into(),
        }
    })?;
    let mut graph_document = serde_json::to_value(&graph).unwrap_or_default();
    if let Some(o) = graph_document.as_object_mut() {
        o.insert(
            "generated".into(),
            serde_json::Value::String(crate::generate::json_banner(
                "the features, what they are made of, and the interfaces that follow, as the derived `product` graph",
            )),
        );
        o.insert(
            "generator".into(),
            serde_json::json!({ "id": "majordomus-cli", "version": crate::VERSION }),
        );
    }

    Ok(vec![
        crate::generate::Artifact::verbatim(
            format!("{}/product.json", crate::generate::SITE_DATA_DIR),
            "site-product",
            crate::generate::ArtifactFormat::Json,
            Some(PRODUCT_SCHEMA.to_string()),
            PRODUCT_SOURCE,
            render_json(&document),
        ),
        crate::generate::Artifact::verbatim(
            format!("{}/product-graph.json", crate::generate::SITE_DATA_DIR),
            "site-product-graph",
            crate::generate::ArtifactFormat::Json,
            None,
            "the features, what they are made of, and the interfaces that follow, as the derived `product` graph",
            render_json(&graph_document),
        ),
    ])
}
