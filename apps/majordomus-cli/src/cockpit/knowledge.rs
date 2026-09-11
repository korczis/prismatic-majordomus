//! The Knowledge and Integrity pages: the repository knowledge system rendered for a
//! person. Every page asks the executor for an `rks.*` capability and lays the answer
//! out; none of them scans anything itself, so the page, the command line, MCP and the
//! HTTP route show one scan.
//!
//! - `/cockpit/knowledge` — the overview: tallies, the check, coverage, gaps, the audit.
//! - `/cockpit/knowledge/nodes` — explore: every node, filtered by kind, freshness,
//!   provenance or text, one page at a time.
//! - `/cockpit/knowledge/node?id=` — one node and *why*: provenance, evidence, claims,
//!   relations, conflicts, gaps, remedies, and its neighbourhood drawn.
//! - `/cockpit/knowledge/graph` — a slice of the graph, drawn and listed.
//! - `/cockpit/knowledge/impact` — what the working tree's changes touch.
//! - `/cockpit/knowledge/coverage`, `/gaps`, `/conflicts`, `/sources`, `/diagnostics`.
//! - `/cockpit/integrity` — the canonicality audit, with one page per capability.

use serde_json::json;

use crate::capability::builtin::knowledge::{ExtractorsReport, GapsReport, StatusReport};
use crate::capability::Context;
use crate::http::router::percent_encode;
use crate::knowledge::baseline::CheckReport;
use crate::knowledge::canonicality::Audit;
use crate::knowledge::impact::ImpactReport;
use crate::knowledge::model::Freshness;
use crate::knowledge::query::{Explanation, GraphSlice, Page as NodePage};

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, asked_page, failed, href_with, Page};
use super::view::{
    alert, badge, card, card_with, cell, chips, details, facts, link, mono, nothing, pagination,
    row, statistic, table, tag, text_cell, Window, PER_PAGE,
};

/// The status word a freshness maps to for a badge.
fn freshness_status(f: Freshness) -> &'static str {
    match f {
        Freshness::Current => "ok",
        Freshness::PossiblyStale | Freshness::Unverified => "warn",
        Freshness::Stale | Freshness::Conflicted => "fail",
    }
}

fn freshness_badge(f: Freshness) -> El {
    badge(freshness_status(f), f.as_str().replace('_', " "))
}

fn verdict_badge(verdict: &str) -> El {
    badge(if verdict == "pass" { "ok" } else { "fail" }, verdict)
}

fn node_href(id: &str) -> String {
    format!("/cockpit/knowledge/node?id={}", percent_encode(id))
}

fn node_link(id: &str) -> El {
    link(node_href(id), id).class("mj-link mj-mono")
}

fn trail<'a>(last: &'a str) -> Vec<(&'a str, Option<&'a str>)> {
    vec![
        ("Cockpit", Some("/cockpit")),
        ("Knowledge", Some("/cockpit/knowledge")),
        (last, None),
    ]
}

/// The links every knowledge page carries to its siblings.
fn sections(here: &str) -> El {
    let items = [
        ("Overview", "/cockpit/knowledge"),
        ("Explore", "/cockpit/knowledge/nodes"),
        ("Graph", "/cockpit/knowledge/graph"),
        ("Impact", "/cockpit/knowledge/impact"),
        ("Coverage", "/cockpit/knowledge/coverage"),
        ("Gaps", "/cockpit/knowledge/gaps"),
        ("Conflicts", "/cockpit/knowledge/conflicts"),
        ("Sources", "/cockpit/knowledge/sources"),
        ("Diagnostics", "/cockpit/knowledge/diagnostics"),
        ("Integrity", "/cockpit/integrity"),
    ];
    chips(
        items
            .iter()
            .map(|(label, href)| (label.to_string(), href.to_string(), 0, *href == here))
            .collect(),
    )
}

// ------------------------------------------------------------------ overview

