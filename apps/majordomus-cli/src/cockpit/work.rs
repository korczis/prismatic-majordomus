//! The development areas: the plan, one milestone, one issue, the sessions and the board.
//!
//! Each page renders what a capability answered and decides nothing of its own. A status
//! here is a word `plan.*` answered, a readiness is a word `devtask.*` derived, and a
//! standing is a word `lifecycle.episodes`, `peers.list` or `server.status` decided. The one
//! thing a page chooses is the colour a word wears, and that is the design system's
//! vocabulary through [`word_badge`], not a rule about the plan.
//!
//! The one write reachable from here is `plan.transition`, offered on an issue page. The page
//! does not judge whether a move is legal: the capability refuses an illegal one and names
//! what is in the way, and `plan.js` shows that answer as it came.

use serde_json::json;

use crate::capability::builtin::lifecycle::Episodes;
use crate::capability::builtin::plan::{PlanIssueList, PlanNextIssue, PlanRoadmap};
use crate::capability::builtin::{LeaseView, PeerList, ServerStanding, ServerStatus, ServerView};
use crate::capability::Context;
use crate::devtask::{
    AttestedCount, AttestedList, AttestedText, DevTask, DevTaskDiagnostic, FieldProvenance,
    MilestoneGraph,
};
use crate::http::router::percent_encode;
use crate::peers::{Overlap, Peer};
use crate::plan::Transition;

use super::html::{el, text, El, Node};
use super::nav::Area;
use super::pages::{ask, episodes_card, failed, param, select, word, Page};
use super::view::{
    alert, badge, card, card_with, cell, details, facts, id_cell, link, mono, nothing, row,
    statistic, table, tag, text_cell, word_badge,
};

// ------------------------------------------------------------------- references

/// Where one issue of the plan is shown.
fn issue_href(id: &str) -> String {
    format!("/cockpit/plan/issues/{}", percent_encode(id))
}

/// Where one milestone of the plan is shown.
fn milestone_href(id: &str) -> String {
    format!("/cockpit/plan/milestones/{}", percent_encode(id))
}

/// A reference to something of the plan, as a link. `plan.issues` names a milestone gate in
/// a list of issue ids as `milestone:<id>`; that is the answer's own spelling, and it links
/// the milestone rather than an issue that does not exist.
fn plan_ref(id: &str) -> El {
    let href = match id.strip_prefix("milestone:") {
        Some(milestone) => milestone_href(milestone),
        None => issue_href(id),
    };
    el("a").class("mj-link mj-mono").attr("href", href).text(id)
}

/// A list of plan references, or a dash when there are none.
fn refs(ids: &[String]) -> El {
    if ids.is_empty() {
        return el("span").class("mj-muted").text("—");
    }
    el("div")
        .class("mj-marks")
        .children(ids.iter().map(|id| plan_ref(id)).collect::<Vec<_>>())
}

/// A list of milestone ids, as links to the milestones.
fn milestone_refs(ids: &[String]) -> El {
    if ids.is_empty() {
        return el("span").class("mj-muted").text("—");
    }
    el("div").class("mj-marks").children(
        ids.iter()
            .map(|id| {
                el("a")
                    .class("mj-link mj-mono")
                    .attr("href", milestone_href(id))
                    .text(id)
            })
            .collect::<Vec<_>>(),
    )
}

// ------------------------------------------------------------------- attested values

/// What is shown for a value `devtask` could not answer: the word and the reason. Never an
/// empty cell, because `objective: ""` and a record with no objective are different facts.
fn unknown(reason: Option<&str>) -> El {
    el("span").class("mj-note").text(match reason {
        Some(reason) => format!("unknown: {reason}"),
        None => "unknown".to_string(),
    })
}

/// Where a value came from, on hover: its provenance and its source.
fn provenance_title(provenance: FieldProvenance, source: &str) -> String {
    format!("{} · {source}", provenance.as_str())
}

/// A text value `devtask` answered.
fn attested_text(value: &AttestedText) -> Node {
    Node::Element(match &value.value {
        Some(v) => el("span")
            .attr("title", provenance_title(value.provenance, &value.source))
            .text(v.clone()),
        None => unknown(value.reason.as_deref()),
    })
}

/// A count `devtask` answered.
fn attested_count(value: &AttestedCount) -> Node {
    Node::Element(match value.value {
        Some(n) => {
            mono(n.to_string()).attr("title", provenance_title(value.provenance, &value.source))
        }
        None => unknown(value.reason.as_deref()),
    })
}

/// A list `devtask` answered, each value laid out by `render`. An empty list the record
/// declared is "none"; an empty list `devtask` could not read is unknown, with the reason.
fn attested_list(value: &AttestedList, render: impl Fn(&str) -> El) -> Node {
    if value.values.is_empty() {
        return Node::Element(if matches!(value.provenance, FieldProvenance::Unknown) {
            unknown(value.reason.as_deref())
        } else {
            el("span").class("mj-muted").text("none")
        });
    }
    Node::Element(
        el("div")
            .class("mj-marks")
            .attr("title", provenance_title(value.provenance, &value.source))
            .children(value.values.iter().map(|v| render(v)).collect::<Vec<_>>()),
    )
}

/// The same list as a checklist: acceptance criteria are sentences, not tags.
fn attested_sentences(value: &AttestedList) -> El {
    if value.values.is_empty() {
        return if matches!(value.provenance, FieldProvenance::Unknown) {
            unknown(value.reason.as_deref())
        } else {
            nothing("The record declares none.")
        };
    }
    el("ul").class("mj-checklist").children(
        value
            .values
            .iter()
            .map(|v| el("li").class("mj-checklist-item").text(v.clone()))
            .collect::<Vec<_>>(),
    )
}

/// Every finding `devtask` reported, with the command that reproduces it.
fn diagnostics_card(diagnostics: &[DevTaskDiagnostic]) -> El {
    if diagnostics.is_empty() {
        return card("Findings", nothing("Nothing is wrong with these records."));
    }
    card(
        "Findings",
        table(
            &["Level", "Code", "Message", "Reproduce"],
            diagnostics
                .iter()
                .map(|d| {
                    row(vec![
                        cell(word_badge(&d.level)),
                        cell(mono(d.code.clone())),
                        text_cell(d.message.clone()),
                        cell(mono(d.reproduce.clone())),
                    ])
                })
                .collect(),
        ),
    )
}

// ------------------------------------------------------------------- the plan

