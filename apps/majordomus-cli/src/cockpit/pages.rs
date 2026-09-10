//! The pages. Each one asks the executor for a capability's output and lays it out; none
//! of them knows anything the registry or the index does not already hold.
//!
//! Every page goes through [`crate::capability::Context::execute`], the same call MCP and
//! the HTTP routes make. That is deliberate and not incidental: the Cockpit is counted in
//! the same perf counters, answered from the same cache, and bound by the same validation
//! as every other caller. A page that reached into the index directly would be a fourth
//! way of reading the repository.

use serde_json::{json, Value};

use crate::capability::builtin::{
    ArtifactReport, Continuity, DirectoryReport, DirectoryState, GraphList, Health, HealthStatus,
    ObjectList, ObjectSummary, Record, RepositoryReport,
};
use crate::capability::{Capability, CapabilityKind, CapabilityRegistry, Context, Provenance};
use crate::generate;
use crate::graph::Graph;
use crate::http::router::percent_encode;
use crate::worktree::{
    BranchState, MigrationPlan, RepositoryTopology, Standing, StepOutcome, TopologyDiagnostic,
    WorktreeState,
};

use super::html::{el, empty, El, Node};
use super::nav::Area;
use super::view::{
    alert, badge, card, card_with, cell, chips, details, facts, id_cell, kind_badge, link, mono,
    nothing, pagination, pre, row, statistic, table, tag, text_cell, Window, PER_PAGE,
};

/// What a page hands back: the area it belongs to, its title and subtitle, its trail, and
/// its content. The shell is added by the router, so no page renders a `<html>`.
pub struct Page {
    /// The area of the navigation.
    pub area: Area,
    /// The heading.
    pub title: String,
    /// One line under the heading.
    pub subtitle: Option<String>,
    /// The trail; the last entry has no href.
    pub breadcrumbs: Vec<(String, Option<String>)>,
    /// The body.
    pub main: El,
    /// The HTTP status: 200, or 404 for a page about something that does not exist.
    pub status: u16,
    /// Script modules this page needs beyond the shell's, by file name under the asset
    /// directory. Lazy by page, which is what keeps the overview free of a graph library.
    pub scripts: Vec<&'static str>,
}

impl Page {
    pub(crate) fn new(area: Area, title: impl Into<String>, main: El) -> Self {
        Page {
            area,
            title: title.into(),
            subtitle: None,
            breadcrumbs: Vec::new(),
            main,
            status: 200,
            scripts: Vec::new(),
        }
    }
    pub(crate) fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }
    pub(crate) fn trail(mut self, trail: Vec<(&str, Option<&str>)>) -> Self {
        self.breadcrumbs = trail
            .into_iter()
            .map(|(l, h)| (l.to_string(), h.map(str::to_string)))
            .collect();
        self
    }
    pub(crate) fn script(mut self, name: &'static str) -> Self {
        self.scripts.push(name);
        self
    }
    pub(crate) fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
}

/// The word a serde enum serialises to (`behaviorally_verified`, `repository`), for a
/// page that shows a variant. `{:?}` would show the Rust spelling, which is not the
/// vocabulary anything else in this repository uses.
fn word<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Ask the executor for a capability's output, typed.
pub(crate) fn ask<T: serde::de::DeserializeOwned>(ctx: &Context, id: &str, input: Value) -> Result<T, String> {
    let value = ctx.execute(id, input).map_err(|e| e.to_string())?;
    serde_json::from_value(value).map_err(|e| format!("{id} answered something unexpected: {e}"))
}

/// A page that says what went wrong instead of showing a blank one.
pub(crate) fn failed(area: Area, title: &str, reason: String) -> Page {
    Page::new(
        area,
        title,
        el("div").child(alert("fail", reason)).child(
            el("p")
                .class("mj-empty")
                .text("The capability behind this page did not answer. Nothing was changed."),
        ),
    )
    .status(500)
}

// ------------------------------------------------------------------- overview

