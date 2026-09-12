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
    ArtifactReport, CheckState, CommandIndex, Continuity, DesignReport, DirectoryReport,
    DirectoryState, EventHistory, ExecutionList, ExecutionView, GraphList, Health, HealthStatus,
    InstallabilityReport, NodeList, ObjectList, ObjectSummary, QualityAnswer, Record,
    RepositoryReport, TokenList,
};
use crate::capability::{
    Capability, CapabilityKind, Context, Effect as CapabilityEffect, Provenance,
};
use crate::command_graph::CommandNode;
use crate::execution::{Execution, ExecutionState, StepState};
use crate::generate;
use crate::graph::Graph;
use crate::http::router::percent_encode;
use crate::release::compat::{Impact, Severity, Status as ReleaseStatus, VersionPlan};
use crate::worktree::{
    BranchState, MigrationPlan, RepositoryTopology, Standing, StepOutcome, TopologyDiagnostic,
    WorktreeState,
};

use crate::capability::builtin::lifecycle::{
    ClosedSessions, EpisodeStanding, Episodes, PointerLayout, ProviderLifecycles, Recovery,
    RuntimeView,
};

use super::html::{el, empty, El, Node};
use super::nav::Area;
use super::view::{
    alert, badge, card, card_with, cell, chips, details, facts, id_cell, kind_badge, link, mono,
    nothing, pagination, pre, row, statistic, table, tag, text_cell, word_badge, Window, PER_PAGE,
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
            .children(distribution_card(ctx).into_iter().collect::<Vec<_>>())
            .child(kinds)
            .child(diagnostics),
    )
    .subtitle(crate::about::SUMMARY)
}

