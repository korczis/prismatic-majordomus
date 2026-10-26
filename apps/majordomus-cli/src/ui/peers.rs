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

use super::prose::Known;
use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{hero, kpi, kpi_count, list, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/peers";

/// The module that follows the live channel for a screen, beside `ui.js`.
pub const LIB: &str = "live.js";

/// The channel the screen follows: the snapshot frames alone, which say that it changed.
pub const CHANNEL: &str = "/events?follow=blackboard&frames=snapshot";

const BLACKBOARD: Source = Source::with("blackboard.snapshot", r#"{"signals":0}"#);
/// The plan's issues and milestones: what an intent, a claim or a review may name, and so
/// link to (I2914).
const ISSUES: Source = Source::of("plan.issues");
const ROADMAP: Source = Source::of("plan.roadmap");

/// The Peers screen; `lib` is the URL of [`LIB`] the page loads to follow the channel.
pub fn screen(ctx: &Context, lib: &str) -> View {
    let answers = ask_all(ctx, &[BLACKBOARD, ISSUES, ROADMAP]);
    let known = super::prose::known(&answers[&ISSUES], &answers[&ROADMAP]);
    let board = answers[&BLACKBOARD].clone();

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
            workers(&board, &known),
            &[&board],
        )))
        .child(panel(
            "Claims that meet",
            "Two workers whose scopes overlap. Confirmed when both claims are exclusive or one is already conflicted; potential otherwise.",
            conflicts(&board, &known),
            &[&board],
        ))
        .child(panel(
            "What waits",
            "Open review requests and handovers nobody has taken yet.",
            waiting(&board, &known),
            &[&board],
        ))
        .child(panel(
            "Relations",
            "Two workers related by a record: verified when an identifier joins them, a candidate when only a weaker match does.",
            relations(&board, &known),
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

fn claims(w: &Value, known: &Known) -> El {
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
        let mut line = el("div")
            .class("mj-ui-mono mj-identity")
            .nodes(super::prose::ids(&scope, known));
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

fn workers(answer: &Answer, known: &Known) -> El {
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
                    .child(el("td").nodes(super::prose::inline(&text(w, "intent"), known)))
                    .child(claims(w, known))
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

fn conflicts(answer: &Answer, known: &Known) -> El {
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
                            .nodes(super::prose::ids(&side(c.get("first")), known)),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-identity")
                            .nodes(super::prose::ids(&side(c.get("second")), known)),
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

fn waiting(answer: &Answer, known: &Known) -> El {
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
                    .child(el("td").nodes(super::prose::ids(
                        &joined(&[text(r, "issue"), text(r, "subject")]),
                        known,
                    )))
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
                .child(el("td").nodes(super::prose::ids(
                    &joined(&[text(h, "issue"), text(h, "name"), text(h, "branch")]),
                    known,
                )))
        }));
        table(
            &["What", "From", "For", "About"],
            rows,
            "No review and no handover waits.",
        )
    })
}

fn relations(answer: &Answer, known: &Known) -> El {
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
                            .nodes(super::prose::ids(&text(r, "from"), known)),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-identity")
                            .nodes(super::prose::ids(&text(r, "to"), known)),
                    )
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
                            .nodes(super::prose::marks(&text(s, "reason"))),
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

    /// An issue a worker names, in what it said or in what it claims, leads to that issue; one
    /// the plan does not hold is said to lead nowhere (I2914).
    #[test]
    fn an_issue_a_worker_names_leads_to_it() {
        let board = serde_json::json!({
            "workers": [{
                "key": "k", "board_peer": "p1", "intent": "I2400 is **done**",
                "claims": [{ "scope": ["issue:I2400..I9999"], "mode": "exclusive" }],
            }],
            "conflicts": [{
                "certainty": "potential",
                "first": { "worker": "board:x/p1", "claim": "I2400" },
                "second": { "worker": "board:x/p2", "claim": "I9999" },
            }],
        });
        let answer = Answer {
            source: BLACKBOARD,
            route: None,
            exposure: crate::ui::source::Exposure::default(),
            value: Ok(std::sync::Arc::new(board)),
            took: std::time::Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        };
        let known = Known {
            issues: ["I2400".to_string()].into(),
            ..Known::default()
        };
        let html = workers(&answer, &known).render();
        assert_eq!(
            html.matches("href=\"/ui/prs?issue=I2400#issue\"").count(),
            2,
            "{html}"
        );
        assert!(html.contains("<strong>done</strong>"), "{html}");
        assert!(html.contains("issue:<a "), "{html}");
        assert!(
            html.contains("..<span class=\"mj-ui-mono\" data-mj-unresolved"),
            "{html}"
        );
        let met = conflicts(&answer, &known).render();
        assert!(met.contains("board:x/p1 · <a "), "{met}");
        assert!(
            met.contains("board:x/p2 · <span class=\"mj-ui-mono\" data-mj-unresolved"),
            "{met}"
        );
    }

    /// Every part of an answer is drawn: an exclusive claim that the mesh marked conflicted,
    /// a pair of claims that meet with the paths where, a review for a named reviewer and one
    /// for anyone, a handover for whoever takes it, and a relation with its evidence.
    #[test]
    fn what_meets_what_waits_and_what_relates_are_drawn() {
        let board = serde_json::json!({
            "workers": [{
                "key": "mesh:s/one",
                "claims": [{ "scope": ["src"], "mode": "exclusive", "conflicted": true }],
            }],
            "conflicts": [{
                "certainty": "confirmed",
                "first": { "worker": "mesh:s/one", "claim": "c-1" },
                "second": { "worker": "board:x/p1", "claim": "announcement" },
                "paths": [{ "yours": "src", "theirs": "src/main.rs" }],
            }],
            "reviews": [
                { "from": "mesh:s/one", "reviewer": "1b", "issue": "I1", "subject": "read #1" },
                { "from": "mesh:s/one", "subject": "anyone, please" },
            ],
            "handovers": [{ "runtime": "r-1", "issue": "I2", "name": "the rest", "branch": "b" }],
            "relations": [{
                "kind": "same_worker",
                "evidence": "verified",
                "from": "board:x/p1",
                "to": "mesh:s/one",
            }],
        });
        let answer = Answer {
            source: BLACKBOARD,
            route: None,
            exposure: crate::ui::source::Exposure::default(),
            value: Ok(std::sync::Arc::new(board)),
            took: std::time::Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        };
        let html = [
            workers(&answer, &Known::default()),
            conflicts(&answer, &Known::default()),
            waiting(&answer, &Known::default()),
            relations(&answer, &Known::default()),
        ]
        .map(|e| e.render())
        .concat();
        for said in [
            " exclusive",
            " conflicted",
            "mesh:s/one · c-1",
            "board:x/p1 · announcement",
            "src · src/main.rs",
            ">1b<",
            ">anyone<",
            "I1 · read #1",
            "whoever takes it",
            "I2 · the rest · b",
            "same worker",
        ] {
            assert!(html.contains(said), "{said:?} is not in {html}");
        }
    }
}