/// The landing page: what this process is serving, and whether it is healthy.
pub fn overview(ctx: &Context) -> Page {
    let report: RepositoryReport = match ask(ctx, "repository.info", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Overview, "Overview", e),
    };
    let health: Health = match ask(ctx, "health.report", json!({})) {
        Ok(h) => h,
        Err(e) => return failed(Area::Overview, "Overview", e),
    };

    let git = match &report.repository.git {
        crate::git::GitState::Available(info) => {
            let head = info.head.as_deref().unwrap_or("(unborn)");
            format!(
                "{} · {} · {}",
                info.branch.as_deref().unwrap_or("(detached)"),
                &head[..12.min(head.len())],
                info.working_tree
            )
        }
        crate::git::GitState::Unavailable { reason } => reason.clone(),
    };

    let statistics = el("div")
        .class("mj-stats")
        .child(statistic(
            report.capabilities.total.to_string(),
            "capabilities",
            "capabilities.list",
        ))
        .child(statistic(
            report.objects.to_string(),
            "objects of the layer",
            "repository.info",
        ))
        .child(statistic(
            report.capabilities.http_routes.to_string(),
            "HTTP routes",
            "the registry's HTTP exposures",
        ))
        .child(statistic(
            report.capabilities.mcp_tools.to_string(),
            "MCP tools",
            "the registry's MCP exposures",
        ))
        .child(statistic(
            report.capabilities.modules.to_string(),
            "modules",
            "compose_modules! and the layer's kinds",
        ))
        .child(statistic(
            report.capabilities.cached.to_string(),
            "cached capabilities",
            "the descriptors' cache policies",
        ));

    let identity = card(
        "This repository",
        facts(vec![
            ("Root", Node::Element(mono(&report.repository.root))),
            (
                "Layer schema",
                Node::Element(mono(&report.repository.layer_schema)),
            ),
            ("Version control", Node::Element(mono(git))),
            (
                "Discovery",
                Node::Element(mono(&report.repository.discovery)),
            ),
            (
                "Scope",
                Node::Element(
                    el("span")
                        .child(mono(&report.repository.scope_path))
                        .text(format!(" ({})", word(&report.repository.scope_origin))),
                ),
            ),
            (
                "Index state",
                Node::Element(match report.state {
                    crate::index::State::Ok => badge("ok", "ok"),
                    crate::index::State::Degraded => badge("fail", "degraded"),
                }),
            ),
            (
                "Executable",
                Node::Element(mono(format!("majordomus {}", crate::VERSION))),
            ),
        ]),
    );

    let health_card = card_with(
        "Health",
        link("/cockpit/health", "Full report"),
        el("div").child(health_badge(health.status)).child(
            el("ul").class("mj-checklist").children(
                health
                    .checks
                    .iter()
                    .map(|c| {
                        el("li")
                            .class("mj-checklist-item")
                            .child(badge(c.status.as_str(), c.status.as_str()))
                            .child(el("span").class("mj-checklist-title").text(&c.title))
                            .child(el("span").class("mj-checklist-detail").text(&c.detail))
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
    );

    let kinds = card(
        "What the layer holds",
        table(
            &["Kind", "Objects", ""],
            report
                .kinds
                .iter()
                .map(|(kind, count)| {
                    row(vec![
                        cell(mono(kind)),
                        text_cell(count.to_string()),
                        cell(link(
                            format!("/cockpit/objects?kind={}", percent_encode(kind)),
                            "browse",
                        )),
                    ])
                })
                .collect(),
        ),
    );

    let diagnostics = if report.diagnostics.is_empty() {
        card(
            "Diagnostics",
            nothing("Every declared file became an object. Nothing to report."),
        )
    } else {
        card(
            "Diagnostics",
            table(
                &["Severity", "Code", "Path", "Message"],
                report
                    .diagnostics
                    .iter()
                    .map(|d| {
                        row(vec![
                            cell(badge(
                                match d.severity {
                                    crate::model::Severity::Error => "fail",
                                    crate::model::Severity::Warning => "warn",
                                    crate::model::Severity::Info => "info",
                                },
                                word(&d.severity),
                            )),
                            cell(mono(&d.code)),
                            cell(mono(d.path.clone().unwrap_or_else(|| "-".into()))),
                            text_cell(&d.message),
                        ])
                    })
                    .collect(),
            ),
        )
    };

    Page::new(
        Area::Overview,
        "Overview",
        el("div")
            .class("mj-grid")
            .child(statistics)
            .child(identity)
            .child(health_card)
            .child(kinds)
            .child(diagnostics),
    )
    .subtitle(crate::about::SUMMARY)
}

fn health_badge(status: HealthStatus) -> El {
    el("p").class("mj-health-line").child(badge(
        status.as_str(),
        match status {
            HealthStatus::Ok => "everything the engines decide is satisfied",
            HealthStatus::Warn => "something is worth looking at",
            HealthStatus::Fail => "an engine is not satisfied",
            HealthStatus::Unknown => "a dimension could not be decided",
        },
    ))
}

// --------------------------------------------------------------- capabilities

/// The listing's own URL with some parameters replaced: what a filter chip, a page link
/// and a cleared filter all are. Paging must never drop a filter and filtering must
/// never keep a page number, so both go through here rather than through a format
/// string at each call site.
pub(crate) fn href_with(base: &str, query: &[(String, String)], set: &[(&str, Option<&str>)]) -> String {
    let overridden = |key: &str| set.iter().any(|(k, _)| *k == key);
    let pairs: Vec<(String, String)> = query
        .iter()
        .filter(|(k, v)| !v.is_empty() && !overridden(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .chain(
            set.iter()
                .filter_map(|(k, v)| v.map(|v| ((*k).to_string(), v.to_string()))),
        )
        .collect();
    if pairs.is_empty() {
        return base.to_string();
    }
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{query}")
}

/// The page a listing was asked for. Anything that is not a page number is page one.
pub(crate) fn asked_page(query: &[(String, String)]) -> usize {
    query
        .iter()
        .find(|(k, _)| k == "page")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(1)
}

/// The capability explorer, filtered by whatever the query string says.
pub fn capabilities(ctx: &Context, query: &[(String, String)]) -> Page {
    let get = |name: &str| {
        query
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    };
    let module = get("module");
    let kind = get("kind");
    let source = get("source");
    let needle = get("q").map(str::to_lowercase);

    let matching: Vec<&Capability> = ctx
        .registry
        .iter()
        .filter(|c| module.is_none_or(|m| c.module.as_str() == m))
        .filter(|c| kind.is_none_or(|k| kind_word(c.kind) == k))
        .filter(|c| {
            source.is_none_or(|s| match &c.provenance {
                Provenance::Builtin { .. } => s == "builtin",
                Provenance::Declarative { .. } => s == "declarative",
            })
        })
        .filter(|c| {
            needle.as_deref().is_none_or(|n| {
                c.id.as_str().to_lowercase().contains(n)
                    || c.title.to_lowercase().contains(n)
                    || c.description.to_lowercase().contains(n)
            })
        })
        .collect();

    let window = Window::new(asked_page(query), PER_PAGE, matching.len());
    let rows: Vec<El> = matching[window.range()]
        .iter()
        .map(|c| {
            row(vec![
                id_cell(
                    format!("/cockpit/capabilities/{}", percent_encode(c.id.as_str())),
                    c.id.as_str(),
                ),
                cell(kind_badge(c.kind)),
                text_cell(&c.title),
                cell(mono(c.module.as_str())),
                cell(projection_marks(c)),
                cell(provenance_badge(c)),
            ])
        })
        .collect();

    // the modules of what matches, as the way into it: nine hundred rows are entered by
    // their module rather than scrolled. The counts are of the other filters in force,
    // which is what makes them a fact about this listing and not about the registry.
    let mut per_module: std::collections::BTreeMap<&str, usize> = Default::default();
    for c in ctx
        .registry
        .iter()
        .filter(|c| kind.is_none_or(|k| kind_word(c.kind) == k))
        .filter(|c| {
            source.is_none_or(|s| match &c.provenance {
                Provenance::Builtin { .. } => s == "builtin",
                Provenance::Declarative { .. } => s == "declarative",
            })
        })
        .filter(|c| {
            needle.as_deref().is_none_or(|n| {
                c.id.as_str().to_lowercase().contains(n)
                    || c.title.to_lowercase().contains(n)
                    || c.description.to_lowercase().contains(n)
            })
        })
    {
        *per_module.entry(c.module.as_str()).or_default() += 1;
    }
    let browse = chips(
        std::iter::once((
            "All modules".to_string(),
            href_with(
                "/cockpit/capabilities",
                query,
                &[("module", None), ("page", None)],
            ),
            per_module.values().sum::<usize>(),
            module.is_none(),
        ))
        .chain(per_module.iter().map(|(m, n)| {
            (
                (*m).to_string(),
                href_with(
                    "/cockpit/capabilities",
                    query,
                    &[("module", Some(m)), ("page", None)],
                ),
                *n,
                module == Some(*m),
            )
        }))
        .collect(),
    );

    let filters = el("form")
        .class("mj-filters")
        .attr("method", "get")
        .attr("action", "/cockpit/capabilities")
        .attr("role", "search")
        .child(
            el("label")
                .class("mj-field")
                .child(el("span").class("mj-field-label").text("Search"))
                .child(
                    el("input")
                        .class("mj-input")
                        .attr("type", "search")
                        .attr("name", "q")
                        .attr("value", needle.clone().unwrap_or_default())
                        .attr("placeholder", "id, title or description"),
                ),
        )
        .child(select(
            "module",
            "Module",
            module,
            ctx.registry
                .modules()
                .map(|m| (m.id.to_string(), m.id.to_string()))
                .collect(),
        ))
        .child(select(
            "kind",
            "Kind",
            kind,
            ["query", "command", "resource"]
                .into_iter()
                .map(|k| (k.to_string(), k.to_string()))
                .collect(),
        ))
        .child(select(
            "source",
            "Source",
            source,
            ["builtin", "declarative"]
                .into_iter()
                .map(|k| (k.to_string(), k.to_string()))
                .collect(),
        ))
        .child(
            el("button")
                .class("mj-button")
                .attr("type", "submit")
                .text("Apply"),
        )
        .child(link("/cockpit/capabilities", "Clear").class("mj-link mj-clear"));

    let body = if rows.is_empty() {
        nothing("No capability matches these filters.")
    } else {
        el("div")
            .child(table(
                &["Id", "Kind", "Title", "Module", "Projections", "Provenance"],
                rows,
            ))
            .child(pagination(window, |n| {
                href_with(
                    "/cockpit/capabilities",
                    query,
                    &[("page", Some(&n.to_string()))],
                )
            }))
    };

    Page::new(
        Area::Capabilities,
        "Capabilities",
        el("div").child(filters).child(browse).child(body),
    )
    .subtitle(format!(
        "{} of {} capabilities. Every one is a canonical declaration; the projections beside it are derived from it.",
        matching.len(),
        ctx.registry.len()
    ))
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Capabilities", None),
    ])
}

fn select(name: &str, label: &str, current: Option<&str>, options: Vec<(String, String)>) -> El {
    let mut field = el("select").class("mj-select").attr("name", name).child(
        el("option")
            .attr("value", "")
            .when(current.is_none(), |o| o.flag("selected"))
            .text(format!("any {}", label.to_lowercase())),
    );
    for (value, text) in options {
        field = field.child(
            el("option")
                .attr("value", value.clone())
                .when(current == Some(value.as_str()), |o| o.flag("selected"))
                .text(text),
        );
    }
    el("label")
        .class("mj-field")
        .child(el("span").class("mj-field-label").text(label))
        .child(field)
}

fn kind_word(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Query => "query",
        CapabilityKind::Command => "command",
        CapabilityKind::Resource => "resource",
    }
}

/// Which projections a capability declares, as short marks.
fn projection_marks(c: &Capability) -> El {
    let mut marks = el("span").class("mj-marks");
    if c.exposure.mcp.is_some() {
        marks = marks.child(tag("MCP"));
    }
    if c.exposure.http.is_some() {
        marks = marks.child(tag("HTTP"));
    }
    if c.exposure.cli.is_some() {
        marks = marks.child(tag("CLI"));
    }
    if c.exposure.is_empty() {
        marks = marks.child(tag("none"));
    }
    marks
}

/// Where a capability came from, in the Cockpit's provenance vocabulary. Each word has a
/// backend fact behind it: `declared` is a `capability!` block, `generated` is a file that
/// carries the generator's header, `cached` is a cache policy that keeps something.
fn provenance_badge(c: &Capability) -> El {
    let mut marks = el("span").class("mj-marks");
    match &c.provenance {
        Provenance::Builtin { .. } => marks = marks.child(badge("declared", "declared")),
        Provenance::Declarative { path, .. } => {
            marks = marks.child(if path.starts_with(generate::OUT_DIR) {
                badge("generated", "generated")
            } else {
                badge("declared", "declared")
            })
        }
    }
    if c.cache.is_enabled() {
        marks = marks.child(badge("cached", "cached"));
    }
    marks
}

/// One capability: its descriptor, its schemas, its projections, its cases, and a form
/// that runs it. The form is generated from the input schema; there is no per-capability
/// markup anywhere in the Cockpit.
pub fn capability(ctx: &Context, id: &str) -> Page {
    let Some(c) = ctx.registry.get(id) else {
        return Page::new(
            Area::Capabilities,
            "No such capability",
            el("div")
                .child(alert(
                    "fail",
                    format!("The registry holds no capability '{id}'."),
                ))
                .child(link("/cockpit/capabilities", "Every capability")),
        )
        .status(404);
    };

    let descriptor = card(
        "Descriptor",
        facts(vec![
            ("Canonical id", Node::Element(mono(c.id.as_str()))),
            ("Module", Node::Element(mono(c.module.as_str()))),
            ("Kind", Node::Element(kind_badge(c.kind))),
            (
                "Stability",
                Node::Element(badge(
                    if c.stability.executable() {
                        "ok"
                    } else {
                        "warn"
                    },
                    format!("{:?}", c.stability).to_lowercase(),
                )),
            ),
            ("Provenance", Node::Element(provenance_badge(c))),
            ("Source", Node::Element(mono(c.provenance.source_path()))),
            (
                "Cache",
                Node::Element(mono(match &c.cache {
                    crate::capability::CachePolicy::Disabled => "disabled".to_string(),
                    crate::capability::CachePolicy::Process {
                        max_entries,
                        ttl_seconds,
                    } => match ttl_seconds {
                        Some(t) => format!("process, {max_entries} entries, {t}s"),
                        None => format!("process, {max_entries} entries, no expiry"),
                    },
                })),
            ),
            (
                "Benchmark",
                Node::Element(mono(match &c.benchmark {
                    crate::capability::BenchmarkPolicy::Required => "required".to_string(),
                    crate::capability::BenchmarkPolicy::Waived { reason } => {
                        format!("waived: {}", word(reason))
                    }
                })),
            ),
            (
                "Tags",
                Node::Element(if c.tags.is_empty() {
                    el("span").text("-")
                } else {
                    el("span")
                        .class("mj-marks")
                        .children(c.tags.iter().map(tag).collect::<Vec<_>>())
                }),
            ),
        ]),
    );

    let projections = card(
        "Projections",
        table(
            &["Interface", "As", "Where"],
            vec![
                projection_row(
                    "MCP tool",
                    c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
                    None,
                ),
                projection_row(
                    "MCP resource",
                    c.exposure
                        .mcp
                        .as_ref()
                        .and_then(|m| m.resource.as_ref())
                        .map(|r| r.uri.clone()),
                    None,
                ),
                projection_row(
                    "HTTP",
                    c.exposure
                        .http
                        .as_ref()
                        .map(|h| format!("{} {}", h.method.as_str(), h.path)),
                    c.exposure.http.as_ref().map(|_| {
                        format!(
                            "{}#/{}/{}",
                            crate::http::swagger::SWAGGER_PATH,
                            c.module,
                            c.id
                        )
                    }),
                ),
                projection_row(
                    "Command line",
                    c.exposure
                        .cli
                        .as_ref()
                        .map(|cli| format!("majordomus {}", cli.path.join(" "))),
                    None,
                ),
            ],
        ),
    );

    let schemas = card(
        "Schemas",
        el("div")
            .child(details(
                format!(
                    "Input · {}",
                    c.input.name.as_deref().unwrap_or("(anonymous)")
                ),
                pre(serde_json::to_string_pretty(&c.input.schema).unwrap_or_default()),
            ))
            .child(details(
                format!(
                    "Output · {}",
                    c.output.name.as_deref().unwrap_or("(anonymous)")
                ),
                pre(serde_json::to_string_pretty(&c.output.schema).unwrap_or_default()),
            )),
    );

    let cases = ctx
        .registry
        .cases(c.id.as_str())
        .map(|provider| provider(&crate::capability::CaseContext { index: &ctx.index }))
        .unwrap_or_default();
    let examples = if cases.is_empty() {
        card(
            "Examples",
            nothing("This capability declares no benchmark cases, so it has no examples."),
        )
    } else {
        card(
            "Examples",
            el("div")
                .child(
                    el("p")
                        .class("mj-note")
                        .text("These are the capability's own benchmark cases: the same inputs the benchmark runs and the OpenAPI document shows."),
                )
                .children(
                    cases
                        .iter()
                        .map(|case| {
                            details(
                                case.name,
                                el("div")
                                    .child(pre(
                                        serde_json::to_string_pretty(&case.input)
                                            .unwrap_or_default(),
                                    ))
                                    .when(c.exposure.http.is_some(), |d| {
                                        d.child(
                                            el("button")
                                                .class("mj-button mj-button--quiet")
                                                .attr("type", "button")
                                                .attr("data-mj-fill", case.input.to_string())
                                                .text("Load into the runner"),
                                        )
                                    }),
                            )
                        })
                        .collect::<Vec<_>>(),
                ),
        )
    };

    let runner = match (&c.exposure.http, c.stability.executable()) {
        (Some(http), true) => runner_form(c, http),
        (Some(_), false) => card(
            "Run",
            alert(
                "warn",
                "This capability is listed and is not executable: its stability says so, and every projection refuses it.",
            ),
        ),
        (None, _) => card(
            "Run",
            nothing("This capability declares no HTTP exposure, so the Cockpit cannot call it. The MCP and command-line projections may still reach it."),
        ),
    };

    let mut page = Page::new(Area::Capabilities, c.id.to_string(), {
        let mut body = el("div").class("mj-grid");
        body = body
            .child(card(
                "What it is",
                el("p").class("mj-prose").text(&c.description),
            ))
            .child(descriptor)
            .child(runner)
            .child(projections)
            .child(schemas)
            .child(examples);
        body
    })
    .subtitle(c.title.clone())
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Capabilities", Some("/cockpit/capabilities")),
        (c.id.as_str(), None),
    ]);
    if c.exposure.http.is_some() && c.stability.executable() {
        page = page.script("runner.js");
    }
    page
}