/// The distribution card: whether the command the README advertises works right now.
///
/// It asks `distribution.status`, which is the same capability the command line, the HTTP
/// route and the MCP tool answer from, so no number here is computed twice and none is
/// written down. A repository that carries no distribution model gets no card rather than
/// a card full of dashes.
fn distribution_card(ctx: &Context) -> Option<El> {
    let report: InstallabilityReport = ask(ctx, "distribution.status", json!({})).ok()?;
    let verdict = if report.installable {
        badge("ok", "healthy")
    } else {
        badge("fail", "blocked")
    };
    let mut rows = vec![
        (
            "Local version",
            Node::Element(mono(format!("v{}", report.local_version))),
        ),
        (
            "Stable release",
            Node::Element(mono(
                report.stable_tag.clone().unwrap_or_else(|| "none".into()),
            )),
        ),
        (
            "Artifacts",
            Node::Element(mono(format!(
                "{}/{}",
                report.published_artifacts, report.required_targets
            ))),
        ),
        ("Public install", Node::Element(verdict)),
    ];
    // Why, and what to do about it — the same cause and next action every other projection
    // of this capability shows, rather than a second wording of them here.
    if !report.installable {
        if let Some(c) = report.checks.iter().find(|c| c.state == CheckState::Failed) {
            if let Some(cause) = &c.cause {
                rows.push(("Reason", Node::Element(el("span").text(cause))));
            }
            if let Some(next) = &c.next {
                rows.push(("Next", Node::Element(mono(next))));
            }
        }
    }
    Some(card_with(
        "Distribution",
        link(
            "/cockpit/capabilities/distribution.status",
            "distribution.status",
        ),
        el("div").child(facts(rows)).child(
            el("p")
                .class("mj-note")
                .child(mono(&report.install_command)),
        ),
    ))
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
fn href_with(base: &str, query: &[(String, String)], set: &[(&str, Option<&str>)]) -> String {
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
fn asked_page(query: &[(String, String)]) -> usize {
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
    kind.as_str()
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

    // the route that starts an execution, from the registry that declares it: the page
    // names no path of its own
    let start_route = ctx
        .registry
        .get("executions.start")
        .and_then(|s| s.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .unwrap_or_default();
    let runner = match (&c.exposure.http, c.stability.executable()) {
        (Some(http), true) => runner_form(c, http, &start_route),
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
    if c.stability.executable() && !start_route.is_empty() {
        page = page.script("executions.js");
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
fn runner_form(c: &Capability, http: &crate::capability::HttpExposure, start_route: &str) -> El {
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
        // what the execution button needs, from the descriptor rather than from a list in
        // the script: which capability to start, where to start it, and whether the page
        // should ask first
        .attr("data-mj-start", start_route)
        .attr("data-mj-executions", "/cockpit/executions")
        .attr(
            "data-mj-confirm",
            if c.execution.needs_confirmation() {
                "yes"
            } else {
                "no"
            },
        )
        .attr("data-mj-effect", word(&c.execution.effect))
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
                    el("button")
                        .class("mj-button")
                        .attr("type", "button")
                        .attr("data-mj-execute", c.id.as_str())
                        .attr(
                            "title",
                            "Start it as an execution: it gets an id, a page of its own and a live stream of what it is doing",
                        )
                        .text("Run as an execution"),
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

    // what the page says about running this comes from the descriptor's own policy, never
    // from a list of capabilities that need care
    // The sentence is derived from the declared effect, not from the kind: two commands are
    // the same kind whether one announces a peer and the other writes a tracked record, and
    // telling a person the second is the first is how a control comes to lie.
    let effect = match (c.kind, c.execution.effect, c.execution.cancellable) {
        (CapabilityKind::Command, CapabilityEffect::RepositoryMutation, _) => alert(
            "warn",
            "A command that writes the repository. It changes tracked files a commit will carry, and is sent as a POST from this page's origin.",
        ),
        (CapabilityKind::Command, _, _) => alert(
            "warn",
            "A command. It changes this process's own memory — not the repository — and is sent as a POST from this page's origin.",
        ),
        (_, _, true) => alert(
            "info",
            "It reads and changes nothing, and it stops when it is asked to. Run it as an execution to watch it happen and to be able to cancel it.",
        ),
        (_, _, false) => alert("info", "A query. It reads and changes nothing."),
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
                    Some(s) => word_badge(s),
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
///
/// Freshness is a second badge beside it and not a shade of the first, because the two
/// answer different questions and their disagreement is the interesting case. This page
/// showed one badge until ADR 0052, so a handover that was `advanced` and six days dead
/// rendered as a calm blue `advanced` with its `Next action` printed underneath in full.
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
    let fresh_level = match r.freshness.as_str() {
        "fresh" => "ok",
        "aging" => "info",
        "unknown" => "warn",
        _ => "fail",
    };
    // One badge carrying both words, at the worse of the two levels. Two elements would
    // want a flex container and a gap, and the stylesheet is a Tailwind build — a new class
    // here is a build step for a separator. What matters is that `advanced` can no longer
    // appear alone: the reader sees `advanced · stale` and the two stop agreeing in public.
    let worst = if fresh_level == "fail" || level == "fail" {
        "fail"
    } else if fresh_level == "warn" || level == "warn" {
        "warn"
    } else if fresh_level == "info" || level == "info" {
        "info"
    } else {
        "ok"
    };
    card_with(
        title,
        badge(
            worst,
            format!("{} · {}", r.divergence.as_str(), r.freshness.as_str()),
        ),
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
                (
                    "Age",
                    Node::Element(el("span").text(&r.freshness_reason)),
                ),
            ]))
            .when(!r.divergence.trustworthy(), |d| {
                d.child(alert(
                    "fail",
                    "The commit this record was written at is not in this history. Trust git over anything it says.",
                ))
            })
            .when(!r.next_action_withheld.is_empty(), |d| {
                d.child(alert("fail", &r.next_action_withheld))
            })
            .when(!r.next_action.is_empty(), |d| {
                d.child(el("h3").class("mj-card-title").text("Next action"))
                    .child(pre(r.next_action.clone()))
            }),
    )
}

/// Every open episode of this checkout's store, and not only the one the pointer follows.
///
/// The card above this one is `continuity.state`'s: the episode `session-current.yaml`
/// resolves to, which is the episode the briefing is about and the only one that surface can
/// name. This table is the store. On 2026-09-11 this repository held five open episodes in
/// one checkout, four of which no surface could see, and an episode nobody can see is an
/// episode that never closes.
///
/// The standing is a word beside a badge because the difference between `open` and
/// `stranded` is the difference between "another window is using this" and "nothing can ever
/// close this", and a reader must not have to infer that from a colour.
fn episodes_card(e: &Episodes) -> El {
    if e.episodes.is_empty() {
        return card(
            "Every open episode",
            nothing(if e.present {
                "The store is here and holds no open episode. Nothing is open in this repository."
            } else {
                "This checkout has no open-episode store yet. That is a fresh clone, not a fault: the first start event creates it."
            }),
        );
    }
    let rows: Vec<El> = e
        .episodes
        .iter()
        .map(|x| {
            let level = match x.standing {
                EpisodeStanding::Current => "ok",
                EpisodeStanding::Open => "info",
                EpisodeStanding::Foreign => "warn",
                EpisodeStanding::Stranded => "fail",
            };
            row(vec![
                cell(mono(x.session_id.clone())),
                cell(badge(level, x.standing.as_str())),
                cell(el("span").text(if x.provider.is_empty() {
                    "(opened by hand)"
                } else {
                    &x.provider
                })),
                cell(mono(x.branch.clone())),
                text_cell(x.started_at.clone()),
                cell(if x.last_activity.is_empty() {
                    el("span").text("(has written nothing)")
                } else {
                    el("span").text(format!("{} · {}", x.last_activity, x.last_event))
                }),
                cell(if x.tasks.is_empty() {
                    el("span").text("—")
                } else {
                    el("div")
                        .class("mj-marks")
                        .children(x.tasks.iter().map(|t| mono(t.clone())).collect::<Vec<_>>())
                }),
                text_cell(x.events.to_string()),
            ])
        })
        .collect();
    card_with(
        "Every open episode",
        badge(
            if e.episodes.len() > 1 { "info" } else { "ok" },
            format!("{} open", e.episodes.len()),
        ),
        el("div")
            .child(el("p").class("mj-prose").text(
                "One episode per provider window, read from the store rather than from the pointer. Only the one marked `current` is the episode the briefing above is about; the others are other workers, and their ledger lines are stamped with their own ids.",
            ))
            .child(table(
                &[
                    "Episode",
                    "Standing",
                    "Provider",
                    "Branch",
                    "Opened",
                    "Last seen in the ledger",
                    "Tasks it touched",
                    "Events",
                ],
                rows,
            ))
            .children(
                e.findings
                    .iter()
                    .map(|f| alert("info", f.clone()))
                    .collect::<Vec<_>>(),
            ),
    )
}

/// The commit this process is answering about, against the commit the repository is on.
///
/// A server that built its index once and held it answered every question — the API, MCP,
/// this page — about a commit from whenever it started, and said nothing about it. There is
/// no way to notice that from inside an answer, so the comparison is put on the page.
fn runtime_card(r: &RuntimeView) -> El {
    let short = |h: &str| h[..7.min(h.len())].to_string();
    card_with(
        "This process against the repository",
        badge(
            if r.agree { "ok" } else { "fail" },
            if r.agree { "current" } else { "behind" },
        ),
        el("div")
            .child(facts(vec![
                ("Serving", Node::Element(mono(short(&r.served_head)))),
                (
                    "Repository is on",
                    Node::Element(mono(short(&r.repository_head))),
                ),
                (
                    "Branch served",
                    Node::Element(mono(r.served_branch.clone())),
                ),
                (
                    "Branch now",
                    Node::Element(mono(r.repository_branch.clone())),
                ),
                (
                    "Working tree now",
                    Node::Element(el("span").text(&r.repository_working_tree)),
                ),
                (
                    "Objects in the served index",
                    Node::Element(el("span").text(r.objects.to_string())),
                ),
                ("Index", Node::Element(el("span").text(&r.index_state))),
            ]))
            .child(alert(if r.agree { "ok" } else { "fail" }, r.note.clone())),
    )
}

/// What each provider's adapter declares it can do, and what this repository wires to it.
///
/// Declared, never guessed. The obvious implementation reads `.claude/hooks/` and reports
/// what it finds, which answers a different question — whether somebody ran the installer —
/// and answers it as though it were a statement about the provider.
fn providers_card(p: &ProviderLifecycles) -> El {
    let rows: Vec<El> = p
        .providers
        .iter()
        .map(|x| {
            row(vec![
                cell(el("span").text(&x.title)),
                cell(if x.lifecycle.is_empty() {
                    el("span").text("(no lifecycle adapter)")
                } else {
                    el("div").class("mj-marks").children(
                        x.lifecycle
                            .iter()
                            .map(|e| tag(e.clone()))
                            .collect::<Vec<_>>(),
                    )
                }),
                cell(badge(
                    if x.prompt_capture { "ok" } else { "info" },
                    if x.prompt_capture { "yes" } else { "no" },
                )),
                cell(if x.client_config.is_empty() {
                    el("span").text("—")
                } else {
                    mono(x.client_config.clone())
                }),
                cell(if x.wired.is_empty() {
                    el("span").text("—")
                } else {
                    el("div")
                        .class("mj-marks")
                        .children(x.wired.iter().map(|w| tag(w.clone())).collect::<Vec<_>>())
                }),
            ])
        })
        .collect();
    card_with(
        "Providers",
        badge(
            "info",
            format!("{} of {} with a lifecycle adapter", p.with_lifecycle, p.providers.len()),
        ),
        el("div")
            .child(el("p").class("mj-prose").text(
                "What the distribution declares each adapter can do, in the provider's own vocabulary, beside the enforcement entries this repository's policy wires to its hook. Whether a shim is installed in this checkout is a different question, and `majordomus capture status` is the command that owns it.",
            ))
            .child(table(
                &["Provider", "Lifecycle events", "Prompt capture", "Client configuration", "Wired here"],
                rows,
            ))
            .children(
                p.findings
                    .iter()
                    .map(|f| alert("warn", f.clone()))
                    .collect::<Vec<_>>(),
            ),
    )
}

/// What the store needs somebody to do: stranded episodes, orphan temporary files, the
/// migration status of the pointer, and the started-against-closed arithmetic.
///
/// Every row carries the command that clears it. A finding with no remedy is a complaint,
/// and a page full of complaints teaches a reader to stop reading it.
fn recovery_card(r: &Recovery) -> El {
    let clean = r.stranded.is_empty()
        && r.orphans.is_empty()
        && r.balance.agrees
        && r.pointer.layout != PointerLayout::Inline;
    let body = el("div")
        .child(facts(vec![
            (
                "Pointer",
                Node::Element(badge(
                    match r.pointer.layout {
                        PointerLayout::Pointer if r.pointer.resolves => "ok",
                        PointerLayout::Absent => "info",
                        _ => "warn",
                    },
                    r.pointer.layout.as_str(),
                )),
            ),
            (
                "Episodes accounted for",
                Node::Element(badge(
                    if r.balance.agrees { "ok" } else { "warn" },
                    format!(
                        "{} started · {} closed · {} open",
                        r.balance.started, r.balance.closed, r.balance.open
                    ),
                )),
            ),
        ]))
        .child(alert("info", r.pointer.note.clone()))
        .child(alert(
            if r.balance.agrees { "info" } else { "warn" },
            r.balance.note.clone(),
        ))
        .when(!r.stranded.is_empty(), |d| {
            d.child(el("h3").class("mj-card-title").text("Cannot close themselves"))
                .child(table(
                    &["Episode", "Why", "Clears it"],
                    r.stranded
                        .iter()
                        .map(|x| {
                            row(vec![
                                cell(mono(x.session_id.clone())),
                                cell(el("span").text(&x.reason)),
                                cell(mono(x.remedy.clone())),
                            ])
                        })
                        .collect(),
                ))
        })
        .when(!r.orphans.is_empty(), |d| {
            d.child(el("h3").class("mj-card-title").text("Temporary files left behind"))
                .child(table(
                    &["Path", "Bytes"],
                    r.orphans
                        .iter()
                        .map(|o| row(vec![cell(mono(o.path.clone())), text_cell(o.bytes.to_string())]))
                        .collect(),
                ))
        })
        .when(clean, |d| {
            d.child(nothing(
                "Nothing to recover: every open record names a worktree that exists, no close left a temporary file behind, and the ledger's arithmetic adds up.",
            ))
        });
    card_with(
        "Recovery",
        badge(
            if clean { "ok" } else { "warn" },
            if clean {
                "clean".to_string()
            } else {
                format!("{} to look at", r.findings.len())
            },
        ),
        body,
    )
}

/// The tracked records a clone receives — the only half of this subsystem that travels.
fn closed_card(c: &ClosedSessions) -> El {
    if c.total == 0 {
        return card(
            "Closed episodes",
            nothing("No episode has closed into the layer's sessions section yet."),
        );
    }
    let rows: Vec<El> = c
        .newest
        .iter()
        .map(|x| {
            row(vec![
                cell(link(
                    format!("/cockpit/object?uri={}", percent_encode(&x.uri)),
                    x.session_id.clone(),
                )),
                text_cell(x.created_at.clone()),
                cell(mono(x.branch.clone())),
                cell(if x.outcome.is_empty() {
                    el("span").text("—")
                } else {
                    word_badge(&x.outcome)
                }),
                cell(el("span").text(&x.title)),
            ])
        })
        .collect();
    card_with(
        "Closed episodes",
        badge("info", format!("{} tracked", c.total)),
        el("div")
            .child(el("div").class("mj-stats").children(vec![
                statistic(c.total.to_string(), "closed records", "the session records"),
                statistic(
                    c.on_this_branch.to_string(),
                    "on this branch",
                    "the session records",
                ),
                statistic(c.window.to_string(), "shown below", "the session records"),
            ]))
            .child(table(
                &["Episode", "Closed", "Branch", "Outcome", "Title"],
                rows,
            )),
    )
}

/// One section that could not be read, as a card rather than as a dead page.
///
/// The continuity page asks six capabilities now. Failing the whole page because one of them
/// could not answer would hide the five that could, and the one a reader most needs is the
/// one most likely to fail: a store nothing has written yet.
fn section_error(title: &str, e: String) -> El {
    card(title, alert("fail", e))
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
                        .map(|(k, n)| {
                            statistic(n.to_string(), k.clone(), "this checkout's local state")
                        })
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
            .child(match ask::<Episodes>(ctx, "lifecycle.episodes", json!({})) {
                Ok(e) => episodes_card(&e),
                Err(e) => section_error("Every open episode", e),
            })
            .child(match ask::<RuntimeView>(ctx, "lifecycle.runtime", json!({})) {
                Ok(r) => runtime_card(&r),
                Err(e) => section_error("This process against the repository", e),
            })
            .child(match ask::<Recovery>(ctx, "lifecycle.recovery", json!({})) {
                Ok(r) => recovery_card(&r),
                Err(e) => section_error("Recovery", e),
            })
            .child(match ask::<ProviderLifecycles>(ctx, "lifecycle.providers", json!({})) {
                Ok(p) => providers_card(&p),
                Err(e) => section_error("Providers", e),
            })
            .child(match ask::<ClosedSessions>(ctx, "lifecycle.closed", json!({})) {
                Ok(c) => closed_card(&c),
                Err(e) => section_error("Closed episodes", e),
            })
            .node(findings),
    )
    .subtitle("What this checkout's lifecycle is holding, and what the subsystem around it is doing. Local to this machine, served here and published nowhere.")
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

/// The model catalogue and its routing: the vendors and models `share/models.yaml`
/// declares, with each vendor's credential *presence* (never a value), rendered from
/// the same `models.list` every other surface reads. No model name lives in this page.
pub fn models(ctx: &Context) -> Page {
    let report: crate::capability::builtin::ModelsReport = match ask(ctx, "models.list", json!({}))
    {
        Ok(r) => r,
        Err(e) => return failed(Area::Models, "Models", e),
    };
    let vendors: Vec<El> = report
        .vendors
        .iter()
        .map(|v| {
            let credential = match v.credential_configured {
                Some(true) => "credential configured",
                Some(false) => "credential not configured",
                None => "no credential declared",
            };
            card_with(
                format!("{} — {}", v.vendor.title, v.vendor.id),
                word_badge(match v.vendor.inference {
                    crate::models::Inference::Remote => "remote",
                    crate::models::Inference::Local => "local",
                }),
                el("p").class("mj-prose").text(credential),
            )
        })
        .collect();
    let models: Vec<El> = report
        .models
        .iter()
        .map(|m| {
            let status = match m.status {
                crate::models::ModelStatus::Available => "available",
                crate::models::ModelStatus::Preview => "preview",
                crate::models::ModelStatus::Deprecated => "deprecated",
                crate::models::ModelStatus::Retired => "retired",
            };
            card_with(
                m.id.clone(),
                word_badge(status),
                el("div")
                    .child(facts(vec![
                        ("Vendor", Node::Element(el("span").text(&m.vendor))),
                        ("Native id", Node::Element(mono(m.native_id.clone()))),
                        (
                            "Context",
                            Node::Element(
                                el("span").text(
                                    m.context_window
                                        .map(|c| c.to_string())
                                        .unwrap_or_else(|| "(not declared)".into()),
                                ),
                            ),
                        ),
                        (
                            "Capabilities",
                            Node::Element(el("span").text(m.capabilities.join(", "))),
                        ),
                    ]))
                    .when(!m.aliases.is_empty(), |d| {
                        d.child(facts(vec![(
                            "Aliases",
                            Node::Element(el("span").text(m.aliases.join(", "))),
                        )]))
                    })
                    .when(m.note.is_some(), |d| {
                        d.child(
                            el("p")
                                .class("mj-prose")
                                .text(m.note.clone().unwrap_or_default()),
                        )
                    }),
            )
        })
        .collect();
    let mut findings = el("ul").class("mj-list");
    for finding in &report.diagnostics {
        findings = findings.child(el("li").text(finding));
    }
    Page::new(
        Area::Models,
        "Models",
        el("div")
            .class("mj-grid")
            .child(card(
                format!("Vendors ({})", report.vendors.len()),
                if vendors.is_empty() {
                    el("p").class("mj-prose").text(
                        "The catalogue declares no vendors. share/models.yaml is the one place to add one.",
                    )
                } else {
                    el("div").class("mj-grid").children(vendors)
                },
            ))
            .child(card(
                format!("Models ({})", report.count),
                if models.is_empty() {
                    el("p").class("mj-prose").text(
                        "The catalogue declares no models; `models.route` answers that nothing qualifies, which is the truthful answer.",
                    )
                } else {
                    el("div").class("mj-grid").children(models)
                },
            ))
            .when(!report.diagnostics.is_empty(), |d| {
                d.child(card("Findings", findings))
            }),
    )
}

/// The mesh: the discovered nodes of this process's runtime, the providers that heard
/// them, and why the mesh is or is not running. Everything on this page is the same
/// `mesh.status` and `mesh.nodes` every other surface renders; the Cockpit holds no
/// node list of its own (project.mesh-is-observation-not-authority).
pub fn mesh(ctx: &Context) -> Page {
    let status: crate::mesh::MeshStatus = match ask(ctx, "mesh.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Mesh, "Mesh", e),
    };
    let nodes: NodeList = match ask(ctx, "mesh.nodes", json!({})) {
        Ok(n) => n,
        Err(e) => return failed(Area::Mesh, "Mesh", e),
    };

    let mut this_node = facts(vec![(
        "Mesh",
        Node::Element(word_badge(if status.active {
            "active"
        } else {
            "inactive"
        })),
    )]);
    if let Some(reason) = &status.reason {
        this_node = this_node.child(el("p").class("mj-prose").text(reason));
    }
    let mut overview = el("div").child(this_node);
    if let Some(identity) = &status.identity {
        overview = overview.child(facts(vec![
            ("Node", Node::Element(mono(identity.node_id.to_string()))),
            (
                "Name",
                Node::Element(el("span").text(&identity.display_name)),
            ),
            (
                "Instance",
                Node::Element(mono(identity.instance_id.to_string())),
            ),
        ]));
    }
    if let Some(policy) = &status.trust_policy {
        overview = overview.child(facts(vec![(
            "Trust policy",
            Node::Element(word_badge(policy)),
        )]));
    }
    let t = &status.tallies;
    let r = &status.refusals;
    overview = overview.child(facts(vec![
        (
            "Nodes",
            Node::Element(el("span").text(format!(
                "{} ({} trusted, {} present)",
                t.nodes, t.trusted, t.present
            ))),
        ),
        (
            "Accepted / replayed / expired",
            Node::Element(el("span").text(format!("{} / {} / {}", t.accepted, t.replayed, t.expired))),
        ),
        (
            "Refused",
            Node::Element(el("span").text(format!(
                "oversized {}, malformed {}, version {}, bounds {}, stale {}, signature {}, self {}",
                r.oversized, r.malformed, r.version, r.bounds, r.stale, r.signature, r.self_heard
            ))),
        ),
    ]));

    let providers: Vec<El> = status
        .providers
        .iter()
        .map(|p| {
            let state = match p.state {
                crate::mesh::provider::MeshProviderState::Running => "running",
                crate::mesh::provider::MeshProviderState::Failed => "failed",
                crate::mesh::provider::MeshProviderState::Stopped => "stopped",
            };
            card_with(
                p.id.clone(),
                word_badge(state),
                el("div")
                    .child(facts(vec![
                        ("Sent", Node::Element(el("span").text(p.sent.to_string()))),
                        (
                            "Received",
                            Node::Element(el("span").text(p.received.to_string())),
                        ),
                    ]))
                    .when(p.detail.is_some(), |d| {
                        d.child(
                            el("p")
                                .class("mj-prose")
                                .text(p.detail.clone().unwrap_or_default()),
                        )
                    }),
            )
        })
        .collect();

    let node_cards: Vec<El> = nodes
        .nodes
        .iter()
        .map(|n| {
            let trust = match &n.trust {
                crate::mesh::TrustState::Trusted(by) => format!("trusted ({by})"),
                crate::mesh::TrustState::Observed => "observed".to_string(),
                crate::mesh::TrustState::Rejected(why) => format!("rejected: {why}"),
            };
            let presence = match n.presence {
                crate::mesh::Presence::Present => "present",
                crate::mesh::Presence::Absent => "absent",
            };
            let sources = n
                .sources
                .iter()
                .map(|s| format!("{} via {} at {}", s.source.as_str(), s.path, s.at))
                .collect::<Vec<_>>()
                .join("; ");
            card_with(
                format!("{} — {}", n.display_name, n.node_id),
                word_badge(presence),
                el("div")
                    .child(el("p").class("mj-prose").text(trust))
                    .child(facts(vec![
                        (
                            "Endpoints",
                            Node::Element(el("span").text(n.endpoints.join(", "))),
                        ),
                        (
                            "Transports",
                            Node::Element(el("span").text(n.capabilities.join(", "))),
                        ),
                        (
                            "Seen",
                            Node::Element(el("span").text(format!(
                                "first {}, last {}, restarts {}",
                                n.first_seen, n.last_seen, n.restarts
                            ))),
                        ),
                        ("Sources", Node::Element(el("span").text(sources))),
                        ("Key", Node::Element(mono(n.public_key.clone()))),
                    ])),
            )
        })
        .collect();

    Page::new(
        Area::Mesh,
        "Mesh",
        el("div")
            .class("mj-grid")
            .child(card("This node", overview))
            .when(!providers.is_empty(), |d| {
                d.child(card(
                    "Discovery providers",
                    el("div").class("mj-grid").children(providers),
                ))
            })
            .child(card(
                format!("Discovered nodes ({})", nodes.count),
                if node_cards.is_empty() {
                    el("p")
                        .class("mj-prose")
                        .text("No nodes observed. The registry fills as advertisements arrive; `majordomus mesh doctor` proves the prerequisites on this machine alone.")
                } else {
                    el("div").class("mj-grid").children(node_cards)
                },
            )),
    )
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

// --------------------------------------------------------------------- release

/// A digest, cut to what a person compares by eye.
///
/// `sha256:` and 64 hex characters is 71, and a 320px viewport is about 40 — so the full
/// value in a table cell is the whole page's overflow. Two fingerprints that differ do so in
/// the first bytes with overwhelming probability, which is what this cell is read for; the
/// whole value stays in the JSON every machine surface answers with, where nothing truncates
/// it. The commit two rows above is abbreviated for the same reason.
fn short_digest(digest: &str) -> String {
    match digest.split_once(':') {
        Some((algo, hex)) => format!("{algo}:{}…", &hex[..hex.len().min(12)]),
        None => digest.chars().take(12).collect::<String>() + "…",
    }
}

/// What the public contract did since the last release, and the smallest version it allows.
///
/// Every number here is read from `release.analysis` — the one engine the command line, the
/// HTTP route, the MCP tool, the CI gate and `release bump` all read. The Cockpit computes
/// no compatibility of its own and increments no version: the page shows the verdict and the
/// exact command that acts on it, because raising the version writes tracked files and the
/// exposure policy keeps repository mutations off every machine surface (ADR 0027).
pub fn release(ctx: &Context) -> Page {
    let plan: VersionPlan = match ask(ctx, "release.analysis", json!({})) {
        Ok(p) => p,
        // Not `failed`, which answers 500. Every way this capability refuses is a *state of
        // the repository* — it has published nothing, or its last release predates the
        // committed registry — and a repository that has not released yet is not a server
        // error. It is still never rendered as an empty diff, which would read as "nothing
        // changed": the page says what could not be measured and why.
        Err(reason) => {
            return Page::new(
                Area::Release,
                "Release",
                el("div").class("mj-grid").child(card(
                    "Nothing to measure against yet",
                    el("div")
                        // `mj-identity`: the reason names what could not be read, and that
                        // is an identity — a 40-character commit, or a ref — with no space
                        // to break at. The cockpit job checks out one commit deep, so this
                        // is the page CI renders, and it is the one that overflowed 320px.
                        .child(el("p").class("mj-prose mj-identity").text(&reason))
                        .child(el("p").class("mj-note").text(
                            "The smallest allowed version is measured against the surface the last release published. Until there is one — a release record, and the registry committed at its commit — there is no baseline, and no bump can be derived. A version named deliberately is still written: `majordomus release bump --level minor`.",
                        )),
                )),
            )
            .subtitle("The smallest version this tree may declare, measured from the public capability surface against the last release.")
            .trail(vec![("Cockpit", Some("/cockpit")), ("Release", None)]);
        }
    };
    let (added, changed, removed) = plan.counts();
    let blocked = plan.status == ReleaseStatus::Blocked;

    let verdict = card_with(
        if blocked {
            format!("{} → {} required", plan.declared_version, plan.required_version)
        } else {
            format!("{} — aligned", plan.declared_version)
        },
        badge(
            if blocked { "fail" } else { "ok" },
            if blocked { "blocked" } else { "aligned" },
        ),
        el("div")
            .child(facts(vec![
                ("Last release", Node::Element(mono(&plan.baseline.reference))),
                ("Required bump", Node::Element(badge(
                    if plan.required == Impact::Major { "fail" } else if plan.required == Impact::Minor { "warn" } else { "ok" },
                    plan.required.as_str(),
                ))),
                ("Declared bump", Node::Element(mono(plan.declared.as_str()))),
                ("Next minimum", Node::Element(mono(&plan.required_version))),
                ("Surface", Node::Element(mono(format!(
                    "{} public atoms, was {}",
                    plan.atoms, plan.baseline.atoms
                )))),
            ]))
            .child(
                el("div").class("mj-marks").children(vec![
                    badge("ok", format!("{added} added")),
                    badge("info", format!("{changed} changed")),
                    badge(if removed > 0 { "fail" } else { "ok" }, format!("{removed} removed")),
                ]),
            )
            .child(el("p").class("mj-note").text(plan.policy.statement()))
            .when(blocked, |d| {
                d.child(el("p").class("mj-prose").text(
                    "The public contract moved by more than the version says. Raise it with the one writer:",
                ))
                .child(mono("majordomus release bump"))
            }),
    );

    // Why — every movement, with the reason it counts for what it does. The list is the
    // evidence for the badge above, so a surprising verdict can be checked rather than
    // believed.
    let why = card(
        "Why",
        el("div")
            .when(plan.changes.is_empty(), |d| {
                d.child(el("p").class("mj-prose").text(format!(
                    "Nothing a caller can hold has moved since {}. No bump is owed.",
                    plan.baseline.reference
                )))
            })
            .when(!plan.changes.is_empty(), |d| {
                d.child(
                    el("ul").class("mj-list").children(
                        plan.changes
                            .iter()
                            .map(|c| {
                                // Prose, not a `mono` chip. A change id is a dotted path
                                // with no spaces to break at
                                // (`$defs.Record.properties.next_action_withheld`), and
                                // `.mj-mono` does not break — only `a.mj-mono` does — so a
                                // chip of one overflows a 320px viewport however its row is
                                // laid out. Rendered as prose beside its reason it wraps,
                                // and the monospace was decoration rather than meaning.
                                el("li")
                                    .child(badge(
                                        match c.impact {
                                            Impact::Major => "fail",
                                            Impact::Minor => "warn",
                                            _ => "ok",
                                        },
                                        c.impact.as_str(),
                                    ))
                                    .child(
                                        el("div")
                                            .class("mj-identity")
                                            .text(format!("{c} — {}", c.detail)),
                                    )
                            })
                            .collect::<Vec<_>>(),
                    ),
                )
            }),
    );

    let mut cards = vec![verdict, why];

    // The line this subsystem exists for: the human account and the measured one, side by
    // side, when they disagree.
    if plan.understated {
        cards.push(card_with(
            "The commits understate the change".to_string(),
            badge("warn", "evidence"),
            el("div")
                .child(facts(vec![
                    ("Commits say", Node::Element(mono(plan.commits.implied.as_str()))),
                    ("The contract moved by", Node::Element(mono(plan.implied.as_str()))),
                    ("Commits since", Node::Element(mono(plan.commits.commits.to_string()))),
                ]))
                .child(el("p").class("mj-note").text(
                    "Conventional commits are how a change explains itself; the contract is what decides the version. The measurement wins.",
                )),
        ));
    }

    if !plan.diagnostics.is_empty() {
        cards.push(card(
            "The release state",
            el("ul").class("mj-list").children(
                plan.diagnostics
                    .iter()
                    .map(|d| {
                        el("li")
                            .child(badge(
                                if d.severity == Severity::Error {
                                    "fail"
                                } else {
                                    "warn"
                                },
                                if d.severity == Severity::Error {
                                    "error"
                                } else {
                                    "warning"
                                },
                            ))
                            .child(el("span").text(&d.message))
                    })
                    .collect::<Vec<_>>(),
            ),
        ));
    }

    cards.push(card(
        "Provenance",
        facts(vec![
            // Not `mono`: the schema id is 28 characters with no space to break at, and
            // `.mj-mono` does not break. Beside a label on one row it is the widest thing
            // on the page at 320px.
            (
                "Policy",
                Node::Element(el("span").class("mj-prose").text(&plan.policy.schema)),
            ),
            (
                "Baseline commit",
                Node::Element(mono(
                    plan.baseline.commit.chars().take(12).collect::<String>(),
                )),
            ),
            (
                "Recorded",
                Node::Element(badge(
                    if plan.baseline.recorded { "ok" } else { "warn" },
                    if plan.baseline.recorded {
                        "yes"
                    } else {
                        "from a tag alone"
                    },
                )),
            ),
            (
                "Baseline digest",
                Node::Element(mono(short_digest(&plan.baseline.fingerprint))),
            ),
            (
                "This digest",
                Node::Element(mono(short_digest(&plan.fingerprint))),
            ),
            (
                "Writers agree",
                Node::Element(badge(
                    if plan.writers_agree { "ok" } else { "fail" },
                    if plan.writers_agree { "yes" } else { "no" },
                )),
            ),
        ]),
    ));

    Page::new(Area::Release, "Release", el("div").class("mj-grid").children(cards))
        .subtitle("The smallest version this tree may declare, measured from the public capability surface against the last release — the same verdict `majordomus release analyze` and the `version-surface` gate reach.")
        .trail(vec![("Cockpit", Some("/cockpit")), ("Release", None)])
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
    // Ordered as routes, before anything is rendered. Sorting the finished markup instead
    // made every CSS class name part of the sort key: a rename nobody thought was visible
    // reordered the table.
    let mut routes: Vec<_> = ctx
        .registry
        .iter()
        .filter_map(|c| c.exposure.http.as_ref().map(|h| (c, h)))
        .collect();
    routes.sort_by(|(a, ha), (b, hb)| {
        crate::order::natural_cmp(&ha.path, &hb.path)
            .then_with(|| crate::order::natural_cmp(ha.method.as_str(), hb.method.as_str()))
            .then_with(|| crate::order::natural_cmp(a.id.as_str(), b.id.as_str()))
    });
    let rows: Vec<El> = routes
        .into_iter()
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

    // the projection's own routes, and where each one answers: the same resolved surfaces
    // the OpenAPI document and the published site read, so a description written once here
    // cannot disagree with the one written there. This table has never held a path of its
    // own and must not start.
    let infrastructure = table(
        &["Path", "What it is", "Answered by"],
        crate::web::projection_routes()
            .into_iter()
            .map(|r| {
                row(vec![
                    cell(mono(&r.path)),
                    text_cell(&r.what),
                    text_cell(if r.linkable() {
                        "a running server, and a publication"
                    } else {
                        "a running server"
                    }),
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

// --------------------------------------------------------------------- quality

/// The crate's own public surface, as the rules hold it.
///
/// Every number and every finding on this page comes from one execution of
/// `quality.report`, through the same executor the command line and the HTTP route use.
/// Nothing is counted here, nothing is listed here, and a code added to the validator
/// tomorrow appears on this page with no edit to it — which is the property the page is
/// about, so it had better be true of the page.
pub fn quality(ctx: &Context) -> Page {
    let answer: QualityAnswer = match ask(ctx, "quality.report", json!({})) {
        Ok(a) => a,
        Err(e) => return failed(Area::Quality, "Quality", e),
    };
    if !answer.measured {
        return Page::new(
            Area::Quality,
            "Quality",
            el("div").child(alert("warn", answer.reason.unwrap_or_default())).child(
                el("p").class("mj-empty").text(
                    "This repository carries no Rust crate, so the public API rules do not apply to it. That is an answer and not a failure.",
                ),
            ),
        )
        .trail(vec![("Cockpit", Some("/cockpit")), ("Quality", None)]);
    }
    let r = &answer.report;
    let ratio = |have: usize, of: usize| {
        if of == 0 {
            "—".to_string()
        } else {
            format!("{have} / {of}")
        }
    };
    let verdict = if answer.passes { "ok" } else { "fail" };

    let surface = card(
        "Public API",
        el("div")
            .child(facts(vec![
                (
                    "Documented",
                    Node::Element(el("span").text(ratio(r.public_api.documented, r.public_api.items))),
                ),
                (
                    "Exampled",
                    Node::Element(
                        el("span").text(ratio(r.public_api.exampled, r.public_api.owe_example)),
                    ),
                ),
            ]))
            .child(
                el("p").class("mj-note").text(
                    "An item owes an example when it carries behaviour. What the policy does not ask of an item is derived from that item's kind and shape, and is listed below rather than kept in an exemption file.",
                ),
            )
            .child(table(
                &["Not asked of", "Items"],
                r.public_api
                    .exempt
                    .iter()
                    .map(|e| row(vec![text_cell(&e.reason), text_cell(e.items.to_string())]))
                    .collect(),
            )),
    );

    let modules = card(
        "Modules",
        facts(vec![
            (
                "Documented",
                Node::Element(el("span").text(ratio(r.modules.documented, r.modules.modules))),
            ),
            (
                "Exampled",
                Node::Element(el("span").text(ratio(r.modules.exampled, r.modules.modules))),
            ),
            (
                "Behaviourally tested",
                Node::Element(
                    el("span").text(ratio(r.modules.behaviourally_tested, r.modules.modules)),
                ),
            ),
        ]),
    );

    let o = &r.operations;
    let operations = card(
        "Operations",
        el("div")
            .child(facts(vec![
                ("Canonical", Node::Element(el("span").text(o.canonical.to_string()))),
                ("HTTP", Node::Element(el("span").text(ratio(o.http, o.canonical)))),
                ("OpenAPI", Node::Element(el("span").text(ratio(o.openapi, o.http)))),
                ("MCP", Node::Element(el("span").text(ratio(o.mcp, o.canonical)))),
                (
                    "Command line",
                    Node::Element(el("span").text(ratio(o.cli, o.canonical))),
                ),
            ]))
            .child(el("p").class("mj-note").text(format!(
                "{} runnable command(s): {} the projection of a capability, {} classified as belonging to the command line alone with a reason the parity check verifies.",
                o.cli_commands, o.cli_from_capability, o.cli_local
            ))),
    );

    // one card per code, because that is the unit somebody fixes: the reason and the
    // remedy belong to the code, and repeating them per finding would be the duplication
    // this whole subsystem is about
    let mut by_code: std::collections::BTreeMap<&str, Vec<&crate::quality::Violation>> =
        Default::default();
    for v in &r.violations {
        by_code.entry(v.code.as_str()).or_default().push(v);
    }
    let findings: Vec<El> = by_code
        .into_iter()
        .map(|(code, group)| {
            let first = group[0];
            card_with(
                code.to_string(),
                badge("fail", format!("{} finding(s)", group.len())),
                el("div")
                    .child(el("p").class("mj-prose").text(&first.why))
                    .child(facts(vec![
                        ("Rule", Node::Element(mono(first.rule.clone()))),
                        ("Remedy", Node::Element(el("span").text(&first.remediation))),
                    ]))
                    .child(details(
                        format!("{} occurrence(s)", group.len()),
                        table(
                            &["Where", "Symbol"],
                            group
                                .iter()
                                .map(|v| {
                                    let at = match v.line {
                                        Some(l) => format!("{}:{l}", v.path),
                                        None => v.path.clone(),
                                    };
                                    row(vec![cell(mono(at)), cell(mono(v.symbol.clone()))])
                                })
                                .collect(),
                        ),
                    )),
            )
        })
        .collect();

    let standing = card_with(
        "Where this stands",
        badge(
            verdict,
            if answer.passes {
                "no finding outside the baseline".to_string()
            } else {
                format!("{} finding(s)", r.violations.len())
            },
        ),
        el("div")
            .child(facts(vec![
                ("Target", Node::Element(mono(r.target.clone()))),
                (
                    "Accepted by the baseline",
                    Node::Element(el("span").text(answer.baselined.to_string())),
                ),
            ]))
            .child(el("p").class("mj-note").text(
                "The baseline records the findings that stood when the rule landed. The gate fails for a finding that is not in it, so the debt can shrink and cannot grow.",
            )),
    );

    Page::new(
        Area::Quality,
        "Quality",
        el("div")
            .class("mj-grid")
            .child(standing)
            .child(surface)
            .child(modules)
            .child(operations)
            .children(findings),
    )
    .subtitle("The same measurement `majordomus quality report` prints and `/api/v1/quality` answers, read through one capability.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Quality", None)])
}

// ------------------------------------------------------------------------ commands

/// Every command this repository offers, from whichever program offers it.
///
/// The page carries no command of its own: it asks `commands.list`, which is the same
/// capability the MCP tool and the HTTP route answer with, over the same graph the
/// generated workflow bridge and the shell completion are derived from. The filters are the
/// graph's vocabulary — the program, and what running a command changes — so a chip here and
/// a `--effect` on the command line select the same set.
pub fn commands(ctx: &Context, query: &[(String, String)]) -> Page {
    let origin = param(query, "origin");
    let effect = param(query, "effect");
    let search = param(query, "q");
    let mut input = json!({});
    if let Some(o) = &origin {
        input["origin"] = json!(o);
    }
    if let Some(e) = &effect {
        input["effect"] = json!(e);
    }
    if let Some(s) = &search {
        input["search"] = json!(s);
    }
    let index: CommandIndex = match ask(ctx, "commands.list", input) {
        Ok(v) => v,
        Err(e) => return failed(Area::Commands, "Commands", e),
    };

    let here = |key: &str, value: Option<&str>| -> String {
        let mut parts: Vec<String> = Vec::new();
        for (k, v) in [
            ("origin", origin.as_deref()),
            ("effect", effect.as_deref()),
            ("q", search.as_deref()),
        ] {
            let v = if k == key { value } else { v };
            if let Some(v) = v {
                parts.push(format!("{k}={}", percent_encode(v)));
            }
        }
        if parts.is_empty() {
            "/cockpit/commands".into()
        } else {
            format!("/cockpit/commands?{}", parts.join("&"))
        }
    };

    let programs = chips(
        [
            ("every program", None),
            ("executable", Some("executable")),
            ("shell tool", Some("tool")),
            ("workflow", Some("workflow")),
        ]
        .into_iter()
        .map(|(label, value)| {
            (
                label.to_string(),
                here("origin", value),
                index
                    .commands
                    .iter()
                    .filter(|c| value.is_none_or(|v| c.origin == v))
                    .count(),
                origin.as_deref() == value,
            )
        })
        .collect(),
    );

    let effects = chips(
        [
            ("any effect", None),
            ("read-only", Some("read_only")),
            ("up to local", Some("local_mutation")),
            ("up to repository", Some("repository_mutation")),
        ]
        .into_iter()
        .map(|(label, value)| {
            (
                label.to_string(),
                here("effect", value),
                0,
                effect.as_deref() == value,
            )
        })
        .collect(),
    );

    let rows = index
        .commands
        .iter()
        .map(|c| {
            row(vec![
                cell(link(
                    format!("/cockpit/commands/{}", percent_encode(&c.id)),
                    c.invocation.clone(),
                )),
                cell(badge(effect_status(&c.effect), c.effect.replace('_', " "))),
                text_cell(c.origin.clone()),
                cell(match &c.projections.workflow {
                    Some(w) => mono(format!("just {w}")),
                    None => el("span").class("mj-note").text("—"),
                }),
                cell(match &c.projections.mcp {
                    Some(t) => mono(t.clone()),
                    None => el("span").class("mj-note").text("—"),
                }),
                text_cell(c.summary.clone()),
            ])
        })
        .collect::<Vec<_>>();

    Page::new(
        Area::Commands,
        "Commands",
        el("div")
            .class("mj-grid")
            .child(card_with(
                "What this repository offers",
                badge("ok", index.fingerprint.clone()),
                el("div")
                    .child(
                        el("div")
                            .class("mj-stats")
                            .child(statistic(
                                index.total.to_string(),
                                "commands",
                                "the command graph",
                            ))
                            .child(statistic(
                                index.commands.len().to_string(),
                                "shown",
                                "this filter",
                            )),
                    )
                    .child(programs)
                    .child(effects)
                    .child(el("p").class("mj-note").text(
                        "Composed from the three declarations that already exist: the clap tree of the Rust executable, the shipped command registry of the shell tool, and the recipes the workflow runner describes. Where a command appears is derived from what running it changes, never declared.",
                    )),
            ))
            .child(card("Every command", table(
                &["command", "effect", "program", "workflow", "mcp tool", "summary"],
                rows,
            ))),
    )
    .subtitle("Every command of every program here, and every surface that carries it.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Commands", None)])
}

/// One command: what it takes, what running it changes, and every surface that carries it.
pub fn command(ctx: &Context, id: &str) -> Page {
    let node: CommandNode = match ask(ctx, "commands.get", json!({ "id": id })) {
        Ok(v) => v,
        Err(e) => {
            return failed(Area::Commands, "Command", e).status(404).trail(vec![
                ("Cockpit", Some("/cockpit")),
                ("Commands", Some("/cockpit/commands")),
                (id, None),
            ])
        }
    };

    let arguments = table(
        &["argument", "values from", "required", "help"],
        node.arguments
            .iter()
            .map(|a| {
                let spelling = match &a.long {
                    Some(long) => format!("--{long}"),
                    None => format!("<{}>", a.name.to_uppercase()),
                };
                row(vec![
                    cell(mono(spelling)),
                    cell(tag(word(&a.source).replace('_', " "))),
                    text_cell(if a.required { "yes" } else { "—" }),
                    text_cell(a.help.clone()),
                ])
            })
            .collect(),
    );

    let p = &node.projections;
    let surfaces = facts(vec![
        (
            "Command line",
            Node::Element(match &p.cli {
                Some(v) => mono(v.clone()),
                None => el("span").class("mj-note").text("—"),
            }),
        ),
        (
            "Workflow",
            Node::Element(match &p.workflow {
                Some(v) => mono(format!("just {v}")),
                None => el("span").class("mj-note").text("—"),
            }),
        ),
        (
            "MCP tool",
            Node::Element(match &p.mcp {
                Some(v) => mono(v.clone()),
                None => el("span").class("mj-note").text("—"),
            }),
        ),
        (
            "HTTP",
            Node::Element(match &p.http {
                Some(v) => mono(v.clone()),
                None => el("span").class("mj-note").text("—"),
            }),
        ),
        (
            "Declared in",
            Node::Element(mono(node.provenance.declared_in.clone())),
        ),
        (
            "Capability",
            Node::Element(match &node.provenance.capability {
                Some(c) => link(
                    format!("/cockpit/capabilities/{}", percent_encode(c.as_str())),
                    c.to_string(),
                ),
                None => el("span").class("mj-note").text("—"),
            }),
        ),
    ]);

    let effect_word = word(&node.effect).replace('_', " ");
    Page::new(
        Area::Commands,
        node.invocation.clone(),
        el("div")
            .class("mj-grid")
            .child(card_with(
                "What it is",
                badge(effect_status(&word(&node.effect)), effect_word),
                el("div")
                    .child(el("p").text(node.summary.clone()))
                    .child(facts(vec![
                        ("Identity", Node::Element(mono(node.id.to_string()))),
                        ("Program", Node::Element(tag(word(&node.origin)))),
                        (
                            "Runs",
                            Node::Element(tag(word(&node.interactivity).replace('_', " "))),
                        ),
                        (
                            "Requires",
                            Node::Element(
                                el("span").class("mj-marks").children(
                                    node.availability
                                        .requires
                                        .iter()
                                        .map(|r| tag(word(r).replace('_', " ")))
                                        .collect::<Vec<_>>(),
                                ),
                            ),
                        ),
                    ]))
                    .when(p.withheld.is_some(), |d| {
                        d.child(alert(
                            "warn",
                            format!(
                                "No machine surface carries this command: {}",
                                p.withheld.clone().unwrap_or_default()
                            ),
                        ))
                    }),
            ))
            .child(card("Where it appears", surfaces))
            .when(!node.arguments.is_empty(), |d| {
                d.child(card("Arguments", arguments))
            }),
    )
    .subtitle(
        node.description
            .clone()
            .unwrap_or_else(|| node.summary.clone()),
    )
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Commands", Some("/cockpit/commands")),
        (node.id.as_str(), None),
    ])
}

/// The badge status for an effect: what a reader should feel about running it.
fn effect_status(effect: &str) -> &'static str {
    match effect {
        "read_only" => "ok",
        "local_mutation" => "info",
        "repository_mutation" | "network_mutation" => "warn",
        _ => "fail",
    }
}

/// One query parameter, when it carries something.
fn param(query: &[(String, String)], key: &str) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty())
}

// ------------------------------------------------------------------- executions

/// The status word a badge uses for an execution state, so the colour of a state is
/// decided once rather than in every place one is shown.
fn execution_status(state: ExecutionState) -> &'static str {
    match state {
        ExecutionState::Succeeded => "ok",
        ExecutionState::Failed => "fail",
        ExecutionState::Cancelled => "warn",
        ExecutionState::Running | ExecutionState::Cancelling | ExecutionState::Queued => "info",
    }
}

/// How long an execution took, or has been going.
fn duration_cell(e: &Execution) -> El {
    match e.duration_ms {
        Some(ms) if ms < 1000 => text_cell(format!("{ms} ms")),
        Some(ms) => text_cell(format!("{:.1} s", ms as f64 / 1000.0)),
        None if e.state.is_active() => cell(el("span").class("mj-muted").text("running")),
        None => cell(el("span").class("mj-muted").text("-")),
    }
}

/// The executions this process is running and remembers.
pub fn executions(ctx: &Context, query: &[(String, String)]) -> Page {
    let asked_state = query
        .iter()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty());
    let asked_capability = query
        .iter()
        .find(|(k, _)| k == "capability")
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty());
    let mut input = json!({});
    if let Some(state) = &asked_state {
        input["state"] = json!(state);
    }
    if let Some(capability) = &asked_capability {
        input["capability"] = json!(capability);
    }
    let list: ExecutionList = match ask(ctx, "executions.list", input) {
        Ok(l) => l,
        Err(e) => return failed(Area::Executions, "Executions", e),
    };

    let counters = el("div")
        .class("mj-stats")
        .child(statistic(
            list.remembered.to_string(),
            "remembered",
            "executions.list",
        ))
        .child(statistic(
            list.active.to_string(),
            "active",
            "executions.list",
        ))
        .child(statistic(
            list.queued.to_string(),
            "queued",
            "executions.list",
        ))
        .child(statistic(
            list.live_channels.to_string(),
            "live channels",
            "http::events",
        ));

    let states = [
        ExecutionState::Queued,
        ExecutionState::Running,
        ExecutionState::Cancelling,
        ExecutionState::Succeeded,
        ExecutionState::Failed,
        ExecutionState::Cancelled,
    ];
    let filters = el("form")
        .class("mj-filters")
        .attr("method", "get")
        .attr("action", "/cockpit/executions")
        .child(select(
            "state",
            "State",
            asked_state.as_deref(),
            states
                .iter()
                .map(|s| (s.as_str().to_string(), s.as_str().to_string()))
                .collect(),
        ))
        .child(
            el("label")
                .class("mj-field")
                .child(el("span").class("mj-field-label").text("Capability"))
                .child(
                    el("input")
                        .class("mj-input")
                        .attr("type", "text")
                        .attr("name", "capability")
                        .attr("spellcheck", "false")
                        .attr_if("value", asked_capability.clone()),
                ),
        )
        .child(
            el("button")
                .class("mj-button")
                .attr("type", "submit")
                .text("Filter"),
        );

    let body = if list.executions.is_empty() {
        card(
            "Executions",
            nothing(
                "This process has run nothing yet. Open a capability, choose Run as an execution, and it will appear here.",
            ),
        )
    } else {
        card(
            "Executions",
            table(
                &[
                    "State",
                    "Execution",
                    "Capability",
                    "Progress",
                    "Took",
                    "Started",
                ],
                list.executions
                    .iter()
                    .map(|view| {
                        let e = &view.execution;
                        row(vec![
                            cell(badge(execution_status(e.state), e.state.as_str())),
                            id_cell(view.links.cockpit.clone(), e.id.to_string()),
                            cell(link(
                                format!("/cockpit/capabilities/{}", percent_encode(&e.capability)),
                                e.capability.clone(),
                            )),
                            cell(match e.percent() {
                                Some(percent) => progress_bar(percent),
                                None => el("span").class("mj-muted").text("-"),
                            }),
                            duration_cell(e),
                            text_cell(e.started_at.clone().unwrap_or_else(|| e.created_at.clone())),
                        ])
                    })
                    .collect(),
            ),
        )
    };

    Page::new(
        Area::Executions,
        "Executions",
        el("div")
            .class("mj-grid")
            .child(counters)
            .child(card("Narrow", filters))
            .child(body)
            .child(live_region()),
    )
    .subtitle(format!(
        "{} of {} remembered by this process",
        list.count, list.remembered
    ))
    .trail(vec![("Cockpit", Some("/cockpit")), ("Executions", None)])
    .script("executions.js")
}

