//! The pages. Each one asks the executor for a capability's output and lays it out; none
//! of them knows anything the registry or the index does not already hold.
//!
//! Every page goes through [`crate::capability::Context::execute`], the same call MCP and
//! the HTTP routes make. That is deliberate and not incidental: the Cockpit is counted in
//! the same perf counters, answered from the same cache, and bound by the same validation
//! as every other caller. A page that reached into the index directly would be a fourth
//! way of reading the repository.

use serde_json::{json, Value};

use crate::capability::builtin::{GraphList, Health, HealthStatus, ObjectList, RepositoryReport};
use crate::capability::{Capability, CapabilityKind, CapabilityRegistry, Context, Provenance};
use crate::generate;
use crate::graph::Graph;
use crate::http::openapi;
use crate::http::router::percent_encode;

use super::html::{el, empty, El, Node};
use super::nav::Area;
use super::view::{
    alert, badge, card, card_with, cell, details, facts, kind_badge, link, mono, nothing, pre, row,
    statistic, table, tag, text_cell,
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
    fn new(area: Area, title: impl Into<String>, main: El) -> Self {
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
    fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }
    fn trail(mut self, trail: Vec<(&str, Option<&str>)>) -> Self {
        self.breadcrumbs = trail
            .into_iter()
            .map(|(l, h)| (l.to_string(), h.map(str::to_string)))
            .collect();
        self
    }
    fn script(mut self, name: &'static str) -> Self {
        self.scripts.push(name);
        self
    }
    fn status(mut self, status: u16) -> Self {
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
fn ask<T: serde::de::DeserializeOwned>(ctx: &Context, id: &str, input: Value) -> Result<T, String> {
    let value = ctx.execute(id, input).map_err(|e| e.to_string())?;
    serde_json::from_value(value).map_err(|e| format!("{id} answered something unexpected: {e}"))
}

/// A page that says what went wrong instead of showing a blank one.
fn failed(area: Area, title: &str, reason: String) -> Page {
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

    let rows: Vec<El> = matching
        .iter()
        .take(500)
        .map(|c| {
            row(vec![
                cell(
                    link(
                        format!("/cockpit/capabilities/{}", percent_encode(c.id.as_str())),
                        c.id.as_str(),
                    )
                    .class("mj-link mj-mono"),
                ),
                cell(kind_badge(c.kind)),
                text_cell(&c.title),
                cell(mono(c.module.as_str())),
                cell(projection_marks(c)),
                cell(provenance_badge(c)),
            ])
        })
        .collect();

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

    let truncated = matching.len() > 500;
    let body = if rows.is_empty() {
        nothing("No capability matches these filters.")
    } else {
        el("div")
            .child(table(
                &["Id", "Kind", "Title", "Module", "Projections", "Provenance"],
                rows,
            ))
            .when(truncated, |d| {
                d.child(alert(
                    "info",
                    format!(
                        "{} capabilities match; the first 500 are shown. Narrow the filters to see the rest.",
                        matching.len()
                    ),
                ))
            })
    };

    Page::new(
        Area::Capabilities,
        "Capabilities",
        el("div").child(filters).child(body),
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
                        format!("{}#/{}/{}", crate::http::swagger::DOCS_PATH, c.module, c.id)
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
                        .attr("tabindex", "0")
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

    let input = match &kind {
        Some(k) => json!({ "kind": k }),
        None => json!({}),
    };
    let list: ObjectList = match ask(ctx, "objects.list", input) {
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

    let matching: Vec<_> = list
        .objects
        .iter()
        .filter(|o| {
            needle.as_deref().is_none_or(|n| {
                o.identity.to_lowercase().contains(n)
                    || o.title
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(n)
                    || o.path.to_lowercase().contains(n)
            })
        })
        .collect();

    let rows: Vec<El> = matching
        .iter()
        .take(1000)
        .map(|o| {
            row(vec![
                cell(
                    link(
                        format!("/cockpit/object?uri={}", percent_encode(&o.uri)),
                        &o.identity,
                    )
                    .class("mj-link mj-mono"),
                ),
                cell(mono(&o.kind)),
                text_cell(o.title.clone().unwrap_or_default()),
                cell(mono(&o.path)),
            ])
        })
        .collect();

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
        el("div").child(filters).child(if rows.is_empty() {
            nothing("No object matches.")
        } else {
            table(&["Identity", "Kind", "Title", "Path"], rows)
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
                cell(link(format!("/cockpit/graphs/{}", g.id), &g.id).class("mj-link mj-mono")),
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

// --------------------------------------------------------------------- health

/// The health report, in full: the expensive comparison included.
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
                cell(
                    link(
                        format!("/cockpit/capabilities/{}", percent_encode(c.id.as_str())),
                        c.id.as_str(),
                    )
                    .class("mj-link mj-mono"),
                ),
                text_cell(&c.title),
                cell(kind_badge(c.kind)),
            ])
        })
        .collect();
    rows.sort_by_key(|r| r.render());

    let infrastructure = table(
        &["Path", "What it is"],
        openapi::INFRASTRUCTURE_ROUTES
            .iter()
            .map(|path| {
                row(vec![
                    cell(mono(*path)),
                    text_cell(if *path == "/" {
                        "the index: what this server is and where its surfaces are"
                    } else if *path == crate::http::swagger::SPEC_PATH {
                        "the OpenAPI document, built from the registry per process"
                    } else if *path == crate::http::swagger::DOCS_PATH {
                        "Swagger UI over that document"
                    } else if *path == crate::http::mcp::PATH {
                        "MCP over HTTP, when this process serves a shared server"
                    } else {
                        "a route of the projection itself"
                    }),
                ])
            })
            .chain(std::iter::once(row(vec![
                cell(mono("/cockpit")),
                text_cell("this Cockpit: server-rendered pages over the same registry"),
            ])))
            .collect(),
    );

    Page::new(
        Area::Api,
        "API",
        el("div")
            .class("mj-grid")
            .child(card_with(
                "Swagger UI",
                link(crate::http::swagger::DOCS_PATH, "Open"),
                el("p").class("mj-prose").text(
                    format!("Swagger UI is served from this process and reads {}, which is generated from the registry at first request. Nothing about an operation is written twice: the descriptions, the schemas and the examples are the capability's own.", crate::http::swagger::SPEC_PATH),
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
                cell(
                    link(
                        format!("/cockpit/capabilities/{}", percent_encode(c.id.as_str())),
                        c.id.as_str(),
                    )
                    .class("mj-link mj-mono"),
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
                        cell(
                            link(
                                format!("/cockpit/object?uri={}", percent_encode(uri)),
                                h.get("identity").and_then(Value::as_str).unwrap_or(uri),
                            )
                            .class("mj-link mj-mono"),
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