/// The plan: the milestones in derived order, the issue the plan hands out next, and every
/// issue, narrowed by milestone, status or wave.
pub fn plan(ctx: &Context, query: &[(String, String)]) -> Page {
    let asked_milestone = param(query, "milestone");
    let asked_status = param(query, "status");
    let asked_wave = param(query, "wave");

    let roadmap: PlanRoadmap = match ask(ctx, "plan.roadmap", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Plan, "Plan", e),
    };
    let next: PlanNextIssue = match ask(ctx, "plan.next", json!({})) {
        Ok(n) => n,
        Err(e) => return failed(Area::Plan, "Plan", e),
    };
    let mut filter = json!({});
    if let Some(milestone) = &asked_milestone {
        filter["milestone"] = json!(milestone);
    }
    if let Some(status) = &asked_status {
        filter["status"] = json!(status);
    }
    if let Some(wave) = &asked_wave {
        match wave.parse::<u32>() {
            Ok(n) => filter["wave"] = json!(n),
            Err(_) => {
                return failed(
                    Area::Plan,
                    "Plan",
                    format!("`wave` is a whole number, and `{wave}` is not one"),
                )
                .status(400)
            }
        }
    }
    let list: PlanIssueList = match ask(ctx, "plan.issues", filter) {
        Ok(l) => l,
        Err(e) => return failed(Area::Plan, "Plan", e),
    };

    let statistics = el("div")
        .class("mj-stats")
        .child(statistic(
            roadmap.milestones.len().to_string(),
            "milestones",
            "plan.roadmap",
        ))
        .child(statistic(
            list.total.to_string(),
            "issues shown",
            "plan.issues",
        ))
        .child(statistic(
            roadmap.now.clone().unwrap_or_else(|| "—".to_string()),
            "milestone now",
            "plan.roadmap",
        ))
        .child(statistic(
            next.issue
                .as_ref()
                .map(|i| i.id.clone())
                .unwrap_or_else(|| "none".to_string()),
            "issue next",
            "plan.next",
        ));

    let next_card = card_with(
        "What to work on",
        link("/cockpit/capabilities/plan.next", "plan.next"),
        match &next.issue {
            Some(i) => facts(vec![
                (
                    "Issue",
                    Node::Element(
                        el("span")
                            .child(plan_ref(&i.id))
                            .text(" ")
                            .child(word_badge(&i.status)),
                    ),
                ),
                ("Title", text(i.title.clone())),
                (
                    "Milestone",
                    Node::Element(milestone_refs(std::slice::from_ref(&i.milestone))),
                ),
                ("Wave", Node::Element(mono(i.wave.to_string()))),
                ("Objective", text(i.objective.clone())),
            ]),
            None => nothing(
                next.reason
                    .clone()
                    .unwrap_or_else(|| "The plan has no issue to hand out.".to_string()),
            ),
        },
    );

    let milestones = card_with(
        "Milestones",
        link("/cockpit/capabilities/plan.roadmap", "plan.roadmap"),
        if roadmap.milestones.is_empty() {
            nothing("The plan declares no milestone.")
        } else {
            table(
                &[
                    "Status",
                    "Milestone",
                    "Title",
                    "Issues",
                    "Rank",
                    "Waiting on",
                ],
                roadmap
                    .milestones
                    .iter()
                    .map(|m| {
                        let now = roadmap.now.as_deref() == Some(m.id.as_str());
                        let after = roadmap.next.as_deref() == Some(m.id.as_str());
                        row(vec![
                            cell(
                                el("span")
                                    .child(word_badge(&m.status))
                                    .when(now, |e| e.text(" ").child(tag("now")))
                                    .when(after, |e| e.text(" ").child(tag("next"))),
                            ),
                            id_cell(milestone_href(&m.id), m.id.clone()),
                            text_cell(m.title.clone()),
                            cell(
                                el("div").class("mj-marks").children(
                                    m.counts
                                        .by_status
                                        .iter()
                                        .filter(|(_, n)| **n > 0)
                                        .map(|(status, n)| tag(format!("{n} {status}")))
                                        .collect::<Vec<_>>(),
                                ),
                            ),
                            text_cell(m.rank.to_string()),
                            cell(milestone_refs(&m.blocked_by)),
                        ])
                    })
                    .collect(),
            )
        },
    );

    let narrowed = asked_milestone.is_some() || asked_status.is_some() || asked_wave.is_some();
    let filters = el("form")
        .class("mj-filters")
        .attr("method", "get")
        .attr("action", "/cockpit/plan")
        .child(select(
            "milestone",
            "Milestone",
            asked_milestone.as_deref(),
            roadmap
                .milestones
                .iter()
                .map(|m| (m.id.clone(), format!("{} — {}", m.id, m.title)))
                .collect(),
        ))
        .child(select(
            "status",
            "Status",
            asked_status.as_deref(),
            list.statuses
                .issue
                .iter()
                .map(|s| (s.clone(), s.clone()))
                .collect(),
        ))
        .child(
            el("label")
                .class("mj-field")
                .child(el("span").class("mj-field-label").text("Wave"))
                .child(
                    el("input")
                        .class("mj-input")
                        .attr("type", "number")
                        .attr("min", "0")
                        .attr("name", "wave")
                        .attr_if("value", asked_wave.clone()),
                ),
        )
        .child(
            el("button")
                .class("mj-button")
                .attr("type", "submit")
                .text("Filter"),
        )
        .when(narrowed, |f| f.child(link("/cockpit/plan", "Every issue")));

    let issues = card_with(
        "Issues",
        link("/cockpit/capabilities/plan.issues", "plan.issues"),
        if list.issues.is_empty() {
            nothing(if narrowed {
                "No issue matches this filter."
            } else {
                "The plan declares no issue."
            })
        } else {
            table(
                &[
                    "Status",
                    "Issue",
                    "Title",
                    "Milestone",
                    "Wave",
                    "Priority",
                    "Waiting on",
                    "Evidence",
                ],
                list.issues
                    .iter()
                    .map(|i| {
                        row(vec![
                            cell(word_badge(&i.status)),
                            id_cell(issue_href(&i.id), i.id.clone()),
                            text_cell(i.title.clone()),
                            cell(milestone_refs(std::slice::from_ref(&i.milestone))),
                            text_cell(i.wave.to_string()),
                            text_cell(i.priority.clone()),
                            cell(refs(&i.blocked_by)),
                            text_cell(format!("{} of {}", i.evidence_have, i.evidence_need)),
                        ])
                    })
                    .collect(),
            )
        },
    );

    Page::new(
        Area::Plan,
        "Plan",
        el("div")
            .class("mj-grid")
            .child(statistics)
            .child(next_card)
            .child(milestones)
            .child(card("Narrow", filters))
            .child(issues),
    )
    .subtitle("The milestones and issues of this repository's plan, as the plan derives them.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Plan", None)])
}