/// The element the live channel writes into, carrying what a reader needs to know when
/// JavaScript is not running — and the channel's own path, so that no path is written down
/// in the browser.
fn live_region() -> El {
    el("div")
        .class("mj-live")
        .attr("data-mj-live", "")
        .attr("data-mj-socket", crate::http::events::PATH)
        .attr("aria-live", "polite")
        .child(
            el("p").class("mj-note").text(
                "This page is complete as it is. With JavaScript, it follows the live channel and updates itself as executions run.",
            ),
        )
}

/// A progress bar with the accessible semantics a progress bar needs.
fn progress_bar(percent: u64) -> El {
    el("div")
        .class("mj-progress")
        .attr("role", "progressbar")
        .attr("aria-valuenow", percent.to_string())
        .attr("aria-valuemin", "0")
        .attr("aria-valuemax", "100")
        .attr("aria-label", "execution progress")
        .child(
            el("div")
                .class("mj-progress-bar")
                .attr("style", format!("width:{percent}%")),
        )
        .child(
            el("span")
                .class("mj-progress-text")
                .text(format!("{percent}%")),
        )
}

/// One execution, in full: what it is, where it is, what it has said, and what it produced.
///
/// The page is complete without JavaScript — this is what a reload restores from — and
/// `executions.js` subscribes from the sequence the page was rendered at, so nothing is
/// missed between the render and the stream.
pub fn execution(ctx: &Context, id: &str) -> Page {
    let view: ExecutionView = match ask(ctx, "executions.get", json!({ "id": id })) {
        Ok(v) => v,
        Err(reason) => {
            return Page::new(
                Area::Executions,
                id.to_string(),
                el("div")
                    .child(alert("warn", reason))
                    .child(el("p").class("mj-empty").text(
                        "An execution lives as long as the process that ran it, and this process remembers a bounded number of them.",
                    ))
                    .child(link("/cockpit/executions", "Every execution this process remembers")),
            )
            .status(404)
            .trail(vec![
                ("Cockpit", Some("/cockpit")),
                ("Executions", Some("/cockpit/executions")),
                (id, None),
            ])
        }
    };
    let e = &view.execution;
    let history: EventHistory =
        ask(ctx, "executions.events", json!({ "id": id })).unwrap_or(EventHistory {
            execution_id: e.id.to_string(),
            state: e.state,
            events: Vec::new(),
            last_sequence: e.last_sequence,
            more: false,
            truncated: e.events_truncated,
        });

    let what = card_with(
        "This execution",
        badge(execution_status(e.state), e.state.as_str()),
        facts(vec![
            (
                "Capability",
                Node::Element(link(
                    format!("/cockpit/capabilities/{}", percent_encode(&e.capability)),
                    e.capability.clone(),
                )),
            ),
            ("Title", Node::Text(e.title.clone())),
            ("Started by", Node::Text(word(&e.actor.kind))),
            ("Repository", Node::Text(e.repository.name.clone())),
            (
                "Branch",
                Node::Text(e.repository.branch.clone().unwrap_or_else(|| "-".into())),
            ),
            ("Created", Node::Text(e.created_at.clone())),
            (
                "Finished",
                Node::Text(e.finished_at.clone().unwrap_or_else(|| "-".into())),
            ),
            (
                "Took",
                Node::Text(match e.duration_ms {
                    Some(ms) => format!("{ms} ms"),
                    None => "-".into(),
                }),
            ),
            (
                "Correlation id",
                Node::Element(mono(e.correlation_id.clone())),
            ),
        ]),
    );

    let control = {
        let cancel = if e.state.is_final() {
            alert(
                "info",
                "This execution has finished; there is nothing left to stop.",
            )
        } else if e.cancellable {
            el("div")
                .child(
                    el("button")
                        .class("mj-button mj-button--primary")
                        .attr("type", "button")
                        .attr("data-mj-cancel", e.id.to_string())
                        .text("Cancel this execution"),
                )
                .child(el("p").class("mj-note").text(
                    "Cancellation is cooperative: the flag is set and the task stops when it next looks at it.",
                ))
        } else {
            alert(
                "warn",
                "This capability does not declare that it looks at its cancellation flag, so asking it to stop would be recorded and change nothing. Its execution policy says so.",
            )
        };
        card("Control", cancel)
    };

    let progress = card(
        "Progress",
        match e.percent() {
            Some(percent) => el("div")
                .attr("data-mj-progress", "")
                .child(progress_bar(percent))
                .child(
                    el("p").class("mj-note").text(
                        e.progress
                            .as_ref()
                            .and_then(|p| p.message.clone())
                            .unwrap_or_default(),
                    ),
                ),
            None => el("div")
                .attr("data-mj-progress", "")
                .child(nothing("This execution reports no measured progress.")),
        },
    );

    let steps = card(
        "Steps",
        if e.steps.is_empty() {
            nothing("It has entered no named step.")
        } else {
            table(
                &["", "Step", "Detail", "Started"],
                e.steps
                    .iter()
                    .map(|s| {
                        row(vec![
                            cell(badge(
                                match s.state {
                                    StepState::Completed => "ok",
                                    StepState::Failed => "fail",
                                    StepState::Running => "info",
                                },
                                word(&s.state),
                            )),
                            text_cell(s.title.clone()),
                            text_cell(s.detail.clone().unwrap_or_default()),
                            text_cell(s.started_at.clone()),
                        ])
                    })
                    .collect(),
            )
        },
    );

    let logs: Vec<El> = history
        .events
        .iter()
        .filter_map(|event| match &event.payload {
            crate::execution::EventPayload::Log { stream, message } => Some(
                el("div")
                    .class("mj-log-line")
                    .child(
                        el("span")
                            .class("mj-log-time")
                            .text(event.timestamp.clone()),
                    )
                    .child(
                        el("span")
                            .class(format!("mj-log-stream mj-log-stream--{}", word(stream)))
                            .text(word(stream)),
                    )
                    .child(el("span").class("mj-log-text").text(message.clone())),
            ),
            _ => None,
        })
        .collect();
    let output_card = card(
        "Output",
        match (&e.output, &e.error) {
            (Some(output), _) => pre(serde_json::to_string_pretty(output).unwrap_or_default()),
            (None, Some(error)) => el("div")
                .child(alert("fail", format!("{}: {}", error.code, error.message)))
                .child(match &error.suggestion {
                    Some(s) => el("p").class("mj-note").text(s.clone()),
                    None => el("span"),
                }),
            (None, None) => nothing("It has not finished."),
        },
    );

    let diagnostics = card(
        "Diagnostics",
        if e.diagnostics.is_empty() {
            nothing("It has reported no finding.")
        } else {
            table(
                &["Severity", "Code", "What"],
                e.diagnostics
                    .iter()
                    .map(|d| {
                        row(vec![
                            cell(badge(severity_status(d.severity), d.severity.as_str())),
                            cell(mono(d.code.clone())),
                            text_cell(d.summary.clone()),
                        ])
                    })
                    .collect(),
            )
        },
    );

    let input_card = card(
        "Input",
        el("div")
            .child(pre(
                serde_json::to_string_pretty(&e.input).unwrap_or_default()
            ))
            .child(el("p").class("mj-note").text(
                "As it was stored: every value the capability's input schema marks sensitive was replaced before this was written down.",
            )),
    );

    let elsewhere = card(
        "The same execution elsewhere",
        table(
            &["Interface", "Where"],
            vec![
                row(vec![
                    text_cell("HTTP"),
                    cell(mono(format!("GET {}", view.links.itself))),
                ]),
                row(vec![
                    text_cell("Events"),
                    cell(mono(format!("GET {}", view.links.events))),
                ]),
                row(vec![
                    text_cell("Live channel"),
                    cell(mono(format!("WebSocket {}", view.links.websocket))),
                ]),
                row(vec![
                    text_cell("MCP"),
                    cell(mono(format!(
                        "majordomus_execution {{ \"id\": \"{}\" }}",
                        e.id
                    ))),
                ]),
                row(vec![
                    text_cell("Command line"),
                    cell(mono(format!("majordomus executions show {}", e.id))),
                ]),
            ],
        ),
    );

    let mut log_card = el("div")
        .class("mj-log")
        .attr("data-mj-log", "")
        .attr("aria-live", "polite");
    if history.truncated {
        log_card = log_card.child(alert(
            "warn",
            "The oldest events of this execution have been dropped: this process retains a bounded number per execution.",
        ));
    }
    log_card = if logs.is_empty() {
        log_card.child(nothing("It has written no output."))
    } else {
        log_card.children(logs)
    };

    Page::new(
        Area::Executions,
        e.id.to_string(),
        el("div")
            .class("mj-grid")
            .attr("data-mj-execution", e.id.to_string())
            .attr("data-mj-sequence", history.last_sequence.to_string())
            .attr("data-mj-socket", view.links.websocket.clone())
            .attr("data-mj-snapshot", view.links.itself.clone())
            .attr("data-mj-events", view.links.events.clone())
            .attr("data-mj-cancel-route", view.links.cancel.clone())
            .child(what)
            .child(control)
            .child(progress)
            .child(steps)
            .child(card("Live output", log_card))
            .child(diagnostics)
            .child(output_card)
            .child(input_card)
            .child(elsewhere),
    )
    .subtitle(e.title.clone())
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Executions", Some("/cockpit/executions")),
        (e.id.as_str(), None),
    ])
    .script("executions.js")
}