fn projection_row(interface: &str, value: Option<String>, docs: Option<String>) -> El {
    match value {
        Some(v) => row(vec![
            text_cell(interface),
            cell(mono(v)),
            cell(match docs {
                Some(href) => link(href, "Swagger"),
                None => el("span").text("-"),
            }),
        ]),
        None => row(vec![
            text_cell(interface),
            cell(el("span").class("mj-muted").text("not exposed")),
            cell(el("span").text("-")),
        ]),
    }
}

/// The generic runner: one form per capability, generated from the input schema. Nothing
/// here is per-capability, and adding a capability adds a working form.
fn runner_form(c: &Capability, http: &crate::capability::HttpExposure) -> El {
    let (properties, required) = c.input.properties();
    let fields: Vec<El> = properties
        .iter()
        .map(|(name, schema)| field(name, schema, required.contains(name)))
        .collect();

    let form = el("form")
        .class("mj-runner")
        .attr("data-mj-runner", c.id.as_str())
        .attr("data-mj-method", http.method.as_str())
        .attr("data-mj-path", http.path.clone())
        .attr("novalidate", "")
        .child(if fields.is_empty() {
            el("p")
                .class("mj-note")
                .text("This capability takes no input.")
        } else {
            el("div").class("mj-fields").children(fields)
        })
        .child(
            el("div")
                .class("mj-runner-actions")
                .child(
                    el("button")
                        .class("mj-button mj-button--primary")
                        .attr("type", "submit")
                        .text(if c.kind == CapabilityKind::Command {
                            "Run (changes this process)"
                        } else {
                            "Run"
                        }),
                )
                .child(
                    el("code")
                        .class("mj-mono mj-preview")
                        .attr("data-mj-preview", "")
                        .text(format!("{} {}", http.method.as_str(), http.path)),
                ),
        )
        .child(
            el("div")
                .class("mj-runner-result")
                .attr("data-mj-result", "")
                .attr("aria-live", "polite"),
        );

    let effect = if c.kind == CapabilityKind::Command {
        alert(
            "warn",
            "A command. It changes this process's own memory — never the repository — and is sent as a POST from this page's origin.",
        )
    } else {
        alert("info", "A query. It reads and changes nothing.")
    };

    card_with(
        "Run",
        badge(kind_word(c.kind), kind_word(c.kind)),
        el("div").child(effect).child(form),
    )
}

/// One input control, from one property's schema.
fn field(name: &str, schema: &Value, required: bool) -> El {
    let description = schema
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let default = schema.get("default");
    let (kind, enumeration) = schema_shape(schema);

    let control = match (&kind, &enumeration) {
        (_, Some(values)) => {
            let mut select = el("select").class("mj-select").attr("name", name);
            if !required {
                select = select.child(el("option").attr("value", "").text("(unset)"));
            }
            for v in values {
                select = select.child(
                    el("option")
                        .attr("value", v.clone())
                        .when(default.and_then(Value::as_str) == Some(v.as_str()), |o| {
                            o.flag("selected")
                        })
                        .text(v),
                );
            }
            select
        }
        (FieldKind::Boolean, _) => el("input")
            .class("mj-checkbox")
            .attr("type", "checkbox")
            .attr("name", name)
            .when(default == Some(&Value::Bool(true)), |i| i.flag("checked")),
        (FieldKind::Integer, _) | (FieldKind::Number, _) => el("input")
            .class("mj-input")
            .attr("type", "number")
            .attr("name", name)
            .attr_if("step", matches!(kind, FieldKind::Number).then_some("any"))
            .attr_if("min", schema.get("minimum").map(|v| v.to_string()))
            .attr_if("max", schema.get("maximum").map(|v| v.to_string()))
            .attr_if("value", default.and_then(scalar_text)),
        (FieldKind::Structured, _) => el("textarea")
            .class("mj-textarea")
            .attr("name", name)
            .attr("rows", "4")
            .attr("spellcheck", "false")
            .attr("placeholder", "JSON")
            .text(default.map(|d| d.to_string()).unwrap_or_default()),
        (FieldKind::Text, _) => el("input")
            .class("mj-input")
            .attr("type", "text")
            .attr("name", name)
            .attr("spellcheck", "false")
            .attr_if("value", default.and_then(scalar_text)),
    }
    .attr("data-mj-type", kind.word())
    .when(required, |c| {
        c.flag("required").attr("aria-required", "true")
    })
    .attr("id", format!("mj-field-{name}"));

    el("div")
        .class("mj-field")
        .child(
            el("label")
                .class("mj-field-label")
                .attr("for", format!("mj-field-{name}"))
                .text(name)
                .node(if required {
                    Node::Element(el("span").class("mj-required").text("required"))
                } else {
                    Node::Element(el("span").class("mj-optional").text("optional"))
                })
                .child(el("span").class("mj-field-type").text(kind.word())),
        )
        .child(control)
        .node(if description.is_empty() {
            empty()
        } else {
            Node::Element(el("p").class("mj-field-help").text(description))
        })
}

/// How a property is edited. Derived from the JSON Schema the input type produced, never
/// from the property's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Text,
    Integer,
    Number,
    Boolean,
    Structured,
}

impl FieldKind {
    fn word(self) -> &'static str {
        match self {
            FieldKind::Text => "string",
            FieldKind::Integer => "integer",
            FieldKind::Number => "number",
            FieldKind::Boolean => "boolean",
            FieldKind::Structured => "json",
        }
    }
}

/// The shape of a property: its editable kind and its enumeration, looking through the
/// `anyOf`/`oneOf` an `Option<T>` produces and through `$ref`-free nesting.
fn schema_shape(schema: &Value) -> (FieldKind, Option<Vec<String>>) {
    if let Some(values) = enumeration(schema) {
        return (FieldKind::Text, Some(values));
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = schema.get(key).and_then(Value::as_array) {
            for branch in branches {
                if branch.get("type").and_then(Value::as_str) == Some("null") {
                    continue;
                }
                return schema_shape(branch);
            }
        }
    }
    let type_word = match schema.get("type") {
        Some(Value::String(s)) => Some(s.as_str()),
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).find(|t| *t != "null"),
        _ => None,
    };
    let kind = match type_word {
        Some("integer") => FieldKind::Integer,
        Some("number") => FieldKind::Number,
        Some("boolean") => FieldKind::Boolean,
        Some("array") | Some("object") => FieldKind::Structured,
        Some("string") => FieldKind::Text,
        // a `$ref` or an untyped schema: JSON is the honest control
        _ => FieldKind::Structured,
    };
    (kind, None)
}

fn enumeration(schema: &Value) -> Option<Vec<String>> {
    let values = schema.get("enum")?.as_array()?;
    let words: Vec<String> = values.iter().filter_map(scalar_text).collect();
    (words.len() == values.len() && !words.is_empty()).then_some(words)
}

fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// -------------------------------------------------------------------- objects

/// The layer's objects, by kind.
pub fn objects(ctx: &Context, query: &[(String, String)]) -> Page {
    let kind = query
        .iter()
        .find(|(k, _)| k == "kind")
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty());
    let needle = query
        .iter()
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.to_lowercase())
        .filter(|v| !v.is_empty());

    // the whole listing, narrowed here rather than in the request: the kinds beside it
    // carry how many objects each holds under the filter in force, and a count nobody can
    // see the rest of is not a way in
    let list: ObjectList = match ask(ctx, "objects.list", json!({})) {
        Ok(l) => l,
        Err(e) => {
            return Page::new(
                Area::Objects,
                "Objects",
                el("div")
                    .child(alert("fail", e))
                    .child(link("/cockpit/objects", "Every object")),
            )
            .status(404)
        }
    };

    let found = |o: &ObjectSummary| {
        needle.as_deref().is_none_or(|n| {
            o.identity.to_lowercase().contains(n)
                || o.title
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(n)
                || o.path.to_lowercase().contains(n)
        })
    };
    let matching: Vec<_> = list
        .objects
        .iter()
        .filter(|o| found(o))
        .filter(|o| kind.as_deref().is_none_or(|k| o.kind == k))
        .collect();

    let window = Window::new(asked_page(query), PER_PAGE, matching.len());
    let rows: Vec<El> = matching[window.range()]
        .iter()
        .map(|o| {
            row(vec![
                id_cell(
                    format!("/cockpit/object?uri={}", percent_encode(&o.uri)),
                    &o.identity,
                ),
                cell(mono(&o.kind)),
                text_cell(o.title.clone().unwrap_or_default()),
                cell(mono(&o.path)),
            ])
        })
        .collect();

    let mut per_kind: std::collections::BTreeMap<&str, usize> = Default::default();
    for o in list.objects.iter().filter(|o| found(o)) {
        *per_kind.entry(o.kind.as_str()).or_default() += 1;
    }
    let browse = chips(
        std::iter::once((
            "All kinds".to_string(),
            href_with("/cockpit/objects", query, &[("kind", None), ("page", None)]),
            per_kind.values().sum::<usize>(),
            kind.is_none(),
        ))
        .chain(per_kind.iter().map(|(k, n)| {
            (
                (*k).to_string(),
                href_with(
                    "/cockpit/objects",
                    query,
                    &[("kind", Some(k)), ("page", None)],
                ),
                *n,
                kind.as_deref() == Some(*k),
            )
        }))
        .collect(),
    );

    let filters = el("form")
        .class("mj-filters")
        .attr("method", "get")
        .attr("action", "/cockpit/objects")
        .attr("role", "search")
        .child(
            el("label")
                .class("mj-field")
                .child(el("span").class("mj-field-label").text("Filter"))
                .child(
                    el("input")
                        .class("mj-input")
                        .attr("type", "search")
                        .attr("name", "q")
                        .attr("value", needle.clone().unwrap_or_default())
                        .attr("placeholder", "identity, title or path"),
                ),
        )
        .child(select(
            "kind",
            "Kind",
            kind.as_deref(),
            ctx.index
                .kinds()
                .into_keys()
                .map(|k| (k.to_string(), k.to_string()))
                .collect(),
        ))
        .child(
            el("button")
                .class("mj-button")
                .attr("type", "submit")
                .text("Apply"),
        )
        .child(link("/cockpit/objects", "Clear").class("mj-link mj-clear"))
        .child(link("/cockpit/search", "Full-text search").class("mj-link mj-clear"));

    Page::new(
        Area::Objects,
        "Objects",
        el("div").child(filters).child(browse).child(if rows.is_empty() {
            nothing("No object matches.")
        } else {
            el("div")
                .child(table(&["Identity", "Kind", "Title", "Path"], rows))
                .child(pagination(window, |n| {
                    href_with("/cockpit/objects", query, &[("page", Some(&n.to_string()))])
                }))
        }),
    )
    .subtitle(format!(
        "{} of {} objects. Each is a file the layer's sources.yaml maps to a kind, read and validated at startup.",
        matching.len(),
        ctx.index.objects.len()
    ))
    .trail(vec![("Cockpit", Some("/cockpit")), ("Objects", None)])
}