/// The overview.
pub fn overview(ctx: &Context) -> Page {
    let status: StatusReport = match ask(ctx, "rks.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Knowledge, "Knowledge", e),
    };
    let statistics = el("div")
        .class("mj-stats")
        .child(statistic(status.nodes.to_string(), "nodes", "rks.status"))
        .child(statistic(status.claims.to_string(), "claims", "rks.status"))
        .child(statistic(status.evidence.to_string(), "evidence", "rks.status"))
        .child(statistic(status.relations.to_string(), "relations", "rks.status"))
        .child(statistic(
            status.freshness.get("current").copied().unwrap_or(0).to_string(),
            "current",
            "freshness",
        ))
        .child(statistic(
            status.nodes.saturating_sub(status.freshness.get("current").copied().unwrap_or(0)).to_string(),
            "in debt",
            "freshness",
        ));
    let freshness_chips = chips(
        status
            .freshness
            .iter()
            .map(|(k, n)| {
                (
                    k.replace('_', " "),
                    format!("/cockpit/knowledge/nodes?freshness={k}"),
                    *n,
                    false,
                )
            })
            .collect(),
    );
    let kind_chips = chips(
        status
            .kinds
            .iter()
            .map(|(k, n)| (k.clone(), format!("/cockpit/knowledge/nodes?kind={}", percent_encode(k)), *n, false))
            .collect(),
    );
    let check = check_card(&status.check, &status.baseline.path, status.baseline.recorded);
    let coverage = card(
        "Coverage",
        table(
            &["Row", "Covered", "Discovered", "Missing"],
            status
                .coverage
                .rows
                .iter()
                .map(|r| {
                    row(vec![
                        cell(link("/cockpit/knowledge/coverage", &r.title).class("mj-link")),
                        cell(mono(r.covered.to_string())),
                        cell(mono(r.discovered.to_string())),
                        cell(mono(r.missing.len().to_string())),
                    ])
                })
                .collect(),
        ),
    );
    let gaps = card(
        "Gaps",
        if status.gaps.is_empty() {
            nothing("Nothing the repository could know and does not.")
        } else {
            chips(
                status
                    .gaps
                    .iter()
                    .map(|(k, n)| (k.clone(), format!("/cockpit/knowledge/gaps?category={k}"), *n, false))
                    .collect(),
            )
        },
    );
    let a = &status.canonicality;
    let integrity = card_with(
        "System integrity",
        verdict_badge(&a.verdict),
        el("div")
            .child(
                el("div")
                    .class("mj-stats")
                    .child(statistic(a.capabilities.to_string(), "capabilities", "rks.canonicality"))
                    .child(statistic(a.canonical.to_string(), "canonical (MMS 1)", "rks.canonicality"))
                    .child(statistic(
                        format!("{}.{:02}", a.mms_centi / 100, a.mms_centi % 100),
                        "mean MMS",
                        "manual maintenance surface",
                    ))
                    .child(statistic(a.violations.to_string(), "violations", "rks.canonicality"))
                    .child(statistic(a.counting.to_string(), "counting", "not tolerated, not excepted")),
            )
            .child(el("p").class("mj-note").child(link("/cockpit/integrity", "The audit, capability by capability").class("mj-link"))),
    );
    let identity = card(
        "This scan",
        facts(vec![
            ("Repository", Node::Element(mono(&status.repository.name))),
            (
                "Branch",
                Node::Element(mono(status.repository.branch.clone().unwrap_or_else(|| "(detached)".into()))),
            ),
            ("Working tree", Node::Element(mono(&status.repository.working_tree))),
            ("Mode", Node::Element(tag(status.mode.as_str()))),
            ("Fingerprint", Node::Element(mono(&status.fingerprint[..12.min(status.fingerprint.len())]))),
            (
                "Extractors",
                Node::Element(
                    el("span").class("mj-marks").children(status.extractors.iter().map(mono).collect::<Vec<_>>()),
                ),
            ),
            (
                "Semantic layer",
                Node::Element(el("span").text(if status.semantic.enabled {
                    format!("enabled, provider {}, remote {}", status.semantic.provider, if status.semantic.allow_remote { "allowed" } else { "refused" })
                } else {
                    "off: no provider runs and nothing leaves the machine".into()
                })),
            ),
            ("Provenance", Node::Element(el("span").text(tallies_line(&status.provenance)))),
            ("As data", Node::Element(link("/api/v1/knowledge", "/api/v1/knowledge").class("mj-link mj-mono"))),
        ]),
    );
    Page::new(
        Area::Knowledge,
        "Knowledge",
        el("div")
            .class("mj-grid")
            .child(sections("/cockpit/knowledge"))
            .child(card_with("What the repository knows about itself", verdict_badge(&status.check.verdict), el("div").child(statistics).child(el("h3").class("mj-subheading").text("By freshness")).child(freshness_chips).child(el("h3").class("mj-subheading").text("By kind")).child(kind_chips)))
            .child(check)
            .child(coverage)
            .child(gaps)
            .child(integrity)
            .child(identity),
    )
    .subtitle(status.summary.clone())
    .trail(vec![("Cockpit", Some("/cockpit")), ("Knowledge", None)])
}

fn tallies_line(m: &std::collections::BTreeMap<String, usize>) -> String {
    let parts: Vec<String> = m.iter().filter(|(_, n)| **n > 0).map(|(k, n)| format!("{k} {n}")).collect();
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(", ")
    }
}

fn check_card(check: &CheckReport, baseline_path: &str, recorded: bool) -> El {
    let debt_rows = |items: &[crate::knowledge::baseline::DebtItem]| {
        items
            .iter()
            .map(|d| {
                row(vec![
                    cell(tag(&d.class)),
                    cell(tag(&d.state)),
                    cell(if d.class == "freshness" { node_link(&d.id) } else { mono(&d.id) }),
                    text_cell(d.reason.clone().unwrap_or_default()),
                ])
            })
            .collect::<Vec<_>>()
    };
    card_with(
        "The check against the baseline",
        verdict_badge(&check.verdict),
        el("div")
            .child(facts(vec![
                ("Mode", Node::Element(tag(check.mode.as_str()))),
                (
                    "Baseline",
                    Node::Element(el("span").child(mono(baseline_path)).text(if recorded {
                        check.recorded_at.as_deref().map(|r| format!(" recorded at {}", &r[..12.min(r.len())])).unwrap_or_else(|| " recorded".into())
                    } else {
                        " not recorded: run `majordomus knowledge bootstrap`".into()
                    })),
                ),
                ("Summary", Node::Element(el("span").text(&check.summary))),
            ]))
            .when(!check.new_debt.is_empty(), |d| {
                d.child(el("h3").class("mj-subheading").text("New debt"))
                    .child(table(&["Class", "State", "Subject", "Why"], debt_rows(&check.new_debt)))
            })
            .when(!check.tolerated.is_empty(), |d| {
                d.child(details(
                    format!("{} tolerated item(s)", check.tolerated.len()),
                    table(&["Class", "State", "Subject", "Why"], debt_rows(&check.tolerated)),
                ))
            })
            .when(!check.resolved.is_empty(), |d| {
                d.child(alert(
                    "ok",
                    format!(
                        "{} tolerated item(s) are resolved; record the baseline again so that they cannot come back.",
                        check.resolved.len()
                    ),
                ))
            }),
    )
}

// --------------------------------------------------------------------- nodes

