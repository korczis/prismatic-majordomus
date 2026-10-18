//! Peers: who works on this repository, on every board and every linked machine, what each
//! claims, which claims meet and what waits for whom (ADR 0176). One source, the blackboard
//! (`blackboard.snapshot`), read as every screen reads a source: its latest reading, which
//! follows the blackboard's change generation as well as the repository
//! (`crate::change::generation_of`), so a screen read after a change frame is either that
//! change or says it is catching up with it. Nothing here joins, counts or ranks a worker:
//! the capability did.
//!
//! The screen follows `/events?follow=blackboard&frames=snapshot` while JavaScript runs:
//! `share/ui/live.js` reads the screen again when the server says the blackboard changed,
//! and the line under the hero says whether it is live, reconnecting or reading on a timer.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask, Answer, Source};
use super::view::{hero, kpi, kpi_count, list, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/peers";

/// The module that follows the live channel for a screen, beside `ui.js`.
pub const LIB: &str = "live.js";

/// The channel the screen follows: the snapshot frames alone, which say that it changed.
pub const CHANNEL: &str = "/events?follow=blackboard&frames=snapshot";

const BLACKBOARD: Source = Source::with("blackboard.snapshot", r#"{"signals":0}"#);

/// The Peers screen; `lib` is the URL of [`LIB`] the page loads to follow the channel.
pub fn screen(ctx: &Context, lib: &str) -> View {
    let board = ask(ctx, BLACKBOARD);

    let top = hero(
        "users",
        "cyan",
        "Peers",
        "Who works on this repository, what each claims, and what waits for whom.",
        "Every board of every checkout and every session on the linked machines, joined only where an explicit identifier joins them. A relation that is only likely is said to be a candidate.",
        vec![
            kpi_count("Workers", &board.read("/workers")),
            kpi("Attached now", &board.read("/present")),
            kpi("Claims that meet", &board.read("/conflicts_total")),
            kpi_count("Reviews open", &board.read("/reviews")),
            kpi_count("Handovers waiting", &board.read("/handovers")),
            kpi("Every source read", &board.read("/complete")),
        ],
    );

    let following = el("p")
        .class("mj-ui-dim")
        .attr("data-follow-state", "")
        .text("Read when this page was served. While JavaScript runs it follows the live channel and is read again when the blackboard changes.");

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Workers",
            "Each worker with where it works, the client it is, whether it is here, what it announced and what it claims.",
            workers(&board),
            &[&board],
        )))
        .child(panel(
            "Claims that meet",
            "Two workers whose scopes overlap. Confirmed when both claims are exclusive or one is already conflicted; potential otherwise.",
            conflicts(&board),
            &[&board],
        ))
        .child(panel(
            "What waits",
            "Open review requests and handovers nobody has taken yet.",
            waiting(&board),
            &[&board],
        ))
        .child(panel(
            "Relations",
            "Two workers related by a record: verified when an identifier joins them, a candidate when only a weaker match does.",
            relations(&board),
            &[&board],
        ))
        .child(panel(
            "Sources",
            "Every board and source the blackboard read, and why one could not be.",
            sources(&board),
            &[&board],
        ));

    View::new(
        "Peers",
        el("div")
            .class("mj-ui-stack")
            .attr("data-follow", CHANNEL)
            .attr("data-follow-lib", lib)
            .child(top)
            .child(following)
            .child(grid),
    )
}

/// A worker's name as people use it: the board position or the session, then its key.
fn who(w: &Value) -> El {
    let name = [text(w, "board_peer"), text(w, "session")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    el("td")
        .child(el("span").class("mj-ui-cell-title").text(name))
        .child(
            el("span")
                .class("mj-ui-mono mj-ui-dim mj-identity")
                .text(text(w, "key")),
        )
}

fn joined(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ")
}

fn claims(w: &Value) -> El {
    let mut cell = el("td");
    for claim in w
        .get("claims")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let scope = claim
            .get("scope")
            .and_then(Value::as_array)
            .map(|s| {
                s.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let mut line = el("div").class("mj-ui-mono mj-identity").text(scope);
        if text(claim, "mode") == "exclusive" {
            line = line.child(el("span").class("mj-tag").text(" exclusive"));
        }
        if claim.get("conflicted").and_then(Value::as_bool) == Some(true) {
            line = line.child(el("span").class("mj-tag").text(" conflicted"));
        }
        cell = cell.child(line);
    }
    cell
}

fn workers(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let rows = list(answer, "/workers")
            .iter()
            .map(|w| {
                el("tr")
                    .attr("id", format!("worker-{}", text(w, "key")))
                    .child(who(w))
                    .child(el("td").class("mj-ui-mono mj-identity").text(joined(&[
                        text(w, "checkout"),
                        text(w, "branch"),
                        text(w, "machine"),
                    ])))
                    .child(el("td").text(joined(&[text(w, "client"), text(w, "origin")])))
                    .child(el("td").child(state(w, "presence")))
                    .child(el("td").text(text(w, "intent")))
                    .child(claims(w))
            })
            .collect::<Vec<_>>();
        table(
            &["Worker", "Where", "Client", "Here", "Intent", "Claims"],
            rows,
            "Nobody is attached to a board or holds a session on the mesh.",
        )
    })
}