// ------------------------------------------------------------------- one milestone

/// A page about something the plan does not declare: a 404 that says so and where to look.
fn undeclared(kind: &str, id: &str) -> Page {
    Page::new(
        Area::Plan,
        id.to_string(),
        el("div")
            .child(alert(
                "warn",
                format!("The plan declares no {kind} `{id}`."),
            ))
            .child(el("p").child(link(
                "/cockpit/plan",
                "Every milestone and issue of the plan",
            ))),
    )
    .status(404)
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Plan", Some("/cockpit/plan")),
        (id, None),
    ])
}

/// One milestone as a graph of work, as `devtask.milestone` answers it.
pub fn milestone(ctx: &Context, id: &str) -> Page {
    let graph: MilestoneGraph = match ask(ctx, "devtask.milestone", json!({ "milestone": id })) {
        Ok(g) => g,
        Err(e) => return failed(Area::Plan, id, e),
    };
    if !graph.declared {
        return undeclared("milestone", id);
    }

    let statistics = el("div")
        .class("mj-stats")
        .child(statistic(
            graph.counts.total.to_string(),
            "issues",
            "devtask.milestone",
        ))
        .child(statistic(
            graph.ready.len().to_string(),
            "ready",
            "devtask.milestone",
        ))
        .child(statistic(
            graph.blocked.len().to_string(),
            "blocked",
            "devtask.milestone",
        ))
        .child(statistic(
            graph.active.len().to_string(),
            "active",
            "devtask.milestone",
        ))
        .child(statistic(
            graph.complete.len().to_string(),
            "complete",
            "devtask.milestone",
        ));

    let about = card_with(
        "The milestone",
        link(
            "/cockpit/capabilities/devtask.milestone",
            "devtask.milestone",
        ),
        facts(vec![
            ("Title", attested_text(&graph.title)),
            ("Outcome", attested_text(&graph.outcome)),
            (
                "Status",
                Node::Element(match &graph.status.value {
                    Some(status) => word_badge(status),
                    None => unknown(graph.status.reason.as_deref()),
                }),
            ),
            ("Rank", attested_count(&graph.rank)),
            (
                "Depends on",
                attested_list(&graph.depends_on, |m| milestone_refs(&[m.to_string()])),
            ),
            (
                "Waiting on",
                attested_list(&graph.blocked_by, |m| milestone_refs(&[m.to_string()])),
            ),
            (
                "Holds back",
                attested_list(&graph.dependents, |m| milestone_refs(&[m.to_string()])),
            ),
            (
                "Record",
                Node::Element(match &graph.record {
                    Some(path) => mono(path.clone()),
                    None => unknown(None),
                }),
            ),
            (
                "Its issues",
                Node::Element(link(
                    format!("/cockpit/plan?milestone={}", percent_encode(id)),
                    "Every issue of this milestone",
                )),
            ),
        ]),
    );

    let partitions = card(
        "Where the work is",
        facts(vec![
            ("Ready", Node::Element(refs(&graph.ready))),
            ("Blocked", Node::Element(refs(&graph.blocked))),
            ("Waiting", Node::Element(refs(&graph.waiting))),
            ("Active", Node::Element(refs(&graph.active))),
            ("In review", Node::Element(refs(&graph.review))),
            (
                "Completion blocked",
                Node::Element(refs(&graph.completion_blocked)),
            ),
            ("Complete", Node::Element(refs(&graph.complete))),
            ("Cancelled", Node::Element(refs(&graph.cancelled))),
        ]),
    );

    let blockers = card(
        "What holds the most back",
        if graph.critical_blockers.is_empty() {
            nothing("No unfinished issue holds another back.")
        } else {
            table(
                &["Issue", "Readiness", "Holds back", "Weight"],
                graph
                    .critical_blockers
                    .iter()
                    .map(|b| {
                        row(vec![
                            id_cell(issue_href(&b.issue), b.issue.clone()),
                            cell(word_badge(&word(&b.readiness))),
                            cell(refs(&b.blocks)),
                            text_cell(b.weight.to_string()),
                        ])
                    })
                    .collect(),
            )
        },
    );

    let parallel = card(
        "What may run at the same time",
        if graph.parallelizable.is_empty() {
            nothing("Nothing here can be started now.")
        } else {
            table(
                &["Wave", "Together", "Serialised by scope"],
                graph
                    .parallelizable
                    .iter()
                    .map(|set| {
                        row(vec![
                            text_cell(set.wave.to_string()),
                            cell(refs(&set.issues)),
                            cell(if set.serialised.is_empty() {
                                el("span").class("mj-muted").text("—")
                            } else {
                                el("div").children(
                                    set.serialised
                                        .iter()
                                        .map(|c| {
                                            el("p").class("mj-note").text(format!(
                                                "{} before {}: both touch {}",
                                                c.held, c.excluded, c.path
                                            ))
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            }),
                        ])
                    })
                    .collect(),
            )
        },
    );

    let nodes = card(
        "Every issue",
        if graph.nodes.is_empty() {
            nothing("This milestone has no issue.")
        } else {
            table(
                &[
                    "Readiness",
                    "Status",
                    "Issue",
                    "Title",
                    "Wave",
                    "Priority",
                    "Waiting on",
                    "Holds back",
                ],
                graph
                    .nodes
                    .iter()
                    .map(|n| {
                        row(vec![
                            cell(word_badge(&word(&n.readiness))),
                            cell(word_badge(&n.canonical_status)),
                            id_cell(issue_href(&n.issue), n.issue.clone()),
                            text_cell(n.title.clone()),
                            text_cell(
                                n.wave
                                    .map(|w| w.to_string())
                                    .unwrap_or_else(|| "—".to_string()),
                            ),
                            text_cell(n.priority.clone()),
                            cell(refs(&n.blocked_by)),
                            text_cell(n.transitive_dependents.to_string()),
                        ])
                    })
                    .collect(),
            )
        },
    );

    let mut body = el("div").class("mj-grid").child(statistics).child(about);
    if !graph.cycles.is_empty() {
        body = body.child(alert(
            "fail",
            format!(
                "The dependencies go round in a circle: {}. Nothing on a cycle can ever start.",
                graph
                    .cycles
                    .iter()
                    .map(|c| c.join(" → "))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        ));
    }
    body = body
        .child(partitions)
        .child(blockers)
        .child(parallel)
        .child(nodes)
        .child(diagnostics_card(&graph.diagnostics));

    let title = graph.title.value.clone().unwrap_or_else(|| id.to_string());
    Page::new(Area::Plan, id.to_string(), body)
        .subtitle(title)
        .trail(vec![
            ("Cockpit", Some("/cockpit")),
            ("Plan", Some("/cockpit/plan")),
            (id, None),
        ])
}

// ------------------------------------------------------------------- one issue

/// The moves an issue can make, each as a button the script enables and as the command that
/// makes the same move from a terminal. Which moves exist comes from the type the capability
/// takes; whether one is allowed is the capability's answer when it is asked, never this
/// page's guess. The one hint is `startable`, which `devtask` answered.
fn moves(ctx: &Context, issue: &str, startable: bool) -> El {
    let Some(capability) = ctx.registry.get("plan.transition") else {
        return card(
            "Moves",
            nothing("This executable has no `plan.transition`, so no move can be made from here."),
        );
    };
    let Some(http) = capability.exposure.http.as_ref() else {
        return card(
            "Moves",
            nothing("`plan.transition` has no HTTP exposure, so the Cockpit cannot call it. The command line still can."),
        );
    };
    let words: Vec<String> = [Transition::Start, Transition::Verify, Transition::Done]
        .iter()
        .map(word)
        .collect();

    let buttons: Vec<El> = words
        .iter()
        .map(|w| {
            el("button")
                .class(if w == "start" && startable {
                    "mj-button mj-button--primary"
                } else {
                    "mj-button"
                })
                .attr("type", "button")
                .attr("data-mj-move", w.clone())
                .attr(
                    "title",
                    format!("Ask plan.transition to {w} {issue}; it refuses a move the plan does not allow"),
                )
                // enabled by plan.js: a page whose script did not load offers nothing it
                // cannot do
                .flag("disabled")
                .text(w.clone())
        })
        .collect();

    card_with(
        "Moves",
        link("/cockpit/capabilities/plan.transition", "plan.transition"),
        el("form")
            .class("mj-runner")
            .attr("data-mj-transition", issue)
            .attr("data-mj-path", http.path.clone())
            .attr("data-mj-effect", word(&capability.execution.effect))
            .attr("novalidate", "")
            .child(alert(
                "warn",
                "A move writes the repository: it stamps one field of this issue's record and appends one event to the ledger, and a commit will carry both.",
            ))
            .child(el("div").class("mj-runner-actions").children(buttons))
            .child(
                el("div")
                    .class("mj-runner-result")
                    .attr("data-mj-result", "")
                    .attr("aria-live", "polite"),
            )
            .child(details(
                "The same moves from a terminal",
                el("div").children(
                    words
                        .iter()
                        .map(|w| el("p").child(mono(format!("majordomus plan {w} {issue}"))))
                        .collect::<Vec<_>>(),
                ),
            )),
    )
}

/// One issue as a task, as `devtask.issue` answers it, with the moves it can make.
pub fn issue(ctx: &Context, id: &str) -> Page {
    let task: DevTask = match ask(ctx, "devtask.issue", json!({ "issue": id })) {
        Ok(t) => t,
        Err(e) => return failed(Area::Plan, id, e),
    };
    if !task.declared {
        return undeclared("issue", id);
    }
    let d = &task.declaration;
    let p = &task.position;
    let r = &task.readiness;
    let x = &task.execution;
    let s = &task.synchronisation;

    let standing = card_with(
        "Where it stands",
        link("/cockpit/capabilities/devtask.issue", "devtask.issue"),
        el("div")
            .child(facts(vec![
                (
                    "Readiness",
                    Node::Element(
                        el("span")
                            .child(word_badge(&word(&r.state)))
                            .text(format!(" {}", r.reason)),
                    ),
                ),
                ("Status", Node::Element(word_badge(&r.canonical_status))),
                (
                    "Can start",
                    Node::Element(if r.startable {
                        tag("startable")
                    } else {
                        el("span").class("mj-muted").text("not now")
                    }),
                ),
                ("Milestone", attested_list_one(&d.milestone)),
                ("Milestone status", attested_text(&p.milestone_status)),
                ("Wave", attested_count(&p.wave)),
                ("Waiting on", attested_list(&p.blocked_by, plan_ref)),
                ("Holds back", attested_list(&p.dependents, plan_ref)),
                ("Behind it in all", attested_count(&p.transitive_dependents)),
                (
                    "Evidence",
                    Node::Element(
                        el("span")
                            .child(count_or_unknown(&p.evidence_have))
                            .text(" of ")
                            .child(count_or_unknown(&p.evidence_need)),
                    ),
                ),
            ]))
            .when(!r.blockers.is_empty(), |e| {
                e.child(table(
                    &["In the way", "Subject", "Detail"],
                    r.blockers
                        .iter()
                        .map(|b| {
                            row(vec![
                                cell(tag(word(&b.kind))),
                                cell(mono(b.subject.clone())),
                                text_cell(b.detail.clone()),
                            ])
                        })
                        .collect(),
                ))
            }),
    );

    let intent = card(
        "What it is",
        facts(vec![
            ("Objective", attested_text(&d.objective)),
            ("Why", attested_text(&d.why)),
            ("Now", attested_text(&d.current_state)),
            ("Wanted", attested_text(&d.desired_state)),
            ("Done when", attested_text(&d.completion)),
            ("Risk", attested_text(&d.risk)),
        ]),
    );

    let acceptance = card(
        "Acceptance",
        el("div")
            .child(attested_sentences(&d.acceptance_criteria))
            .child(facts(vec![
                (
                    "Validation",
                    attested_list(&d.validation, |v| mono(v.to_string())),
                ),
                (
                    "Evidence required",
                    attested_list(&d.evidence_required, |v| mono(v.to_string())),
                ),
                (
                    "Evidence present",
                    attested_list(&d.evidence_present, |v| mono(v.to_string())),
                ),
            ])),
    );

    let scope = card(
        "Scope",
        facts(vec![
            ("Touches", attested_list(&d.scope, |v| mono(v.to_string()))),
            ("Not", attested_list(&d.non_scope, |v| mono(v.to_string()))),
            ("Depends on", attested_list(&d.depends_on, plan_ref)),
            ("Priority", attested_text(&d.priority)),
            ("Profile", attested_text(&d.profile)),
            ("Parallel safe", attested_text(&d.parallel_safe)),
            ("Owner", attested_text(&d.owner)),
        ]),
    );

    let work = card(
        "The work so far",
        el("div")
            .when(!x.git_consulted, |e| {
                e.child(alert(
                    "info",
                    "git was not asked, so branches, commits and sessions are not known here.",
                ))
            })
            .child(facts(vec![
                (
                    "Branches",
                    attested_list(&x.branches, |v| mono(v.to_string())),
                ),
                (
                    "Merged",
                    attested_list(&x.merged_branches, |v| mono(v.to_string())),
                ),
                ("Commits", attested_count(&x.commits)),
                ("Trunk", attested_text(&x.trunk)),
                (
                    "Sessions",
                    attested_list(&x.sessions, |v| mono(v.to_string())),
                ),
                ("Created", attested_text(&d.created_at)),
                ("Updated", attested_text(&d.updated_at)),
                ("Started", attested_text(&d.started_at)),
                ("Verified", attested_text(&d.verified_at)),
                ("Completed", attested_text(&d.completed_at)),
            ])),
    );

    let synchronisation = card(
        "Synchronisation",
        facts(vec![
            ("Adapter", Node::Element(mono(s.adapter.clone()))),
            ("Repository", attested_text(&s.repository)),
            ("External issue", attested_text(&s.external_id)),
            ("State there", attested_text(&s.state)),
        ]),
    );

    let title = d.title.value.clone().unwrap_or_else(|| id.to_string());
    Page::new(
        Area::Plan,
        id.to_string(),
        el("div")
            .class("mj-grid")
            .child(standing)
            .child(moves(ctx, id, r.startable))
            .child(intent)
            .child(acceptance)
            .child(scope)
            .child(work)
            .child(synchronisation)
            .child(diagnostics_card(&task.diagnostics)),
    )
    .subtitle(title)
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Plan", Some("/cockpit/plan")),
        (id, None),
    ])
    .script("plan.js")
}

/// A milestone named by an issue's record, as a link when the record names one.
fn attested_list_one(value: &AttestedText) -> Node {
    match &value.value {
        Some(m) => Node::Element(milestone_refs(std::slice::from_ref(m))),
        None => Node::Element(unknown(value.reason.as_deref())),
    }
}

/// A count inline, or the word unknown.
fn count_or_unknown(value: &AttestedCount) -> El {
    match value.value {
        Some(n) => mono(n.to_string()),
        None => el("span").class("mj-note").text("unknown"),
    }
}

// ------------------------------------------------------------------- sessions

/// Every open episode of this repository's session store.
pub fn sessions(ctx: &Context) -> Page {
    let episodes: Episodes = match ask(ctx, "lifecycle.episodes", json!({})) {
        Ok(e) => e,
        Err(e) => return failed(Area::Sessions, "Sessions", e),
    };
    Page::new(
        Area::Sessions,
        "Sessions",
        el("div")
            .class("mj-grid")
            .child(el("div").class("mj-stats").child(statistic(
                episodes.episodes.len().to_string(),
                "open episodes",
                "lifecycle.episodes",
            )))
            .child(episodes_card(&episodes))
            .child(el("p").class("mj-note").child(link(
                "/cockpit/continuity",
                "What this checkout's own episode is holding",
            ))),
    )
    .subtitle("The episodes this repository's providers have opened and not yet closed.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Sessions", None)])
}