/// Explore: every node, filtered.
pub fn nodes(ctx: &Context, query: &[(String, String)]) -> Page {
    let get = |name: &str| query.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let page = asked_page(query);
    let mut input = json!({
        "offset": (page - 1) * PER_PAGE,
        "limit": PER_PAGE,
    });
    for key in ["kind", "freshness", "provenance", "ownership", "extractor"] {
        if let Some(v) = get(key) {
            input[key] = json!(v);
        }
    }
    if let Some(q) = get("q") {
        input["query"] = json!(q);
    }
    if get("debt").is_some() {
        input["debt"] = json!(true);
    }
    let listing: NodePage = match ask(ctx, "rks.list", input) {
        Ok(l) => l,
        Err(e) => return failed(Area::Knowledge, "Explore", e),
    };
    let status: StatusReport = match ask(ctx, "rks.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Knowledge, "Explore", e),
    };
    let base = "/cockpit/knowledge/nodes";
    let kind_chips = chips(
        std::iter::once(("every kind".to_string(), href_with(base, query, &[("kind", None), ("page", None)]), status.nodes, get("kind").is_none()))
            .chain(status.kinds.iter().map(|(k, n)| {
                (k.clone(), href_with(base, query, &[("kind", Some(k)), ("page", None)]), *n, get("kind").as_deref() == Some(k))
            }))
            .collect(),
    );
    let freshness_chips = chips(
        std::iter::once(("any freshness".to_string(), href_with(base, query, &[("freshness", None), ("page", None)]), status.nodes, get("freshness").is_none()))
            .chain(status.freshness.iter().map(|(k, n)| {
                (k.replace('_', " "), href_with(base, query, &[("freshness", Some(k)), ("page", None)]), *n, get("freshness").as_deref() == Some(k))
            }))
            .collect(),
    );
    let search = el("form")
        .class("mj-search")
        .attr("method", "get")
        .attr("action", base)
        .child(
            el("input")
                .class("mj-input")
                .attr("type", "search")
                .attr("name", "q")
                .attr("value", get("q").unwrap_or_default())
                .attr("aria-label", "Find a node by id or title")
                .attr("placeholder", "Find a node by id or title"),
        )
        .child(el("button").class("mj-button").attr("type", "submit").text("Find"));
    let rows: Vec<El> = listing
        .nodes
        .iter()
        .map(|n| {
            row(vec![
                cell(node_link(&n.id)),
                cell(tag(&n.kind)),
                cell(freshness_badge(n.freshness)),
                cell(tag(n.provenance.as_str())),
                text_cell(n.title.clone()),
                text_cell(n.freshness_reason.clone().unwrap_or_default()),
            ])
        })
        .collect();
    let window = Window::new(page, PER_PAGE, listing.total);
    Page::new(
        Area::Knowledge,
        "Explore the knowledge",
        el("div")
            .class("mj-grid")
            .child(sections(base))
            .child(card("Filter", el("div").child(search).child(kind_chips).child(freshness_chips)))
            .child(card_with(
                "Nodes",
                el("span").class("mj-note").text(format!("{} of {}", listing.nodes.len(), listing.total)),
                if rows.is_empty() {
                    nothing("No node matches the filter.")
                } else {
                    el("div")
                        .child(table(&["Node", "Kind", "Freshness", "Provenance", "Title", "Why"], rows))
                        .child(pagination(window, |p| href_with(base, query, &[("page", Some(&p.to_string()))])))
                },
            )),
    )
    .subtitle("Every node of the model, one page at a time; a node's page says why the model holds it.")
    .trail(trail("Explore"))
}

// ---------------------------------------------------------------------- node