fn side(r: Option<&Value>) -> String {
    r.map(|r| joined(&[text(r, "worker"), text(r, "claim")]))
        .unwrap_or_default()
}

fn conflicts(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let rows = list(answer, "/conflicts")
            .iter()
            .map(|c| {
                let paths = c
                    .get("paths")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|p| joined(&[text(p, "yours"), text(p, "theirs")]))
                    .collect::<Vec<_>>()
                    .join("; ");
                el("tr")
                    .child(el("td").child(state(c, "certainty")))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-identity")
                            .text(side(c.get("first"))),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-identity")
                            .text(side(c.get("second"))),
                    )
                    .child(el("td").class("mj-ui-mono mj-identity").text(paths))
            })
            .collect::<Vec<_>>();
        table(
            &["Certainty", "One", "The other", "Where they meet"],
            rows,
            "No two workers claim the same paths.",
        )
    })
}

fn waiting(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let mut rows = list(answer, "/reviews")
            .iter()
            .map(|r| {
                let reviewer = text(r, "reviewer");
                el("tr")
                    .child(el("td").text("review"))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-identity")
                            .text(text(r, "from")),
                    )
                    .child(el("td").text(if reviewer.is_empty() {
                        "anyone".to_string()
                    } else {
                        reviewer
                    }))
                    .child(el("td").text(joined(&[text(r, "issue"), text(r, "subject")])))
            })
            .collect::<Vec<_>>();
        rows.extend(list(answer, "/handovers").iter().map(|h| {
            el("tr")
                .child(el("td").text("handover"))
                .child(
                    el("td")
                        .class("mj-ui-mono mj-identity")
                        .text(text(h, "runtime")),
                )
                .child(el("td").text("whoever takes it"))
                .child(el("td").text(joined(&[
                    text(h, "issue"),
                    text(h, "name"),
                    text(h, "branch"),
                ])))
        }));
        table(
            &["What", "From", "For", "About"],
            rows,
            "No review and no handover waits.",
        )
    })
}

fn relations(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let rows = list(answer, "/relations")
            .iter()
            .map(|r| {
                el("tr")
                    .child(el("td").text(text(r, "kind").replace('_', " ")))
                    .child(el("td").child(state(r, "evidence")))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-identity")
                            .text(text(r, "from")),
                    )
                    .child(el("td").class("mj-ui-mono mj-identity").text(text(r, "to")))
            })
            .collect::<Vec<_>>();
        table(
            &["Relation", "Evidence", "From", "To"],
            rows,
            "No record relates two workers.",
        )
    })
}

fn sources(answer: &Answer) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let rows = list(answer, "/sources")
            .iter()
            .map(|s| {
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(s, "source")))
                    .child(el("td").child(state(s, "state")))
                    .child(
                        el("td")
                            .class("mj-ui-dim mj-identity")
                            .text(text(s, "reason")),
                    )
            })
            .collect::<Vec<_>>();
        table(
            &["Source", "State", "Why"],
            rows,
            "The blackboard read no source.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A worker announced on this checkout's board is a row with its intent and its claim,
    /// and the screen names the channel it follows and the module that follows it.
    #[test]
    fn an_announced_worker_is_a_row_and_the_screen_names_its_channel() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let id = ctx.peers.attach(crate::peers::Transport::Http);
        ctx.peers.identify(
            &id,
            crate::peers::ClientInfo {
                name: "case-peer".into(),
                version: "1".into(),
                title: None,
            },
        );
        ctx.peers
            .announce(&id, "the peers screen", vec!["docs".into()]);
        let html = screen(&ctx, "/ui/assets/live.js?v=1").main.render();
        assert!(html.contains("the peers screen"), "{html}");
        assert!(html.contains(">docs<"), "{html}");
        assert!(
            html.contains("data-follow=\"/events?follow=blackboard&amp;frames=snapshot\""),
            "{html}"
        );
        assert!(
            html.contains("data-follow-lib=\"/ui/assets/live.js?v=1\""),
            "{html}"
        );
        assert!(html.contains("data-follow-state"), "{html}");
    }
}
