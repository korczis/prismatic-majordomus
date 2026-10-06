//! The plan area: the plan, one milestone and one issue.
//!
//! Each page renders what a capability answered and decides nothing of its own. A status
//! here is a word `plan.*` answered and a readiness is a word `devtask.*` derived; the one
//! thing a page chooses is the colour a word wears, and that is the design system's
//! vocabulary through [`word_badge`], not a rule about the plan.
//!
//! The one write reachable from here is `plan.transition`, offered on an issue page and run
//! as an execution. The page does not judge whether a move is legal: the capability refuses
//! an illegal one and names what is in the way, and `plan.js` shows that answer as it came.
//!
//! Rebuilt on the current page model from PR #552, whose board and sessions pages are
//! superseded by `/cockpit/peers` and `/cockpit/continuity`.

use serde_json::json;

use crate::capability::builtin::plan::{PlanIssueList, PlanNextIssue, PlanRoadmap};
use crate::capability::Context;
use crate::devtask::{
    AttestedCount, AttestedList, AttestedText, DevTask, DevTaskDiagnostic, FieldProvenance,
    MilestoneGraph,
};
use crate::http::router::percent_encode;
use crate::plan::Transition;

use super::html::{el, text, El, Node};
use super::nav::Area;
use super::pages::{ask, failed, param, select, word, Page};
use super::view::{
    alert, card, card_with, cell, details, facts, id_cell, link, mono, nothing, row, statistic,
    table, tag, text_cell, word_badge,
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
///
/// A move is started as an execution of `plan.transition` — the same `executions.start` a
/// capability page's "Run as an execution" uses — so it has an identity, a page of its own
/// and events, and the issue page reads the record back once the execution has finished.
/// Both routes come from the registry that declares them; this page names no path.
fn moves(ctx: &Context, issue: &str, startable: bool) -> El {
    // every fact about a capability is asked of `capabilities.describe`, the way every page
    // learns one, so this page reads no registry of its own (ADR 0089)
    let describe =
        |id: &str| ask::<serde_json::Value>(ctx, "capabilities.describe", json!({ "id": id })).ok();
    let http_path = |id: &str| {
        describe(id).and_then(|c| {
            c.pointer("/exposure/http/path")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
        })
    };
    let Some(capability) = describe("plan.transition") else {
        return card(
            "Moves",
            nothing("This executable has no `plan.transition`, so no move can be made from here."),
        );
    };
    let capability_id = capability["id"]
        .as_str()
        .unwrap_or("plan.transition")
        .to_string();
    let effect = capability
        .pointer("/execution/effect")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("repository_mutation")
        .to_string();
    let (Some(start), Some(follow)) = (http_path("executions.start"), http_path("executions.get"))
    else {
        return card(
            "Moves",
            nothing("This executable starts no execution over HTTP, so the Cockpit cannot make a move. The command line still can."),
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
                    format!("Run plan.transition to {w} {issue} as an execution; it refuses a move the plan does not allow"),
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
            .attr("data-mj-capability", capability_id)
            .attr("data-mj-start", start)
            .attr("data-mj-follow", follow)
            .attr("data-mj-effect", effect)
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
    issue_page(ctx, id, &task)
}

/// The issue page over what `devtask.issue` answered. Separate from [`issue`] so that an
/// answer the page does not ask for itself — `git: false`, where `git_consulted` is false —
/// is rendered by the same code a reader reaches.
fn issue_page(ctx: &Context, id: &str, task: &DevTask) -> Page {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin;
    use crate::capability::CapabilityRegistry;
    use crate::synthetic::SyntheticRepository;
    use std::sync::Arc;

    // ------------------------------------------------------------- the values

    fn html(node: Node) -> String {
        el("div").node(node).render()
    }

    /// A gate in a list of issue ids is spelled `milestone:<id>` by `plan.issues`, and it
    /// links the milestone; an empty list is a dash, never an empty cell.
    #[test]
    fn a_reference_links_what_it_names_and_an_empty_list_is_a_dash() {
        assert!(plan_ref("milestone:m-first")
            .render()
            .contains("href=\"/cockpit/plan/milestones/m-first\""));
        assert!(plan_ref("I0001")
            .render()
            .contains("href=\"/cockpit/plan/issues/I0001\""));
        assert!(refs(&[]).render().contains('—'));
        assert!(milestone_refs(&[]).render().contains('—'));
        assert!(milestone_refs(&["m-first".to_string()])
            .render()
            .contains("href=\"/cockpit/plan/milestones/m-first\""));
    }

    /// A value `devtask` could not answer says so and why, and a value it answered carries
    /// where it came from; an authored empty list is "none", an unread one is unknown.
    #[test]
    fn an_unknown_value_is_the_word_unknown_with_its_reason() {
        assert!(unknown(None).render().contains(">unknown<"));
        assert!(unknown(Some("no key")).render().contains("unknown: no key"));

        let text = AttestedText::explicit("Ship it", "issues/I0001.yaml#title");
        assert!(html(attested_text(&text)).contains("explicit · issues/I0001.yaml#title"));
        let text = AttestedText::unknown("github", "not read here");
        assert!(html(attested_text(&text)).contains("unknown: not read here"));

        let count = AttestedCount::derived(3, "plan.issues");
        assert!(html(attested_count(&count)).contains(">3<"));
        let count = AttestedCount::unknown("git", "git was not asked");
        assert!(html(attested_count(&count)).contains("unknown: git was not asked"));
        assert!(count_or_unknown(&count).render().contains(">unknown<"));
        assert!(count_or_unknown(&AttestedCount::explicit(2, "x"))
            .render()
            .contains(">2<"));

        let mono_of = |v: &str| mono(v.to_string());
        let none = AttestedList::explicit(vec![], "issues/I0001.yaml#scope");
        assert!(html(attested_list(&none, mono_of)).contains(">none<"));
        let unread = AttestedList::unknown("git", "git was not asked");
        assert!(html(attested_list(&unread, mono_of)).contains("unknown: git was not asked"));
        let some = AttestedList::derived(vec!["lib".into(), "docs".into()], "scope");
        let shown = html(attested_list(&some, mono_of));
        assert!(
            shown.contains(">lib<") && shown.contains(">docs<"),
            "{shown}"
        );
        assert!(shown.contains("derived · scope"), "{shown}");

        assert!(attested_sentences(&unread)
            .render()
            .contains("unknown: git was not asked"));
        assert!(attested_sentences(&none)
            .render()
            .contains("The record declares none."));
        let criteria = AttestedList::explicit(vec!["It works".into()], "acceptance");
        assert!(attested_sentences(&criteria)
            .render()
            .contains("mj-checklist-item"));

        let milestone = AttestedText::explicit("m-first", "issues/I0001.yaml#milestone");
        assert!(html(attested_list_one(&milestone)).contains("/cockpit/plan/milestones/m-first"));
        let milestone = AttestedText::unknown("issues/I0001.yaml", "no milestone key");
        assert!(html(attested_list_one(&milestone)).contains("unknown: no milestone key"));
    }

    /// Every finding is a row with the command that reproduces it.
    #[test]
    fn a_finding_is_a_row_with_its_reproduction() {
        let shown = diagnostics_card(&[DevTaskDiagnostic {
            level: "FAIL".into(),
            code: "issue.cycle".into(),
            message: "I0010 depends on itself".into(),
            reproduce: "majordomus plan validate".into(),
        }])
        .render();
        for part in [
            "FAIL",
            "issue.cycle",
            "I0010 depends on itself",
            "majordomus plan validate",
        ] {
            assert!(shown.contains(part), "{part}: {shown}");
        }
        assert!(diagnostics_card(&[])
            .render()
            .contains("Nothing is wrong with these records."));
    }

    // ------------------------------------------------------------- the pages

    const SOURCES: &str = "
  - id: milestone
    kind: milestone
    discovery: vcs
    pathspec: ':(glob).ai/repo/project/milestones/*.yaml'
    required: false
  - id: issue
    kind: issue
    discovery: vcs
    pathspec: ':(glob).ai/repo/project/issues/*.yaml'
    required: false
";

    fn milestone_record(id: &str, order: u32, depends_on: &[&str]) -> String {
        format!(
            "id: {id}\ntitle: The milestone {id}\nslug: {id}\norder: {order}\npriority: p1\n\
             problem: \"A problem.\"\noutcome: \"The outcome of {id}.\"\n\
             depends_on: [{}]\nacceptance_criteria:\n  - It is reached\n\
             validation:\n  - \"true\"\nevidence_required:\n  - proof\n",
            depends_on.join(", ")
        )
    }

    fn issue_record(id: &str, milestone: &str, scope: &str, depends_on: &[&str]) -> String {
        format!(
            "id: {id}\nmilestone: {milestone}\ntitle: The work of {id}\nslug: work-{id}\n\
             priority: p1\nprofile: implementation\nobjective: \"Do {id}.\"\n\
             scope:\n  - {scope}\ndepends_on: [{}]\nacceptance_criteria:\n  - {id} is done\n\
             validation:\n  - \"true\"\nevidence_required:\n  - proof\n",
            depends_on.join(", ")
        )
    }

    /// A repository with a plan: `m-first` holds a ready issue, one waiting on it and one
    /// that shares its scope; `m-second` waits on `m-first` and holds nothing; `m-loop`
    /// holds two issues that wait on each other.
    fn planned() -> SyntheticRepository {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        // `devtask.issue` traces an issue through git, and refuses a directory that is not
        // a git work tree; the index itself is read from the filesystem
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.root())
            .args(["init", "-q"])
            .status()
            .expect("git")
            .success());
        let write = |rel: &str, body: &str| {
            let path = repo.root().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).unwrap();
        text.push_str(SOURCES);
        std::fs::write(&sources, text).unwrap();
        write(
            ".ai/repo/project/project.yaml",
            "schema_version: 1\nname: Synthetic\nrepository: example/synthetic\ndefault_branch: master\n",
        );
        for (id, order, deps) in [
            ("m-first", 0, vec![]),
            ("m-second", 1, vec!["m-first"]),
            ("m-loop", 2, vec![]),
        ] {
            write(
                &format!(".ai/repo/project/milestones/{id}.yaml"),
                &milestone_record(id, order, &deps),
            );
        }
        for (id, milestone, scope, deps) in [
            ("I0001", "m-first", "lib", vec![]),
            ("I0002", "m-first", "docs", vec!["I0001"]),
            ("I0003", "m-first", "lib", vec![]),
            ("I0010", "m-loop", "lib", vec!["I0011"]),
            ("I0011", "m-loop", "lib", vec!["I0010"]),
        ] {
            write(
                &format!(".ai/repo/project/issues/{id}.yaml"),
                &issue_record(id, milestone, scope, &deps),
            );
        }
        repo
    }

    /// A context over `repo` whose executable lacks the capabilities in `without`: the
    /// shape of an executable that does not carry them, which is what each page's refusal
    /// arm answers.
    fn context(repo: &SyntheticRepository, without: &[&str]) -> Context {
        let index = repo.index().expect("the synthetic repository indexes");
        let modules = builtin::modules()
            .into_iter()
            .map(|mut m| {
                m.capabilities
                    .retain(|e| !without.contains(&e.capability.id.as_str()));
                m
            })
            .collect();
        let registry = CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("the registry builds");
        Context::new(Arc::new(index), Arc::new(registry))
    }

    fn query(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Each capability a page reads is one it may not find, and the page then says which
    /// one did not answer, with a 500 — never a page rendered from half an answer.
    #[test]
    fn a_page_whose_capability_does_not_answer_says_so() {
        let repo = planned();
        for missing in ["plan.roadmap", "plan.next", "plan.issues"] {
            let page = plan(&context(&repo, &[missing]), &[]);
            assert_eq!(page.status, 500, "without {missing}");
            assert!(
                page.main.render().contains("did not answer"),
                "without {missing}: {}",
                page.main.render()
            );
        }
        let page = milestone(&context(&repo, &["devtask.milestone"]), "m-first");
        assert_eq!(page.status, 500);
        let page = issue(&context(&repo, &["devtask.issue"]), "I0001");
        assert_eq!(page.status, 500);
    }

    /// The plan page over a plan: the roadmap's `now` and `next`, a milestone waiting on
    /// another, an issue waiting on another, and a filter that narrows.
    #[test]
    fn the_plan_page_shows_the_roadmap_and_what_waits_on_what() {
        let repo = planned();
        let ctx = context(&repo, &[]);
        let page = plan(&ctx, &[]);
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains(">now<"), "{body}");
        assert!(body.contains(">next<"), "{body}");
        assert!(body.contains("The milestone m-second"), "{body}");
        assert!(
            body.contains("href=\"/cockpit/plan/issues/I0001\""),
            "{body}"
        );
        assert!(!body.contains("Every issue<"), "unfiltered: {body}");

        let page = plan(&ctx, &query(&[("milestone", "m-first"), ("wave", "0")]));
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains("Every issue"), "{body}");
    }

    /// A plan with nothing in it: no milestone, no issue, and nothing to hand out.
    #[test]
    fn an_empty_plan_says_there_is_nothing_in_it() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let page = plan(&context(&repo, &[]), &[]);
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains("The plan declares no milestone."), "{body}");
        assert!(body.contains("The plan declares no issue."), "{body}");
        assert!(!body.contains("Every issue<"), "{body}");
    }

    /// A milestone page carries what holds the most back, what may run together and what
    /// is serialised by scope; a milestone with no issue says so; a cycle is an alert.
    #[test]
    fn the_milestone_page_shows_blockers_parallel_work_and_cycles() {
        let repo = planned();
        let ctx = context(&repo, &[]);

        let page = milestone(&ctx, "m-first");
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains("What holds the most back"), "{body}");
        assert!(
            !body.contains("No unfinished issue holds another back."),
            "{body}"
        );
        assert!(body.contains("both touch lib"), "{body}");

        let page = milestone(&ctx, "m-second");
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains("This milestone has no issue."), "{body}");
        assert!(body.contains("Nothing here can be started now."), "{body}");
        assert!(
            body.contains("href=\"/cockpit/plan/milestones/m-first\""),
            "m-second depends on m-first: {body}"
        );

        let page = milestone(&ctx, "m-loop");
        let body = page.main.render();
        assert!(body.contains("go round in a circle"), "{body}");

        let page = milestone(&ctx, "m-none");
        assert_eq!(page.status, 404);
    }

    /// An issue waiting on another names what is in the way; an undeclared issue is a 404.
    #[test]
    fn the_issue_page_names_what_is_in_the_way() {
        let repo = planned();
        let ctx = context(&repo, &[]);
        let page = issue(&ctx, "I0002");
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains("In the way"), "{body}");
        assert!(!body.contains("git was not asked"), "{body}");
        assert_eq!(page.scripts, vec!["plan.js"]);

        assert_eq!(issue(&ctx, "I9999").status, 404);
    }

    /// An answer given without git — `devtask.issue` asked with `git: false` — renders the
    /// page with the note that branches, commits and sessions are not known, rather than
    /// empty lists that would read as "nothing realised this issue".
    #[test]
    fn an_issue_answered_without_git_says_git_was_not_asked() {
        let repo = planned();
        let ctx = context(&repo, &[]);
        let task: DevTask = ask(
            &ctx,
            "devtask.issue",
            json!({ "issue": "I0001", "git": false }),
        )
        .expect("devtask.issue answers without git");
        assert!(!task.execution.git_consulted);
        let page = issue_page(&ctx, "I0001", &task);
        let body = page.main.render();
        assert_eq!(page.status, 200, "{body}");
        assert!(body.contains("git was not asked"), "{body}");
    }

    /// The moves are offered only when the executable carries `plan.transition` and can
    /// start and follow an execution over HTTP; otherwise the card says which is missing.
    #[test]
    fn the_moves_say_what_the_executable_lacks() {
        let repo = planned();
        let card = moves(&context(&repo, &["plan.transition"]), "I0001", true).render();
        assert!(card.contains("no `plan.transition`"), "{card}");
        assert!(!card.contains("data-mj-move"), "{card}");

        for missing in ["executions.start", "executions.get"] {
            let card = moves(&context(&repo, &[missing]), "I0001", true).render();
            assert!(
                card.contains("starts no execution over HTTP"),
                "without {missing}: {card}"
            );
        }

        let card = moves(&context(&repo, &[]), "I0001", true).render();
        assert!(card.contains("mj-button mj-button--primary"), "{card}");
        let card = moves(&context(&repo, &[]), "I0002", false).render();
        assert!(!card.contains("mj-button--primary"), "{card}");
    }
}