/// One node: the explanation, and its neighbourhood drawn.
pub fn node(ctx: &Context, id: &str) -> Page {
    let e: Explanation = match ctx.execute("rks.explain", json!({ "id": id })) {
        Ok(v) => match serde_json::from_value(v) {
            Ok(e) => e,
            Err(err) => return failed(Area::Knowledge, "Node", err.to_string()),
        },
        Err(err) => {
            return Page::new(
                Area::Knowledge,
                "No such node",
                el("div")
                    .child(alert("fail", err.to_string()))
                    .child(link("/cockpit/knowledge/nodes", "Explore the knowledge").class("mj-link")),
            )
            .status(404)
        }
    };
    let why = card_with(
        "Why the model holds this",
        freshness_badge(e.freshness),
        el("div")
            .child(el("ul").class("mj-list").children(e.why.iter().map(|w| el("li").text(w)).collect::<Vec<_>>()))
            .child(facts(vec![
                ("Kind", Node::Element(tag(&e.kind))),
                ("Provenance", Node::Element(tag(e.provenance.as_str()))),
                ("Ownership", Node::Element(tag(e.ownership.as_str()))),
                ("Visibility", Node::Element(tag(e.visibility.as_str()))),
                ("Extractor", Node::Element(mono(&e.extractor))),
                ("Source", Node::Element(match &e.source { Some(s) => mono(s), None => el("span").text("-") })),
                ("Confidence", Node::Element(el("span").text(&e.confidence))),
                ("As data", Node::Element(link(format!("/api/v1/knowledge/explain?id={}", percent_encode(&e.id)), "/api/v1/knowledge/explain").class("mj-link mj-mono"))),
            ]))
            .when(!e.remedies.is_empty(), |d| {
                d.child(el("h3").class("mj-subheading").text("What to do"))
                    .child(el("ul").class("mj-list").children(e.remedies.iter().map(|r| el("li").text(r)).collect::<Vec<_>>()))
            }),
    );
    let claims = card(
        "Claims",
        if e.claims.is_empty() {
            nothing("No claim: the node is a name something pointed at.")
        } else {
            table(
                &["Predicate", "Value", "Provenance", "Freshness", "Evidence", "Why"],
                e.claims
                    .iter()
                    .map(|c| {
                        row(vec![
                            cell(mono(&c.predicate)),
                            text_cell(match &c.value { serde_json::Value::String(s) => s.clone(), v => v.to_string() }),
                            cell(tag(c.provenance.as_str())),
                            cell(freshness_badge(c.freshness)),
                            cell(el("span").class("mj-marks").children(c.evidence.iter().map(mono).collect::<Vec<_>>())),
                            text_cell(c.reason.clone().unwrap_or_default()),
                        ])
                    })
                    .collect(),
            )
        },
    );
    let evidence = card(
        "Evidence",
        table(
            &["Evidence", "Kind", "Path", "Fingerprint", "Extractor", "Leaves the machine"],
            e.evidence
                .iter()
                .map(|ev| {
                    row(vec![
                        cell(mono(&ev.id)),
                        cell(tag(&ev.kind)),
                        cell(match &ev.path { Some(p) => mono(p), None => el("span").text("-") }),
                        cell(mono(&ev.fingerprint)),
                        cell(mono(&ev.extractor)),
                        cell(badge(if ev.remote_processing { "ok" } else { "warn" }, if ev.remote_processing { "may" } else { "never" })),
                    ])
                })
                .collect(),
        ),
    );
    let relations = card(
        "Relations",
        el("div")
            .child(el("h3").class("mj-subheading").text("This node relates to"))
            .child(if e.relates_to.is_empty() { nothing("Nothing.") } else { relation_table(&e.relates_to, true) })
            .child(el("h3").class("mj-subheading").text("Related from"))
            .child(if e.related_from.is_empty() { nothing("Nothing.") } else { relation_table(&e.related_from, false) }),
    );
    let drawing = el("section")
        .class("mj-card")
        .child(el("div").class("mj-card-head").child(el("h2").class("mj-card-title").text("Neighbourhood")))
        .child(
            el("div")
                .class("mj-graph")
                .attr("data-mj-graph", format!("knowledge:{}", e.id))
                .attr("data-mj-graph-src", format!("/api/v1/knowledge/graph?root={}&depth=1", percent_encode(&e.id)))
                .attr("role", "img")
                .attr("aria-label", "The node and everything one relation away. The same data is listed above."),
        )
        .child(el("p").class("mj-note").text("The drawing is an enhancement; the relations are listed above for a reader without it."));
    let findings = el("div")
        .when(!e.conflicts.is_empty(), |d| {
            d.child(card(
                "Conflicts",
                table(
                    &["Conflict", "Severity", "Resolution", "Basis"],
                    e.conflicts.iter().map(|c| row(vec![cell(mono(&c.id)), cell(tag(format!("{:?}", c.severity).to_lowercase())), cell(tag(format!("{:?}", c.resolution).to_lowercase())), text_cell(c.basis.clone())])).collect(),
                ),
            ))
        })
        .when(!e.gaps.is_empty(), |d| {
            d.child(card(
                "Gaps",
                table(
                    &["Gap", "Why", "Remedy"],
                    e.gaps.iter().map(|g| row(vec![cell(mono(&g.id)), text_cell(g.reason.clone()), text_cell(g.remedy.clone())])).collect(),
                ),
            ))
        });
    Page::new(
        Area::Knowledge,
        e.title.clone(),
        el("div").class("mj-grid").child(sections("")).child(why).child(claims).child(evidence).child(relations).child(drawing).child(findings),
    )
    .subtitle(e.id.clone())
    .trail(vec![("Cockpit", Some("/cockpit")), ("Knowledge", Some("/cockpit/knowledge")), ("Explore", Some("/cockpit/knowledge/nodes")), (&e.id, None)])
    .script("graph.js")
}

fn relation_table(lines: &[String], outgoing: bool) -> El {
    table(
        if outgoing { &["Relation", "Target"] } else { &["Source", "Relation"] },
        lines
            .iter()
            .map(|l| {
                let (a, b) = l.split_once(" -> ").unwrap_or((l.as_str(), ""));
                if outgoing {
                    row(vec![cell(tag(a)), cell(node_link(b))])
                } else {
                    row(vec![cell(node_link(a)), cell(tag(b))])
                }
            })
            .collect(),
    )
}

// --------------------------------------------------------------------- graph