// ---------------------------------------------------------------------- design

/// The design system, as this executable carries it: every role with its light and dark
/// value, every status with the words filed under it, the type scale at its own sizes,
/// the layout values, the theme contract, and where the declaration is projected. Nothing
/// on this page is written here — it is `design.system` and `design.tokens` rendered — and
/// the badge at the top is the one check no file-level gate can make: whether the
/// stylesheet this page loaded was projected from the declaration this executable was
/// built with.
pub fn design(ctx: &Context) -> Page {
    let report: DesignReport = match ask(ctx, "design.system", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Design, "Design", e),
    };
    let list: TokenList = match ask(ctx, "design.tokens", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Design, "Design", e),
    };
    use crate::design::TokenKind;
    let of = |kind: TokenKind| list.tokens.iter().filter(move |t| t.kind == kind);

    // a swatch of a resolved value: the class shows the value in the theme in force, the
    // inline style shows the value of the theme the reader is not in, resolved from the
    // declaration at request time rather than chosen here
    let swatch = |literal: &str| {
        el("span")
            .class("mj-swatch")
            .attr("style", format!("background: {literal}"))
            .attr("title", literal.to_string())
    };
    let colour_cell = |literal: &str, reference: &str| {
        el("td")
            .child(swatch(literal))
            .text(" ")
            .child(mono(reference.to_string()))
    };

    let roles =
        table(
            &["role", "light", "dark", "the site's names", "for"],
            of(TokenKind::Role)
                .map(|t| {
                    let p = &t.parts[0];
                    row(vec![
                        cell(mono(p.css.clone())),
                        colour_cell(&p.light.literal, &p.light.reference),
                        colour_cell(&p.dark.literal, &p.dark.reference),
                        cell(el("span").class("mj-marks").children(
                            t.aliases.iter().map(|a| tag(a.clone())).collect::<Vec<_>>(),
                        )),
                        text_cell(t.about.clone()),
                    ])
                })
                .collect(),
        );

    let statuses = table(
        &[
            "status",
            "as a badge",
            "text",
            "ground",
            "border",
            "the words filed under it",
        ],
        of(TokenKind::Status)
            .map(|t| {
                let part = |name: &str| t.parts.iter().find(|p| p.part == name).cloned();
                let fg = part("fg").expect("a status has text");
                let bg = part("bg").expect("a status has a ground");
                let line = part("line").expect("a status has a border");
                row(vec![
                    cell(mono(t.name.clone())),
                    cell(badge(&t.name, t.name.clone())),
                    colour_cell(&fg.light.literal, &fg.light.reference),
                    colour_cell(&bg.light.literal, &bg.light.reference),
                    colour_cell(&line.light.literal, &line.light.reference),
                    cell(
                        el("span").class("mj-marks").children(
                            t.states
                                .iter()
                                .map(|w| badge(w, w.clone()))
                                .collect::<Vec<_>>(),
                        ),
                    ),
                ])
            })
            .collect(),
    );

    let type_scale = table(
        &["step", "as itself", "size / leading", "utility", "for"],
        of(TokenKind::Type)
            .map(|t| {
                row(vec![
                    cell(mono(t.css[0].clone())),
                    cell(
                        el("span")
                            .class(format!("mj-type-sample--{}", t.name))
                            .text("The quick brown fox jumps over the lazy dog"),
                    ),
                    text_cell(t.value.clone().unwrap_or_default()),
                    cell(mono(t.css[2].clone())),
                    text_cell(t.about.clone()),
                ])
            })
            .collect(),
    );

    let scalars = table(
        &["token", "kind", "value", "for"],
        list.tokens
            .iter()
            .filter(|t| {
                matches!(
                    t.kind,
                    TokenKind::Font
                        | TokenKind::Tracking
                        | TokenKind::Layout
                        | TokenKind::Radius
                        | TokenKind::Motion
                )
            })
            .map(|t| {
                row(vec![
                    cell(mono(t.css[0].clone())),
                    cell(tag(t.kind.as_str())),
                    cell(mono(t.value.clone().unwrap_or_default())),
                    text_cell(t.about.clone()),
                ])
            })
            .collect(),
    );

    let projections = table(
        &["projection", "read by"],
        report
            .projections
            .iter()
            .map(|p| {
                row(vec![
                    cell(mono(p.path.clone())),
                    text_cell(p.read_by.clone()),
                ])
            })
            .collect(),
    );

    let t = &report.tallies;
    Page::new(
        Area::Design,
        "Design",
        el("div")
            .class("mj-grid")
            .child(card_with(
                "One declaration, every surface",
                // filled by cockpit.js from the stylesheet's own --mj-design; until then, and
                // without JavaScript, the badge says what it is waiting for
                el("span")
                    .attr("id", "mj-design-check")
                    .class("mj-badge mj-badge--pending")
                    .text("comparing the stylesheet with the executable"),
                el("div")
                    .child(
                        el("div")
                            .class("mj-stats")
                            .child(statistic(t.roles.to_string(), "roles", "the declaration"))
                            .child(statistic(t.statuses.to_string(), "statuses", "the declaration"))
                            .child(statistic(t.states.to_string(), "state words", "the declaration"))
                            .child(statistic(t.type_steps.to_string(), "type steps", "the declaration"))
                            .child(statistic(t.aliases.to_string(), "site synonyms", "the declaration"))
                            .child(statistic(t.palette.to_string(), "palette entries", "the declaration")),
                    )
                    .child(facts(vec![
                        ("Source", Node::Element(mono(report.source.clone()))),
                        ("Fingerprint", Node::Element(mono(report.fingerprint.clone()))),
                        ("On every page", Node::Element(mono(format!("--mj-design: \"{}\"", report.design)))),
                        ("Dark means", Node::Element(mono(format!(".{}", report.theme.class)))),
                        ("Choice stored under", Node::Element(mono(report.theme.storage_key.clone()))),
                        (
                            "Audited at",
                            Node::Element(el("span").class("mj-marks").children(
                                report
                                    .viewports
                                    .iter()
                                    .map(|w| tag(format!("{w}px")))
                                    .collect::<Vec<_>>(),
                            )),
                        ),
                    ]))
                    .child(el("p").class("mj-note").text(
                        "The site published to GitHub Pages, this Cockpit, the pages the executable renders itself and the Swagger shell are projections of the one file named above. A value changed there and regenerated reaches every surface; a value chosen anywhere else fails the design gate.",
                    )),
            ))
            .child(card("Roles", roles))
            .child(card("Status", statuses))
            .child(card("Type", type_scale))
            .child(card("Type stacks, layout, radius, motion", scalars))
            .child(card("Projections", projections)),
    )
    .subtitle("What every surface of this tool is rendered with, from the one declaration this executable carries.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Design", None)])
}