// ------------------------------------------------------------------- board

/// The declared status word a server standing wears, so that a badge on this page is
/// coloured by the design system's vocabulary rather than by a word this file invented.
/// The standing's own word is what the reader sees; only the colour is translated.
fn server_status(standing: ServerStanding) -> &'static str {
    match standing {
        ServerStanding::Ready => "ok",
        ServerStanding::Starting => "info",
        ServerStanding::Outdated => "warn",
        ServerStanding::Stale => "fail",
        // nothing serves that checkout, which is the ordinary state of a worktree nobody
        // is working in and not a fault of any kind
        ServerStanding::Absent => "neutral",
    }
}

/// How a peer is named wherever this page names one: the board's id and the client behind
/// it, because `p2` alone is not something a person recognises and `claude-code` alone is
/// not something a person can tell from the other three.
fn peer_label(peer: &Peer) -> String {
    format!("{} {}", peer.id, peer.client.name)
}

/// How long ago, in the coarsest unit that still says something. A board is read at a
/// glance, and the seconds of an hour-old session are noise on it.
fn ago(seconds: u64) -> String {
    match seconds {
        0 => "just now".to_string(),
        1..=90 => format!("{seconds}s ago"),
        91..=5_400 => format!("{}m ago", seconds / 60),
        5_401..=172_800 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

/// Which listed peer holds the near side of an overlap.
///
/// `peers.list` reports each colliding pair once and names only the far peer; the claims
/// on the near side arrive as paths under `yours` with nobody's name on them. The near
/// peer is resolved out of the same answer — it is the peer whose own announcement holds
/// every one of those claims — and never by comparing two paths, which is the server's
/// judgement and not this page's. A claim no listed announcement holds resolves to
/// `None`, and the collision is shown with one side unnamed rather than with a guess.
fn near_side<'a>(peers: &'a [Peer], overlap: &Overlap) -> Option<&'a Peer> {
    peers.iter().find(|p| {
        p.id != overlap.peer
            && p.announcement.as_ref().is_some_and(|a| {
                overlap
                    .paths
                    .iter()
                    .all(|path| a.scope.contains(&path.yours))
            })
    })
}