/// A slice of the graph, drawn and listed.
pub fn graph(ctx: &Context, query: &[(String, String)]) -> Page {
    let get = |name: &str| query.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let mut input = json!({ "limit": 200 });
    if let Some(root) = get("root") {
        input["root"] = json!(root);
    }
    if let Some(depth) = get("depth").and_then(|d| d.parse::<usize>().ok()) {
        input["depth"] = json!(depth);
    }
    let kind = get("kind").unwrap_or_else(|| if get("root").is_some() { String::new() } else { "component".into() });
    if !kind.is_empty() {
        input["kind"] = json!(kind);
    }
    let slice: GraphSlice = match ask(ctx, "rks.graph", input.clone()) {
        Ok(s) => s,
        Err(e) => return failed(Area::Knowledge, "Graph", e),
    };
    let status: StatusReport = match ask(ctx, "rks.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Knowledge, "Graph", e),
    };
    let base = "/cockpit/knowledge/graph";
    let kind_chips = chips(
        status
            .kinds
            .iter()
            .map(|(k, n)| (k.clone(), href_with(base, &[], &[("kind", Some(k))]), *n, get("root").is_none() && *k == kind))
            .collect(),
    );
    let src = {
        let mut pairs = Vec::new();
        for (k, v) in input.as_object().into_iter().flatten() {
            let v = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            pairs.push(format!("{}={}", percent_encode(k), percent_encode(&v)));
        }
        format!("/api/v1/knowledge/graph?{}", pairs.join("&"))
    };
    let viewer = el("section")
        .class("mj-card")
        .child(el("div").class("mj-card-head").child(el("h2").class("mj-card-title").text("Graph")))
        .child(
            el("div")
                .class("mj-graph")
                .attr("data-mj-graph", "knowledge")
                .attr("data-mj-graph-src", src.clone())
                .attr("role", "img")
                .attr("aria-label", format!("{} nodes, {} edges. The same data is listed below.", slice.nodes.len(), slice.edges.len())),
        )
        .child(el("p").class("mj-note").text("The drawing is an enhancement. Everything it shows is in the lists below."));
    let node_rows = slice
        .nodes
        .iter()
        .map(|n| row(vec![cell(node_link(&n.id)), cell(tag(&n.kind)), cell(freshness_badge(n.freshness)), cell(tag(n.provenance.as_str())), text_cell(n.title.clone())]))
        .collect();
    let edge_rows = slice
        .edges
        .iter()
        .map(|e| row(vec![cell(node_link(&e.source)), cell(tag(&e.kind)), cell(node_link(&e.target)), cell(tag(e.provenance.as_str()))]))
        .collect();
    Page::new(
        Area::Knowledge,
        "The knowledge graph",
        el("div")
            .class("mj-grid")
            .child(sections(base))
            .child(card("Cut a slice", el("div").child(el("p").class("mj-note").text("Every node of one kind, or the neighbourhood of one node (open a node and follow its drawing).")).child(kind_chips)))
            .child(card("Where it comes from", facts(vec![
                ("Root", Node::Element(match &slice.root { Some(r) => node_link(r), None => el("span").text("(none: by kind)") })),
                ("Nodes", Node::Element(mono(slice.nodes.len().to_string()))),
                ("Edges", Node::Element(mono(slice.edges.len().to_string()))),
                ("Omitted by the cap", Node::Element(mono(slice.omitted.to_string()))),
                ("As data", Node::Element(link(src.clone(), "/api/v1/knowledge/graph").class("mj-link mj-mono"))),
            ])))
            .child(viewer)
            .child(card("Nodes", table(&["Node", "Kind", "Freshness", "Provenance", "Title"], node_rows)))
            .child(card("Edges", table(&["From", "Relation", "To", "Provenance"], edge_rows))),
    )
    .subtitle("Typed relations among typed nodes, each coloured by whether it still holds.")
    .trail(trail("Graph"))
    .script("graph.js")
}

// -------------------------------------------------------------------- impact

/// What the working tree's changes touch.
pub fn impact(ctx: &Context, query: &[(String, String)]) -> Page {
    let base_rev = query.iter().find(|(k, _)| k == "base").map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let mut input = json!({});
    if let Some(b) = &base_rev {
        input["base"] = json!(b);
    }
    let report: ImpactReport = match ask(ctx, "rks.impact", input) {
        Ok(r) => r,
        Err(e) => return failed(Area::Knowledge, "Impact", e),
    };
    let form = el("form")
        .class("mj-search")
        .attr("method", "get")
        .attr("action", "/cockpit/knowledge/impact")
        .child(el("input").class("mj-input").attr("type", "text").attr("name", "base").attr("value", base_rev.clone().unwrap_or_default()).attr("aria-label", "Compare with this revision").attr("placeholder", "HEAD, origin/master, a commit"))
        .child(el("button").class("mj-button").attr("type", "submit").text("Compare"));
    let changed = table(
        &["Status", "Path"],
        report.changed.iter().map(|c| row(vec![cell(tag(&c.status)), cell(mono(&c.path))])).collect(),
    );
    let affected = |items: &[crate::knowledge::impact::Affected]| {
        table(
            &["Depth", "Node", "Kind", "Why"],
            items.iter().map(|a| row(vec![cell(mono(a.depth.to_string())), cell(node_link(&a.id)), cell(tag(&a.kind)), text_cell(a.reason.clone())])).collect(),
        )
    };
    Page::new(
        Area::Knowledge,
        "Impact",
        el("div")
            .class("mj-grid")
            .child(sections("/cockpit/knowledge/impact"))
            .child(card("The change set", el("div").child(form).child(facts(vec![
                ("Change set", Node::Element(tag(&report.change_set))),
                ("Base", Node::Element(mono(report.base.clone().unwrap_or_else(|| "-".into())))),
                ("Changed paths", Node::Element(mono(report.changed.len().to_string()))),
                ("Changed entries", Node::Element(mono(report.entries.len().to_string()))),
                ("Extractors to re-run", Node::Element(el("span").class("mj-marks").children(report.extractors.iter().map(mono).collect::<Vec<_>>()))),
            ])).child(if report.changed.is_empty() { nothing("Nothing changed against the base.") } else { changed })))
            .child(card("Directly affected", if report.direct.is_empty() { nothing("No node rests on a changed path.") } else { affected(&report.direct) }))
            .child(card("Reached along relations", if report.transitive.is_empty() { nothing("Nothing depends on what changed.") } else { affected(&report.transitive) }))
            .when(!report.unmodelled.is_empty(), |d| {
                d.child(card("Changed and unknown to the model", el("ul").class("mj-list").children(report.unmodelled.iter().map(|p| el("li").child(mono(p))).collect::<Vec<_>>())))
            }),
    )
    .subtitle("The nodes whose evidence changed, and everything that rests on them, nearest first.")
    .trail(trail("Impact"))
}

// ------------------------------------------------------------------ coverage

