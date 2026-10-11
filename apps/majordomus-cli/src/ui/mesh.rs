//! Mesh: the machines working on this repository, their runtimes and sessions, and what the
//! mesh recorded. Read from `mesh.peers` and `mesh.events`.
//!
//! The topology is a graph of what the mesh runtime observed: a machine runs runtimes, a
//! runtime holds sessions. Every node is selectable; the selection is in the URL (`?node=`),
//! so it survives a reload and the browser's Back, and opens the inspector of that machine.
//! Liveness is the mesh runtime's own word per runtime; nothing here decides that a machine
//! is up, and discovery grants no authority (docs/MESH.md), so the inspector shows what a
//! node is and does, and offers no action the executor would not authorise.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::dashboard::ROWS;
use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    graph, hero, kpi, kpi_count, list, nothing, panel, table, text, unanswered, wide, GraphNode,
    Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/mesh";

const PEERS: Source = Source::of("mesh.peers");
const EVENTS: Source = Source::of("mesh.events");

/// The tone a runtime's liveness is drawn in: presentation of the runtime's own word.
fn liveness_tone(word: &str) -> &'static str {
    match word {
        "own" => "blue",
        "live" => "green",
        "expired" | "gone" | "dead" => "rose",
        _ => "orange",
    }
}

/// The liveness words of a machine's runtimes, as the mesh runtime observed them.
fn livenesses(m: &Value) -> Vec<String> {
    m.get("runtimes")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(|r| r.get("liveness").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Where selecting a machine leads: this screen, with the machine in the URL.
fn select(name: &str) -> String {
    format!("{ROUTE}?node={name}#node")
}

/// A short form of a long runtime or stream id, for a label: its last segment.
fn short(id: &str) -> String {
    id.rsplit('-')
        .next()
        .unwrap_or(id)
        .chars()
        .take(10)
        .collect()
}

/// The Mesh screen; `query` may select one machine (`node`).
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let answers = ask_all(ctx, &[PEERS, EVENTS]);
    let (peers, events) = (&answers[&PEERS], &answers[&EVENTS]);
    let selected = query
        .iter()
        .find(|(k, _)| k == "node")
        .map(|(_, v)| v.clone());
    let machines = list(peers, "/machines");

    let top = hero(
        "nodes",
        "cyan",
        "Mesh",
        "The machines working on this repository, and what they told each other.",
        "Select a machine, a runtime or a session to inspect it: what it runs, how alive each runtime is, who is working there and what the mesh recorded from it.",
        vec![
            kpi("Mesh active", &peers.read("/active")),
            kpi_count("Machines", &peers.read("/machines")),
            kpi_count("Refused links", &peers.read("/refused")),
            kpi("Journal clock", &events.read("/lamport")),
        ],
    );

    let topology = unanswered(peers).unwrap_or_else(|| {
        let mut nodes: Vec<GraphNode> = Vec::new();
        let mut edges: Vec<(String, String)> = Vec::new();
        for m in &machines {
            let name = text(m, "name");
            let words = livenesses(m);
            let alive = words.iter().any(|w| w == "live" || w == "own");
            let machine_id = format!("machine:{name}");
            nodes.push(GraphNode {
                id: machine_id.clone(),
                label: if m.get("local").and_then(Value::as_bool) == Some(true) {
                    format!("{name} (this machine)")
                } else {
                    name.clone()
                },
                tone: if alive { "cyan" } else { "rose" },
                href: select(&name),
                kind: "node",
            });
            for r in m
                .get("runtimes")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let rid = text(r, "runtime");
                let runtime_id = format!("runtime:{rid}");
                nodes.push(GraphNode {
                    id: runtime_id.clone(),
                    label: short(&rid),
                    tone: liveness_tone(&text(r, "liveness")),
                    href: select(&name),
                    kind: "node",
                });
                edges.push((machine_id.clone(), runtime_id.clone()));
                for s in r
                    .get("sessions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let key = text(s, "key");
                    let session_id = format!("session:{key}");
                    let label = s
                        .pointer("/info/session")
                        .and_then(Value::as_str)
                        .unwrap_or("session")
                        .to_string();
                    nodes.push(GraphNode {
                        id: session_id.clone(),
                        label,
                        tone: "violet",
                        href: select(&name),
                        kind: "leaf",
                    });
                    edges.push((runtime_id.clone(), session_id));
                }
            }
        }
        let rows = machines.iter().map(machine_row).collect::<Vec<_>>();
        graph(
            "The mesh: machines, the runtimes they run and the sessions those runtimes hold",
            Layout::Force,
            &nodes,
            &edges,
            table(
                &["Machine", "Where", "Runtimes"],
                rows,
                "No machine is known to the mesh runtime.",
            ),
        )
    });

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(name) = selected {
        grid = grid.child(wide(inspector(&name, &machines, peers, events)));
    }
    grid = grid
        .child(wide(panel(
            "Topology",
            "Every machine, runtime and session the mesh runtime knows; select one to inspect it.",
            topology,
            &[peers],
        )))
        .child(panel(
            "Journal",
            "The latest events the mesh recorded, newest first.",
            journal(events, None),
            &[events],
        ))
        .child(panel(
            "Refused links",
            "Links this runtime would not make, and why.",
            refused(peers),
            &[peers],
        ));

    View::new(
        "Mesh",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// One machine as a table row that selects it.
fn machine_row(m: &Value) -> El {
    let name = text(m, "name");
    el("tr")
        .child(
            el("td").child(
                el("a")
                    .class("mj-ui-row-link")
                    .attr("href", select(&name))
                    .child(
                        el("span")
                            .class("mj-ui-mono mj-ui-cell-title")
                            .text(name.clone()),
                    ),
            ),
        )
        .child(
            el("td").text(match m.get("local").and_then(Value::as_bool) {
                Some(true) => "this machine",
                Some(false) => "remote",
                None => "unknown",
            }),
        )
        .child(
            el("td")
                .class("mj-ui-badges")
                .children(livenesses(m).iter().map(|w| word_badge(w))),
        )
}

/// The inspector of one machine: what it is, its runtimes, the sessions they hold, their
/// claims, and the events the journal holds from it.
fn inspector(name: &str, machines: &[Value], peers: &Answer, events: &Answer) -> El {
    let Some(m) = machines.iter().find(|m| text(m, "name") == name) else {
        return panel(
            name,
            "The mesh runtime knows no machine by this name.",
            nothing("It may have left the mesh since the link was made; the topology below is what is known now."),
            &[peers],
        )
        .attr("id", "node");
    };
    let node = text(m, "node");
    let runtimes = m
        .get("runtimes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let runtime_rows = runtimes
        .iter()
        .map(|r| {
            let beat = r.get("last_beat_ms").and_then(Value::as_u64).map_or_else(
                || "unknown".to_string(),
                |ms| format!("{} s ago", ms / 1000),
            );
            let sessions = r
                .get("sessions")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            el("tr")
                .child(
                    el("td")
                        .class("mj-ui-mono")
                        .text(short(&text(r, "runtime"))),
                )
                .child(el("td").child(word_badge(&text(r, "liveness"))))
                .child(el("td").class("mj-ui-mono mj-ui-dim").text(beat))
                .child(el("td").class("mj-ui-num").text(sessions.to_string()))
        })
        .collect::<Vec<_>>();
    let session_rows = runtimes
        .iter()
        .flat_map(|r| {
            r.get("sessions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .map(|s| {
            let info = |k: &str| {
                s.pointer(&format!("/info/{k}"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let claims = s
                .get("claims")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let intent: String = info("intent").chars().take(160).collect();
            el("tr")
                .child(el("td").class("mj-ui-mono").text(info("session")))
                .child(
                    el("td").text(
                        format!("{} {}", info("worker"), info("client"))
                            .trim()
                            .to_string(),
                    ),
                )
                .child(el("td").class("mj-ui-num").text(claims.to_string()))
                .child(el("td").class("mj-ui-dim").text(intent))
        })
        .collect::<Vec<_>>();
    let body = el("div")
        .class("mj-ui-stack")
        .child(
            el("dl").class("mj-ui-facts").children(
                [
                    ("node", node.clone()),
                    (
                        "where",
                        match m.get("local").and_then(Value::as_bool) {
                            Some(true) => "this machine".into(),
                            Some(false) => "remote".into(),
                            None => "unknown".into(),
                        },
                    ),
                    ("runtimes", runtimes.len().to_string()),
                ]
                .into_iter()
                .map(|(k, v)| {
                    el("div")
                        .class("mj-ui-fact")
                        .child(el("dt").text(k))
                        .child(el("dd").class("mj-ui-mono").text(v))
                }),
            ),
        )
        .child(table(
            &["Runtime", "Liveness", "Last beat", "Sessions"],
            runtime_rows,
            "It runs no runtime the mesh knows.",
        ))
        .child(table(
            &["Session", "Worker", "Claims", "What it said"],
            session_rows,
            "No session is open on it.",
        ))
        .child(el("h3").class("mj-ui-panel-title").text("Its events"))
        .child(journal(events, Some(&node)));
    panel(
        name,
        "One machine of the mesh, as the mesh runtime observed it. Discovery grants no authority: what it may run is the executor's decision, not this page's.",
        body,
        &[peers, events],
    )
    .attr("id", "node")
}

/// The journal's latest events, newest first; with `node`, only the streams of that node.
fn journal(events: &Answer, node: Option<&str>) -> El {
    unanswered(events).unwrap_or_else(|| {
        let rows = list(events, "/events")
            .iter()
            .rev()
            .filter(|e| node.is_none_or(|n| text(e, "stream").starts_with(n)))
            .take(ROWS * 2)
            .map(|e| {
                let kind = e
                    .pointer("/body/kind")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                // the journal stamps an event in seconds since the epoch, UTC
                let at = e
                    .get("at")
                    .and_then(Value::as_u64)
                    .map(|s| {
                        crate::peers::rfc3339(
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(s),
                        )
                    })
                    .unwrap_or_else(|| "unknown".into());
                el("tr")
                    .child(el("td").class("mj-ui-mono mj-ui-dim").text(at))
                    .child(el("td").text(kind))
                    .child(el("td").class("mj-ui-mono").text(short(&text(e, "stream"))))
            })
            .collect::<Vec<_>>();
        table(
            &["At", "Event", "Stream"],
            rows,
            "The journal holds no event from here.",
        )
    })
}

fn refused(peers: &Answer) -> El {
    unanswered(peers).unwrap_or_else(|| {
        let rows = list(peers, "/refused")
            .iter()
            .map(|r| {
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(r, "endpoint")))
                    .child(el("td").text(text(r, "direction")))
                    .child(
                        el("td").class("mj-ui-dim").text(
                            r.pointer("/refusal/detail")
                                .and_then(Value::as_str)
                                .unwrap_or_default(),
                        ),
                    )
            })
            .collect::<Vec<_>>();
        table(
            &["Endpoint", "Direction", "Why"],
            rows,
            "No link was refused.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runtime's tone is its own word's presentation; an unknown word never reads as alive.
    #[test]
    fn an_unknown_liveness_is_never_drawn_as_alive() {
        assert_eq!(liveness_tone("live"), "green");
        assert_eq!(liveness_tone("own"), "blue");
        assert_eq!(liveness_tone("expired"), "rose");
        assert_eq!(liveness_tone("something-new"), "orange");
    }

    /// Selecting a machine is a URL, so it survives a reload and the browser's Back.
    #[test]
    fn a_selection_is_a_url() {
        assert_eq!(select("641bdb94"), "/ui/mesh?node=641bdb94#node");
    }

    /// A machine the mesh does not know is said to be unknown, not drawn from nothing.
    #[test]
    fn an_unknown_machine_says_so() {
        let peers = Answer {
            source: PEERS,
            route: None,
            exposure: Default::default(),
            value: Ok(serde_json::json!({ "machines": [] })),
            took: std::time::Duration::ZERO,
        };
        let html = inspector("nowhere", &[], &peers, &peers).render();
        assert!(html.contains("knows no machine by this name"), "{html}");
    }
}