/// One object of the layer: its metadata, its provenance, and its content as it is.
pub fn object(ctx: &Context, uri: &str) -> Page {
    let value = match ctx.execute("objects.get", json!({ "uri": uri })) {
        Ok(v) => v,
        Err(e) => {
            return Page::new(
                Area::Objects,
                "No such object",
                el("div")
                    .child(alert("fail", e.to_string()))
                    .child(link("/cockpit/objects", "Every object")),
            )
            .status(404)
        }
    };

    let identity = value
        .get("identity")
        .and_then(Value::as_str)
        .unwrap_or(uri)
        .to_string();
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = value
        .get("provenance")
        .and_then(|p| p.get("path"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let content = value
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let generated = content.contains(generate::HEADER) || path.starts_with(generate::OUT_DIR);

    let facts_card = card(
        "What it is",
        facts(vec![
            ("URI", Node::Element(mono(uri))),
            ("Kind", Node::Element(mono(&kind))),
            ("Identity", Node::Element(mono(&identity))),
            ("Path", Node::Element(mono(&path))),
            (
                "Provenance",
                Node::Element(if generated {
                    badge("generated", "generated")
                } else {
                    badge("declared", "declared")
                }),
            ),
            (
                "Media type",
                Node::Element(mono(
                    value
                        .get("media_type")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )),
            ),
            (
                "As a capability",
                Node::Element(link(
                    format!(
                        "/cockpit/capabilities/{}",
                        percent_encode(&format!("{kind}.{identity}"))
                    ),
                    format!("{kind}.{identity}"),
                )),
            ),
        ]),
    );

    let metadata = value
        .get("metadata")
        .filter(|m| !m.is_null())
        .map(|m| {
            card(
                "Front matter",
                pre(serde_json::to_string_pretty(m).unwrap_or_default()),
            )
        })
        .unwrap_or_else(|| card("Front matter", nothing("This object carries none.")));

    Page::new(
        Area::Objects,
        identity.clone(),
        el("div")
            .class("mj-grid")
            .child(facts_card)
            .when(generated, |d| {
                d.child(alert(
                    "info",
                    "This file is generated. Editing it is pointless: the generator overwrites it, and `majordomus generate --check` fails while it differs.",
                ))
            })
            .child(metadata)
            .child(card("Content", pre(content))),
    )
    .subtitle(if title.is_empty() {
        format!("An object of kind {kind}")
    } else {
        title.to_string()
    })
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Objects", Some("/cockpit/objects")),
        (&identity, None),
    ])
}

// --------------------------------------------------------------------- graphs

/// Every derived graph.
pub fn graphs(ctx: &Context) -> Page {
    let list: GraphList = match ask(ctx, "graph.list", json!({})) {
        Ok(l) => l,
        Err(e) => return failed(Area::Graphs, "Graphs", e),
    };
    let rows = list
        .graphs
        .iter()
        .map(|g| {
            row(vec![
                id_cell(format!("/cockpit/graphs/{}", g.id), &g.id),
                text_cell(&g.title),
                text_cell(&g.description),
                text_cell(&g.source),
            ])
        })
        .collect();
    Page::new(
        Area::Graphs,
        "Graphs",
        table(&["Id", "Title", "What it shows", "Derived from"], rows),
    )
    .subtitle("Every graph is derived from the registry and the index. None is authored, and the drawing library is a consumer of the same nodes and edges the API answers with.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Graphs", None)])
}

/// One graph: the nodes and edges as a list, which is what a reader without JavaScript
/// gets, and a canvas the script fills when the drawing library is there.
pub fn graph(ctx: &Context, id: &str) -> Page {
    let value = match ctx.execute("graph.get", json!({ "id": id })) {
        Ok(v) => v,
        Err(e) => {
            return Page::new(
                Area::Graphs,
                "No such graph",
                el("div")
                    .child(alert("fail", e.to_string()))
                    .child(link("/cockpit/graphs", "Every graph")),
            )
            .status(404)
        }
    };
    let g: Graph = match serde_json::from_value(value) {
        Ok(g) => g,
        Err(e) => return failed(Area::Graphs, "Graph", e.to_string()),
    };

    let vocabulary = card(
        "What the shapes mean",
        el("div")
            .child(el("h3").class("mj-subheading").text("Nodes"))
            .child(facts(
                g.node_kinds
                    .iter()
                    .map(|(k, meaning)| (k.as_str(), Node::Element(el("span").text(meaning))))
                    .collect(),
            ))
            .child(el("h3").class("mj-subheading").text("Edges"))
            .child(facts(
                g.edge_kinds
                    .iter()
                    .map(|(k, meaning)| (k.as_str(), Node::Element(el("span").text(meaning))))
                    .collect(),
            )),
    );

    let viewer = el("section")
        .class("mj-card")
        .child(
            el("div")
                .class("mj-card-head")
                .child(el("h2").class("mj-card-title").text("Graph"))
                .child(
                    el("div")
                        .class("mj-graph-controls")
                        .child(
                            el("input")
                                .class("mj-input")
                                .attr("type", "search")
                                .attr("data-mj-graph-search", "")
                                .attr("aria-label", "Find a node")
                                .attr("placeholder", "Find a node"),
                        )
                        .child(
                            el("button")
                                .class("mj-button mj-button--quiet")
                                .attr("type", "button")
                                .attr("data-mj-graph-fit", "")
                                .text("Fit"),
                        ),
                ),
        )
        .child(
            el("div")
                .class("mj-graph")
                .attr("data-mj-graph", g.id.clone())
                .attr("data-mj-graph-src", format!("/api/v1/graph?id={}", percent_encode(&g.id)))
                .attr("role", "img")
                .attr(
                    "aria-label",
                    format!(
                        "{}: {} nodes, {} edges. The same data is listed below.",
                        g.title, g.metadata.nodes, g.metadata.edges
                    ),
                ),
        )
        .child(
            el("p")
                .class("mj-note")
                .text("The drawing is an enhancement. Everything it shows is in the lists below, which is what a reader without JavaScript, a crawler and a screen reader get."),
        );

    // both tables list everything: the drawing is an enhancement, and what a reader
    // without JavaScript, a crawler and a screen reader get is these lists whole
    let node_rows = g
        .nodes
        .iter()
        .map(|n| {
            row(vec![
                cell(match &n.route {
                    Some(route) => link(route.clone(), &n.label).class("mj-link mj-mono"),
                    None => mono(&n.label),
                }),
                cell(mono(&n.kind)),
                text_cell(n.summary.clone().unwrap_or_default()),
                cell(match &n.status {
                    Some(s) => badge(s, s),
                    None => el("span").text("-"),
                }),
                cell(match &n.source {
                    Some(s) => mono(s),
                    None => el("span").text("-"),
                }),
            ])
        })
        .collect();

    let edge_rows = g
        .edges
        .iter()
        .map(|e| {
            row(vec![
                cell(mono(&e.source)),
                cell(tag(&e.kind)),
                cell(mono(&e.target)),
            ])
        })
        .collect();

    Page::new(
        Area::Graphs,
        g.title.clone(),
        el("div")
            .class("mj-grid")
            .child(card("Where it comes from", facts(vec![
                ("Derived from", Node::Element(el("span").text(&g.source))),
                ("Nodes", Node::Element(mono(g.metadata.nodes.to_string()))),
                ("Edges", Node::Element(mono(g.metadata.edges.to_string()))),
                (
                    "Acyclic",
                    Node::Element(badge(
                        if g.metadata.acyclic { "ok" } else { "warn" },
                        if g.metadata.acyclic { "yes" } else { "no" },
                    )),
                ),
                (
                    "As data",
                    Node::Element(link(
                        format!("/api/v1/graph?id={}", percent_encode(&g.id)),
                        "/api/v1/graph",
                    )),
                ),
            ])))
            .when(g.metadata.truncated, |d| {
                d.child(alert(
                    "warn",
                    "This graph reached the derivation's node limit and is a prefix of what the repository holds.",
                ))
            })
            .child(viewer)
            .child(vocabulary)
            .child(card(
                "Nodes",
                table(&["Node", "Kind", "Summary", "Status", "Source"], node_rows),
            ))
            .child(card("Edges", table(&["From", "Edge", "To"], edge_rows))),
    )
    .subtitle(g.description.clone())
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Graphs", Some("/cockpit/graphs")),
        (&g.id, None),
    ])
    .script("graph.js")
}

// --------------------------------------------------------------------- continuity

