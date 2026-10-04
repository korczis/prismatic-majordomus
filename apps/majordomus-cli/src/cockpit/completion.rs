//! The completion area: whether the active task may be called finished, and everything the
//! answer rests on.
//!
//! One page, a projection of two capabilities: `gates.completion` is the judgement — the same
//! execution `majordomus check`, the HTTP route and the MCP tool read — and `gates.policy` is
//! the stage list the distribution ships (`share/completion.yaml`). Nothing here decides a
//! stage, ticks a question or grades a gate; every status word on the page came out of one of
//! those two answers. Rebuilt on the current page model from PR #292 (ADR 0057).

use serde_json::json;

use crate::capability::Context;
use crate::gates::{Completion, DoneQuestion, StageReport};
use crate::http::router::percent_encode;

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, failed, word, Page};
use super::view::{
    alert, badge, card, card_with, cell, details, facts, id_cell, link, mono, nothing, row, table,
    tag, word_badge,
};

/// Where one question's `source` field points, rendered with the capability it names
/// linked to its own page.
///
/// A question states its source as a sentence — `obligation tests (obligations.closure)`,
/// `gate rust-check`, `release.analysis (ADR 0051)` — and somewhere in that sentence there
/// may be the id of a capability this Cockpit already has a page for. Nothing here is a
/// list of sources and no id is written in this file: the sentence is split into words and
/// each is asked of the registry, so a source the completion policy starts naming tomorrow
/// is a link tomorrow if its capability exists and plain text if it does not.
///
/// The registry is asked because the registry is what decides: a page derived from a table
/// of "these three sources have pages" would go stale the first time a fourth was added.
fn question_source_link(ctx: &Context, source: &str) -> El {
    let mut out = el("span").class("mj-identity");
    let mut pending = String::new();
    for word in source.split_inclusive([' ', '(', ')']) {
        let id = word.trim_matches(|c: char| c == ' ' || c == '(' || c == ')' || c == ',');
        if !id.is_empty() && ctx.registry.get(id).is_some() {
            if !pending.is_empty() {
                out = out.child(el("span").text(std::mem::take(&mut pending)));
            }
            let (before, after) = word.split_once(id).unwrap_or(("", ""));
            out = out
                .child(el("span").text(before))
                .child(link(
                    format!("/cockpit/capabilities/{}", percent_encode(id)),
                    id,
                ))
                .child(el("span").text(after));
        } else {
            pending.push_str(word);
        }
    }
    if !pending.is_empty() {
        out = out.child(el("span").text(pending));
    }
    out
}

/// One stage of the lifecycle with the questions the policy places under it.
fn completion_stage(ctx: &Context, stage: &StageReport, questions: &[DoneQuestion]) -> El {
    let mine: Vec<&DoneQuestion> = questions.iter().filter(|q| q.stage == stage.id).collect();
    el("section")
        .class("mj-card")
        .child(
            el("div")
                .class("mj-card-head")
                .child(el("h3").class("mj-card-title").text(&stage.title))
                .child(word_badge(stage.state.as_str())),
        )
        .when(mine.is_empty(), |s| {
            s.child(nothing(
                "The policy places no question of this stage over this change.",
            ))
        })
        .when(!mine.is_empty(), |s| {
            s.child(
                el("ul").class("mj-checklist").children(
                    mine.iter()
                        .map(|q| {
                            el("li")
                                .class("mj-checklist-item")
                                .child(word_badge(q.status.as_str()))
                                .child(el("span").class("mj-checklist-title").text(&q.question))
                                .child(
                                    el("span")
                                        .class("mj-checklist-detail")
                                        .text(&q.evidence)
                                        .child(el("br"))
                                        .child(question_source_link(ctx, &q.source))
                                        .child(el("br"))
                                        .child(mono(&q.remediation)),
                                )
                        })
                        .collect::<Vec<_>>(),
                ),
            )
        })
}

