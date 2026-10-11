//! The Cockpit's Trains page (ADR 0138): the open pull requests stacked on each other, as
//! `integration.trains` answers them.
//!
//! The page is complete without JavaScript: every train is a table of its links in landing
//! order, with every fact the capability carries about each one. `share/cockpit/trains.js`
//! adds a second view of the same answer — a graph in the manner of the Neo4j Browser, with
//! master as its anchor, a property panel, filters and search — and polls the same HTTP
//! route the table was rendered from, so the drawing and the table cannot disagree about
//! what a train is. Nothing here holds a train of its own and nothing here moves a ref.

use serde_json::json;

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, disposition_status, failed, word, Page};
use super::view::{
    alert, badge, card, card_with, cell, details, facts, link, mono, nothing, pre, row, statistic,
    table, tag, text_cell, word_badge,
};
use crate::capability::Context;
use crate::integration::train::{PrepState, Train, TrainLink, TrainsView};

/// The capability the page renders, and the route the drawing polls.
const CAPABILITY: &str = "integration.trains";
const ROUTE: &str = "/api/v1/pull-requests/trains";

/// How often the drawing asks again, in seconds. The answer is read from a recorded
/// observation, so asking more often than the executor observes shows nothing new, and each
/// answer decides every link's relation to master again, which costs seconds on a busy
/// machine: once a minute, and never while the last ask is still out.
const POLL_SECONDS: u32 = 60;

/// `/cockpit/trains`.
pub fn page(ctx: &Context) -> Page {
    let view: TrainsView = match ask(ctx, CAPABILITY, json!({})) {
        Ok(v) => v,
        Err(e) => return failed(Area::Trains, "Trains", e),
    };
    let trail = vec![
        ("Cockpit", Some("/cockpit")),
        ("Integration", Some("/cockpit/integration")),
        ("Trains", None),
    ];
    let subtitle = "Open pull requests stacked on each other, in the order they land: each link's disposition, relation to master, prepared head and turn.";
    let commands = card(
        "Commands",
        el("div")
            .child(el("p").class("mj-prose").text(
                "The page reads; nothing here pushes, fetches or moves a ref. The prepared heads under refs/prep/ are the executor's, pushed at each link's turn.",
            ))
            .child(pre(format!(
                "majordomus prs refresh                  # observe the forge now; the one network read\n\
                 majordomus prs trains                   # these trains, in landing order\n\
                 majordomus prs trains --format json     # the same answer this page draws\n\
                 majordomus prs explain <n>              # why one link is where it is\n\
                 GET {ROUTE}"
            ))),
    );

    if !view.observed {
        return Page::new(
            Area::Trains,
            "Trains",
            el("div")
                .class("mj-grid")
                .child(card(
                    "No trains",
                    el("div")
                        .child(alert(
                            "info",
                            view.reason
                                .clone()
                                .unwrap_or_else(|| "nothing is observed".into()),
                        ))
                        .child(nothing(
                            "Trains are read from a recorded forge observation, and this checkout has none. `majordomus prs refresh` records one.",
                        )),
                ))
                .child(commands),
        )
        .subtitle(subtitle)
        .trail(trail);
    }

    let source = format!("{CAPABILITY} · tallies");
    let stats = el("div")
        .class("mj-stats")
        .child(statistic(
            view.tallies.trains.to_string(),
            "trains",
            source.clone(),
        ))
        .child(statistic(
            view.tallies.links.to_string(),
            "links",
            source.clone(),
        ))
        .child(statistic(
            view.tallies.pending_prep.to_string(),
            "prepared heads pending",
            source.clone(),
        ))
        .child(statistic(
            view.unattached_prep.len().to_string(),
            "prepared heads no train holds",
            format!("{CAPABILITY} · unattached_prep"),
        ));

    let observed_at = view.observed_at.clone().unwrap_or_default();
    let identity = card_with(
        "This answer",
        link(format!("/cockpit/capabilities/{CAPABILITY}"), CAPABILITY),
        facts(vec![
            (
                "Repository",
                Node::Element(mono(view.repository.clone().unwrap_or_default())),
            ),
            (
                "Base",
                Node::Element(
                    el("span")
                        .child(mono(view.base.clone().unwrap_or_default()))
                        .text(" at ")
                        .child(mono(view.master_sha.clone().unwrap_or_default())),
                ),
            ),
            (
                "Observed",
                Node::Element(
                    el("span")
                        .attr("data-mj-trains-observed", observed_at.clone())
                        .text(observed_at),
                ),
            ),
            (
                "Asked",
                Node::Element(
                    el("span")
                        .attr("data-mj-trains-asked", "")
                        .text(format!(
                            "when this page was rendered; the drawing asks {ROUTE} again every {POLL_SECONDS} s"
                        )),
                ),
            ),
        ]),
    );

    let mut grid = el("div")
        .class("mj-grid")
        .child(stats)
        .child(identity)
        .child(viewer(&view));
    if !view.diagnostics.is_empty() {
        grid = grid.child(card_with(
            "Diagnostics",
            badge("warn", view.diagnostics.len().to_string()),
            el("div").children(
                view.diagnostics
                    .iter()
                    .map(|d| alert("warn", d.clone()))
                    .collect::<Vec<_>>(),
            ),
        ));
    }
    if view.trains.is_empty() {
        grid = grid.child(card(
            "Trains",
            nothing("No open pull request is stacked on another, and none declares a dependency."),
        ));
    }
    for t in &view.trains {
        grid = grid.child(train_card(t));
    }
    if !view.unattached_prep.is_empty() {
        let rows: Vec<El> = view
            .unattached_prep
            .iter()
            .map(|p| {
                row(vec![
                    cell(mono(format!("refs/prep/{}", p.number))),
                    cell(mono(short(&p.sha))),
                    text_cell(if p.open {
                        "open, stacked on nothing"
                    } else {
                        "not open"
                    }),
                ])
            })
            .collect();
        grid = grid.child(card(
            "Prepared heads no train holds",
            table(&["Ref", "Commit", "Pull request"], rows),
        ));
    }
    Page::new(Area::Trains, "Trains", grid.child(commands))
        .subtitle(subtitle)
        .trail(trail)
        .script("trains.js")
}