/// One collision, rendered so that a reader who scrolls past everything else still sees
/// it: both sessions named, both intents quoted, and every pair of claims that meet.
fn collision(peers: &[Peer], overlap: &Overlap) -> El {
    let near = near_side(peers, overlap);
    let near_name = near
        .map(peer_label)
        .unwrap_or_else(|| "a session no longer on the board".to_string());
    let far_name = peers
        .iter()
        .find(|p| p.id == overlap.peer)
        .map(peer_label)
        .unwrap_or_else(|| overlap.peer.to_string());
    let near_claims = format!("{near_name} claims");
    let far_claims = format!("{far_name} claims");
    let rows: Vec<El> = overlap
        .paths
        .iter()
        .map(|p| row(vec![cell(mono(&p.yours)), cell(mono(&p.theirs))]))
        .collect();

    card_with(
        format!("{near_name} and {far_name}"),
        if overlap.attached {
            badge("blocked", "both attached")
        } else {
            badge("warn", "one has left")
        },
        el("div")
            .child(alert(
                "fail",
                format!(
                    "{} claim(s) meet. Two sessions have said they will touch the same ground; \
                     whichever commits second finds out.",
                    overlap.paths.len()
                ),
            ))
            .when(near.is_some(), |d| {
                d.child(el("p").class("mj-prose").text(format!(
                    "{near_name}: {}",
                    near.and_then(|p| p.announcement.as_ref())
                        .map(|a| a.intent.as_str())
                        .unwrap_or_default()
                )))
            })
            .child(
                el("p")
                    .class("mj-prose")
                    .text(format!("{far_name}: {}", overlap.intent)),
            )
            .child(table(&[near_claims.as_str(), far_claims.as_str()], rows)),
    )
}