// --------------------------------------------------------------------- entities

/// One kind's index: every object of it, at the kind's own address.
///
/// Not a page per kind. The kind is a path segment, the listing is `objects.list` narrowed
/// by it, and a kind the layer stops holding stops having an index — which is the whole
/// reason a kind never appears in this file by name.
pub fn objects_of_kind(ctx: &Context, kind: &str, query: &[(String, String)]) -> Page {
    let kinds: crate::capability::builtin::entity::KindList =
        match ask(ctx, "entity.kinds", json!({})) {
            Ok(k) => k,
            Err(e) => return failed(Area::Objects, "Objects", e),
        };
    let Some(entry) = kinds.kinds.iter().find(|k| k.kind == kind) else {
        return Page::new(
            Area::Objects,
            "No such kind",
            el("div")
                .child(alert(
                    "fail",
                    format!(
                        "This layer holds no kind '{kind}'. The kinds it holds are listed under Objects."
                    ),
                ))
                .child(link("/cockpit/objects", "Every object")),
        )
        .status(404);
    };

    let list: ObjectList = match ask(ctx, "objects.list", json!({ "kind": kind })) {
        Ok(l) => l,
        Err(e) => return failed(Area::Objects, "Objects", e),
    };
    let needle = query
        .iter()
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.to_lowercase())
        .filter(|v| !v.is_empty());
    let matching: Vec<&ObjectSummary> = list
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
    let base = crate::entity::kind_route(kind);
    let window = Window::new(asked_page(query), PER_PAGE, matching.len());
    let rows: Vec<El> = matching[window.range()]
        .iter()
        .map(|o| {
            row(vec![
                id_cell(crate::entity::route(&o.kind, &o.identity), &o.identity),
                text_cell(o.title.clone().unwrap_or_default()),
                cell(mono(&o.path)),
            ])
        })
        .collect();

    let filters = el("form")
        .class("mj-filters")
        .attr("method", "get")
        .attr("action", &base)
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
        .child(
            el("button")
                .class("mj-button")
                .attr("type", "submit")
                .text("Apply"),
        )
        .child(link(&base, "Clear").class("mj-link mj-clear"))
        .child(link("/cockpit/objects", "Every kind").class("mj-link mj-clear"));

    Page::new(
        Area::Objects,
        kind.to_string(),
        el("div").child(filters).child(if rows.is_empty() {
            nothing("No object matches.")
        } else {
            el("div")
                .child(table(&["Identity", "Title", "Source"], rows))
                .child(pagination(window, |n| {
                    href_with(&base, query, &[("page", Some(&n.to_string()))])
                }))
        }),
    )
    .subtitle(format!(
        "{} of the {} object(s) of kind {kind}. Each has an address of its own, derived from its identity.",
        matching.len(),
        entry.count
    ))
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Objects", Some("/cockpit/objects")),
        (kind, None),
    ])
}

