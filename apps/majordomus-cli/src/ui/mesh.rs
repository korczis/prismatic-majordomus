//! Mesh: the machines and runtimes this runtime knows, and what the mesh recorded. Read
//! from `mesh.peers` and `mesh.events`. Liveness is the mesh runtime's own word per
//! runtime; nothing here decides that a machine is up.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::{mesh_panel, ROWS};
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{hero, kpi, kpi_count, list, panel, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/mesh";

const PEERS: Source = Source::of("mesh.peers");
const EVENTS: Source = Source::of("mesh.events");

/// The Mesh screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[PEERS, EVENTS]);
    let (peers, events) = (&answers[&PEERS], &answers[&EVENTS]);

    let top = hero(
        "nodes",
        "cyan",
        "Mesh",
        "The machines working on this repository, and what they told each other.",
        "Each machine with the liveness of its runtimes, and the latest events the mesh journal holds.",
        vec![
            kpi("Mesh active", &peers.read("/active")),
            kpi_count("Machines", &peers.read("/machines")),
            kpi_count("Refused links", &peers.read("/refused")),
            kpi("Journal clock", &events.read("/lamport")),
        ],
    );

    let journal = unanswered(events).unwrap_or_else(|| {
        let rows = list(events, "/events")
            .iter()
            .rev()
            .take(ROWS * 2)
            .map(|e| {
                let kind = e
                    .pointer("/body/kind")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                // the journal stamps an event in seconds since the epoch, UTC
                let at = e
                    .get("at")
                    .and_then(serde_json::Value::as_u64)
                    .map(|s| {
                        crate::peers::rfc3339(
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(s),
                        )
                    })
                    .unwrap_or_else(|| "unknown".into());
                el("tr")
                    .child(el("td").class("mj-ui-mono mj-ui-dim").text(at))
                    .child(el("td").text(kind))
                    .child(el("td").class("mj-ui-mono").text(text(e, "stream")))
            })
            .collect::<Vec<_>>();
        table(
            &["At", "Event", "Stream"],
            rows,
            "The journal holds no event.",
        )
    });

    let refused = unanswered(peers).unwrap_or_else(|| {
        let rows = list(peers, "/refused")
            .iter()
            .map(|r| {
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(r, "endpoint")))
                    .child(el("td").text(text(r, "direction")))
                    .child(el("td").class("mj-ui-dim").text(text(r, "refusal")))
            })
            .collect::<Vec<_>>();
        table(
            &["Endpoint", "Direction", "Why"],
            rows,
            "No link was refused.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(mesh_panel(peers)))
        .child(panel(
            "Journal",
            "The latest events the mesh recorded, newest first.",
            journal,
            &[events],
        ))
        .child(panel(
            "Refused links",
            "Links this runtime would not make, and why.",
            refused,
            &[peers],
        ));

    View::new(
        "Mesh",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}
