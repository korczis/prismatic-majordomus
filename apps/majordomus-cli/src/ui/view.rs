//! The UI's components: the hero band of a screen, its KPI group, a panel, a donut, a
//! timeline and a meter. Each is a function of readings (`super::source`), never of a value
//! of its own, and each says where it read from.
//!
//! The source of a value is shown in the UI itself — a tile's title, a panel's "Source"
//! line. The UI is its own product; the Cockpit is its low-level reference, so each source
//! carries one quiet "technical view" link to the Cockpit's page for that capability, for the
//! reader who wants the registry behind a value. Nothing else leads there (ADR 0135).
//!
//! Charts are drawn here as SVG, on the server: they are complete without JavaScript, they
//! carry their numbers as text beside them, and they draw only what a capability answered —
//! a series nobody measured is not drawn at all.

use serde_json::Value;

use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::icons::icon;
use super::source::{Answer, Availability, Reading};

/// The tones the design system declares for a glyph or a series (`share/design/tokens.yaml`,
/// `tones`). A component takes one by name; nothing here names a colour.
pub const TONES: &[&str] = &["blue", "violet", "green", "cyan", "orange", "rose"];

// ---------------------------------------------------------------- the hero

/// The hero band every screen opens with: the screen's glyph, its title, one line saying
/// what it is for, one sentence of lead, and the group of KPI tiles beside them.
pub fn hero(glyph: &str, tone: &str, title: &str, tagline: &str, lead: &str, kpis: Vec<El>) -> El {
    el("section")
        .class("mj-ui-hero")
        .attr("aria-labelledby", "mj-ui-title")
        .child(
            el("div")
                .class("mj-ui-hero-text")
                .child(
                    el("div")
                        .class("mj-ui-hero-heading")
                        .child(
                            el("span")
                                .class(format!("mj-ui-hero-glyph mj-tone--{tone}"))
                                .child(icon(glyph)),
                        )
                        .child(
                            el("h1")
                                .attr("id", "mj-ui-title")
                                .class("mj-ui-title")
                                .text(title),
                        ),
                )
                .child(el("p").class("mj-ui-tagline").text(tagline))
                .child(el("p").class("mj-ui-lead").text(lead)),
        )
        .child(
            el("div")
                .class("mj-ui-hero-art")
                .attr("aria-hidden", "true")
                .raw(super::shell::MARK),
        )
        .child(
            el("ul")
                .class("mj-ui-hero-kpis")
                .attr("aria-label", format!("{title} at a glance"))
                .children(
                    kpis.into_iter()
                        .map(|k| el("li").class("mj-ui-kpi-item").child(k)),
                ),
        )
}

/// One KPI tile of a hero group: a label, the value the reading holds and, as its title,
/// where the value was read from.
pub fn kpi(label: &str, reading: &Reading) -> El {
    kpi_with(label, reading, shown(reading), None)
}

/// A KPI tile whose value is the length of the list or map at the reading's pointer: the
/// one count a screen may take of an answer, because it is the answer's own size.
pub fn kpi_count(label: &str, reading: &Reading) -> El {
    let value = match &reading.state {
        Availability::Available(Value::Array(items)) => items.len().to_string(),
        Availability::Available(Value::Object(map)) => map.len().to_string(),
        _ => shown(reading),
    };
    kpi_with(label, reading, value, None)
}

/// A KPI tile whose value is a state word, drawn with its status.
pub fn kpi_state(label: &str, reading: &Reading) -> El {
    let badge = reading
        .text()
        .filter(|w| !w.is_empty())
        .map(|w| word_badge(&w));
    kpi_with(label, reading, shown(reading), badge)
}

fn kpi_with(label: &str, reading: &Reading, value: String, badge: Option<El>) -> El {
    el("span")
        .class("mj-ui-kpi")
        .attr("title", format!("read from {}", reading.cite()))
        .child(el("span").class("mj-ui-kpi-label").text(label))
        .child(match badge {
            Some(b) => el("span").class("mj-ui-kpi-value").child(b),
            None => el("span").class("mj-ui-kpi-value").text(value),
        })
        .child(el("span").class("mj-ui-kpi-source").text(reading.cite()))
}