/// One resolved record: where it is, how far its commit is from this one, and the section
/// a resuming worker acts on.
///
/// The divergence label is a badge and a word, never a colour alone, because the difference
/// between `advanced` and `diverged` is the difference between "some of this is already
/// done" and "this describes a history that no longer exists".
fn record_card(title: &str, r: Option<&Record>, empty_note: &str) -> El {
    let Some(r) = r else {
        return card(title, nothing(empty_note));
    };
    let level = match r.divergence.as_str() {
        "exact" => "ok",
        "advanced" => "info",
        "unknown" => "warn",
        _ => "fail",
    };
    card_with(
        title,
        badge(level, r.divergence.as_str()),
        el("div")
            .child(facts(vec![
                ("Path", Node::Element(mono(r.path.clone()))),
                ("Written", Node::Element(el("span").text(&r.created_at))),
                ("Task", Node::Element(mono(r.task_id.clone()))),
                (
                    "At",
                    Node::Element(mono(r.head[..7.min(r.head.len())].to_string())),
                ),
                (
                    "Matched",
                    Node::Element(el("span").text(word(&r.matched))),
                ),
                (
                    "Working tree then",
                    Node::Element(el("span").text(&r.working_tree)),
                ),
            ]))
            .when(!r.divergence.trustworthy(), |d| {
                d.child(alert(
                    "fail",
                    "The commit this record was written at is not in this history. Trust git over anything it says.",
                ))
            })
            .when(!r.next_action.is_empty(), |d| {
                d.child(el("h3").class("mj-card-title").text("Next action"))
                    .child(pre(r.next_action.clone()))
            }),
    )
}

/// What this checkout's lifecycle is holding.
///
/// This is the only page that shows the local half of the layer, and it is the reason the
/// Cockpit is bound to the loopback interface. Nothing here is projected into the static
/// site: these records name this machine, and a site that published them would publish the
/// one part of the layer no other clone can reproduce.
pub fn continuity(ctx: &Context) -> Page {
    let c: Continuity = match ask(ctx, "continuity.state", json!({})) {
        Ok(c) => c,
        Err(e) => return failed(Area::Continuity, "Continuity", e),
    };

    let episode = match &c.session {
        Some(s) => card_with(
            "Open episode",
            badge(if s.foreign { "fail" } else { "ok" }, if s.foreign { "foreign" } else { "open" }),
            el("div")
                .child(facts(vec![
                    ("Episode", Node::Element(mono(s.session_id.clone()))),
                    ("Opened", Node::Element(el("span").text(&s.started_at))),
                    ("Owner", Node::Element(el("span").text(&s.owner))),
                    ("Worker", Node::Element(el("span").text(if s.worker.is_empty() { "(not recorded)" } else { &s.worker }))),
                    ("Provider", Node::Element(el("span").text(if s.provider.is_empty() { "(not recorded)" } else { &s.provider }))),
                    ("Branch", Node::Element(mono(s.branch.clone()))),
                ]))
                .when(s.foreign, |d| {
                    d.child(alert(
                        "fail",
                        "This open record belongs to another checkout. Nothing about it is about the work here.",
                    ))
                }),
        ),
        None => card(
            "Open episode",
            nothing(
                "No episode is open in this worktree. The provider's start event opens one; `majordomus session start` opens one by hand.",
            ),
        ),
    };

    let task = match &c.task {
        Some(t) => card_with(
            "Active task",
            badge(if t.outcome == "active" { "ok" } else { "info" }, t.outcome.clone()),
            el("div")
                .child(el("p").class("mj-prose").text(&t.task))
                .child(facts(vec![
                    ("Id", Node::Element(mono(t.id.clone()))),
                    ("Profile", Node::Element(el("span").text(&t.profile))),
                    ("Started", Node::Element(el("span").text(&t.started_at))),
                ]))
                .child(
                    el("div")
                        .class("mj-marks")
                        .children(t.scope.iter().map(|p| mono(p.clone())).collect::<Vec<_>>()),
                ),
        ),
        None => card(
            "Active task",
            nothing(
                "No task is active here. Work outside a task is permitted; it records nothing a task would, and no checkpoint can be written against it.",
            ),
        ),
    };

    let blockers = if c.blockers.is_empty() {
        card(
            "Blockers",
            nothing("Nothing on this branch is refusing completion."),
        )
    } else {
        card_with(
            "Blockers",
            badge("fail", format!("{} open", c.blockers.len())),
            el("div")
                .child(el("p").class("mj-prose").text(
                    "Every unresolved question on this branch refuses `majordomus finish --outcome completed`, whichever task opened it.",
                ))
                .child(
                    el("ul")
                        .class("mj-list")
                        .children(c.blockers.iter().map(|b| el("li").text(b)).collect::<Vec<_>>()),
                ),
        )
    };

    let where_ = card(
        "This checkout",
        el("div")
            .child(facts(vec![
                ("Worktree", Node::Element(mono(c.worktree.clone()))),
                ("Branch", Node::Element(mono(c.branch.clone()))),
                (
                    "HEAD",
                    Node::Element(mono(c.head[..7.min(c.head.len())].to_string())),
                ),
                (
                    "Working tree",
                    Node::Element(el("span").text(&c.working_tree)),
                ),
            ]))
            .child(
                el("div").class("mj-stats").children(
                    c.tallies
                        .iter()
                        .map(|(k, n)| statistic(n.to_string(), k.clone(), ".ai/local/state"))
                        .collect::<Vec<_>>(),
                ),
            )
            .when(!c.present, |d| {
                d.child(alert(
                    "info",
                    "This checkout has no local state yet. That is a fresh clone, not a fault: the first command that writes a record creates it.",
                ))
            }),
    );

    let findings = if c.findings.is_empty() {
        empty()
    } else {
        Node::Element(card(
            "Before you trust the above",
            el("div").children(
                c.findings
                    .iter()
                    .map(|f| alert("warn", f.clone()))
                    .collect::<Vec<_>>(),
            ),
        ))
    };

    Page::new(
        Area::Continuity,
        "Continuity",
        el("div")
            .class("mj-grid")
            .child(where_)
            .child(episode)
            .child(task)
            .child(record_card(
                "Resume from",
                c.handover.as_ref(),
                "No relevant handover for this worktree and branch. That is an answer, not a gap: a record from another branch is never offered, because a briefing quietly about somebody else is worse than none.",
            ))
            .child(record_card(
                "Newest progress note",
                c.checkpoint.as_ref(),
                "No checkpoint resolves here yet.",
            ))
            .child(blockers)
            .node(findings),
    )
    .subtitle("What this checkout's lifecycle is holding. Local to this machine, served here and published nowhere.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Continuity", None)])
}

// --------------------------------------------------------------------- health

/// The health report, in full: the expensive comparison included.
/// The layer's directory contracts: the hierarchy, what each directory owes, and — for one
/// directory — the local contract beside the chain that actually applies to it.
///
/// The tree is not walked here. `directories.list` derives it from the index, so a
/// directory with tracked content appears in this page, in `/api/v1/directories` and in
/// the MCP resource at the same moment, and the Cockpit names no directory of its own.
pub fn directories(ctx: &Context, query: &[(String, String)]) -> Page {
    let focus = query
        .iter()
        .find(|(k, _)| k == "path")
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty());

    let report: DirectoryReport = match ask(ctx, "directories.list", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Directories, "Directories", e),
    };

    let summary = card(
        "Coverage",
        el("div")
            .child(
                el("div")
                    .class("mj-stats")
                    .child(statistic(
                        report.tallies.directories.to_string(),
                        "directories",
                        "the layer, from the index",
                    ))
                    .child(statistic(
                        report.tallies.documented.to_string(),
                        "documented",
                        "carry a contract",
                    ))
                    .child(statistic(
                        report.tallies.exempt.to_string(),
                        "exempt",
                        "released from above",
                    ))
                    .child(statistic(
                        report.tallies.owed.to_string(),
                        "owed",
                        "owe one and have none",
                    )),
            )
            .child(if report.tallies.owed == 0 {
                el("p").class("mj-note").text(
                    "Every directory here says what it is for, or a contract above it says why it need not. These are the directories the index holds content for; the gate that walks the working tree and refuses a commit is `majordomus context validate`.",
                )
            } else {
                alert(
                    "fail",
                    format!(
                        "{} directory(ies) owe a contract and carry none; `majordomus context validate` names them.",
                        report.tallies.owed
                    ),
                )
            }),
    );

    let rows: Vec<El> = report
        .directories
        .iter()
        .map(|d| {
            let indent = "\u{00a0}".repeat(d.depth * 3);
            let name = d
                .path
                .rsplit_once('/')
                .map(|(_, n)| n.to_string())
                .unwrap_or_else(|| d.path.clone());
            let state = match d.state {
                DirectoryState::Documented => badge("ok", "documented"),
                DirectoryState::Exempt => badge("info", "exempt"),
                DirectoryState::Owed => badge("fail", "owed"),
            };
            let decided = match (&d.exempted_by, &d.governed_by) {
                (Some(by), _) | (None, Some(by)) => mono(by.clone()),
                (None, None) => el("span").class("mj-note").text("nothing declares it"),
            };
            row(vec![
                cell(
                    el("span")
                        .child(el("span").class("mj-note").text(indent))
                        .child(link(
                            format!("/cockpit/directories?path={}", percent_encode(&d.path)),
                            name,
                        )),
                ),
                cell(state),
                cell(match &d.contract {
                    Some(c) => el("span").child(mono(c.id.clone())).child(
                        el("span")
                            .class("mj-note")
                            .text(c.description.clone().unwrap_or_default()),
                    ),
                    None => el("span").class("mj-note").text("no contract of its own"),
                }),
                cell(decided),
                text_cell(d.objects.to_string()),
            ])
        })
        .collect();

    let tree = card(
        "The hierarchy",
        if rows.is_empty() {
            nothing("The index holds no directory of the layer.")
        } else {
            table(
                &["Directory", "State", "Contract", "Decided by", "Objects"],
                rows,
            )
        },
    );

    let detail = focus.as_ref().map(|path| {
        let one: Result<DirectoryReport, String> =
            ask(ctx, "directories.list", json!({ "path": path }));
        match one {
            Err(e) => alert("fail", e),
            Ok(r) => match r.directories.into_iter().next() {
                None => alert("fail", format!("no directory '{path}' in the layer")),
                Some(d) => {
                    let local = match &d.contract {
                        Some(c) => facts(vec![
                            ("Identity", Node::Element(mono(c.id.clone()))),
                            ("Document", Node::Element(mono(c.path.clone()))),
                            ("Scope", Node::Element(tag(c.scope.clone()))),
                            ("Composition", Node::Element(tag(c.composition.clone()))),
                            (
                                "Order",
                                Node::Element(el("span").text(c.order.to_string())),
                            ),
                            ("Status", Node::Element(tag(c.status.clone()))),
                            (
                                "Providers",
                                Node::Element(el("span").text(c.providers.join(", "))),
                            ),
                            (
                                "Audience",
                                Node::Element(el("span").text(c.audience.join(", "))),
                            ),
                        ]),
                        None => el("p")
                            .class("mj-note")
                            .text("This directory declares no contract of its own."),
                    };
                    let chain: Vec<El> = d
                        .effective
                        .iter()
                        .map(|e| {
                            row(vec![
                                text_cell(e.depth.to_string()),
                                cell(mono(e.id.clone())),
                                cell(if e.local {
                                    badge("ok", "local")
                                } else {
                                    badge("info", "inherited")
                                }),
                                cell(tag(e.composition.clone())),
                                text_cell(e.order.to_string()),
                                text_cell(e.reason.clone()),
                            ])
                        })
                        .collect();
                    el("div")
                        .class("mj-grid")
                        .child(card_with(
                            format!("Local contract — {}", d.path),
                            match d.state {
                                DirectoryState::Documented => badge("ok", "documented"),
                                DirectoryState::Exempt => badge("info", "exempt"),
                                DirectoryState::Owed => badge("fail", "owed"),
                            },
                            local,
                        ))
                        .child(card(
                            "Effective contract",
                            el("div")
                                .child(el("p").class("mj-prose").text(
                                    "What applies here once inheritance is resolved: every document whose scope reaches this directory, least specific first — depth, then declared order, then path. This is the chain `majordomus context resolve` composes.",
                                ))
                                .child(if chain.is_empty() {
                                    nothing("No document reaches this directory.")
                                } else {
                                    table(
                                        &["Depth", "Document", "Origin", "Composition", "Order", "Why"],
                                        chain,
                                    )
                                }),
                        ))
                }
            },
        }
    });

    let mut main = el("div").class("mj-grid").child(summary);
    if let Some(d) = detail {
        main = main.child(d);
    }
    main = main.child(tree);

    Page::new(Area::Directories, "Directories", main)
        .subtitle(
            "Every directory of the layer, the contract it declares, and the chain it inherits. Derived from the index through `directories.list`; the Cockpit lists no directory itself.",
        )
        .trail(vec![
            ("Cockpit", Some("/cockpit")),
            ("Directories", None),
        ])
}

