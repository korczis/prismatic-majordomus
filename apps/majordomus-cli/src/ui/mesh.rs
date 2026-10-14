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
/// The journal as it stands now: its newest events, not the first page from its beginning.
const EVENTS: Source = Source::with("mesh.events", r#"{"newest":true}"#);

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
/// How many characters of a session's intent its row shows; the rest is in the row's title.
const INTENT: usize = 160;

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
    let selected = query
        .iter()
        .find(|(k, _)| k == "node")
        .map(|(_, v)| v.as_str());
    render(&answers[&PEERS], &answers[&EVENTS], selected)
}

/// The screen drawn from the two answers it reads, with `selected` the machine to inspect.
fn render(peers: &Answer, events: &Answer, selected: Option<&str>) -> View {
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
        grid = grid.child(wide(inspector(name, &machines, peers, events)));
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

/// How many claims a session holds. The summary view of `mesh.peers` counts them in
/// `claims_held` and leaves the `claims` list empty; an answer without the count lists them.
fn live_claims(session: &Value) -> u64 {
    session
        .get("claims_held")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            session
                .get("claims")
                .and_then(Value::as_array)
                .map_or(0, |c| c.len() as u64)
        })
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
            let claims = live_claims(&s);
            // a long intent is cut for the row and said whole in its title
            let said = info("intent");
            let intent: String = if said.chars().count() > INTENT {
                format!("{}…", said.chars().take(INTENT).collect::<String>())
            } else if said.is_empty() {
                "none".into()
            } else {
                said.clone()
            };
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
                .child(
                    el("td")
                        .class("mj-ui-dim")
                        .attr_if("title", (!said.is_empty()).then_some(said))
                        .text(intent),
                )
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
            &["Runtime", "Liveness", "Last beat", "Active sessions"],
            runtime_rows,
            "It runs no runtime the mesh knows.",
        ))
        .child(table(
            &["Session", "Worker", "Claims held", "What it said"],
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
        let from_here: Vec<Value> = list(events, "/events")
            .into_iter()
            .rev()
            .filter(|e| node.is_none_or(|n| text(e, "stream").starts_with(n)))
            .collect();
        let rows = from_here
            .iter()
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
        el("div")
            .child(table(
                &["At", "Event", "Stream"],
                rows,
                "The journal holds no event from here.",
            ))
            .children(super::view::more(
                (ROWS * 2).min(from_here.len()),
                from_here.len(),
                None,
            ))
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

    /// The summary view counts a session's claims and lists none; the count is what is drawn.
    #[test]
    fn a_session_shows_the_claims_the_summary_counts() {
        let summarised = serde_json::json!({ "claims": [], "claims_held": 3 });
        assert_eq!(live_claims(&summarised), 3);
        let listed = serde_json::json!({ "claims": [{}, {}] });
        assert_eq!(live_claims(&listed), 2);
        assert_eq!(live_claims(&serde_json::json!({})), 0);
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
            standing: crate::ui::source::Standing::NOW,
        };
        let html = inspector("nowhere", &[], &peers, &peers).render();
        assert!(html.contains("knows no machine by this name"), "{html}");
    }

    fn answer(source: Source, value: Result<Value, String>) -> Answer {
        Answer {
            source,
            route: None,
            exposure: Default::default(),
            value,
            took: std::time::Duration::ZERO,
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// A `mesh.peers` answer in the shape `MachineNode`/`RuntimeNode`/`SessionNode` serialise
    /// to: this machine with an own and an expired runtime, a remote machine that beats, one
    /// whose only runtime is gone, and one that says neither where it is nor what it runs.
    fn peers() -> Answer {
        answer(
            PEERS,
            Ok(serde_json::json!({
                "active": true,
                "machines": [
                    {
                        "node": "641bdb94aa",
                        "name": "studio",
                        "local": true,
                        "runtimes": [
                            {
                                "runtime": "641bdb94aa-r1abcdef1234",
                                "this_runtime": true,
                                "liveness": "own",
                                "last_beat_ms": 4200,
                                "sessions": [
                                    {
                                        "key": "641bdb94aa-r1abcdef1234/s-1",
                                        "info": {
                                            "session": "s-1",
                                            "worker": "claude-code",
                                            "client": "2.1",
                                            "intent": "build the ui"
                                        },
                                        "state": "open",
                                        "claims": [{ "path": "a" }, { "path": "b" }],
                                        "reviews": []
                                    },
                                    {
                                        "key": "641bdb94aa-r1abcdef1234/anon",
                                        "info": {},
                                        "state": "open",
                                        "claims": [],
                                        "reviews": []
                                    }
                                ]
                            },
                            {
                                "runtime": "641bdb94aa-r2",
                                "this_runtime": false,
                                "liveness": "expired",
                                "sessions": []
                            }
                        ]
                    },
                    {
                        "node": "77aa",
                        "name": "lundra",
                        "local": false,
                        "runtimes": [
                            { "runtime": "77aa-r9", "liveness": "live", "last_beat_ms": 1000, "sessions": [] }
                        ]
                    },
                    {
                        "node": "88bb",
                        "name": "attic",
                        "local": false,
                        "runtimes": [{ "runtime": "88bb-r3", "liveness": "gone", "sessions": [] }]
                    },
                    { "node": "99cc", "name": "ghost" }
                ],
                "refused": [
                    {
                        "endpoint": "10.0.0.9:7000",
                        "direction": "outbound",
                        "refusal": { "detail": "not on the trust list" }
                    },
                    { "endpoint": "10.0.0.10:7000", "direction": "inbound" }
                ]
            })),
        )
    }

    /// A `mesh.events` answer: one event of this machine's runtime with its time and kind, and
    /// one of the remote runtime that says neither.
    fn events() -> Answer {
        answer(
            EVENTS,
            Ok(serde_json::json!({
                "lamport": 42,
                "events": [
                    {
                        "stream": "641bdb94aa-r1abcdef1234",
                        "at": 1_760_000_000u64,
                        "body": { "kind": "session.opened" }
                    },
                    { "stream": "77aa-r9", "body": {} }
                ]
            })),
        )
    }

    /// Every machine, runtime and session becomes a node of the graph and a row of its table,
    /// in the words the mesh runtime used, and the journal and the refused links are listed.
    #[test]
    fn the_topology_draws_every_machine_runtime_and_session() {
        let view = render(&peers(), &events(), None);
        assert_eq!(view.status, 200);
        let html = view.main.render();
        // the graph: machines, their runtimes by short id, and the sessions those hold
        for drawn in [
            "machine:studio",
            "studio (this machine)",
            "runtime:641bdb94aa-r1abcdef1234",
            "r1abcdef12",
            "session:641bdb94aa-r1abcdef1234/s-1",
            "machine:lundra",
            "machine:attic",
            "rose",
        ] {
            assert!(
                html.contains(drawn),
                "the graph does not draw {drawn}: {html}"
            );
        }
        // the table under it: where each machine is, and its runtimes' own words
        for row in [
            "this machine",
            "remote",
            "unknown",
            "/ui/mesh?node=ghost#node",
        ] {
            assert!(html.contains(row), "no row says {row}");
        }
        for word in ["own", "expired", "live", "gone"] {
            assert!(html.contains(word), "the liveness {word} is not shown");
        }
        // no machine is selected, so there is no inspector
        assert!(!html.contains("id=\"node\""), "{html}");
        // the journal, newest first, with what it does not know said as unknown
        assert!(html.contains("session.opened"));
        assert!(
            html.contains("2025-10-09"),
            "the event's time is its own: {html}"
        );
        // the refused links, with the reason when the runtime gave one
        assert!(html.contains("10.0.0.9:7000") && html.contains("not on the trust list"));
        assert!(html.contains("10.0.0.10:7000") && html.contains("inbound"));
    }

    /// Selecting a machine opens its inspector: what it is, its runtimes with their beats,
    /// the sessions they hold with their claims, and only its own events.
    #[test]
    fn a_selected_machine_opens_its_inspector() {
        let html = render(&peers(), &events(), Some("studio")).main.render();
        assert!(html.contains("id=\"node\""), "{html}");
        assert!(html.contains("641bdb94aa"));
        assert!(
            html.contains("4 s ago"),
            "the beat is the runtime's own: {html}"
        );
        assert!(html.contains("claude-code 2.1"));
        assert!(html.contains("build the ui"));
        assert!(html.contains("Its events"));
        let inspector = html
            .split("Its events")
            .nth(1)
            .and_then(|rest| rest.split("Topology").next())
            .expect("the inspector's events");
        assert!(inspector.contains("session.opened"), "{inspector}");
        assert!(
            !inspector.contains("r9"),
            "another machine's event is not this machine's: {inspector}"
        );
    }

    /// A machine that says nothing about itself is shown with what is not known said so,
    /// never as a machine that runs nothing on purpose.
    #[test]
    fn a_machine_that_says_nothing_is_inspected_as_unknown() {
        let all = peers();
        let machines = list(&all, "/machines");
        let html = inspector("ghost", &machines, &all, &events()).render();
        assert!(html.contains("unknown"), "{html}");
        assert!(html.contains("It runs no runtime the mesh knows."));
        assert!(html.contains("No session is open on it."));
        assert!(html.contains("The journal holds no event from here."));
        // a remote machine is said to be remote, and a runtime with no beat has an unknown one
        let html = inspector("attic", &machines, &all, &events()).render();
        assert!(
            html.contains("remote") && html.contains("unknown"),
            "{html}"
        );
    }

    /// A source that could not answer is said to have failed, in every panel that reads it,
    /// and nothing is drawn from it.
    #[test]
    fn a_mesh_that_cannot_answer_is_said_and_never_drawn() {
        let peers = answer(PEERS, Err("the mesh is off".into()));
        let events = answer(EVENTS, Err("no journal".into()));
        let html = render(&peers, &events, None).main.render();
        assert!(
            html.contains("mesh.peers could not answer: the mesh is off"),
            "{html}"
        );
        assert!(
            html.contains("mesh.events could not answer: no journal"),
            "{html}"
        );
        assert!(
            !html.contains("data-graph"),
            "a failed source draws no graph: {html}"
        );
    }

    /// An empty mesh says so in words rather than drawing an empty picture.
    #[test]
    fn an_empty_mesh_says_so() {
        let peers = answer(
            PEERS,
            Ok(serde_json::json!({ "machines": [], "refused": [] })),
        );
        let events = answer(EVENTS, Ok(serde_json::json!({ "events": [] })));
        let html = render(&peers, &events, None).main.render();
        assert!(html.contains("No machine is known to the mesh runtime."));
        assert!(html.contains("No link was refused."));
        assert!(html.contains("The journal holds no event from here."));
    }

    /// A label is the last segment of an id, at most ten characters, and the id itself when
    /// it has no segments.
    #[test]
    fn a_label_is_the_last_segment_of_an_id() {
        assert_eq!(short("641bdb94aa-r1abcdef1234"), "r1abcdef12");
        assert_eq!(short("77aa-r9"), "r9");
        assert_eq!(short("plain"), "plain");
        assert!(livenesses(&serde_json::json!({ "name": "ghost" })).is_empty());
    }

    /// A long intent is cut for its row and said whole in the row's title, a session that
    /// said nothing reads none, and a journal longer than its rows says how many it leaves out.
    #[test]
    fn a_long_intent_is_cut_and_said_whole_and_a_long_journal_counts_the_rest() {
        let long = "x".repeat(INTENT + 40);
        let peers = answer(
            PEERS,
            Ok(
                serde_json::json!({ "machines": [{ "node": "aa", "name": "studio", "runtimes": [{
                "runtime": "aa-r1", "liveness": "own",
                "sessions": [ { "info": { "session": "s-1", "intent": long } },
                              { "info": { "session": "s-2" } } ] }] }] }),
            ),
        );
        let many: Vec<Value> = (0..ROWS * 2 + 5)
            .map(|i| serde_json::json!({ "stream": "aa-r1", "at": 1_760_000_000u64 + i as u64, "body": { "kind": "beat" } }))
            .collect();
        let events = answer(EVENTS, Ok(serde_json::json!({ "events": many })));
        let html = render(&peers, &events, Some("studio")).main.render();
        assert!(
            html.contains(&format!("title=\"{long}\"")),
            "the whole intent is in the title"
        );
        assert!(
            html.contains(&format!("{}…", "x".repeat(INTENT))),
            "the row shows the cut"
        );
        assert!(html.contains("<td class=\"mj-ui-dim\">none</td>"), "{html}");
        assert!(html.contains("5 more not shown"), "{html}");
    }
}