/// Coverage.
pub fn coverage(ctx: &Context) -> Page {
    let status: StatusReport = match ask(ctx, "rks.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Knowledge, "Coverage", e),
    };
    let cards: Vec<El> = status
        .coverage
        .rows
        .iter()
        .map(|r| {
            card_with(
                r.title.clone(),
                badge(if r.missing.is_empty() { "ok" } else { "warn" }, format!("{} / {}", r.covered, r.discovered)),
                el("div")
                    .child(el("p").class("mj-prose").text(format!("Denominator: {}.", r.denominator)))
                    .when(!r.missing.is_empty(), |d| {
                        d.child(details(
                            format!("{} missing", r.missing.len()),
                            el("ul").class("mj-list").children(
                                r.missing
                                    .iter()
                                    .map(|m| el("li").child(if m.contains(':') && !m.contains(": ") { node_link(m) } else { mono(m) }))
                                    .collect::<Vec<_>>(),
                            ),
                        ))
                    }),
            )
        })
        .collect();
    Page::new(Area::Knowledge, "Coverage", el("div").class("mj-grid").child(sections("/cockpit/knowledge/coverage")).children(cards))
        .subtitle("Over denominators the executable defines deterministically. Numbers, and what is missing; never a percentage.")
        .trail(trail("Coverage"))
}

// ---------------------------------------------------------------------- gaps

/// The gaps.
pub fn gaps(ctx: &Context, query: &[(String, String)]) -> Page {
    let category = query.iter().find(|(k, _)| k == "category").map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
    let report: GapsReport = match ask(ctx, "rks.gaps", json!({})) {
        Ok(g) => g,
        Err(e) => return failed(Area::Knowledge, "Gaps", e),
    };
    let base = "/cockpit/knowledge/gaps";
    let category_chips = chips(
        std::iter::once(("every category".to_string(), base.to_string(), report.gaps.len(), category.is_none()))
            .chain(report.tallies.iter().map(|(k, n)| (k.clone(), format!("{base}?category={k}"), *n, category.as_deref() == Some(k))))
            .collect(),
    );
    let rows: Vec<El> = report
        .gaps
        .iter()
        .filter(|g| category.as_deref().is_none_or(|c| crate::knowledge::extract::gap_word(g.category) == c))
        .map(|g| row(vec![cell(tag(crate::knowledge::extract::gap_word(g.category))), cell(node_link(g.subject.split('#').next().unwrap_or(&g.subject))), text_cell(g.reason.clone()), text_cell(g.remedy.clone())]))
        .collect();
    Page::new(
        Area::Knowledge,
        "Gaps",
        el("div")
            .class("mj-grid")
            .child(sections(base))
            .child(card("Category", category_chips))
            .child(card("Gaps", if rows.is_empty() { nothing("Nothing the repository could know and does not.") } else { table(&["Category", "Subject", "Why", "Remedy"], rows) })),
    )
    .subtitle("Everything the repository could know and does not, with the remedy: a worklist, not a score.")
    .trail(trail("Gaps"))
}

// ----------------------------------------------------------------- conflicts

/// The conflicts.
pub fn conflicts(ctx: &Context) -> Page {
    let report: crate::capability::builtin::knowledge::ConflictsReport = match ask(ctx, "rks.conflicts", json!({})) {
        Ok(c) => c,
        Err(e) => return failed(Area::Knowledge, "Conflicts", e),
    };
    let cards: Vec<El> = report
        .conflicts
        .iter()
        .map(|c| {
            card_with(
                c.id.clone(),
                badge(match c.resolution { crate::knowledge::model::Resolution::Open => "fail", crate::knowledge::model::Resolution::Accepted => "warn" }, format!("{:?} · {:?}", c.resolution, c.severity).to_lowercase()),
                el("div")
                    .child(el("p").class("mj-prose").text(&c.basis))
                    .child(table(
                        &["Says", "Provenance", "Evidence"],
                        c.sides.iter().map(|s| row(vec![text_cell(match &s.value { serde_json::Value::String(v) => v.clone(), v => v.to_string() }), cell(tag(s.provenance.as_str())), cell(el("span").class("mj-marks").children(s.evidence.iter().map(mono).collect::<Vec<_>>()))])).collect(),
                    ))
                    .child(el("p").class("mj-note").text(format!("Remedy: {}", c.remedy)))
                    .child(node_link(&c.subject)),
            )
        })
        .collect();
    Page::new(
        Area::Knowledge,
        "Conflicts",
        el("div")
            .class("mj-grid")
            .child(sections("/cockpit/knowledge/conflicts"))
            .child(if cards.is_empty() { card("Conflicts", nothing("No two sources disagree about a functional predicate.")) } else { el("div").class("mj-grid").children(cards) }),
    )
    .subtitle(format!("{} open, {} accepted. Nothing here is resolved silently.", report.open, report.accepted))
    .trail(trail("Conflicts"))
}

// ------------------------------------------------------------------- sources