/// The health report: the verdicts the engines already reach, read through one capability.
pub fn health(ctx: &Context) -> Page {
    let health: Health = match ask(ctx, "health.report", json!({})) {
        Ok(h) => h,
        Err(e) => return failed(Area::Health, "Health", e),
    };
    let cards: Vec<El> = health
        .checks
        .iter()
        .map(|c| {
            card_with(
                c.title.clone(),
                badge(c.status.as_str(), c.status.as_str()),
                el("div")
                    .child(el("p").class("mj-prose").text(&c.detail))
                    .child(facts(vec![(
                        "Decided by",
                        Node::Element(el("span").text(&c.decided_by)),
                    )]))
                    .when(!c.evidence.is_empty(), |d| {
                        d.child(
                            el("div")
                                .class("mj-marks")
                                .children(c.evidence.iter().map(mono).collect::<Vec<_>>()),
                        )
                    })
                    .when(!c.findings.is_empty(), |d| {
                        d.child(details(
                            format!("{} finding(s)", c.findings.len()),
                            el("ul").class("mj-list").children(
                                c.findings
                                    .iter()
                                    .map(|f| el("li").text(f))
                                    .collect::<Vec<_>>(),
                            ),
                        ))
                    }),
            )
        })
        .collect();

    Page::new(
        Area::Health,
        "Health",
        el("div")
            .class("mj-grid")
            .child(card(
                "Where this stands",
                el("div")
                    .child(health_badge(health.status))
                    .child(el("div").class("mj-marks").children(
                        health
                            .tallies
                            .iter()
                            .map(|(word, n)| badge(word, format!("{n} {word}")))
                            .collect::<Vec<_>>(),
                    ))
                    .child(el("p").class("mj-note").text(
                        "Every check is decided by an engine that already decides it elsewhere. The Cockpit has no health checks of its own.",
                    )),
            ))
            .children(cards),
    )
    .subtitle("The same verdicts `capabilities validate`, `bench coverage --check` and `generate --check` reach, read through one capability.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Health", None)])
}

// --------------------------------------------------------------------- artifacts

/// What the generator writes: every document with the encodings it is committed in, and
/// every file with its contract and its state against the working tree. Read through
/// `artifacts.list`, which reads the generator's own manifest; this page keeps no list of
/// generated files and gains one the moment the generator does.
pub fn artifacts(ctx: &Context) -> Page {
    let report: ArtifactReport = match ask(ctx, "artifacts.list", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Artifacts, "Artifacts", e),
    };
    let t = &report.tallies;
    let overall = if !report.present {
        ("warn", "not generated")
    } else if t.missing > 0 {
        ("fail", "missing")
    } else if t.stale > 0 {
        ("warn", "stale")
    } else {
        ("ok", "current")
    };

    let documents = table(
        &["document", "encodings", "schema", "source"],
        report
            .documents
            .iter()
            .map(|d| {
                row(vec![
                    cell(mono(d.id.clone())),
                    cell(
                        el("span").class("mj-marks").children(
                            d.formats
                                .iter()
                                .map(|f| tag(f.suffix()))
                                .collect::<Vec<_>>(),
                        ),
                    ),
                    cell(match &d.schema {
                        Some(s) => mono(s.clone()),
                        None => el("span").class("mj-note").text("—"),
                    }),
                    text_cell(d.source.clone()),
                ])
            })
            .collect(),
    );

    // every file, on one page: this is the manifest as it stands, and a reader checking
    // whether a path is in it must be able to find it with the browser's own search
    let files = table(
        &["path", "document", "format", "bytes", "state"],
        report
            .artifacts
            .iter()
            .map(|a| {
                row(vec![
                    cell(mono(a.path.clone())),
                    cell(mono(a.document.clone())),
                    text_cell(a.format.suffix()),
                    text_cell(a.bytes.map(|b| b.to_string()).unwrap_or_else(|| "—".into())),
                    cell(badge(a.state.as_str(), a.state.as_str())),
                ])
            })
            .collect(),
    );

    Page::new(
        Area::Artifacts,
        "Artifacts",
        el("div")
            .class("mj-grid")
            .child(card_with(
                "Where the generated tree stands",
                badge(overall.0, overall.1),
                el("div")
                    .child(
                        el("div")
                            .class("mj-stats")
                            .child(statistic(
                                t.documents.to_string(),
                                "documents",
                                "the manifest",
                            ))
                            .child(statistic(
                                t.artifacts.to_string(),
                                "files",
                                "the manifest",
                            ))
                            .child(statistic(t.current.to_string(), "current", "sha256"))
                            .child(statistic(t.stale.to_string(), "stale", "sha256"))
                            .child(statistic(t.missing.to_string(), "missing", "the tree")),
                    )
                    .child(facts(vec![
                        ("Manifest", Node::Element(mono(report.manifest.clone()))),
                        ("Schema", Node::Element(mono(report.schema.clone()))),
                        ("Rewrite", Node::Element(mono(report.regenerate.clone()))),
                        ("Verify", Node::Element(mono(report.verify.clone()))),
                    ]))
                    .when(!report.present, |d| {
                        d.child(alert(
                            "warn",
                            "This repository has no generated tree yet: the manifest is written by `majordomus generate`.",
                        ))
                    })
                    .child(el("p").class("mj-note").text(
                        "A document is written in every encoding this repository commits it in, from one value: JSON for a program, YAML beside it, Markdown for a reader. The hashes here are the manifest's; `majordomus generate --check` compares every byte, which is the stronger statement.",
                    )),
            ))
            .child(card("Documents", documents))
            .child(card("Files", files)),
    )
    .subtitle("Every file `majordomus generate` writes, from the generator's own manifest.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Artifacts", None)])
}

// ------------------------------------------------------------------------ api