/// One entity: what it is, what it is joined to, what can be said about what it names, and
/// where else it is answered.
///
/// The order of the sections is the order a governance object is read in: what it is, then
/// whether to believe it, then what it is joined to, then where it came from, and the file
/// itself last. Raw Markdown first would be the page a directory listing already gives.
pub fn entity(ctx: &Context, kind: &str, slug: &str) -> Page {
    use crate::capability::builtin::entity::{EntityView, EvidenceState};
    let view: EntityView = match ask(ctx, "entity.show", json!({ "kind": kind, "slug": slug })) {
        Ok(v) => v,
        Err(e) => {
            return Page::new(
                Area::Objects,
                "No such entity",
                el("div")
                    .child(alert("fail", e))
                    .child(link(
                        crate::entity::kind_route(kind),
                        format!("Every {kind}"),
                    ))
                    .child(link("/cockpit/objects", "Every object")),
            )
            .status(404)
        }
    };

    let generated = view.content.contains(generate::HEADER)
        || view.provenance.path.starts_with(generate::OUT_DIR);
    let identity_card = card(
        "What it is",
        facts(vec![
            ("Kind", Node::Element(link(&view.kind_route, &view.kind))),
            ("Identity", Node::Element(mono(&view.identity))),
            ("URI", Node::Element(mono(&view.uri))),
            ("Source", Node::Element(mono(&view.provenance.path))),
            (
                "Provenance",
                Node::Element(if generated {
                    badge("generated", "generated")
                } else {
                    badge("declared", "declared")
                }),
            ),
        ]),
    );

    // the state is the word the capability answered with; this page may not round it up
    let ev = &view.evidence;
    let mut evidence_body = el("div").child(
        el("p")
            .class("mj-lede")
            .child(word_badge(match ev.state {
                EvidenceState::Unclaimed => "unclaimed",
                EvidenceState::Dangling => "dangling",
                EvidenceState::Resolved => "resolved",
            }))
            .child(el("span").text(format!(" {}", ev.meaning))),
    );
    if !ev.artifacts.is_empty() {
        evidence_body = evidence_body.child(table(
            &["Named under", "Artefact", "In the tree"],
            ev.artifacts
                .iter()
                .map(|a| {
                    row(vec![
                        cell(mono(&a.field)),
                        cell(mono(&a.path)),
                        cell(if a.present {
                            badge("present", "yes")
                        } else {
                            badge("fail", "no")
                        }),
                    ])
                })
                .collect(),
        ));
    }
    if let Some(p) = &ev.proof {
        evidence_body = evidence_body.child(
            el("p")
                .class("mj-note")
                .text(format!(
                    "Whether any of it ever ran is a different question, and {} answers it: ",
                    p.capability
                ))
                .child(mono(&p.address)),
        );
    }

    let edge_rows = |outgoing: bool| -> Vec<El> {
        view.relations
            .iter()
            .filter(|e| (e.direction == crate::entity::Direction::Outgoing) == outgoing)
            .map(|e| {
                row(vec![
                    cell(mono(&e.edge)),
                    cell(mono(&e.kind)),
                    match &e.route {
                        Some(r) => id_cell(r.clone(), &e.label),
                        None => text_cell(&e.label),
                    },
                    if e.external {
                        cell(badge("external", "outside the layer"))
                    } else {
                        text_cell("")
                    },
                ])
            })
            .collect()
    };
    let references = edge_rows(true);
    let referenced_by = edge_rows(false);
    let relations_card = card(
        "What it is joined to",
        el("div")
            .child(el("h3").class("mj-subheading").text("References"))
            .child(if references.is_empty() {
                nothing("This entity declares no reference.")
            } else {
                table(&["Relation", "Kind", "Target", ""], references)
            })
            .child(el("h3").class("mj-subheading").text("Referenced by"))
            .child(if referenced_by.is_empty() {
                nothing("Nothing in this layer names it. Backlinks are derived, never declared.")
            } else {
                table(&["Relation", "Kind", "Declared by", ""], referenced_by)
            }),
    );

    let surfaces_card = card(
        "Where else it is answered",
        table(
            &["Surface", "Address", "What it gives"],
            view.surfaces
                .iter()
                .map(|s| {
                    row(vec![
                        cell(mono(&s.surface)),
                        cell(mono(&s.address)),
                        text_cell(&s.detail),
                    ])
                })
                .collect(),
        ),
    );

    let metadata = match &view.metadata {
        Value::Null => card("Front matter", nothing("This object carries none.")),
        m => card(
            "Front matter",
            pre(serde_json::to_string_pretty(m).unwrap_or_default()),
        ),
    };

    Page::new(
        Area::Objects,
        view.identity.clone(),
        el("div")
            .class("mj-grid")
            .child(identity_card)
            .when(generated, |d| {
                d.child(alert(
                    "info",
                    "This file is generated. Editing it is pointless: the generator overwrites it, and `majordomus generate --check` fails while it differs.",
                ))
            })
            .child(card("What can be said about it", evidence_body))
            .child(relations_card)
            .child(surfaces_card)
            .child(metadata)
            .child(details("The file as it is", pre(&view.content))),
    )
    .subtitle(
        view.title
            .clone()
            .or_else(|| view.description.clone())
            .unwrap_or_else(|| format!("An object of kind {}", view.kind)),
    )
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Objects", Some("/cockpit/objects")),
        (&view.kind, Some(&view.kind_route)),
        (&view.identity, None),
    ])
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