/// How the model is made.
pub fn sources(ctx: &Context) -> Page {
    let report: ExtractorsReport = match ask(ctx, "rks.extractors", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Knowledge, "Sources", e),
    };
    let extractors: Vec<El> = report
        .extractors
        .iter()
        .map(|x| {
            card_with(
                format!("{} — {}", x.id, x.title),
                badge(if x.deterministic { "ok" } else { "warn" }, if x.deterministic { "deterministic" } else { "not deterministic" }),
                el("div")
                    .child(el("p").class("mj-prose").text(&x.description))
                    .child(facts(vec![
                        ("Version", Node::Element(mono(x.version.to_string()))),
                        ("Reads sensitive files", Node::Element(el("span").text(if x.reads_sensitive { "yes" } else { "no" }))),
                        ("Kinds", Node::Element(el("span").class("mj-marks").children(x.kinds.iter().map(|k| tag(&k.kind)).collect::<Vec<_>>()))),
                        ("Relations", Node::Element(el("span").class("mj-marks").children(x.relations.iter().map(|r| tag(&r.kind)).collect::<Vec<_>>()))),
                        ("Predicates", Node::Element(el("span").class("mj-marks").children(x.predicates.iter().map(|p| tag(&p.name)).collect::<Vec<_>>()))),
                    ])),
            )
        })
        .collect();
    let kinds = card(
        "Node kinds",
        table(&["Kind", "Meaning", "Declared by"], report.kinds.iter().map(|k| row(vec![cell(tag(&k.kind.kind)), text_cell(k.kind.meaning.clone()), cell(mono(&k.extractor))])).collect()),
    );
    let relations = card(
        "Relation kinds",
        table(&["Relation", "Meaning", "Propagates change"], report.relations.iter().map(|r| row(vec![cell(tag(&r.kind)), text_cell(r.meaning.clone()), cell(el("span").text(if r.propagates { "yes" } else { "no" }))])).collect()),
    );
    let predicates = card(
        "Predicates",
        table(&["Predicate", "Meaning", "Functional"], report.predicates.iter().map(|p| row(vec![cell(tag(&p.name)), text_cell(p.meaning.clone()), cell(el("span").text(if p.functional { "one value" } else { "many values" }))])).collect()),
    );
    let providers = card(
        "Semantic providers",
        table(&["Provider", "Model", "Remote", "Operations"], report.providers.iter().map(|p| row(vec![cell(mono(&p.id)), text_cell(p.model.clone()), cell(badge(if p.remote { "warn" } else { "ok" }, if p.remote { "sends text off the machine" } else { "offline" })), cell(el("span").class("mj-marks").children(p.operations.iter().map(tag).collect::<Vec<_>>()))])).collect()),
    );
    let schemas = card(
        "Schemas and migrations",
        el("pre").class("mj-pre").text(serde_json::to_string_pretty(&report.schemas).unwrap_or_default()),
    );
    Page::new(
        Area::Knowledge,
        "Sources",
        el("div").class("mj-grid").child(sections("/cockpit/knowledge/sources")).children(extractors).child(kinds).child(relations).child(predicates).child(providers).child(schemas),
    )
    .subtitle("Every extractor with the vocabulary it declares, the providers this executable ships, and the schema versions it reads and writes.")
    .trail(trail("Sources"))
}

// --------------------------------------------------------------- diagnostics

/// Diagnostics: the check, and what the scan raised.
pub fn diagnostics(ctx: &Context) -> Page {
    let status: StatusReport = match ask(ctx, "rks.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Knowledge, "Diagnostics", e),
    };
    let findings = card(
        "What the scan raised",
        if status.findings.is_empty() {
            nothing("Nothing: every extractor read what it was given, and the model validates.")
        } else {
            el("ul").class("mj-list").children(status.findings.iter().map(|f| el("li").text(f)).collect::<Vec<_>>())
        },
    );
    Page::new(
        Area::Knowledge,
        "Diagnostics",
        el("div")
            .class("mj-grid")
            .child(sections("/cockpit/knowledge/diagnostics"))
            .child(check_card(&status.check, &status.baseline.path, status.baseline.recorded))
            .child(findings)
            .child(card("Commands", el("ul").class("mj-list").children(vec![
                el("li").child(mono("majordomus knowledge check")).text(" — the same verdict, exit 10 when it fails"),
                el("li").child(mono("majordomus knowledge validate")).text(" — the model, the baseline and the exceptions against their contracts"),
                el("li").child(mono("majordomus knowledge reconcile")).text(" — what to do about every finding"),
                el("li").child(mono("majordomus knowledge baseline record --force")).text(" — record the present state deliberately"),
            ]))),
    )
    .subtitle("The check against the baseline, and everything the scan could not read or validate.")
    .trail(trail("Diagnostics"))
}

// ----------------------------------------------------------------- integrity

fn audit_of(ctx: &Context, capability: Option<&str>) -> Result<Audit, String> {
    let mut input = json!({});
    if let Some(c) = capability {
        input["capability"] = json!(c);
    }
    ask(ctx, "rks.canonicality", input)
}