/// The drawing's frame, its controls and its property panel. Empty until the script fills
/// it; without the script the note under it says where the same facts are.
fn viewer(view: &TrainsView) -> El {
    let mut choose = el("select")
        .class("mj-input")
        .attr("data-mj-trains-train", "")
        .attr("aria-label", "Show one train")
        .child(el("option").attr("value", "").text("every train"));
    for t in &view.trains {
        choose = choose.child(el("option").attr("value", t.id.clone()).text(format!(
            "{} ({} links)",
            t.id,
            t.links.len()
        )));
    }
    let button = |data: &str, label: &str, aria: &str| {
        el("button")
            .class("mj-button mj-button--quiet")
            .attr("type", "button")
            .attr(data, "")
            .attr("aria-label", aria)
            .text(label)
    };
    let links: usize = view.trains.iter().map(|t| t.links.len()).sum();
    el("section")
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
                                .attr("data-mj-trains-search", "")
                                .attr("aria-label", "Find a pull request by number, title or branch")
                                .attr("placeholder", "Find #, title, branch"),
                        )
                        .child(choose)
                        .child(button("data-mj-trains-zoom-in", "+", "Zoom in"))
                        .child(button("data-mj-trains-zoom-out", "−", "Zoom out"))
                        .child(button("data-mj-trains-fit", "Fit", "Fit the drawing to the frame"))
                        .child(button(
                            "data-mj-trains-layout",
                            "Layered",
                            "Switch between the force layout and the layered layout",
                        ))
                        .child(button("data-mj-trains-refresh", "Ask now", "Ask for the trains now")),
                ),
        )
        .child(
            // the drawing and its property panel side by side where there is room, the panel
            // under the drawing where there is not: layout only, no colour and no size step
            el("div")
                .attr(
                    "style",
                    "display:flex;flex-wrap:wrap;gap:1rem;align-items:flex-start",
                )
                .child(frame(view, links))
                .child(panel()),
        )
        .child(
            el("p")
                .class("mj-note")
                .attr("id", "mj-trains-keys")
                .text("With the drawing focused: the arrow keys move from link to link, Enter opens the selected link's page, + and − zoom, 0 fits, Escape clears the selection. Click a link to select it, drag to move it, scroll to zoom."),
        )
}