/// One peer's announcement as a cell: what it said it is doing, the ground it claimed, and
/// when it said so. A peer that has announced nothing says so, because a silent session is
/// the thing this board exists to make visible.
fn announcement_cell(peer: &Peer) -> El {
    match &peer.announcement {
        Some(a) => el("div")
            .child(el("p").class("mj-prose").text(&a.intent))
            .when(!a.scope.is_empty(), |d| {
                d.child(
                    el("div")
                        .class("mj-marks")
                        // `mono`, not `tag`: a claim is a path, and a path is the one thing
                        // on this page long enough to push a phone-width column sideways.
                        // `.mj-mono` breaks inside a word; `.mj-tag` does not.
                        .children(a.scope.iter().map(mono).collect::<Vec<_>>()),
                )
            })
            .child(el("p").class("mj-note").text(format!("announced {}", a.at))),
        None => nothing("said nothing"),
    }
}

/// The lease a reader is being shown, as facts: the address, the process, the version and
/// the moment it was taken, plus the executable it names when the lease carries one.
fn lease_facts(lease: Option<&LeaseView>) -> El {
    let Some(l) = lease else {
        return nothing("No lease: nothing has taken this checkout's server.");
    };
    facts(vec![
        (
            "Address",
            match &l.url {
                Some(url) => Node::Element(mono(url)),
                None => Node::Element(el("span").class("mj-note").text("not bound yet")),
            },
        ),
        ("Process", Node::Element(mono(l.pid.to_string()))),
        (
            "Version",
            match &l.version {
                Some(v) => Node::Element(mono(v)),
                None => Node::Element(
                    el("span")
                        .class("mj-note")
                        .text("written by a server too old to record one"),
                ),
            },
        ),
        ("Taken", Node::Element(mono(&l.started_at))),
        (
            "Executable",
            match &l.executable {
                Some(e) => Node::Element(mono(e.path.display().to_string())),
                None => Node::Element(el("span").class("mj-note").text("not recorded")),
            },
        ),
    ])
}