/// The surfaces: every HTTP route the registry projects, the projection's own routes, and
/// the way into Swagger UI.
pub fn api(ctx: &Context) -> Page {
    let mut rows: Vec<El> = ctx
        .registry
        .iter()
        .filter_map(|c| c.exposure.http.as_ref().map(|h| (c, h)))
        .map(|(c, h)| {
            row(vec![
                cell(mono(h.method.as_str())),
                cell(mono(&h.path)),
                id_cell(
                    format!("/cockpit/capabilities/{}", percent_encode(c.id.as_str())),
                    c.id.as_str(),
                ),
                text_cell(&c.title),
                cell(kind_badge(c.kind)),
            ])
        })
        .collect();
    rows.sort_by_key(|r| r.render());

    // the projection's own routes and what each one is, read off the surfaces that declare
    // them: this table has never held a path of its own and must not start
    let infrastructure = table(
        &["Path", "What it is"],
        crate::web::discover::native_all()
            .into_iter()
            .map(|surface| {
                row(vec![
                    cell(mono(surface.mount.as_str())),
                    text_cell(&surface.title),
                ])
            })
            .collect(),
    );

    Page::new(
        Area::Api,
        "API",
        el("div")
            .class("mj-grid")
            .child(card_with(
                "Swagger UI",
                link(crate::http::swagger::SWAGGER_PATH, "Open"),
                el("p").class("mj-prose").text(
                    "Swagger UI is served from this process and reads /openapi.json, which is generated from the registry at first request. Nothing about an operation is written twice: the descriptions, the schemas and the examples are the capability's own.",
                ),
            ))
            .child(card(
                "Capability routes",
                table(&["Method", "Path", "Capability", "Title", "Kind"], rows),
            ))
            .child(card("The projection's own routes", infrastructure)),
    )
    .subtitle("Every route under /api/v1/ is a capability's HTTP exposure. The rest belong to the projection and are documented as such.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("API", None)])
}

// --------------------------------------------------------------------- search

/// Unified search: the index's own search over objects, and a match over the registry's
/// capabilities. Two sources because there are two, not because there are two indexes.
pub fn search(ctx: &Context, query: &[(String, String)]) -> Page {
    let needle = query
        .iter()
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    let form = el("form")
        .class("mj-filters")
        .attr("method", "get")
        .attr("action", "/cockpit/search")
        .attr("role", "search")
        .child(
            el("label")
                .class("mj-field")
                .child(el("span").class("mj-field-label").text("Search"))
                .child(
                    el("input")
                        .class("mj-input")
                        .attr("type", "search")
                        .attr("name", "q")
                        .attr("autofocus", "")
                        .attr("value", needle.clone())
                        .attr("placeholder", "text in any object, or a capability"),
                ),
        )
        .child(
            el("button")
                .class("mj-button")
                .attr("type", "submit")
                .text("Search"),
        );

    if needle.trim().is_empty() {
        return Page::new(
            Area::None,
            "Search",
            el("div").child(form).child(nothing(
                "Type something. The search is the repository's own: case-insensitive, over identities, titles, descriptions and content.",
            )),
        )
        .trail(vec![("Cockpit", Some("/cockpit")), ("Search", None)]);
    }

    let lowered = needle.to_lowercase();
    let capability_rows: Vec<El> = ctx
        .registry
        .iter()
        .filter(|c| {
            c.id.as_str().to_lowercase().contains(&lowered)
                || c.title.to_lowercase().contains(&lowered)
        })
        .take(50)
        .map(|c| {
            row(vec![
                id_cell(
                    format!("/cockpit/capabilities/{}", percent_encode(c.id.as_str())),
                    c.id.as_str(),
                ),
                text_cell(&c.title),
                cell(kind_badge(c.kind)),
            ])
        })
        .collect();

    let hits = ctx
        .execute("objects.search", json!({ "query": needle, "limit": 50 }))
        .ok();
    let object_rows: Vec<El> = hits
        .as_ref()
        .and_then(|v| v.get("hits"))
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .map(|h| {
                    let uri = h.get("uri").and_then(Value::as_str).unwrap_or_default();
                    row(vec![
                        id_cell(
                            format!("/cockpit/object?uri={}", percent_encode(uri)),
                            h.get("identity").and_then(Value::as_str).unwrap_or(uri),
                        ),
                        cell(mono(h.get("kind").and_then(Value::as_str).unwrap_or(""))),
                        text_cell(h.get("title").and_then(Value::as_str).unwrap_or("")),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();

    Page::new(
        Area::None,
        format!("Search: {needle}"),
        el("div")
            .class("mj-grid")
            .child(form)
            .child(card(
                format!("Capabilities ({})", capability_rows.len()),
                if capability_rows.is_empty() {
                    nothing("No capability matches.")
                } else {
                    table(&["Id", "Title", "Kind"], capability_rows)
                },
            ))
            .child(card(
                format!("Objects ({})", object_rows.len()),
                if object_rows.is_empty() {
                    nothing("No object matches.")
                } else {
                    table(&["Identity", "Kind", "Title"], object_rows)
                },
            )),
    )
    .trail(vec![("Cockpit", Some("/cockpit")), ("Search", None)])
}

// ------------------------------------------------------------------- topology

/// The system topology: the registry graph in three dimensions, for the one question a
/// flat drawing answers badly — how the projections fan out of the modules. Optional in
/// the strict sense: without the library, the page is the same node and edge lists the
/// graph page shows.
pub fn topology(ctx: &Context) -> Page {
    let summary = ctx.registry.summary();
    Page::new(
        Area::Graphs,
        "Topology",
        el("div")
            .class("mj-grid")
            .child(card(
                "What this is",
                el("p").class("mj-prose").text(
                    "The registry graph laid out in three dimensions: modules on one plane, the capabilities they compose on another, and the MCP, HTTP and command-line projections above them. It answers one question a flat drawing answers badly — how wide each projection fans out — and nothing else. Every fact in it is on the registry graph page too.",
                ),
            ))
            .child(
                el("section")
                    .class("mj-card")
                    .child(el("h2").class("mj-card-title").text("Constellation"))
                    .child(
                        el("div")
                            .class("mj-topology")
                            .attr("data-mj-topology", "registry")
                            .attr("data-mj-topology-src", "/api/v1/graph?id=registry")
                            .attr("role", "img")
                            .attr("aria-label", "A three-dimensional rendering of the registry graph. The same data is on the registry graph page as text."),
                    )
                    .child(
                        el("p").class("mj-note").text(
                            "Rendered only when the drawing library is present, only while the tab is visible, and without animation when the system asks for reduced motion.",
                        ),
                    ),
            )
            .child(card(
                "The same thing, as text",
                el("div")
                    .child(el("div").class("mj-stats")
                        .child(statistic(summary.builtin.to_string(), "builtin capabilities", "the registry"))
                        .child(statistic(summary.mcp_tools.to_string(), "MCP tools", "the registry's MCP exposures"))
                        .child(statistic(summary.http_routes.to_string(), "HTTP routes", "the registry's HTTP exposures"))
                        .child(statistic(summary.cli_commands.to_string(), "CLI commands", "the registry's CLI exposures")))
                    .child(link("/cockpit/graphs/registry", "The registry graph, node by node")),
            )),
    )
    .subtitle("Optional. The Cockpit works without it, and so does everything it shows.")
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Graphs", Some("/cockpit/graphs")),
        ("Topology", None),
    ])
    .script("topology.js")
}

// ------------------------------------------------------------------- activity

/// What this process has actually done: the perf counters, as numbers and as a shape.
pub fn activity(ctx: &Context) -> Page {
    let counters = match ctx.execute("perf.counters", json!({})) {
        Ok(v) => v,
        Err(e) => return failed(Area::Overview, "Activity", e.to_string()),
    };
    let rows: Vec<El> = counters
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v.is_number())
                .map(|(k, v)| row(vec![cell(mono(k)), text_cell(v.to_string())]))
                .collect()
        })
        .unwrap_or_default();
    let phase_rows: Vec<El> = counters
        .get("phases")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    let count = v.get("count").and_then(Value::as_u64).unwrap_or(0);
                    let nanos = v.get("total_nanos").and_then(Value::as_u64).unwrap_or(0);
                    row(vec![
                        cell(mono(k)),
                        text_cell(count.to_string()),
                        text_cell(format!("{:.3} ms", nanos as f64 / 1e6)),
                        text_cell(if count == 0 {
                            "-".to_string()
                        } else {
                            format!("{:.3} ms", nanos as f64 / 1e6 / count as f64)
                        }),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();

    Page::new(
        Area::Overview,
        "Activity",
        el("div")
            .class("mj-grid")
            .child(card(
                "What this is",
                el("p").class("mj-prose").text(
                    "The counters this process keeps: work that must happen once, work that happens per call, and the phases both spend their time in. They are what the structural tests read to prove that no request rebuilds canonical state.",
                ),
            ))
            .child(
                el("section")
                    .class("mj-card")
                    .child(el("h2").class("mj-card-title").text("Executions over time"))
                    .child(
                        el("div")
                            .class("mj-activity")
                            .attr("data-mj-activity", "/api/v1/perf")
                            .attr("role", "img")
                            .attr("aria-label", "A running plot of this process's execution and cache counters. The same numbers are in the table below."),
                    )
                    .child(el("p").class("mj-note").text(
                        "Sampled from /api/v1/perf while this tab is visible. Paused when it is not, and static when the system asks for reduced motion.",
                    )),
            )
            .child(card("Counters", table(&["Counter", "Value"], rows)))
            .child(card(
                "Phases",
                table(&["Phase", "Count", "Total", "Mean"], phase_rows),
            )),
    )
    .subtitle("Measured by this process, not written down anywhere.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Activity", None)])
    .script("activity.js")
}

// ------------------------------------------------------------------ worktrees

/// The word a standing is coloured by. Shown as well as coloured, never colour alone.
fn standing_status(standing: Standing) -> &'static str {
    match standing {
        Standing::Primary | Standing::Canonical => "ok",
        Standing::Misplaced | Standing::Missing => "fail",
        Standing::Ephemeral => "warn",
        Standing::Detached => "info",
    }
}

fn severity_status(severity: crate::model::Severity) -> &'static str {
    match severity {
        crate::model::Severity::Error => "fail",
        crate::model::Severity::Warning => "warn",
        crate::model::Severity::Info => "info",
    }
}

fn short_head(head: &Option<String>) -> String {
    head.as_deref()
        .map(|h| h[..12.min(h.len())].to_string())
        .unwrap_or_else(|| "-".into())
}

fn dirty_cell(w: &WorktreeState) -> El {
    match &w.dirty {
        None => text_cell("-"),
        Some(d) if d.clean => cell(badge("ok", "clean")),
        Some(d) => cell(el("span").child(badge("warn", "dirty")).text(format!(
                    " {} staged · {} unstaged · {} untracked{}",
                    d.staged,
                    d.unstaged,
                    d.untracked,
                    d.in_progress
                        .as_deref()
                        .map(|op| format!(" · {op} in progress"))
                        .unwrap_or_default()
                ))),
    }
}

fn upstream_cell(w: &WorktreeState) -> El {
    match &w.upstream {
        None => text_cell("-"),
        Some(u) if u.gone => cell(el("span").child(mono(&u.name)).text(" (gone)")),
        Some(u) => text_cell(format!(
            "{} +{} −{}",
            u.name,
            u.ahead.unwrap_or(0),
            u.behind.unwrap_or(0)
        )),
    }
}

fn diagnostics_table(diagnostics: &[TopologyDiagnostic]) -> El {
    table(
        &["Severity", "Code", "Where", "What", "Remedy"],
        diagnostics
            .iter()
            .map(|d| {
                row(vec![
                    cell(badge(severity_status(d.severity), d.severity.as_str())),
                    cell(mono(d.code.as_str())),
                    cell(
                        el("span")
                            .child(mono(d.path.clone().unwrap_or_else(|| "-".into())))
                            .when(d.expected.is_some(), |e| {
                                e.child(el("br"))
                                    .text("belongs at ")
                                    .child(mono(d.expected.clone().unwrap_or_default()))
                            }),
                    ),
                    text_cell(&d.message),
                    cell(mono(&d.remedy)),
                ])
            })
            .collect(),
    )
}