/// What a reading shows: its value, or which way it is missing. A number is shown as the
/// source gave it, rounded to two places when it is a fraction.
pub fn shown(r: &Reading) -> String {
    match &r.state {
        Availability::Available(Value::String(s)) => s.clone(),
        Availability::Available(Value::Number(n)) => number(n),
        Availability::Available(Value::Bool(b)) => if *b { "yes" } else { "no" }.into(),
        Availability::Available(v) => v.to_string(),
        Availability::Absent => "none".into(),
        Availability::Failed(_) => "not answered".into(),
    }
}

fn number(n: &serde_json::Number) -> String {
    match (n.as_u64(), n.as_i64(), n.as_f64()) {
        (Some(u), _, _) => u.to_string(),
        (_, Some(i), _) => i.to_string(),
        (_, _, Some(f)) => {
            let s = format!("{f:.2}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        _ => n.to_string(),
    }
}

// ---------------------------------------------------------------- panels

/// A panel: a heading, one line under it, the body, and the line naming its sources.
pub fn panel(title: &str, sub: &str, body: El, answers: &[&Answer]) -> El {
    panel_to(title, sub, None, body, answers)
}

/// A panel that leads to the screen holding the whole of what it summarises: its heading
/// carries "View all", as every panel of the owner's references does.
pub fn panel_to(title: &str, sub: &str, more: Option<&str>, body: El, answers: &[&Answer]) -> El {
    let mut head = el("header").class("mj-ui-panel-head").child(
        el("div")
            .class("mj-ui-panel-heading")
            .child(el("h2").class("mj-ui-panel-title").text(title))
            .child(el("p").class("mj-ui-panel-sub").text(sub)),
    );
    if let Some(href) = more {
        head = head.child(
            el("a")
                .class("mj-ui-more")
                .attr("href", href)
                .attr("aria-label", format!("View all: {title}"))
                .child(el("span").text("View all"))
                .child(icon("arrow-right")),
        );
    }
    el("section")
        .class("mj-ui-panel")
        .attr("aria-label", title)
        .child(head)
        .child(el("div").class("mj-ui-panel-body").child(body))
        .child(sources(answers))
}

/// A KPI tile that leads to the screen explaining its value.
pub fn kpi_to(tile: El, href: &str) -> El {
    el("a")
        .class("mj-ui-kpi-anchor")
        .attr("href", href)
        .child(tile)
}

/// A row of KPI tiles outside a hero, each in its own list item.
pub fn strip(tiles: Vec<El>) -> El {
    el("ul").class("mj-ui-strip").children(
        tiles
            .into_iter()
            .map(|k| el("li").class("mj-ui-kpi-item").child(k)),
    )
}

/// A wide panel, spanning the grid.
pub fn wide(panel: El) -> El {
    el("div").class("mj-ui-span").child(panel)
}

/// The sources of a panel: for each, the capability and how long it took, then every other
/// way to ask it the same question — the API route a browser opens, the command a person
/// types, the MCP tool an agent calls — and its technical reference in the Cockpit. Each is
/// the capability's own declaration, as `capabilities.describe` answered it.
pub fn sources(answers: &[&Answer]) -> El {
    let mut box_ = el("div").class("mj-ui-sources");
    for a in answers {
        let mut line = el("p")
            .class("mj-ui-source")
            .child(
                el("span")
                    .class("mj-ui-source-name")
                    .text(a.source.capability),
            )
            .child(
                el("span")
                    .class("mj-ui-dim")
                    .text(format!("{} ms", a.took.as_millis())),
            );
        if let Some(path) = &a.exposure.get {
            line = line.child(chip_link("API", path, "open the same answer as JSON"));
        }
        if let Some(cli) = &a.exposure.cli {
            line = line.child(chip("CLI", cli));
        }
        if let Some(tool) = &a.exposure.mcp {
            line = line.child(chip("MCP", tool));
        }
        line = line.child(
            el("a")
                .class("mj-ui-technical")
                .attr("href", technical(a.source.capability))
                .attr("title", "the capability behind this value, in the Cockpit")
                .text("technical view"),
        );
        box_ = box_.child(line);
    }
    box_
}

fn chip(kind: &str, value: &str) -> El {
    el("span")
        .class("mj-ui-chip")
        .child(el("span").class("mj-ui-chip-kind").text(kind))
        .child(el("code").class("mj-ui-chip-value").text(value))
}

fn chip_link(kind: &str, href: &str, title: &str) -> El {
    el("a")
        .class("mj-ui-chip mj-ui-chip--link")
        .attr("href", href)
        .attr("title", title)
        .child(el("span").class("mj-ui-chip-kind").text(kind))
        .child(el("code").class("mj-ui-chip-value").text(href))
}

/// The Cockpit's page for a capability: the low-level reference behind a value.
pub fn technical(capability: &str) -> String {
    format!("{}/capabilities/{capability}", crate::cockpit::PREFIX)
}

/// The body of a panel whose source could not answer.
pub fn unanswered(a: &Answer) -> Option<El> {
    a.value.as_ref().err().map(|reason| {
        el("p")
            .class("mj-ui-empty")
            .child(word_badge("unknown"))
            .text(format!(
                " {} could not answer: {reason}",
                a.source.capability
            ))
    })
}

/// An empty list, said in words.
pub fn nothing(what: &str) -> El {
    el("p").class("mj-ui-empty").text(what)
}

/// A state word an entry carries, or `unknown` when it carries none: an absent status is
/// never an empty badge, and never a healthy one (project.ui-derived-state).
pub fn state_word(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .filter(|w| !w.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

/// A state word as its badge.
pub fn state(entry: &Value, key: &str) -> El {
    word_badge(&state_word(entry, key).to_lowercase())
}

/// A string field of an entry, or the empty string.
pub fn text(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The list at `pointer` in an answer, or none.
pub fn list(answer: &Answer, pointer: &str) -> Vec<Value> {
    answer
        .ok()
        .and_then(|v| v.pointer(pointer))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// The Cockpit's table, reused: its wrapper scrolls and is therefore in the tab order. An
/// empty list says so instead of drawing an empty table.
pub fn table(headers: &[&str], rows: Vec<El>, empty: &str) -> El {
    if rows.is_empty() {
        return nothing(empty);
    }
    crate::cockpit::view::table(headers, rows)
}

/// A meter of `done` out of `total`, with both numbers as text: the bar is a picture of the
/// two numbers the source gave, not a percentage of its own.
pub fn meter(done: Option<u64>, total: Option<u64>) -> El {
    match (done, total) {
        (Some(d), Some(t)) if t > 0 => {
            let width = (d.min(t) * 100 / t).to_string();
            el("span")
                .class("mj-ui-meter")
                .child(
                    el("span")
                        .class("mj-ui-meter-track")
                        .attr("aria-hidden", "true")
                        .child(
                            el("span")
                                .class("mj-ui-meter-fill")
                                .attr("style", format!("width:{width}%")),
                        ),
                )
                .child(
                    el("span")
                        .class("mj-ui-meter-text")
                        .text(format!("{d} of {t}")),
                )
        }
        (Some(d), Some(t)) => el("span")
            .class("mj-ui-meter-text")
            .text(format!("{d} of {t}")),
        _ => el("span").class("mj-ui-meter-text").text("unknown"),
    }
}

// ---------------------------------------------------------------- graphs

/// A node of a relationship graph: an entity the source named, its label, the tone its
/// state is drawn in, and where clicking it leads.
pub struct GraphNode {
    /// The entity's id, as the source gave it.
    pub id: String,
    /// The label drawn on the node.
    pub label: String,
    /// The tone of the design system it is drawn in.
    pub tone: &'static str,
    /// Where it leads.
    pub href: String,
    /// `leaf` for a node drawn small and labelled only when selected; anything else is a
    /// node of the graph's main kind.
    pub kind: &'static str,
}

/// How a graph is laid out: `directed` for a dependency order, `force` for a topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Breadth-first along the edges' direction.
    Directed,
    /// Force-directed, for a topology with no order.
    Force,
}

/// A relationship graph: the nodes and the typed edges a source answered, drawn by the
/// vendored Cytoscape once the screen has painted. Without JavaScript the `fallback` — a
/// table of the same edges — is what the reader gets, and with it the graph is a second view
/// of the same data, never a different one. An edge whose end is not a node is dropped
/// rather than drawn to nothing.
///
/// Only nodes that take part in an edge are drawn: a node with no relation says nothing in a
/// graph that its row in the fallback does not say better.
pub fn graph(
    label: &str,
    layout: Layout,
    nodes: &[GraphNode],
    edges: &[(String, String)],
    fallback: El,
) -> El {
    let ids: std::collections::BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let related: std::collections::BTreeSet<&str> = edges
        .iter()
        .filter(|(a, b)| ids.contains(a.as_str()) && ids.contains(b.as_str()))
        .flat_map(|(a, b)| [a.as_str(), b.as_str()])
        .collect();
    let data = serde_json::json!({
        "layout": match layout { Layout::Directed => "directed", Layout::Force => "force" },
        "nodes": nodes.iter().filter(|n| related.contains(n.id.as_str())).map(|n| serde_json::json!({
            "id": n.id, "label": n.label, "tone": n.tone, "href": n.href, "kind": n.kind,
        })).collect::<Vec<_>>(),
        "edges": edges.iter()
            .filter(|(a, b)| ids.contains(a.as_str()) && ids.contains(b.as_str()))
            .map(|(a, b)| serde_json::json!({ "source": a, "target": b }))
            .collect::<Vec<_>>(),
    });
    el("div")
        .class("mj-ui-graph")
        .attr("data-graph", data.to_string())
        .child(
            el("div")
                .class("mj-ui-graph-canvas")
                .attr("role", "img")
                .attr("aria-label", label)
                .flag("hidden"),
        )
        .child(el("div").class("mj-ui-graph-fallback").child(fallback))
}

/// The tone a plan status is drawn in: presentation of the status word the source gave, so
/// an unknown word gets the neutral tone rather than a meaning.
pub fn status_tone(status: &str) -> &'static str {
    match status.to_ascii_uppercase().as_str() {
        "DONE" => "green",
        "ACTIVE" | "VERIFY" => "blue",
        "READY" => "cyan",
        "BLOCKED" => "rose",
        "PLANNED" => "violet",
        _ => "orange",
    }
}

// ---------------------------------------------------------------- charts

/// A donut of named counts, each in a tone of the design system, with the counts beside it
/// as a legend: a reader without the picture reads the same numbers. Segments that are
/// zero are listed and not drawn; a donut of nothing says so.
pub fn donut(center: &str, segments: &[(String, u64)]) -> El {
    let total: u64 = segments.iter().map(|(_, n)| n).sum();
    if total == 0 {
        return nothing("Nothing has been counted yet.");
    }
    // a circle of circumference 100, so a segment's dash is its share in percent
    let radius = 15.915_494_309;
    let mut offset = 25.0_f64;
    let mut svg = el("svg")
        .class("mj-ui-donut-svg")
        .attr("viewBox", "0 0 42 42")
        .attr("aria-hidden", "true")
        .child(
            el("circle")
                .class("mj-ui-donut-track")
                .attr("cx", "21")
                .attr("cy", "21")
                .attr("r", format!("{radius}")),
        );
    for (i, (_, n)) in segments.iter().enumerate() {
        if *n == 0 {
            continue;
        }
        let share = *n as f64 * 100.0 / total as f64;
        svg = svg.child(
            el("circle")
                .class(format!(
                    "mj-ui-donut-segment mj-tone--{}",
                    TONES[i % TONES.len()]
                ))
                .attr("cx", "21")
                .attr("cy", "21")
                .attr("r", format!("{radius}"))
                .attr(
                    "stroke-dasharray",
                    format!("{share:.3} {:.3}", 100.0 - share),
                )
                .attr("stroke-dashoffset", format!("{offset:.3}")),
        );
        offset -= share;
    }
    let legend = el("ul")
        .class("mj-ui-legend")
        .children(segments.iter().enumerate().map(|(i, (label, n))| {
            el("li")
                .class(format!(
                    "mj-ui-legend-item mj-tone--{}",
                    TONES[i % TONES.len()]
                ))
                .child(
                    el("span")
                        .class("mj-ui-legend-dot")
                        .attr("aria-hidden", "true"),
                )
                .child(el("span").class("mj-ui-legend-name").text(label))
                .child(el("span").class("mj-ui-legend-value").text(n.to_string()))
        }));
    el("div")
        .class("mj-ui-donut")
        .child(
            el("div").class("mj-ui-donut-figure").child(svg).child(
                el("span")
                    .class("mj-ui-donut-center")
                    .child(el("strong").text(total.to_string()))
                    .child(el("span").text(center)),
            ),
        )
        .child(legend)
}

/// The counts of an object at `pointer`, as donut segments in the order the source gave.
pub fn counts(answer: &Answer, pointer: &str) -> Vec<(String, u64)> {
    answer
        .ok()
        .and_then(|v| v.pointer(pointer))
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_u64().map(|n| (k.to_lowercase(), n)))
                .collect()
        })
        // ui-integrity: an absent object is no segments, never a zero; the donut then says nothing was counted
        .unwrap_or_default()
}

/// A vertical timeline of dated entries, newest first, as the source ordered them.
pub fn timeline(entries: Vec<(String, String, El)>) -> El {
    el("ol")
        .class("mj-ui-timeline")
        .children(entries.into_iter().map(|(when, title, detail)| {
            el("li")
                .class("mj-ui-timeline-item")
                .child(
                    el("span")
                        .class("mj-ui-timeline-dot")
                        .attr("aria-hidden", "true"),
                )
                .child(
                    el("div")
                        .class("mj-ui-timeline-body")
                        .child(el("span").class("mj-ui-timeline-title").text(title))
                        .child(detail),
                )
                .child(
                    el("time")
                        .class("mj-ui-timeline-when")
                        .attr("datetime", when.clone())
                        .text(day(&when)),
                )
        }))
}

/// The day of an RFC 3339 timestamp, as written: no clock, no zone arithmetic.
pub fn day(at: &str) -> String {
    at.split('T').next().unwrap_or(at).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::{Exposure, Source};
    use std::time::{Duration, SystemTime};

    fn answer(value: Result<Value, String>) -> Answer {
        Answer {
            source: Source::of("plan.status"),
            route: Some("GET /api/v1/plan/status".into()),
            exposure: Exposure::default(),
            value,
            took: Duration::from_millis(5),
            at: SystemTime::now(),
        }
    }

    #[test]
    fn a_number_is_shown_as_the_source_gave_it_rounded_only_when_fractional() {
        let a = answer(Ok(
            serde_json::json!({ "n": 16, "r": 2.2857142857142856, "z": 0 }),
        ));
        assert_eq!(shown(&a.read("/n")), "16");
        assert_eq!(shown(&a.read("/r")), "2.29");
        assert_eq!(shown(&a.read("/z")), "0", "a measured zero is a zero");
        assert_eq!(shown(&a.read("/missing")), "none");
    }

    #[test]
    fn a_donut_draws_the_counts_it_lists_and_says_when_there_are_none() {
        let html = donut("issues", &[("ready".into(), 3), ("blocked".into(), 1)]).render();
        assert!(
            html.contains("stroke-dasharray=\"75.000 25.000\""),
            "{html}"
        );
        assert!(html.contains(">ready<") && html.contains(">3<"));
        assert!(html.contains("<strong>4</strong>"));
        assert!(donut("issues", &[("ready".into(), 0)])
            .render()
            .contains("Nothing has been counted"));
    }

    #[test]
    fn a_meter_shows_its_two_numbers_and_never_invents_a_total() {
        assert!(meter(Some(3), Some(4)).render().contains("3 of 4"));
        assert!(meter(Some(3), Some(4)).render().contains("width:75%"));
        assert_eq!(
            meter(None, Some(4)).render(),
            "<span class=\"mj-ui-meter-text\">unknown</span>"
        );
    }

    #[test]
    fn a_panel_names_its_source_and_its_technical_reference_once() {
        let a = answer(Ok(Value::Null));
        let html = panel("Plan", "sub", el("p"), &[&a]).render();
        assert!(html.contains(">plan.status<") && html.contains("5 ms"));
        // the Cockpit is the low-level reference: one link per source, and nowhere else
        assert_eq!(html.matches("/cockpit/").count(), 1, "{html}");
        assert!(html.contains("href=\"/cockpit/capabilities/plan.status\""));
    }

    #[test]
    fn an_absent_state_word_is_unknown() {
        assert_eq!(state_word(&serde_json::json!({}), "status"), "unknown");
        assert_eq!(
            state_word(&serde_json::json!({ "status": "" }), "status"),
            "unknown"
        );
        assert_eq!(
            state_word(&serde_json::json!({ "status": "warn" }), "status"),
            "warn"
        );
    }
}