/// The element the drawing is put in: focusable, named, and pointing at the route it polls.
fn frame(view: &TrainsView, links: usize) -> El {
    el("div")
        .class("mj-graph")
        .attr("style", "flex:1 1 36rem;min-width:0")
        .attr("data-mj-trains", "")
        .attr("data-mj-trains-src", ROUTE)
        .attr("data-mj-trains-poll", POLL_SECONDS.to_string())
        .attr("tabindex", "0")
        .attr("role", "application")
        .attr("aria-roledescription", "graph")
        .attr("aria-describedby", "mj-trains-keys")
        .attr(
            "aria-label",
            format!(
                "{} train(s) of {} link(s), anchored on {}. The same facts are listed in the tables below.",
                view.trains.len(),
                links,
                view.base.clone().unwrap_or_default()
            ),
        )
}

/// The property panel: announced when it changes, and reachable by keyboard because it
/// scrolls.
fn panel() -> El {
    el("div")
        .attr("data-mj-trains-panel", "")
        .attr("aria-live", "polite")
        .attr("role", "region")
        .attr("aria-label", "The selected link")
        .attr("tabindex", "0")
        .attr(
            "style",
            "flex:0 1 24rem;min-width:0;max-height:min(70vh,640px);overflow:auto",
        )
        .child(el("p").class("mj-note").text(
            "Select a link to see every field of it here. The drawing is an enhancement: everything it shows is in the tables below, which is what a reader without JavaScript, a crawler and a screen reader get.",
        ))
}

/// One train as a table of its links, in landing order.
fn train_card(t: &Train) -> El {
    let rows: Vec<El> = t.links.iter().map(link_row).collect();
    let mut body = el("div").child(table(
        &[
            "Turn",
            "Pull request",
            "Disposition",
            "Required checks",
            "Relation to master",
            "Prepared head",
            "Stands on",
            "Carries",
            "Branch",
            "Title",
        ],
        rows,
    ));
    let edges: Vec<El> = t
        .edges
        .iter()
        .map(|e| {
            row(vec![
                text_cell(format!("#{}", e.from)),
                cell(mono(e.kind.label())),
                text_cell(format!("#{}", e.to)),
                text_cell(if e.satisfied {
                    "satisfied — it landed"
                } else {
                    "not yet"
                }),
            ])
        })
        .collect();
    body = body.child(details(
        format!("Relationships ({})", t.edges.len()),
        table(&["From", "Relationship", "To", "Satisfied"], edges),
    ));
    if !t.beneath.is_empty() {
        let mut line = el("p")
            .class("mj-prose")
            .text("Beneath it, no longer open: ");
        for (i, b) in t.beneath.iter().enumerate() {
            if i > 0 {
                line = line.text(", ");
            }
            line = match &b.url {
                Some(u) => line.child(link(u.clone(), format!("#{}", b.number))),
                None => line.child(mono(format!("#{}", b.number))),
            };
            line = line.text(format!(" ({})", word(&b.state)));
        }
        body = body.child(line);
    }
    let roots = numbers(&t.roots);
    let tops = numbers(&t.tops);
    el("section")
        .class("mj-card")
        .attr("id", t.id.clone())
        .child(
            el("div")
                .class("mj-card-head")
                .child(
                    el("h2")
                        .class("mj-card-title")
                        .text(format!("{} — roots {roots}, tops {tops}", t.id)),
                )
                .child(match t.next {
                    Some(n) => badge("ok", format!("next #{n}")),
                    None => badge("unknown", "no link at its turn"),
                }),
        )
        .child(body)
}

/// One link as a row of its train's table.
fn link_row(l: &TrainLink) -> El {
    let pr = match &l.url {
        Some(u) => el("span")
            .child(link(u.clone(), format!("#{}", l.number)))
            .when(l.at_turn, |e| e.text(" ").child(tag("at its turn")))
            .when(l.in_cycle, |e| e.text(" ").child(tag("in a cycle"))),
        None => el("span").child(mono(format!("#{}", l.number))),
    };
    let relation = match l.behind {
        Some(n) if l.relation_kind == "behind" => format!("behind by {n}"),
        _ => l.relation_kind.replace('_', " "),
    };
    let relation = if l.carries_master {
        format!("{relation} (carries master)")
    } else {
        relation
    };
    let prep = match &l.prep {
        None => el("span").text("none"),
        Some(p) => el("span").child(mono(short(&p.sha))).text(match p.state {
            PrepState::Pushed => " pushed",
            PrepState::Pending => " pending",
        }),
    };
    let checks = match &l.checks_url {
        Some(u) => el("span")
            .child(word_badge(&word(&l.required_checks)))
            .text(" ")
            .child(link(u.clone(), "checks")),
        None => el("span").child(word_badge(&word(&l.required_checks))),
    };
    row(vec![
        text_cell(l.position.to_string()),
        cell(pr),
        cell(badge(
            disposition_status(l.disposition),
            l.disposition.as_str(),
        )),
        cell(checks),
        text_cell(relation),
        cell(prep),
        text_cell(if l.on.is_empty() {
            "master".to_string()
        } else {
            numbers(&l.on)
        }),
        text_cell(if l.carries.is_empty() {
            "nothing".to_string()
        } else {
            numbers(&l.carries)
        }),
        cell(mono(l.head_ref.clone())),
        text_cell(l.title.clone()),
    ])
}