/// The branch-to-worktree topology: every worktree with its standing, every branch without
/// one, every diagnostic with its remedy, and the migration plan with the command that
/// applies it. Rendered from `worktree.topology` and `worktree.migration_plan`; the page
/// reloads itself when the topology changes.
pub fn worktrees(ctx: &Context) -> Page {
    let t: RepositoryTopology = match ask(ctx, "worktree.topology", json!({})) {
        Ok(t) => t,
        Err(e) => return failed(Area::Worktrees, "Worktrees", e),
    };
    let plan: Option<MigrationPlan> = ask(ctx, "worktree.migration_plan", json!({})).ok();

    let identity = card(
        "This repository",
        facts(vec![
            ("Primary checkout", Node::Element(mono(&t.repository.primary_worktree))),
            (
                "Container",
                Node::Element(
                    el("span")
                        .child(mono(&t.container.path))
                        .text(if t.container.exists { "" } else { " (not created yet)" }),
                ),
            ),
            (
                "Trunk",
                Node::Element(
                    el("span")
                        .child(mono(t.trunk.branch.clone().unwrap_or_else(|| "(unknown)".into())))
                        .text(format!(" — decided by {}", word(&t.trunk.source).replace('_', " "))),
                ),
            ),
            ("Rule", Node::Element(el("span").text("<repo>").child(mono(&t.container.suffix)).text("/<branch>: the branch name is the path, hierarchy kept, derived from git and registered nowhere"))),
            (
                "Topology",
                Node::Element(if t.valid {
                    badge("ok", "valid")
                } else {
                    badge("fail", format!("{} error(s)", t.tallies.errors))
                }),
            ),
        ]),
    );

    let ta = &t.tallies;
    let statistics = el("div")
        .class("mj-stats")
        .child(statistic(
            ta.worktrees.to_string(),
            "worktrees",
            "worktree.topology",
        ))
        .child(statistic(
            ta.canonical.to_string(),
            "canonical",
            "worktree.topology",
        ))
        .child(statistic(
            ta.misplaced.to_string(),
            "misplaced",
            "worktree.topology",
        ))
        .child(statistic(
            ta.ephemeral.to_string(),
            "ephemeral",
            "worktree.topology",
        ))
        .child(statistic(
            ta.detached.to_string(),
            "detached",
            "worktree.topology",
        ))
        .child(statistic(
            ta.dirty.to_string(),
            "with uncommitted work",
            "git status, one per worktree",
        ))
        .child(statistic(
            ta.branches_without_worktree.to_string(),
            "branches without a worktree",
            "for-each-ref",
        ))
        .child(statistic(
            ta.cleanup_eligible.to_string(),
            "cleanup-eligible branches",
            "merged into the trunk, clean or absent",
        ));

    let worktree_rows: Vec<El> = t
        .worktrees
        .iter()
        .map(|w| {
            row(vec![
                cell(badge(standing_status(w.standing), w.standing.as_str())),
                cell(
                    el("span")
                        .child(mono(&w.label))
                        .when(w.current, |e| e.text(" ").child(tag("here")))
                        .when(w.issue.is_some(), |e| {
                            e.text(" ").child(tag(w.issue.clone().unwrap_or_default()))
                        }),
                ),
                cell(
                    el("span")
                        .child(mono(&w.path))
                        .when(
                            w.expected_path.is_some() && !matches!(w.standing, Standing::Canonical),
                            |e| {
                                e.child(el("br"))
                                    .text("belongs at ")
                                    .child(mono(w.expected_path.clone().unwrap_or_default()))
                            },
                        )
                        .when(w.locked.is_some(), |e| e.text(" ").child(tag("locked"))),
                ),
                cell(mono(short_head(&w.head))),
                dirty_cell(w),
                upstream_cell(w),
            ])
        })
        .collect();
    let worktrees_card = card_with(
        "Worktrees",
        link(
            "/cockpit/capabilities/worktree.topology",
            "worktree.topology",
        ),
        table(
            &[
                "Standing",
                "Branch",
                "Path",
                "HEAD",
                "Uncommitted work",
                "Upstream",
            ],
            worktree_rows,
        ),
    );

    let without: Vec<&BranchState> = t.branches.iter().filter(|b| b.worktree.is_none()).collect();
    let branches_card = if without.is_empty() {
        card(
            "Branches without a worktree",
            nothing("Every local branch is checked out somewhere."),
        )
    } else {
        card(
            "Branches without a worktree",
            table(
                &[
                    "Branch",
                    "HEAD",
                    "Merged into the trunk",
                    "Would go to",
                    "Start",
                ],
                without
                    .iter()
                    .map(|b| {
                        row(vec![
                            cell(
                                el("span")
                                    .child(mono(&b.name))
                                    .when(b.issue.is_some(), |e| {
                                        e.text(" ").child(tag(b.issue.clone().unwrap_or_default()))
                                    }),
                            ),
                            cell(mono(short_head(&Some(b.head.clone())))),
                            cell(match b.merged_into_trunk {
                                Some(true) if b.cleanup_eligible => {
                                    badge("ok", "merged, cleanup-eligible")
                                }
                                Some(true) => badge("ok", "merged"),
                                Some(false) => badge("info", "unmerged"),
                                None => badge("unknown", "trunk unknown"),
                            }),
                            cell(mono(b.expected_path.clone().unwrap_or_else(|| "-".into()))),
                            cell(mono(format!("majordomus worktree create {}", b.name))),
                        ])
                    })
                    .collect(),
            ),
        )
    };

    let diagnostics_card = if t.diagnostics.is_empty() {
        card(
            "Diagnostics",
            nothing("Every worktree is where it belongs. Nothing to report."),
        )
    } else {
        card_with(
            "Diagnostics",
            badge(
                if t.valid { "ok" } else { "fail" },
                format!("{} error(s), {} warning(s)", ta.errors, ta.warnings),
            ),
            diagnostics_table(&t.diagnostics),
        )
    };

    let migration_card = match &plan {
        None => card("Migration", nothing("The migration plan could not be computed.")),
        Some(p) if p.steps.is_empty() => card_with(
            "Migration",
            link("/cockpit/capabilities/worktree.migration_plan", "worktree.migration_plan"),
            el("div")
                .child(nothing("Nothing to migrate: every worktree with a branch is at its canonical path."))
                .when(!p.exceptions.is_empty(), |d| {
                    d.child(el("p").class("mj-note").text(format!(
                        "{} worktree(s) are not migrated by design — detached, ephemeral, or the primary checkout — and are listed under diagnostics.",
                        p.exceptions.len()
                    )))
                }),
        ),
        Some(p) => card_with(
            "Migration",
            badge("warn", format!("{} movable, {} blocked", p.movable, p.blocked)),
            el("div")
                .child(el("p").class("mj-prose").text(
                    "Each step moves one worktree with git, uncommitted work included, fingerprinted before and after; a step is reported as moved only when the two fingerprints are equal. Nothing here changes anything: the commands below do, from a terminal.",
                ))
                .child(table(
                    &["Branch", "From", "To", "Uncommitted work", "Action", "Blocked by"],
                    p.steps
                        .iter()
                        .map(|s| {
                            row(vec![
                                cell(mono(&s.branch)),
                                cell(mono(&s.from)),
                                cell(mono(&s.to)),
                                text_cell(s.dirty.summary()),
                                cell(match s.outcome {
                                    StepOutcome::Planned => badge("ok", word(&s.action).replace('_', " ")),
                                    StepOutcome::Blocked => badge("fail", "blocked"),
                                    StepOutcome::Moved => badge("ok", "moved"),
                                    StepOutcome::Failed => badge("fail", "failed"),
                                }),
                                cell(el("span").children(
                                    s.blockers
                                        .iter()
                                        .map(|b| el("div").child(mono(b.code.as_str())).text(format!(" {}", b.message)))
                                        .collect::<Vec<_>>(),
                                )),
                            ])
                        })
                        .collect(),
                ))
                .child(pre(format!(
                    "majordomus worktree migrate --plan   # the same plan, from a terminal\nmajordomus worktree migrate          # apply the {} movable step(s), verified\n{}",
                    p.movable,
                    if p.blocked > 0 { "# blocked steps say what to do; nothing is overwritten or forced\n" } else { "" }
                ))),
        ),
    };

    let actions = card(
        "Commands",
        el("div")
            .child(el("p").class("mj-prose").text(
                "The Cockpit reads; the command line changes things. Every command below is the same service this page renders, and none of them takes a path — the path is derived.",
            ))
            .child(pre(
                "majordomus worktree                          # where am I, and is that where I belong\nmajordomus worktree create <branch>          # start work: the branch from the trunk, the worktree at its path\ncd \"$(majordomus worktree path <branch>)\"\nmajordomus worktree migrate --plan           # what would move; nothing changes\nmajordomus worktree migrate                  # move, verify, report\nmajordomus worktree repair                   # drop stale registrations; deletes no directory\nmajordomus worktree cleanup                  # merged and clean: what could go, and how; deletes nothing\nmajordomus worktree remove <branch>          # one linked worktree; never dirty work unforced, never a branch",
            )),
    );

    let live = el("section")
        .class("mj-card")
        .attr("data-mj-worktrees", "/api/v1/worktrees")
        .child(el("h2").class("mj-card-title").text("Live"))
        .child(el("p").class("mj-note").attr("data-mj-worktrees-note", "").text(
            "While this page is visible it asks /api/v1/worktrees every few seconds and reloads when a worktree is created, moved or removed, so what you see is what git holds now.",
        ));

    Page::new(
        Area::Worktrees,
        "Worktrees",
        el("div")
            .class("mj-grid")
            .child(statistics)
            .child(identity)
            .child(worktrees_card)
            .child(diagnostics_card)
            .child(migration_card)
            .child(branches_card)
            .child(actions)
            .child(live),
    )
    .subtitle("Where every branch's worktree belongs and where each one is: <repo>-wt/<branch>, derived from git and registered nowhere.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Worktrees", None)])
    .script("worktrees.js")
}

// ------------------------------------------------------------------ not found

/// A page for a path the Cockpit does not serve.
pub fn not_found(path: &str) -> Page {
    Page::new(
        Area::None,
        "Not a page",
        el("div")
            .child(alert(
                "fail",
                format!("The Cockpit serves no page at {path}."),
            ))
            .child(
                el("ul")
                    .class("mj-list")
                    .child(el("li").child(link("/cockpit", "Overview")))
                    .child(el("li").child(link("/cockpit/capabilities", "Capabilities")))
                    .child(el("li").child(link("/cockpit/objects", "Objects")))
                    .child(el("li").child(link("/cockpit/graphs", "Graphs")))
                    .child(el("li").child(link("/cockpit/health", "Health"))),
            ),
    )
    .status(404)
}

/// Whether a registry holds a capability at all: used by the router to tell a missing
/// page from a missing capability.
pub fn exists(registry: &CapabilityRegistry, id: &str) -> bool {
    registry.get(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nullable_integer_is_edited_as_a_number() {
        let schema = json!({ "anyOf": [{ "type": "integer" }, { "type": "null" }] });
        assert_eq!(schema_shape(&schema).0, FieldKind::Integer);
    }

    #[test]
    fn an_enumeration_becomes_a_select() {
        let schema = json!({ "enum": ["a", "b"] });
        let (_, values) = schema_shape(&schema);
        assert_eq!(
            values.as_deref(),
            Some(["a".to_string(), "b".to_string()].as_slice())
        );
    }

    #[test]
    fn an_untyped_property_is_edited_as_json() {
        assert_eq!(schema_shape(&json!({})).0, FieldKind::Structured);
        assert_eq!(
            schema_shape(&json!({ "$ref": "#/$defs/Thing" })).0,
            FieldKind::Structured
        );
    }

    #[test]
    fn a_field_carries_its_description_and_its_requiredness() {
        let rendered = field(
            "limit",
            &json!({ "type": "integer", "description": "How many.", "maximum": 50 }),
            true,
        )
        .render();
        assert!(rendered.contains("How many."), "{rendered}");
        assert!(rendered.contains("required"), "{rendered}");
        assert!(rendered.contains("max=\"50\""), "{rendered}");
        assert!(rendered.contains("data-mj-type=\"integer\""), "{rendered}");
    }
}