/// Where the active task stands against the completion policy: every stage of the
/// lifecycle in order, the questions under each one with what answered them, and the three
/// words — finishable, verified, complete — as the document states them.
///
/// Nothing on this page decides anything. `gates.completion` is the one judgement (the same
/// execution `majordomus check`, the HTTP route and the MCP tool read), `gates.policy` is
/// the stage list the distribution ships, and every status word rendered here came out of
/// one of those two documents. The page holds no vocabulary of its own: a stage the policy
/// adds, a question it moves, a status word the judgement starts using — each appears here
/// with no edit to this file.
///
/// It is rendered with `live: false`, so no request leaves this process while a page is
/// being laid out. The obligations something must be *reached* to settle — the push, the
/// trunk, the publication, the deployed revision — are therefore reported as owed rather
/// than proven, and the page says so and names what establishes them.
pub fn completion(ctx: &Context) -> Page {
    let c: Completion = match ask(ctx, "gates.completion", json!({ "live": false })) {
        Ok(c) => c,
        Err(e) => return failed(Area::Completion, "Completion", e),
    };

    let live_note = el("p").class("mj-note").text(
        "This page is rendered without reaching the network, so an obligation only a remote can settle — the push, the trunk, the publication, the deployed revision — is reported as owed here rather than as proven. `majordomus check` establishes them live; so does running the verification below.",
    );

    if !c.present {
        return Page::new(
            Area::Completion,
            "Completion",
            el("div").class("mj-grid").child(card(
                "No active task in this checkout",
                el("div")
                    .child(nothing(
                        "Completion is a property of a task, and no task record was found here. Work outside a task is permitted; nothing about it can be called finished, because there is nothing that promised anything. `majordomus start` opens one.",
                    ))
                    .child(mono(format!("policy: {}", c.policy)))
                    .child(live_note),
            )),
        )
        .subtitle("Whether the active task may be called finished, and everything the answer rests on.")
        .trail(vec![("Cockpit", Some("/cockpit")), ("Completion", None)]);
    }

    // The three words, as the document states them. Each is a different fact and the page
    // states all three rather than reducing them: finishable is "nothing refuses",
    // verified is "and nothing is silent", complete is "and every question is answered".
    let words = card_with(
        "Where this task stands",
        word_badge(c.stage.state.as_str()),
        el("div")
            .child(facts(vec![
                ("Stage", Node::Element(el("span").text(&c.stage.title))),
                (
                    "Finishable",
                    Node::Element(badge(
                        if c.finishable { "ok" } else { "fail" },
                        if c.finishable { "yes" } else { "no" },
                    )),
                ),
                (
                    "Verified",
                    Node::Element(badge(
                        if c.verified { "ok" } else { "warn" },
                        if c.verified { "yes" } else { "no" },
                    )),
                ),
                (
                    "Complete",
                    Node::Element(badge(
                        if c.complete { "ok" } else { "info" },
                        if c.complete { "yes" } else { "no" },
                    )),
                ),
                ("Policy", Node::Element(mono(&c.policy))),
                ("Gate model", Node::Element(mono(&c.model))),
            ]))
            .when(!c.stage.owing.is_empty(), |d| {
                d.child(el("p").class("mj-prose").text("Owed at this stage:"))
                    .child(
                        el("div").class("mj-marks").children(
                            c.stage
                                .owing
                                .iter()
                                .map(|q| mono(q.clone()))
                                .collect::<Vec<_>>(),
                        ),
                    )
            })
            .child(live_note),
    );

    let task = match &c.task {
        Some(t) => card(
            "The task this is about",
            el("div")
                .child(el("p").class("mj-prose").text(&t.task))
                .child(facts(vec![
                    ("Id", Node::Element(mono(t.id.clone()))),
                    ("Profile", Node::Element(el("span").text(&t.profile))),
                    (
                        "Issue",
                        Node::Element(if t.issue.is_empty() {
                            el("span").class("mj-muted").text("(never declared)")
                        } else {
                            mono(t.issue.clone())
                        }),
                    ),
                    (
                        "Started at",
                        Node::Element(mono(t.head.chars().take(12).collect::<String>())),
                    ),
                ]))
                .child(el("h3").class("mj-subheading").text("Scope"))
                .child(
                    el("div")
                        .class("mj-marks")
                        .children(t.scope.iter().map(|p| mono(p.clone())).collect::<Vec<_>>()),
                )
                .child(el("h3").class("mj-subheading").text("Declared obligations"))
                .child(
                    el("div").class("mj-marks").children(
                        t.requires
                            .iter()
                            .map(|r| mono(r.clone()))
                            .collect::<Vec<_>>(),
                    ),
                ),
        ),
        None => card("The task this is about", nothing("The report carries no task record, though it says one is present. Read `gates.completion` directly.")),
    };

    // The lifecycle. Every stage of the policy in order, with the questions under it —
    // the shape the whole page exists for.
    let lifecycle = card(
        "The lifecycle",
        el("div")
            .child(el("p").class("mj-prose").text(
                "Every stage the completion policy declares, in its order, with the questions it places under it. The task stands at the first stage that is blocked, else the first that is pending; nothing here is ticked by a person, because every answer was taken from the source the question names.",
            ))
            .children(
                c.stage
                    .stages
                    .iter()
                    .map(|s| completion_stage(ctx, s, &c.questions))
                    .collect::<Vec<_>>(),
            ),
    );

    let mut cards = vec![words, task, lifecycle];

    if let Some(v) = &c.version {
        cards.push(card_with(
            "Version impact",
            word_badge(&v.status),
            el("div")
                .child(facts(vec![
                    ("Baseline", Node::Element(mono(&v.baseline))),
                    ("Declared", Node::Element(mono(&v.declared))),
                    ("Required", Node::Element(mono(&v.required))),
                    ("Impact", Node::Element(word_badge(&v.impact))),
                    (
                        "Breaking",
                        Node::Element(badge(
                            if v.breaking { "fail" } else { "ok" },
                            if v.breaking { "yes" } else { "no" },
                        )),
                    ),
                    (
                        "Movements",
                        Node::Element(el("span").text(v.changes.to_string())),
                    ),
                ]))
                .child(
                    el("p")
                        .class("mj-note")
                        .text("Measured by the release analysis; this page shows what it said."),
                )
                .child(link("/cockpit/release", "The whole analysis")),
        ));
    }

    // The deployment plan: which surfaces the change reaches, and the address each states
    // its own identity at. Nothing is asked of any of them here.
    cards.push(card(
        "Deployment",
        el("div")
            .child(table(
                &[
                    "Target",
                    "Kind",
                    "Reaches it",
                    "Expects",
                    "Identity address",
                ],
                c.deployment
                    .targets
                    .iter()
                    .map(|t| {
                        row(vec![
                            cell(mono(t.id.clone())),
                            cell(word_badge(t.kind.as_str())),
                            cell(
                                el("span")
                                    .child(badge(
                                        if t.applicable { "ok" } else { "neutral" },
                                        if t.applicable { "yes" } else { "no" },
                                    ))
                                    .child(el("span").class("mj-note").text(&t.reason)),
                            ),
                            cell(expected_identity(&t.expected)),
                            cell(match &t.identity_url {
                                Some(u) => link(u.clone(), u.clone()).class("mj-link mj-identity"),
                                None => el("span").class("mj-muted").text("—"),
                            }),
                        ])
                    })
                    .collect(),
            ))
            .nodes(
                c.deployment
                    .findings
                    .iter()
                    .map(|f| Node::Element(alert("warn", f.clone())))
                    .collect::<Vec<_>>(),
            ),
    ));

    cards.push(card(
        "Gates",
        table(
            &["Gate", "Status", "Why", "What would settle it"],
            c.gates
                .iter()
                .map(|g| {
                    row(vec![
                        id_cell(
                            format!("/cockpit/capabilities/{}", percent_encode("gates.model")),
                            g.id.clone(),
                        ),
                        cell(
                            el("span")
                                .child(word_badge(g.status.as_str()))
                                .when(g.required, |s| s.child(tag("required"))),
                        ),
                        cell(el("span").class("mj-identity").text(&g.reason)),
                        cell(mono(&g.remediation)),
                    ])
                })
                .collect(),
        ),
    ));

    cards.push(card(
        "Obligations the change implies",
        table(
            &["Token", "Applies", "Declared", "Why", "What discharges it"],
            c.obligations
                .iter()
                .map(|o| {
                    row(vec![
                        cell(mono(o.id.clone())),
                        cell(badge(
                            if o.applicable { "ok" } else { "neutral" },
                            if o.applicable { "yes" } else { "no" },
                        )),
                        cell(badge(
                            if o.declared { "ok" } else { "warn" },
                            if o.declared { "yes" } else { "no" },
                        )),
                        cell(el("span").class("mj-identity").text(&o.reason)),
                        cell(mono(&o.remediation)),
                    ])
                })
                .collect(),
        ),
    ));

    // The plan the gates were judged against: the mode, the reason, and how many paths it
    // was computed over — measured from the array rather than restated.
    cards.push(card(
        "The plan",
        el("div")
            .child(facts(vec![
                ("Mode", Node::Element(word_badge(&word(&c.plan.mode)))),
                (
                    "Reason",
                    Node::Element(el("span").class("mj-identity").text(&c.plan.reason)),
                ),
                (
                    "Changed paths",
                    Node::Element(el("span").text(c.plan.changed.len().to_string())),
                ),
                (
                    "Gates selected",
                    Node::Element(el("span").text(c.plan.selected.len().to_string())),
                ),
                (
                    "Gates excluded",
                    Node::Element(el("span").text(c.plan.excluded.len().to_string())),
                ),
            ]))
            .child(details(
                format!("The {} changed path(s)", c.plan.changed.len()),
                el("ul").class("mj-list").children(
                    c.plan
                        .changed
                        .iter()
                        .map(|p| el("li").child(mono(p.clone())))
                        .collect::<Vec<_>>(),
                ),
            )),
    ));

    // The live half, offered and never called: both of these reach something this render
    // deliberately did not.
    cards.push(card(
        "Establish the live half",
        el("div")
            .child(el("p").class("mj-prose").text(
                "Two capabilities answer what this page could not. Each is offered through the runner, which is where a capability is run from; neither was called to draw this page.",
            ))
            .child(
                el("ul")
                    .class("mj-list")
                    .child(el("li").child(link(
                        format!("/cockpit/capabilities/{}", percent_encode("deploy.verify")),
                        "deploy.verify — ask every surface the change reaches what it is serving",
                    )))
                    .child(el("li").child(link(
                        format!("/cockpit/capabilities/{}", percent_encode("gates.completion")),
                        "gates.completion — the same report with live: true",
                    )))
                    .child(el("li").child(link(
                        format!("/cockpit/capabilities/{}", percent_encode("gates.policy")),
                        "gates.policy — the policy these stages and questions come from",
                    ))),
            )
            .child(mono("majordomus check")),
    ));

    if !c.findings.is_empty() {
        cards.push(card(
            "Before you trust the above",
            el("div").children(
                c.findings
                    .iter()
                    .map(|f| alert("warn", f.clone()))
                    .collect::<Vec<_>>(),
            ),
        ));
    }

    Page::new(
        Area::Completion,
        "Completion",
        el("div").class("mj-grid").children(cards),
    )
    .subtitle("Whether the active task may be called finished, and everything the answer rests on — the same judgement `majordomus check`, the HTTP route and the MCP tool read.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("Completion", None)])
}

/// What a deployment target is expected to be serving, as one cell: whichever of the
/// commit, the version and the tag the plan could name.
fn expected_identity(id: &crate::deploy::targets::DeploymentIdentity) -> El {
    if id.is_empty() {
        return el("span").class("mj-muted").text("—");
    }
    let parts: Vec<El> = [
        id.commit
            .as_ref()
            .map(|c| mono(c.chars().take(12).collect::<String>())),
        id.version.as_ref().map(|v| mono(v.clone())),
        id.tag.as_ref().map(|t| mono(t.clone())),
    ]
    .into_iter()
    .flatten()
    .collect();
    el("span").class("mj-marks").children(parts)
}