fn numbers(ns: &[u64]) -> String {
    ns.iter()
        .map(|n| format!("#{n}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn short(sha: &str) -> String {
    sha.chars().take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::model::{
        CheckRunState, IntegrationLane, PullRequestDisposition, RelationToMaster,
        RequiredCheckState,
    };
    use crate::integration::train::{LinkCheck, PreparedHead, TrainEdge, TrainEdgeKind};

    fn link_of(number: u64, prep: Option<PrepState>) -> TrainLink {
        TrainLink {
            number,
            title: format!("change {number}"),
            author: "someone".into(),
            head_ref: format!("feature/{number}"),
            base_ref: "master".into(),
            head_sha: format!("{number:040}"),
            url: Some(format!("https://github.com/o/r/pull/{number}")),
            checks_url: Some(format!("https://github.com/o/r/pull/{number}/checks")),
            disposition: PullRequestDisposition::Ready,
            lane: IntegrationLane::Ready,
            reasons: vec!["ready".into()],
            next_action: None,
            required_checks: RequiredCheckState::Passed,
            checks: vec![LinkCheck {
                name: "ci".into(),
                state: CheckRunState::Passed,
            }],
            relation: RelationToMaster::Behind {
                behind: 4,
                authored: Vec::new(),
            },
            relation_kind: "behind".into(),
            behind: Some(4),
            carries_master: false,
            prep: prep.map(|state| PreparedHead {
                reference: format!("refs/prep/{number}"),
                sha: "abcdef0123456789".into(),
                state,
            }),
            position: 1,
            depth: 0,
            at_turn: true,
            on: Vec::new(),
            carries: Vec::new(),
            rank: 1,
            in_cycle: false,
        }
    }

    #[test]
    fn a_train_card_lists_every_link_with_its_prepared_head_and_relationships() {
        let mut upper = link_of(8, Some(PrepState::Pending));
        upper.position = 2;
        upper.on = vec![7];
        upper.at_turn = false;
        let t = Train {
            id: "train-7".into(),
            roots: vec![7],
            tops: vec![8],
            next: Some(7),
            links: vec![link_of(7, Some(PrepState::Pushed)), upper],
            edges: vec![TrainEdge {
                from: 8,
                to: 7,
                kind: TrainEdgeKind::StackedOn,
                satisfied: false,
            }],
            beneath: Vec::new(),
            by_disposition: Default::default(),
        };
        let html = train_card(&t).render();
        for want in [
            "id=\"train-7\"",
            "next #7",
            "https://github.com/o/r/pull/8/checks",
            "abcdef0123</code> pending",
            "abcdef0123</code> pushed",
            "behind by 4",
            "STACKED_ON",
            "at its turn",
            "feature/8",
        ] {
            assert!(html.contains(want), "the card lacks {want:?}: {html}");
        }
    }

    #[test]
    fn the_viewer_names_the_route_it_polls_and_every_train_it_can_show() {
        let mut v = TrainsView::unobserved(String::new());
        v.observed = true;
        v.base = Some("master".into());
        v.trains = vec![Train {
            id: "train-3".into(),
            roots: vec![3],
            tops: vec![3],
            next: None,
            links: vec![link_of(3, None)],
            edges: Vec::new(),
            beneath: Vec::new(),
            by_disposition: Default::default(),
        }];
        let html = viewer(&v).render();
        assert!(
            html.contains(&format!("data-mj-trains-src=\"{ROUTE}\"")),
            "{html}"
        );
        assert!(html.contains("value=\"train-3\""), "{html}");
        assert!(
            html.contains("tabindex=\"0\""),
            "the drawing is reachable by keyboard"
        );
        assert!(
            html.contains("aria-live=\"polite\""),
            "the panel is announced"
        );
    }
}