/// The board: every session attached to this checkout's shared server, what each announced
/// it is working on, where two of them have claimed the same ground, and where the server
/// of this checkout and of every other checkout of this repository stands.
///
/// Read through `peers.list` and `server.status` and derived from nothing else. Every
/// standing, address, pid, version, timestamp, intent, claim and overlap on the page is a
/// field one of those two capabilities already answers; this page reads no lease, probes no
/// port and compares no path. The one thing it works out for itself is which of the listed
/// peers holds the near side of an overlap, which it resolves by looking the claim up among
/// the announcements in the very same answer — see [`near_side`].
pub fn board(ctx: &Context) -> Page {
    let peers: PeerList = match ask(ctx, "peers.list", json!({})) {
        Ok(p) => p,
        Err(e) => return failed(Area::Board, "Board", e),
    };
    let status: ServerStatus = match ask(ctx, "server.status", json!({})) {
        Ok(s) => s,
        Err(e) => return failed(Area::Board, "Board", e),
    };

    let announced = peers
        .peers
        .iter()
        .filter(|p| p.announcement.is_some())
        .count();
    let ready = status
        .servers
        .iter()
        .filter(|s| s.standing == ServerStanding::Ready)
        .count();
    let statistics = el("div")
        .class("mj-stats")
        .child(statistic(
            peers.count.to_string(),
            "sessions attached",
            "peers.list",
        ))
        .child(statistic(
            announced.to_string(),
            "have announced",
            "peers.list",
        ))
        .child(statistic(
            peers.overlaps.len().to_string(),
            "collisions",
            "peers.list",
        ))
        .child(statistic(
            status.servers.len().to_string(),
            "checkouts of this repository",
            "server.status",
        ))
        .child(statistic(
            ready.to_string(),
            "servers ready",
            "server.status",
        ));

    // first, and never behind anything: a collision is the one thing on this page that
    // costs somebody a day if it is scrolled past
    let collisions: Vec<El> = if peers.overlaps.is_empty() {
        vec![card(
            "Collisions",
            nothing(
                "No two sessions on this board have claimed the same ground. Every announcement \
                 stands on its own paths.",
            ),
        )]
    } else {
        peers
            .overlaps
            .iter()
            .map(|o| collision(&peers.peers, o))
            .collect()
    };

    let peer_rows: Vec<El> = peers
        .peers
        .iter()
        .map(|p| {
            row(vec![
                cell(mono(p.id.as_str())),
                cell(if p.attached {
                    badge("connected", "attached")
                } else {
                    badge("disconnected", "gone")
                }),
                cell(
                    el("span")
                        .child(mono(&p.client.name))
                        .when(!p.client.version.is_empty(), |e| {
                            e.text(format!(" {}", p.client.version))
                        })
                        .when(p.client.title.is_some(), |e| {
                            e.child(el("br"))
                                .child(tag(p.client.title.clone().unwrap_or_default()))
                        }),
                ),
                cell(word_badge(&word(&p.transport))),
                cell(mono(&p.connected_at)),
                text_cell(ago(p.last_seen_seconds_ago)),
                cell(announcement_cell(p)),
            ])
        })
        .collect();
    let peers_card = card_with(
        "Sessions on this board",
        link("/cockpit/capabilities/peers.list", "peers.list"),
        if peer_rows.is_empty() {
            nothing(
                "Nothing is attached. The board is this process's own memory: a server that has \
                 just started has an empty one, and so has one every client has left.",
            )
        } else {
            table(
                &[
                    "Peer",
                    "Standing",
                    "Client",
                    "Transport",
                    "Attached at",
                    "Last seen",
                    "Working on",
                ],
                peer_rows,
            )
        },
    );

    let here = status.servers.iter().find(|s| s.this_checkout);
    let lease = status
        .this_process
        .as_ref()
        .or_else(|| here.and_then(|s| s.lease.as_ref()));
    let this_server = card_with(
        "This checkout's server",
        link("/cockpit/capabilities/server.status", "server.status"),
        el("div")
            .child(facts(vec![
                (
                    "Standing",
                    Node::Element(
                        el("span")
                            .child(badge(
                                server_status(status.standing),
                                status.standing.as_str(),
                            ))
                            .when(here.and_then(|s| s.reason.as_ref()).is_some(), |e| {
                                e.text(format!(
                                    " {}",
                                    here.and_then(|s| s.reason.as_deref()).unwrap_or_default()
                                ))
                            }),
                    ),
                ),
                (
                    "Checkout",
                    Node::Element(mono(
                        here.map(|s| s.worktree.display().to_string())
                            .unwrap_or_else(|| "(not enumerated)".to_string()),
                    )),
                ),
                (
                    "Branch",
                    Node::Element(mono(
                        here.and_then(|s| s.branch.clone())
                            .unwrap_or_else(|| "(detached)".to_string()),
                    )),
                ),
                ("Checkout id", Node::Element(mono(&status.checkout_id))),
                (
                    "Repository",
                    match &status.git {
                        Some(g) => Node::Element(mono(&g.id)),
                        None => Node::Element(
                            el("span").class("mj-note").text("git cannot be asked here"),
                        ),
                    },
                ),
                // the keys of `mj-facts` size a `max-content` grid column, so a long one
                // pushes its own value off a phone screen: what this key means is said in
                // the note under the card instead
                (
                    "Would serve",
                    Node::Element(mono(format!(
                        "{}:{} · {}",
                        status.desired.host, status.desired.port, status.desired.version
                    ))),
                ),
            ]))
            .child(
                el("h3")
                    .class("mj-card-title")
                    .text(if status.this_process.is_some() {
                        "The lease this process holds"
                    } else {
                        "The lease this checkout's file holds"
                    }),
            )
            .child(lease_facts(lease))
            .child(el("p").class("mj-note").text(
                "\"Would serve\" is what this executable would bind and answer as, and it is what \
                 the standing is measured against. The standing itself is decided by \
                 `server.status` from the lease above, from whether the server it names answers \
                 for this checkout, and from whether what answers is this executable's code. \
                 Nothing on this page reads a lease itself.",
            )),
    );

    let checkout_row = |s: &ServerView| {
        row(vec![
            cell(badge(server_status(s.standing), s.standing.as_str())),
            cell(
                el("span")
                    .child(mono(s.worktree.display().to_string()))
                    .when(s.primary, |e| e.text(" ").child(tag("primary")))
                    .when(s.this_checkout, |e| e.text(" ").child(tag("here"))),
            ),
            cell(mono(
                s.branch.clone().unwrap_or_else(|| "(detached)".to_string()),
            )),
            text_cell(
                s.peers
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ),
            cell(match s.lease.as_ref().and_then(|l| l.url.clone()) {
                Some(url) => mono(url),
                None => el("span").class("mj-note").text("—"),
            }),
            cell(match s.lease.as_ref().and_then(|l| l.version.clone()) {
                Some(v) => mono(v),
                None => el("span").class("mj-note").text("—"),
            }),
            cell(match s.lease.as_ref() {
                Some(l) => mono(&l.started_at),
                None => el("span").class("mj-note").text("—"),
            }),
            text_cell(s.reason.clone().unwrap_or_default()),
        ])
    };
    const CHECKOUT_COLUMNS: &[&str] = &[
        "Standing", "Checkout", "Branch", "Peers", "Address", "Version", "Started", "Why",
    ];
    // a checkout nothing serves is the ordinary state of a worktree nobody is working in,
    // and on a machine with a hundred and sixty of them it is also every row of the table.
    // The ones with a server are what a reader came for; the rest are complete, counted and
    // one click away, so nothing is dropped and nothing is scrolled past.
    let (served, unserved): (Vec<&ServerView>, Vec<&ServerView>) = status
        .servers
        .iter()
        .partition(|s| s.standing != ServerStanding::Absent);
    let checkouts = card_with(
        "Every checkout of this repository",
        link("/cockpit/capabilities/server.status", "server.status"),
        el("div")
            .child(if served.is_empty() {
                nothing("No checkout of this repository has a server. This process is answering without a lease.")
            } else {
                table(
                    CHECKOUT_COLUMNS,
                    served.iter().map(|s| checkout_row(s)).collect(),
                )
            })
            .when(!unserved.is_empty(), |d| {
                d.child(details(
                    format!("{} checkout(s) with no server", unserved.len()),
                    table(
                        CHECKOUT_COLUMNS,
                        unserved.iter().map(|s| checkout_row(s)).collect(),
                    ),
                ))
            }),
    );

    Page::new(
        Area::Board,
        "Board",
        el("div")
            .class("mj-grid")
            .child(statistics)
            .children(collisions)
            .child(peers_card)
            .child(this_server)
            .child(checkouts),
    )
    .subtitle(
        "Who else is working in this repository, what each of them announced, and where two \
         of them are about to collide.",
    )
    .trail(vec![("Cockpit", Some("/cockpit")), ("Board", None)])
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------- plan

    #[test]
    fn a_milestone_gate_in_a_list_of_issues_links_the_milestone_not_an_issue() {
        let rendered = refs(&["I0001".to_string(), "milestone:m1".to_string()]).render();
        assert!(
            rendered.contains("href=\"/cockpit/plan/issues/I0001\""),
            "{rendered}"
        );
        assert!(
            rendered.contains("href=\"/cockpit/plan/milestones/m1\""),
            "{rendered}"
        );
        assert!(
            !rendered.contains("/cockpit/plan/issues/milestone"),
            "{rendered}"
        );
        // nothing to wait on is a dash, not an empty cell
        assert!(refs(&[]).render().contains('—'));
    }

    #[test]
    fn a_value_devtask_could_not_answer_is_shown_as_unknown_with_its_reason() {
        let missing = AttestedText {
            value: None,
            provenance: FieldProvenance::Unknown,
            source: "I0001.yaml#owner".into(),
            reason: Some("the record declares no `owner`".into()),
        };
        let rendered = el("div").node(attested_text(&missing)).render();
        assert!(
            rendered.contains("unknown: the record declares no `owner`"),
            "{rendered}"
        );

        let declared = AttestedText {
            value: Some("korczis".into()),
            provenance: FieldProvenance::Explicit,
            source: "I0001.yaml#owner".into(),
            reason: None,
        };
        let rendered = el("div").node(attested_text(&declared)).render();
        assert!(rendered.contains(">korczis<"), "{rendered}");
        // where it came from travels with it
        assert!(
            rendered.contains("explicit · I0001.yaml#owner"),
            "{rendered}"
        );
    }

    #[test]
    fn an_empty_list_the_record_declared_differs_from_one_nobody_could_read() {
        let declared_empty = AttestedList {
            values: vec![],
            provenance: FieldProvenance::Explicit,
            source: "s".into(),
            reason: None,
        };
        let unreadable = AttestedList {
            values: vec![],
            provenance: FieldProvenance::Unknown,
            source: "s".into(),
            reason: Some("git was not asked".into()),
        };
        let a = el("div")
            .node(attested_list(&declared_empty, plan_ref))
            .render();
        let b = el("div")
            .node(attested_list(&unreadable, plan_ref))
            .render();
        assert!(a.contains(">none<"), "{a}");
        assert!(b.contains("unknown: git was not asked"), "{b}");
    }

    #[test]
    fn the_moves_offered_are_the_words_the_capability_takes() {
        let words: Vec<String> = [Transition::Start, Transition::Verify, Transition::Done]
            .iter()
            .map(word)
            .collect();
        assert_eq!(words, ["start", "verify", "done"]);
    }

    // ------------------------------------------------------------------- board

    /// A peer as the board lists one, for the tests below. Every field is the one the
    /// capability answers; nothing here is a shape this page invented.
    fn a_peer(id: &str, client: &str, attached: bool, scope: &[&str]) -> Peer {
        Peer {
            id: serde_json::from_value(json!(id)).expect("a peer id"),
            client: crate::peers::ClientInfo {
                name: client.to_string(),
                version: "1".into(),
                title: None,
            },
            transport: crate::peers::Transport::Http,
            connected_at: "2026-09-10T21:00:00Z".into(),
            last_seen_seconds_ago: 4,
            attached,
            claims: vec![],
            announcement: (!scope.is_empty()).then(|| crate::peers::Announcement {
                name: None,
                intent: format!("what {id} is doing"),
                scope: scope.iter().map(|s| s.to_string()).collect(),
                at: "2026-09-10T21:00:01Z".into(),
            }),
            checkout: None,
        }
    }

    #[test]
    fn a_standing_wears_a_status_word_the_design_declares() {
        for standing in [
            ServerStanding::Absent,
            ServerStanding::Starting,
            ServerStanding::Ready,
            ServerStanding::Outdated,
            ServerStanding::Stale,
        ] {
            let status = server_status(standing);
            assert!(
                crate::design::DesignSystem::compiled()
                    .expect("the compiled declaration")
                    .role_of_state(status)
                    .is_some(),
                "{standing:?} wears '{status}', which the design system does not file"
            );
        }
        // the reader still sees the standing's own word, not the colour's
        assert_eq!(ServerStanding::Outdated.as_str(), "outdated");
    }

    #[test]
    fn a_board_glance_reads_in_the_coarsest_unit_that_still_says_something() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(4), "4s ago");
        assert_eq!(ago(600), "10m ago");
        assert_eq!(ago(7_200), "2h ago");
        assert_eq!(ago(864_000), "10d ago");
    }

    #[test]
    fn the_near_side_of_a_collision_is_resolved_out_of_the_same_answer() {
        let peers = vec![
            a_peer("p1", "codex", true, &["apps/majordomus-cli"]),
            a_peer("p2", "claude-code", true, &["docs"]),
            a_peer(
                "p3",
                "claude-code",
                true,
                &["apps/majordomus-cli/src/cockpit"],
            ),
        ];
        let overlap = Overlap {
            peer: serde_json::from_value(json!("p1")).expect("a peer id"),
            attached: true,
            intent: "what p1 is doing".into(),
            checkout: None,
            paths: vec![crate::peers::OverlapPath {
                yours: "apps/majordomus-cli/src/cockpit".into(),
                theirs: "apps/majordomus-cli".into(),
            }],
        };
        let near = near_side(&peers, &overlap).expect("the peer whose claim it is");
        assert_eq!(near.id.as_str(), "p3");
        assert_eq!(peer_label(near), "p3 claude-code");

        // a claim nobody on the board announced names nobody, rather than the first peer
        let orphan = Overlap {
            paths: vec![crate::peers::OverlapPath {
                yours: "site".into(),
                theirs: "apps/majordomus-cli".into(),
            }],
            ..overlap
        };
        assert!(near_side(&peers, &orphan).is_none());
    }

    #[test]
    fn a_collision_names_both_sessions_and_every_claim_that_meets() {
        let peers = vec![
            a_peer("p1", "codex", true, &["test"]),
            a_peer("p2", "claude-code", true, &["test/cases/125.sh"]),
        ];
        let overlap = Overlap {
            peer: serde_json::from_value(json!("p1")).expect("a peer id"),
            attached: true,
            intent: "what p1 is doing".into(),
            checkout: None,
            paths: vec![crate::peers::OverlapPath {
                yours: "test/cases/125.sh".into(),
                theirs: "test".into(),
            }],
        };
        let rendered = collision(&peers, &overlap).render();
        assert!(
            rendered.contains("p2 claude-code and p1 codex"),
            "{rendered}"
        );
        assert!(rendered.contains("what p1 is doing"), "{rendered}");
        assert!(rendered.contains("what p2 is doing"), "{rendered}");
        assert!(rendered.contains("test/cases/125.sh"), "{rendered}");
        // a collision is an alert and not a table row
        assert!(rendered.contains("mj-alert--fail"), "{rendered}");
        assert!(rendered.contains("mj-badge--blocked"), "{rendered}");
    }

    #[test]
    fn a_session_that_announced_nothing_says_so_and_one_that_did_shows_its_claims() {
        let silent = announcement_cell(&a_peer("p9", "gemini-cli", true, &[])).render();
        assert!(silent.contains("said nothing"), "{silent}");

        let spoken = announcement_cell(&a_peer("p8", "codex", true, &["lib", "docs"])).render();
        assert!(spoken.contains("what p8 is doing"), "{spoken}");
        assert!(
            spoken.contains(">lib<") && spoken.contains(">docs<"),
            "{spoken}"
        );
        assert!(
            spoken.contains("announced 2026-09-10T21:00:01Z"),
            "{spoken}"
        );
    }

    #[test]
    fn a_checkout_with_no_lease_says_so_rather_than_rendering_an_empty_fact_list() {
        assert!(lease_facts(None).render().contains("No lease"));
        let held = lease_facts(Some(&LeaseView {
            pid: 4321,
            url: Some("http://127.0.0.1:8741".into()),
            started_at: "2026-09-10T20:00:00Z".into(),
            executable: None,
            version: Some("0.5.0".into()),
        }))
        .render();
        assert!(held.contains("http://127.0.0.1:8741"), "{held}");
        assert!(held.contains("4321"), "{held}");
        assert!(held.contains("0.5.0"), "{held}");
        assert!(held.contains("not recorded"), "{held}");
    }
}