/// System integrity: the canonicality audit.
pub fn integrity(ctx: &Context) -> Page {
    let audit = match audit_of(ctx, None) {
        Ok(a) => a,
        Err(e) => return failed(Area::Integrity, "System integrity", e),
    };
    let summary = card_with(
        "Canonicality",
        verdict_badge(&audit.verdict),
        el("div")
            .child(
                el("div")
                    .class("mj-stats")
                    .child(statistic(audit.capabilities.len().to_string(), "capabilities", "rks.canonicality"))
                    .child(statistic(audit.canonical.to_string(), "canonical (MMS 1)", "rks.canonicality"))
                    .child(statistic(format!("{}.{:02}", audit.mms_centi / 100, audit.mms_centi % 100), "mean MMS", "the goal is 1"))
                    .child(statistic(audit.violations.len().to_string(), "violations", "rks.canonicality"))
                    .child(statistic(audit.violations.iter().filter(|v| !v.tolerated && v.excepted_by.is_none()).count().to_string(), "counting", "neither tolerated nor excepted")),
            )
            .child(el("p").class("mj-prose").text("Every concept has one canonical source of truth; everything else is derived from it. The manual maintenance surface (MMS) of a capability is the number of hand-written files a person must touch to change it: its declaration counts one, every hand-written file that names it one more, generated files none. The architecture is complete when every MMS is one."))
            .child(facts(vec![
                ("Audited on", Node::Element(mono(&audit.date))),
                ("As data", Node::Element(link("/api/v1/canonicality", "/api/v1/canonicality").class("mj-link mj-mono"))),
                ("Command", Node::Element(mono("majordomus canonicality check"))),
            ])),
    );
    let violations = card(
        "Violations",
        if audit.violations.is_empty() {
            nothing("No orphan projection, no undeclared generated file, no missing artifact, no suspected mirror, no expired exception.")
        } else {
            table(
                &["Class", "Standing", "Subject", "Why", "Remedy"],
                audit
                    .violations
                    .iter()
                    .map(|v| {
                        let (status, standing) = if let Some(e) = &v.excepted_by {
                            ("ok", format!("excepted by {e}"))
                        } else if v.tolerated {
                            ("warn", "tolerated".into())
                        } else {
                            ("fail", "counting".into())
                        };
                        row(vec![cell(tag(&v.class)), cell(badge(status, standing)), cell(node_link(&v.subject)), text_cell(v.reason.clone()), text_cell(v.remedy.clone())])
                    })
                    .collect(),
            )
        },
    );
    let capabilities = card(
        "Capabilities",
        table(
            &["Capability", "MMS", "Verdict", "Canonical source", "Surfaces"],
            audit
                .capabilities
                .iter()
                .map(|c| {
                    row(vec![
                        cell(link(format!("/cockpit/integrity/{}", percent_encode(&c.id)), &c.id).class("mj-link mj-mono")),
                        cell(mono(c.mms.to_string())),
                        cell(badge(if c.verdict == "pass" { "ok" } else { "warn" }, &c.verdict)),
                        cell(mono(&c.canonical_source)),
                        cell(el("span").class("mj-marks").children(c.surfaces.iter().map(|s| tag(&s.name)).collect::<Vec<_>>())),
                    ])
                })
                .collect(),
        ),
    );
    let exceptions = card(
        "Exceptions",
        if audit.exceptions.is_empty() {
            nothing("No exception is declared. One is typed and time-limited: id, reason, owner, expires, validation.")
        } else {
            table(
                &["Exception", "Status", "Owner", "Expires", "Reason", "Validation"],
                audit
                    .exceptions
                    .iter()
                    .map(|e| row(vec![cell(mono(&e.exception.id)), cell(badge(match e.status.as_str() { "active" => "ok", "expired" => "fail", _ => "warn" }, &e.status)), text_cell(e.exception.owner.clone()), cell(mono(&e.exception.expires)), text_cell(e.exception.reason.clone()), text_cell(e.exception.validation.clone())]))
                    .collect(),
            )
        },
    );
    Page::new(Area::Integrity, "System integrity", el("div").class("mj-grid").child(summary).child(violations).child(capabilities).child(exceptions))
        .subtitle(audit.summary.clone())
        .trail(vec![("Cockpit", Some("/cockpit")), ("Integrity", None)])
}

/// One capability's canonicality.
pub fn integrity_capability(ctx: &Context, id: &str) -> Page {
    let audit = match audit_of(ctx, Some(id)) {
        Ok(a) => a,
        Err(e) => {
            return Page::new(Area::Integrity, "No such capability", el("div").child(alert("fail", e)).child(link("/cockpit/integrity", "System integrity").class("mj-link"))).status(404)
        }
    };
    let Some(c) = audit.capabilities.first() else {
        return Page::new(Area::Integrity, "No such capability", el("div").child(alert("fail", format!("no query or command `{id}`"))).child(link("/cockpit/integrity", "System integrity").class("mj-link"))).status(404);
    };
    let surfaces = table(
        &["Derived", "Surface", "What"],
        c.surfaces.iter().map(|s| row(vec![cell(badge(if s.derived { "ok" } else { "fail" }, if s.derived { "✓ derived" } else { "✗ by hand" })), cell(tag(&s.name)), text_cell(s.detail.clone())])).collect(),
    );
    let mentions = if c.mentions.is_empty() {
        nothing("No hand-written file names it apart from its declaration.")
    } else {
        table(&["File", "Line", "Text"], c.mentions.iter().map(|m| row(vec![cell(mono(&m.path)), cell(mono(m.line.to_string())), text_cell(m.text.clone())])).collect())
    };
    Page::new(
        Area::Integrity,
        c.id.clone(),
        el("div")
            .class("mj-grid")
            .child(card_with(
                "Canonicality",
                badge(if c.verdict == "pass" { "ok" } else { "warn" }, format!("MMS {} · {}", c.mms, c.verdict)),
                facts(vec![
                    ("Canonical source", Node::Element(mono(&c.canonical_source))),
                    ("Manual maintenance surface", Node::Element(mono(c.mms.to_string()))),
                    ("Descriptor", Node::Element(link(format!("/cockpit/capabilities/{}", percent_encode(&c.id)), "the capability page").class("mj-link"))),
                    ("Why", Node::Element(link(node_href(&format!("capability:{}", c.id)), "the knowledge model's explanation").class("mj-link"))),
                ]),
            ))
            .child(card("Derived surfaces", surfaces))
            .child(card("Hand-written mentions", mentions)),
    )
    .subtitle("One canonical source; every surface derived from it; every hand-written file that names it.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Integrity", Some("/cockpit/integrity")), (&c.id, None)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_maps_to_the_three_badge_words() {
        assert_eq!(freshness_status(Freshness::Current), "ok");
        assert_eq!(freshness_status(Freshness::PossiblyStale), "warn");
        assert_eq!(freshness_status(Freshness::Stale), "fail");
        assert_eq!(freshness_status(Freshness::Conflicted), "fail");
    }
}
