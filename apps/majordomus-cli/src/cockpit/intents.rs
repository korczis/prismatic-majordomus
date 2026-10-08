//! The intents area: what must become true, how far reality is from it, which work is making
//! it true, and the test behind every criterion.
//!
//! Two pages, both projections. The list is `intent_realization.work`; one intent is
//! `intent_realization.explain` and, for the review of its plan, `intent_opposition.review`.
//! Nothing here derives a stage, a verdict, a guard's standing, a review's state or a
//! disposition, grades a link or decides whether a criterion is met: every word on the page
//! is a word a capability answered, and the links go to the objects the index already holds
//! — the test a criterion names, the issues serving it, the milestones realising the intent
//! (ADR 0075). What this module chooses is the colour a word is read in, and it never lets a
//! colour say more than the word: a disposition is green only under a review that is current.
//!
//! Every card is a function of the typed answer it renders, so each state of an answer is
//! tested by constructing the answer, and the page is those functions in order.

use serde_json::json;

use crate::capability::Context;
use crate::evidence::ProofState;
use crate::http::router::percent_encode;
use crate::intent::{
    IntentEvaluation, IntentEvidenceState, IntentGuard, IntentGuardStanding, IntentStage,
    IntentVerdictState, IntentView,
};
use crate::intent_opposition::{
    IntentOpposition, OppositionDisposition, OppositionFinding, OppositionReviewState,
};
use crate::intent_realization::{
    IntentExplanation, IntentLinkProvenance, IntentRealization, IntentRealizedWork,
};

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, failed, Page};
use super::view::{
    alert, badge, card, cell, details, facts, id_cell, link, mono, nothing, row, statistic, table,
    tag, text_cell,
};

/// The status colour a stage is read in, the stage itself being the label.
fn stage_badge(stage: IntentStage) -> El {
    let status = match stage {
        IntentStage::Satisfied => "ok",
        IntentStage::Verifying | IntentStage::Cancelled => "warn",
        IntentStage::Executing => "info",
        IntentStage::Superseded => "bad",
        IntentStage::Declared | IntentStage::Planned => "neutral",
    };
    badge(status, stage.as_str())
}

/// The status colour a verdict is read in, the verdict itself being the label.
fn verdict_badge(verdict: IntentVerdictState) -> El {
    let status = match verdict {
        IntentVerdictState::Satisfied => "ok",
        IntentVerdictState::Unsatisfied => "bad",
        IntentVerdictState::Unknown => "neutral",
    };
    badge(status, verdict.as_str())
}

/// A criterion's state, and for a met one the verdict it rests on: a proof at this revision
/// and a pass whose named inputs are unchanged both meet it, and never wear the same badge.
fn evidence_badge(state: IntentEvidenceState, proof: Option<ProofState>) -> El {
    let (status, label) = match state {
        IntentEvidenceState::Current => match proof {
            Some(ProofState::Proven) => ("ok", "current · proven"),
            Some(ProofState::InputsUnchanged) => ("info", "current · inputs unchanged"),
            _ => ("ok", "current"),
        },
        IntentEvidenceState::Stale => ("warn", "stale"),
        IntentEvidenceState::Failing => ("bad", "failing"),
        IntentEvidenceState::NotRun => ("neutral", "not run"),
        IntentEvidenceState::NotDerivable => ("neutral", "not derivable"),
        IntentEvidenceState::Unresolved => ("bad", "unresolved"),
    };
    badge(status, label)
}

/// Where a guard stands, in the engine's word (ADR 0113): the page chooses a colour for it
/// and works nothing out.
fn guard_badge(standing: IntentGuardStanding) -> El {
    let status = match standing {
        IntentGuardStanding::Violated => "bad",
        IntentGuardStanding::Holds => "ok",
        IntentGuardStanding::NotJudged => "neutral",
    };
    badge(status, standing.words())
}

/// Where a critique's stamp stands against the plan. No review at all is neutral, never
/// green: nothing was said about the plan.
fn review_badge(state: OppositionReviewState) -> El {
    let status = match state {
        OppositionReviewState::Current => "ok",
        OppositionReviewState::Stale | OppositionReviewState::NotStamped => "warn",
        OppositionReviewState::None => "neutral",
    };
    badge(status, state.as_str().replace('_', " "))
}

/// What the state of a review means for the reader, one sentence per state.
fn review_sentence(state: OppositionReviewState) -> &'static str {
    match state {
        OppositionReviewState::Current => {
            "The critique was stamped against the plan as it stands now."
        }
        OppositionReviewState::Stale => {
            "The plan changed after the critique was stamped: it reviewed another plan."
        }
        OppositionReviewState::NotStamped => {
            "A critique is recorded and nobody stamped it: it does not say which plan it reviewed."
        }
        OppositionReviewState::None => {
            "No critique of this plan is recorded. What follows is what the plan's own structure says."
        }
    }
}

/// The disposition, in the engine's word. `reject` is red whoever reviewed; an accepting
/// word is coloured only under a current review, because without one it says only that the
/// structure has no blocker — which is not a reviewed plan.
fn disposition_badge(disposition: OppositionDisposition, review: OppositionReviewState) -> El {
    let status = match (disposition, review) {
        (OppositionDisposition::Reject, _) => "bad",
        (OppositionDisposition::Accept, OppositionReviewState::Current) => "ok",
        (OppositionDisposition::AcceptWithRequiredChanges, OppositionReviewState::Current) => {
            "info"
        }
        _ => "neutral",
    };
    badge(status, disposition.as_str().replace('_', " "))
}

/// The run a criterion or a guard was judged by: what it reported, at which commit over
/// which working tree and when, and — behind a disclosure — the inputs that changed since
/// and the evidence module's sentence. A dash when nothing was judged.
fn evaluation_cell(evaluation: Option<&IntentEvaluation>) -> El {
    let Some(e) = evaluation else {
        return el("span").text("—");
    };
    let short: String = e.commit.chars().take(10).collect();
    let mut out = el("div")
        .child(tag(e.outcome.clone()))
        .text(" at ")
        .child(mono(short))
        .text(format!(" ({} tree), {}", e.working_tree, e.at));
    if !e.changed.is_empty() {
        let mut changed = el("ul").class("mj-list");
        for path in &e.changed {
            changed = changed.child(el("li").child(mono(path.clone())));
        }
        out = out.child(details(
            format!("{} input(s) changed since", e.changed.len()),
            changed,
        ));
    }
    if let Some(detail) = &e.detail {
        out = out.child(el("p").class("mj-prose").text(detail));
    }
    out
}

fn criterion_anchor(id: &str) -> String {
    format!("criterion-{id}")
}

fn guard_anchor(id: &str) -> String {
    format!("guard-{id}")
}

fn provenance_badge(p: IntentLinkProvenance) -> El {
    let status = match p {
        IntentLinkProvenance::Declared => "ok",
        IntentLinkProvenance::Observed => "info",
        IntentLinkProvenance::Derived => "neutral",
        IntentLinkProvenance::Inferred => "warn",
    };
    badge(status, p.as_str())
}

fn object_href(uri: &str) -> String {
    format!("/cockpit/object?uri={}", percent_encode(uri))
}

/// The object page of an index object, when the index holds it; otherwise the reference as
/// text, so a link never points at a page that would answer 404. Whether it is held is asked
/// of `objects.get`, the capability every surface resolves a URI with, rather than read from
/// the index beside it (ADR 0012).
fn object_link(ctx: &Context, kind: &str, identity: &str, label: &str) -> El {
    let uri = format!("majordomus://{kind}/{identity}");
    match ctx.execute("objects.get", json!({ "uri": uri })) {
        Ok(_) => link(object_href(&uri), label),
        Err(_) => mono(label.to_string()),
    }
}

/// How many criteria have current evidence, as the Cockpit's progress bar.
fn criteria_bar(met: usize, total: usize) -> El {
    let percent = (met * 100).checked_div(total).unwrap_or(0);
    el("div")
        .class("mj-progress")
        .attr("role", "progressbar")
        .attr("aria-valuenow", percent.to_string())
        .attr("aria-valuemin", "0")
        .attr("aria-valuemax", "100")
        .attr(
            "aria-label",
            format!("{met} of {total} criteria have current evidence"),
        )
        .child(
            el("div")
                .class("mj-progress-bar")
                .attr("style", format!("width:{percent}%")),
        )
        .child(
            el("span")
                .class("mj-progress-text")
                .text(format!("{met}/{total}")),
        )
}

/// One unit of work as one intent sees it: its strongest link to that intent and the issues it
/// links through. A unit with no link to the intent shows a dash rather than a grade.
fn work_row(intent: &str, w: &IntentRealizedWork) -> El {
    let links: Vec<&crate::intent_realization::IntentLink> =
        w.links.iter().filter(|l| l.intent == intent).collect();
    let strongest = links.iter().map(|l| l.provenance).min();
    let issues: Vec<String> = links
        .iter()
        .map(|l| format!("{} ({})", l.issue, l.via.as_str()))
        .collect();
    row(vec![
        text_cell(w.work.kind.as_str()),
        cell(mono(w.work.id.clone())),
        text_cell(w.work.outcome.clone()),
        cell(match strongest {
            Some(p) => provenance_badge(p),
            None => el("span").text("—"),
        }),
        text_cell(issues.join(", ")),
        text_cell(w.work.providers().join(", ")),
        text_cell(w.work.handovers.len().to_string()),
    ])
}

fn intent_href(id: &str) -> String {
    format!("/cockpit/intents/{}", percent_encode(id))
}

// ---------------------------------------------------------------- the list

/// Every intent with how far reality is from it and who is realising it.
pub fn list(ctx: &Context) -> Page {
    let r: IntentRealization = match ask(ctx, "intent_realization.work", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Intents, "Intents", e),
    };
    let met: usize = r.intents.iter().map(|i| i.met).sum();
    let criteria: usize = r.intents.iter().map(|i| i.criteria).sum();
    let rows: Vec<El> = r
        .intents
        .iter()
        .map(|i| {
            row(vec![
                id_cell(intent_href(&i.intent), i.intent.clone()),
                cell(stage_badge(i.stage)),
                cell(verdict_badge(i.verdict)),
                text_cell(format!("{}/{}", i.met, i.criteria)),
                text_cell(i.work.len().to_string()),
                text_cell(if i.providers.is_empty() {
                    "—".to_string()
                } else {
                    i.providers.join(", ")
                }),
                text_cell(i.findings.len().to_string()),
                text_cell(i.title.clone()),
            ])
        })
        .collect();
    let body = el("div")
        .class("mj-grid")
        .child(
            el("div")
                .class("mj-stats")
                .child(statistic(r.intents.len().to_string(), "intents", "intent_realization.work"))
                .child(statistic(format!("{met}/{criteria}"), "criteria met", "intent_realization.work"))
                .child(statistic(
                    r.orphans.to_string(),
                    "units of work serving no intent",
                    "intent_realization.work",
                )),
        )
        .child(card(
            "Intents",
            if rows.is_empty() {
                nothing("This repository declares no intent. One is a YAML record of the intent kind; `majordomus intent --help` says how to declare one.")
            } else {
                table(
                    &["Intent", "Stage", "Verdict", "Met", "Work", "Providers", "Findings", "Title"],
                    rows,
                )
            },
        ))
        .when(!r.findings.is_empty(), |d| d.child(findings_card(&r.findings)));
    Page::new(Area::Intents, "Intents", body)
        .subtitle(
            "What must become true, derived from the plan and the recorded evidence on every read",
        )
        .trail(vec![("Cockpit", Some("/cockpit")), ("Intents", None)])
}

fn findings_card(findings: &[crate::intent::IntentFinding]) -> El {
    let mut list = el("div");
    for f in findings {
        let level = if f.level == crate::intent::FAIL {
            "fail"
        } else {
            "warn"
        };
        list = list.child(alert(
            level,
            format!("{} — {}: {}", f.code, f.subject, f.message),
        ));
    }
    card("Findings", list)
}

// ---------------------------------------------------------------- one intent

/// Why the verdict is what it is: each required criterion holding it back and each violated
/// guard, as the verdict itself lists them, each linked to its row further down the page.
fn verdict_card(i: &IntentView) -> El {
    let v = &i.verdict;
    let mut body = el("div").child(facts(vec![(
        "Verdict",
        Node::Element(verdict_badge(v.state)),
    )]));
    if v.reasons.is_empty() && v.guards.is_empty() {
        return card(
            "Why the verdict is what it is",
            body.child(nothing(match v.state {
                IntentVerdictState::Satisfied => {
                    "Every required criterion has current evidence and no guard is violated."
                }
                _ => "The verdict names no criterion and no guard: the evidence cannot answer for this intent.",
            })),
        );
    }
    let mut rows: Vec<El> = v
        .reasons
        .iter()
        .map(|r| {
            row(vec![
                text_cell("criterion"),
                cell(link(
                    format!("#{}", criterion_anchor(&r.criterion)),
                    r.criterion.clone(),
                )),
                text_cell(r.evidence.clone()),
                cell(evidence_badge(r.state, None)),
            ])
        })
        .collect();
    rows.extend(v.guards.iter().map(|g| {
        row(vec![
            text_cell("guard"),
            cell(link(
                format!("#{}", guard_anchor(&g.guard)),
                g.guard.clone(),
            )),
            text_cell(g.evidence.clone()),
            cell(guard_badge(IntentGuardStanding::Violated)),
        ])
    }));
    body = body.child(table(
        &["Held back by", "Which", "Evidence", "Standing"],
        rows,
    ));
    card("Why the verdict is what it is", body)
}

/// The test a criterion or a guard names as a link to its object, or its reference as text.
fn evidence_link(ctx: &Context, evidence: &str, reference: &str) -> El {
    if evidence == "test" {
        object_link(ctx, "test", reference, reference)
    } else {
        mono(format!("{evidence} {reference}"))
    }
}

fn reproduce_cell(reproduce: Option<&String>) -> El {
    match reproduce {
        Some(r) => mono(r.clone()),
        None => el("span").text("—"),
    }
}

/// Each criterion with whether it is required, the state of its evidence, the run that was
/// judged, the test it names and the issues declaring they serve it.
fn criteria_card(ctx: &Context, e: &IntentExplanation) -> El {
    let rows: Vec<El> = e
        .intent
        .satisfaction
        .iter()
        .map(|c| {
            let issues: Vec<String> = e
                .coverage
                .iter()
                .find(|k| k.criterion == c.id)
                .map(|k| k.issues.iter().map(|x| x.id.clone()).collect())
                .unwrap_or_default();
            let mut served = el("span");
            if issues.is_empty() {
                served = served.text("—");
            }
            for (n, issue) in issues.iter().enumerate() {
                if n > 0 {
                    served = served.text(" ");
                }
                served = served.child(object_link(ctx, "issue", issue, issue));
            }
            row(vec![
                text_cell(c.id.clone()),
                text_cell(c.criterion.clone()),
                cell(tag(if c.optional { "optional" } else { "required" })),
                cell(evidence_badge(c.state, c.proof)),
                cell(evaluation_cell(c.evaluation.as_ref())),
                cell(evidence_link(ctx, &c.evidence, &c.reference)),
                cell(served),
                cell(reproduce_cell(c.reproduce.as_ref())),
            ])
            .attr("id", criterion_anchor(&c.id))
        })
        .collect();
    card(
        "Criteria and the evidence behind each",
        table(
            &[
                "Criterion",
                "What must be true",
                "Holds the verdict",
                "Evidence",
                "Judged by",
                "Test",
                "Served by",
                "Reproduce",
            ],
            rows,
        ),
    )
}

/// The guards: what must stay true, where each stands in the engine's word, and the run it
/// was judged by.
fn guards_card(ctx: &Context, guards: &[IntentGuard]) -> El {
    let rows: Vec<El> = guards
        .iter()
        .map(|g| {
            row(vec![
                text_cell(g.id.clone()),
                text_cell(g.invariant.clone()),
                cell(guard_badge(g.standing)),
                cell(evidence_badge(g.state, None)),
                cell(evaluation_cell(g.evaluation.as_ref())),
                cell(evidence_link(ctx, &g.evidence, &g.reference)),
                cell(reproduce_cell(g.reproduce.as_ref())),
            ])
            .attr("id", guard_anchor(&g.id))
        })
        .collect();
    card(
        "Guards: what must stay true, judged",
        table(
            &[
                "Guard",
                "What must stay true",
                "Standing",
                "Evidence",
                "Judged by",
                "Test",
                "Reproduce",
            ],
            rows,
        ),
    )
}

fn statements(title: &str, items: &[String]) -> Option<El> {
    if items.is_empty() {
        return None;
    }
    let mut list = el("ul").class("mj-list");
    for item in items {
        list = list.child(el("li").text(item));
    }
    Some(el("div").child(el("h3").text(title)).child(list))
}

/// What the record declares beside its criteria: the invariants nothing judges, what it
/// deliberately does not require, and what governs it. Absent when the record says none.
fn declared_card(i: &IntentView) -> Option<El> {
    let parts: Vec<El> = [
        statements("Invariants, as stated and judged by nothing", &i.invariants),
        statements("Deliberately not required", &i.non_goals),
        statements("Governed by", &i.governance),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        return None;
    }
    Some(card("What the record declares", el("div").children(parts)))
}

/// One finding of a review: what it is about, whether it blocks, how it was resolved, and —
/// behind a disclosure — the finding itself with who found it and who resolved it.
fn finding_row(ctx: &Context, f: &OppositionFinding) -> El {
    let mut more = el("div").child(el("p").class("mj-prose").text(&f.finding));
    let mut said = Vec::new();
    if !f.class.is_empty() {
        said.push(("Class", Node::Element(tag(f.class.clone()))));
    }
    if !f.because.is_empty() {
        said.push(("Because", Node::Text(f.because.clone())));
    }
    if !f.source.is_empty() {
        said.push(("Found by", Node::Text(f.source.clone())));
    }
    if !f.resolved_by.is_empty() {
        said.push(("Resolved by", Node::Text(f.resolved_by.clone())));
    }
    if !said.is_empty() {
        more = more.child(facts(said));
    }
    let resolution = match (f.origin.as_str(), f.resolution.as_str()) {
        ("structural", _) => tag("by changing the plan"),
        (_, "planned") => badge("ok", "planned"),
        (_, "rejected") => badge("neutral", "rejected"),
        (_, other) => badge(
            if f.blocking { "bad" } else { "warn" },
            if other.is_empty() { "open" } else { other },
        ),
    };
    row(vec![
        text_cell(f.origin.clone()),
        cell(mono(f.id.clone())),
        text_cell(f.subject.clone()),
        cell(if f.blocking {
            badge("bad", "blocking")
        } else {
            tag("advises")
        }),
        cell(resolution),
        cell(if f.issue.is_empty() {
            el("span").text("—")
        } else {
            object_link(ctx, "issue", &f.issue, &f.issue)
        }),
        cell(details("the finding", more)),
    ])
}

/// The review of the intent's plan as `intent_opposition.review` answers it: where the
/// critique stands against the plan, the disposition, the stamp, every finding of both
/// halves, and the conditions of the recorded gap. An answer that could not be had is said
/// in this card, and the rest of the page stands.
fn review_card(ctx: &Context, answer: &Result<IntentOpposition, String>) -> El {
    let title = "The review of its plan";
    let o = match answer {
        Ok(o) => o,
        Err(reason) => {
            return card(
                title,
                alert(
                    "warn",
                    format!("intent_opposition.review did not answer: {reason}"),
                ),
            )
        }
    };
    let r = &o.review;
    let or_dash = |v: &str| {
        if v.is_empty() {
            Node::Text("—".into())
        } else {
            Node::Element(mono(v.to_string()))
        }
    };
    let mut body = el("div")
        .child(el("p").class("mj-prose").text(review_sentence(r.state)))
        .child(facts(vec![
            ("Review", Node::Element(review_badge(r.state))),
            (
                "Disposition",
                Node::Element(disposition_badge(o.disposition, r.state)),
            ),
            ("Reviewed by", or_dash(&r.reviewed_by)),
            ("Reviewed at", or_dash(&r.reviewed_at)),
            ("Stamped with", or_dash(&r.reviewed_with)),
            ("Critique", or_dash(&r.source)),
        ]));
    if !o.rejecting.is_empty() {
        let mut list = el("ul").class("mj-list");
        for line in &o.rejecting {
            list = list.child(el("li").child(mono(line.clone())));
        }
        body = body
            .child(el("h3").text("What rejects the plan"))
            .child(list);
    }
    let findings: Vec<El> = o
        .structural
        .iter()
        .chain(&o.recorded)
        .map(|f| finding_row(ctx, f))
        .collect();
    body = body
        .child(el("h3").text("Findings"))
        .child(if findings.is_empty() {
            nothing("Neither the plan's structure nor a reviewer has a finding.")
        } else {
            table(
                &[
                    "Origin",
                    "Finding",
                    "About",
                    "Blocks",
                    "Resolution",
                    "Issue",
                    "What it says",
                ],
                findings,
            )
        });
    body = body.child(el("h3").text("The recorded gap"));
    body = if o.gap.is_empty() {
        body.child(nothing(
            "No condition of a gap is recorded for this intent: nobody wrote down how far the repository is from it.",
        ))
    } else {
        let mut list = el("ul").class("mj-list");
        for condition in &o.gap {
            list = list.child(el("li").text(condition));
        }
        body.child(list)
    };
    card(title, body)
}

fn summary_card(ctx: &Context, e: &IntentExplanation) -> El {
    let i = &e.intent;
    // required criteria, as the realization counts them: no arithmetic of the page's own
    let total = e.realization.criteria;
    card(
        "How far reality is from it",
        el("div")
            .child(el("p").class("mj-prose").text(&i.statement))
            .child(
                el("div")
                    .class("mj-stats")
                    .child(statistic(
                        i.stage.as_str().to_string(),
                        "stage",
                        "intent_realization.explain",
                    ))
                    .child(statistic(
                        format!("{}/{total}", e.realization.met),
                        "required criteria with current evidence",
                        "intent_realization.explain",
                    ))
                    .child(statistic(
                        e.realization.work.len().to_string(),
                        "units of work",
                        "intent_realization.explain",
                    )),
            )
            .child(criteria_bar(e.realization.met, total))
            .child(facts(vec![
                ("Stage", Node::Element(stage_badge(i.stage))),
                ("Verdict", Node::Element(verdict_badge(i.verdict.state))),
                (
                    "Record",
                    Node::Element(object_link(ctx, "intent", &i.id, &i.source)),
                ),
            ])),
    )
}

/// The body of one intent's page from the two answers it renders.
fn intent_body(
    ctx: &Context,
    e: &IntentExplanation,
    review: &Result<IntentOpposition, String>,
) -> El {
    let i = &e.intent;
    let milestone_rows: Vec<El> = i
        .milestones
        .iter()
        .map(|m| {
            row(vec![
                cell(object_link(ctx, "milestone", &m.id, &m.id)),
                text_cell(
                    m.status
                        .clone()
                        .unwrap_or_else(|| "does not resolve".into()),
                ),
            ])
        })
        .collect();
    let milestones = card(
        "Milestones realising it",
        table(&["Milestone", "Status"], milestone_rows),
    );

    let work_rows: Vec<El> = e.work.iter().map(|w| work_row(&i.id, w)).collect();
    let work = card(
        "Work realising it",
        if work_rows.is_empty() {
            nothing("No recorded task, session or live claim realises this intent.")
        } else {
            table(
                &[
                    "Kind",
                    "Work",
                    "Outcome",
                    "Link",
                    "Via issue",
                    "Providers",
                    "Handovers",
                ],
                work_rows,
            )
        },
    );

    let mut because = el("ul").class("mj-list");
    for b in &e.because {
        because = because.child(el("li").text(b));
    }

    let mut body = el("div")
        .class("mj-grid")
        .child(summary_card(ctx, e))
        .child(verdict_card(i))
        .child(criteria_card(ctx, e))
        .when(!i.guards.is_empty(), |d| {
            d.child(guards_card(ctx, &i.guards))
        })
        .child(review_card(ctx, review));
    if let Some(declared) = declared_card(i) {
        body = body.child(declared);
    }
    body.child(milestones)
        .child(work)
        .child(card("Why it stands where it stands", because))
        .when(!e.realization.findings.is_empty(), |d| {
            d.child(findings_card(&e.realization.findings))
        })
}

/// One intent: the statement, the distance to it, why its verdict is what it is, each
/// criterion and guard with its evidence and the run that judged it, the review of its plan,
/// the milestones and issues realising it, the work and providers, and why it stands where
/// it stands.
pub fn intent(ctx: &Context, id: &str) -> Page {
    let e: IntentExplanation = match ask(ctx, "intent_realization.explain", json!({ "id": id })) {
        Ok(e) => e,
        Err(reason) => {
            return Page::new(
                Area::Intents,
                "No such intent",
                el("div")
                    .child(alert("fail", reason))
                    .child(link("/cockpit/intents", "Every intent")),
            )
            .status(404)
            .trail(vec![
                ("Cockpit", Some("/cockpit")),
                ("Intents", Some("/cockpit/intents")),
                (id, None),
            ])
        }
    };
    let review = ask(ctx, "intent_opposition.review", json!({ "intent": id }));
    let i = &e.intent;
    Page::new(
        Area::Intents,
        i.title.clone(),
        intent_body(ctx, &e, &review),
    )
    .subtitle(format!("intent {}", i.id))
    .trail(vec![
        ("Cockpit", Some("/cockpit")),
        ("Intents", Some("/cockpit/intents")),
        (&i.id, None),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{IntentFinding, FAIL, WARN};
    use crate::intent_realization::{IntentLink, IntentLinkVia, IntentWorkKind, IntentWorkUnit};
    use crate::synthetic::SyntheticRepository;

    #[test]
    fn every_stage_state_and_provenance_is_read_in_its_own_colour() {
        for (stage, status) in [
            (IntentStage::Declared, "neutral"),
            (IntentStage::Planned, "neutral"),
            (IntentStage::Executing, "info"),
            (IntentStage::Verifying, "warn"),
            (IntentStage::Satisfied, "ok"),
            (IntentStage::Cancelled, "warn"),
            (IntentStage::Superseded, "bad"),
        ] {
            let html = stage_badge(stage).render();
            assert_eq!(html, badge(status, stage.as_str()).render(), "{stage:?}");
        }
        for (verdict, status) in [
            (IntentVerdictState::Satisfied, "ok"),
            (IntentVerdictState::Unsatisfied, "bad"),
            (IntentVerdictState::Unknown, "neutral"),
        ] {
            let html = verdict_badge(verdict).render();
            assert_eq!(
                html,
                badge(status, verdict.as_str()).render(),
                "{verdict:?}"
            );
        }
        for (state, status, label) in [
            (IntentEvidenceState::Current, "ok", "current"),
            (IntentEvidenceState::Stale, "warn", "stale"),
            (IntentEvidenceState::Failing, "bad", "failing"),
            (IntentEvidenceState::NotRun, "neutral", "not run"),
            (
                IntentEvidenceState::NotDerivable,
                "neutral",
                "not derivable",
            ),
            (IntentEvidenceState::Unresolved, "bad", "unresolved"),
        ] {
            assert_eq!(
                evidence_badge(state, None).render(),
                badge(status, label).render(),
                "{state:?}"
            );
        }
        // a met criterion says which verdict it rests on, and the two never render alike
        let proven = evidence_badge(IntentEvidenceState::Current, Some(ProofState::Proven));
        let unchanged = evidence_badge(
            IntentEvidenceState::Current,
            Some(ProofState::InputsUnchanged),
        );
        assert_eq!(proven.render(), badge("ok", "current · proven").render());
        assert_eq!(
            unchanged.render(),
            badge("info", "current · inputs unchanged").render()
        );
        assert_ne!(proven.render(), unchanged.render());
        for (p, status) in [
            (IntentLinkProvenance::Declared, "ok"),
            (IntentLinkProvenance::Observed, "info"),
            (IntentLinkProvenance::Derived, "neutral"),
            (IntentLinkProvenance::Inferred, "warn"),
        ] {
            assert_eq!(
                provenance_badge(p).render(),
                badge(status, p.as_str()).render(),
                "{p:?}"
            );
        }
    }

    #[test]
    fn a_reference_the_index_does_not_hold_is_text_and_never_a_link() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let absent = object_link(&ctx, "test", "test/cases/00_absent.sh", "00_absent").render();
        assert!(!absent.contains("href"), "{absent}");
        assert!(absent.contains("00_absent"), "{absent}");
        let listed: serde_json::Value = ask(&ctx, "objects.list", json!({})).unwrap();
        let held = &listed["objects"][0];
        let (kind, identity) = (
            held["kind"].as_str().unwrap(),
            held["identity"].as_str().unwrap(),
        );
        let present = object_link(&ctx, kind, identity, "here").render();
        assert!(present.contains("href"), "{present}");
        assert!(
            present.contains(&object_href(held["uri"].as_str().unwrap())),
            "{present}"
        );
    }

    #[test]
    fn a_repository_without_intents_says_how_to_declare_one() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let page = list(&ctx);
        assert_eq!(page.status, 200);
        let html = page.main.render();
        assert!(
            html.contains("This repository declares no intent"),
            "{html}"
        );
        assert!(!html.contains("Findings"), "{html}");
        // an intent it does not hold is a 404, and says why
        let page = intent(&ctx, "absent");
        assert_eq!(page.status, 404);
        assert!(page.main.render().contains("no intent"));
    }

    #[test]
    fn a_failure_is_read_as_a_failure_and_anything_else_as_a_warning() {
        let finding = |level: &str| IntentFinding {
            level: level.into(),
            code: "c".into(),
            subject: "x".into(),
            message: "m".into(),
            reproduce: "r".into(),
        };
        let html = findings_card(&[finding(FAIL), finding(WARN)]).render();
        let fail = html.find(&alert("fail", "c — x: m").render());
        let warn = html.find(&alert("warn", "c — x: m").render());
        assert!(fail.is_some() && warn.is_some() && fail < warn, "{html}");
    }

    #[test]
    fn work_with_no_link_to_the_intent_is_shown_without_a_grade() {
        let mut w = IntentRealizedWork {
            work: IntentWorkUnit::new(IntentWorkKind::Task, "t-1"),
            links: vec![IntentLink {
                intent: "other".into(),
                stage: IntentStage::Executing,
                milestone: "m".into(),
                issue: "I0001".into(),
                criteria: vec![],
                via: IntentLinkVia::NamedIssue,
                provenance: IntentLinkProvenance::Declared,
            }],
            unlinked: None,
        };
        let html = work_row("x", &w).render();
        assert!(html.contains("—"), "{html}");
        assert!(!html.contains("I0001"), "{html}");
        w.links[0].intent = "x".into();
        let html = work_row("x", &w).render();
        assert!(html.contains("I0001 (named_issue)"), "{html}");
        assert!(
            html.contains(&provenance_badge(IntentLinkProvenance::Declared).render()),
            "{html}"
        );
    }

    fn run() -> IntentEvaluation {
        IntentEvaluation {
            commit: "c0ffee1234567890".into(),
            working_tree: "clean".into(),
            outcome: "pass".into(),
            at: "2026-10-08T00:00:00Z".into(),
            changed: vec![],
            detail: None,
        }
    }

    fn guard(id: &str, state: IntentEvidenceState) -> IntentGuard {
        IntentGuard {
            id: id.into(),
            invariant: format!("{id} stays true"),
            evidence: "claim".into(),
            reference: format!("claim-{id}"),
            state,
            violated: state == IntentEvidenceState::Failing,
            standing: IntentGuardStanding::of(state),
            reproduce: Some(format!("run {id}")),
            evaluation: None,
        }
    }

    /// The fixture's one intent, explained, and the review of its plan, as the two
    /// capabilities answer them in process.
    fn answered() -> (
        crate::synthetic::SyntheticRepository,
        IntentExplanation,
        IntentOpposition,
    ) {
        let repo = crate::intent_binding::tests::planned();
        let (e, o) = {
            let ctx = repo.context().unwrap();
            (
                ask(&ctx, "intent_realization.explain", json!({ "id": "x" })).unwrap(),
                ask(&ctx, "intent_opposition.review", json!({ "intent": "x" })).unwrap(),
            )
        };
        (repo, e, o)
    }

    #[test]
    fn a_guard_a_review_and_a_disposition_are_each_read_in_the_engines_word() {
        for (standing, status, word) in [
            (IntentGuardStanding::Violated, "bad", "violated"),
            (IntentGuardStanding::Holds, "ok", "holds"),
            (IntentGuardStanding::NotJudged, "neutral", "not judged"),
        ] {
            assert_eq!(guard_badge(standing).render(), badge(status, word).render());
        }
        let mut sentences = std::collections::BTreeSet::new();
        for (state, status, word) in [
            (OppositionReviewState::Current, "ok", "current"),
            (OppositionReviewState::Stale, "warn", "stale"),
            (OppositionReviewState::NotStamped, "warn", "not stamped"),
            (OppositionReviewState::None, "neutral", "none"),
        ] {
            assert_eq!(review_badge(state).render(), badge(status, word).render());
            sentences.insert(review_sentence(state));
        }
        assert_eq!(sentences.len(), 4, "each state has a sentence of its own");
        // a rejection is red whoever reviewed; an acceptance is coloured only under a
        // current review, so `none` beside `accept` never reads as healthy
        use OppositionDisposition::{Accept, AcceptWithRequiredChanges, Reject};
        for review in [
            OppositionReviewState::Current,
            OppositionReviewState::Stale,
            OppositionReviewState::NotStamped,
            OppositionReviewState::None,
        ] {
            let current = review == OppositionReviewState::Current;
            assert_eq!(
                disposition_badge(Reject, review).render(),
                badge("bad", "reject").render()
            );
            assert_eq!(
                disposition_badge(Accept, review).render(),
                badge(if current { "ok" } else { "neutral" }, "accept").render()
            );
            assert_eq!(
                disposition_badge(AcceptWithRequiredChanges, review).render(),
                badge(
                    if current { "info" } else { "neutral" },
                    "accept with required changes"
                )
                .render()
            );
        }
    }

    #[test]
    fn the_run_that_was_judged_is_shown_with_what_changed_since_or_a_dash() {
        assert_eq!(
            evaluation_cell(None).render(),
            el("span").text("—").render()
        );
        let plain = evaluation_cell(Some(&run())).render();
        assert!(plain.contains("c0ffee1234"), "{plain}");
        assert!(
            !plain.contains("c0ffee12345"),
            "the commit is abbreviated: {plain}"
        );
        assert!(
            plain.contains("(clean tree), 2026-10-08T00:00:00Z"),
            "{plain}"
        );
        assert!(plain.contains(&tag("pass").render()), "{plain}");
        assert!(!plain.contains("<details"), "{plain}");
        let changed = evaluation_cell(Some(&IntentEvaluation {
            changed: vec!["lib/a.sh".into(), "lib/b.sh".into()],
            detail: Some("an input changed since the run".into()),
            ..run()
        }))
        .render();
        assert!(changed.contains("2 input(s) changed since"), "{changed}");
        assert!(changed.contains("lib/b.sh"), "{changed}");
        assert!(
            changed.contains("an input changed since the run"),
            "{changed}"
        );
    }

    #[test]
    fn the_verdict_names_what_holds_it_back_and_links_each_to_its_row() {
        let (_repo, mut e, _) = answered();
        // the fixture's criterion was never run: the verdict names it
        let html = verdict_card(&e.intent).render();
        assert!(html.contains("href=\"#criterion-case\""), "{html}");
        assert!(
            html.contains(&evidence_badge(e.intent.verdict.reasons[0].state, None).render()),
            "{html}"
        );
        // a violated guard is named beside it
        e.intent.verdict.guards = vec![crate::intent::IntentVerdictGuard {
            guard: "broken".into(),
            evidence: "test".into(),
            state: IntentEvidenceState::Failing,
        }];
        let html = verdict_card(&e.intent).render();
        assert!(html.contains("href=\"#guard-broken\""), "{html}");
        assert!(
            html.contains(&guard_badge(IntentGuardStanding::Violated).render()),
            "{html}"
        );
        // nothing holds it back: the card says which of the two that is
        e.intent.verdict.reasons.clear();
        e.intent.verdict.guards.clear();
        e.intent.verdict.state = IntentVerdictState::Satisfied;
        let html = verdict_card(&e.intent).render();
        assert!(
            html.contains("Every required criterion has current evidence"),
            "{html}"
        );
        assert!(!html.contains("<table"), "{html}");
        e.intent.verdict.state = IntentVerdictState::Unknown;
        let html = verdict_card(&e.intent).render();
        assert!(html.contains("the evidence cannot answer"), "{html}");
    }

    #[test]
    fn the_page_of_one_intent_renders_both_answers_and_the_list_says_the_verdict() {
        let (repo, e, o) = answered();
        let ctx = repo.context().unwrap();
        let page = intent(&ctx, "x");
        assert_eq!(page.status, 200);
        let html = page.main.render();
        // the page is the cards over the two answers, and nothing besides
        assert_eq!(html, intent_body(&ctx, &e, &Ok(o.clone())).render());
        assert!(html.contains("id=\"criterion-case\""), "{html}");
        assert!(html.contains(&tag("required").render()), "{html}");
        assert!(!html.contains("Guards: what must stay true"), "{html}");
        // nobody reviewed the fixture's plan: said in words, and never in green
        assert_eq!(o.review.state, OppositionReviewState::None);
        assert!(
            html.contains(
                &el("p")
                    .class("mj-prose")
                    .text(review_sentence(OppositionReviewState::None))
                    .render()
            ),
            "{html}"
        );
        assert!(
            html.contains(&disposition_badge(o.disposition, o.review.state).render()),
            "{html}"
        );
        assert!(
            !html.contains(&badge("ok", o.disposition.as_str()).render()),
            "{html}"
        );
        assert!(html.contains("No condition of a gap is recorded"), "{html}");
        // the invariant the record states is shown, and said to be judged by nothing
        assert!(html.contains("Nothing else breaks"), "{html}");
        assert!(html.contains("judged by nothing"), "{html}");

        let listed = list(&ctx).main.render();
        assert!(listed.contains(">Verdict<"), "{listed}");
        assert!(
            listed.contains(&verdict_badge(e.intent.verdict.state).render()),
            "{listed}"
        );
        assert!(listed.contains(&intent_href("x")), "{listed}");
    }

    #[test]
    fn guards_optional_criteria_and_a_review_that_did_not_answer_each_have_their_place() {
        let (repo, mut e, _) = answered();
        let ctx = repo.context().unwrap();
        e.intent.guards = vec![
            IntentGuard {
                evaluation: Some(IntentEvaluation {
                    outcome: "fail".into(),
                    ..run()
                }),
                ..guard("broken", IntentEvidenceState::Failing)
            },
            guard("fine", IntentEvidenceState::Current),
            IntentGuard {
                evidence: "test".into(),
                reproduce: None,
                ..guard("idle", IntentEvidenceState::NotRun)
            },
        ];
        e.intent.satisfaction[0].optional = true;
        e.intent.satisfaction[0].evaluation = Some(run());
        e.intent.invariants.clear();
        let html = intent_body(&ctx, &e, &Err("the ledger could not be read".into())).render();
        for expected in [
            "id=\"guard-broken\"".to_string(),
            "id=\"guard-idle\"".to_string(),
            guard_badge(IntentGuardStanding::Violated).render(),
            guard_badge(IntentGuardStanding::Holds).render(),
            guard_badge(IntentGuardStanding::NotJudged).render(),
            mono("claim claim-fine").render(),
            mono("run broken").render(),
            tag("optional").render(),
            tag("fail").render(),
            alert(
                "warn",
                "intent_opposition.review did not answer: the ledger could not be read",
            )
            .render(),
            // the rest of the page stands
            "Work realising it".to_string(),
            "Why it stands where it stands".to_string(),
        ] {
            assert!(html.contains(&expected), "{expected}\n{html}");
        }
        assert!(!html.contains(&tag("required").render()), "{html}");
        // a record that declares nothing beside its criteria has no card for it
        assert!(declared_card(&e.intent).is_none());
        assert!(!html.contains("What the record declares"), "{html}");
        e.intent.non_goals = vec!["Not this".into()];
        e.intent.governance = vec!["rule:project.derived-once".into()];
        let declared = declared_card(&e.intent).unwrap().render();
        assert!(declared.contains("Not this"), "{declared}");
        assert!(declared.contains("rule:project.derived-once"), "{declared}");
        assert!(!declared.contains("judged by nothing"), "{declared}");
    }

    #[test]
    fn a_review_shows_its_stamp_every_finding_of_both_halves_and_the_gap() {
        let (repo, _, mut o) = answered();
        let ctx = repo.context().unwrap();
        // as answered: no review, no gap, and what the structure alone advises
        let html = review_card(&ctx, &Ok(o.clone())).render();
        assert!(html.contains(&tag("advises").render()), "{html}");
        o.structural.clear();
        let html = review_card(&ctx, &Ok(o.clone())).render();
        assert!(html.contains("nor a reviewer has a finding"), "{html}");
        assert!(!html.contains("<table"), "{html}");
        assert!(!html.contains("What rejects the plan"), "{html}");

        let recorded = |id: &str, blocking: bool, resolution: &str| OppositionFinding {
            origin: "recorded".into(),
            id: id.into(),
            class: "missed_requirement".into(),
            subject: "x#case".into(),
            finding: format!("the finding {id}"),
            blocking,
            resolution: resolution.into(),
            issue: if resolution == "planned" {
                "I0001".into()
            } else {
                String::new()
            },
            because: format!("because {id}"),
            source: "reviewer-one".into(),
            resolved_by: "author-two".into(),
        };
        o.review.state = OppositionReviewState::Stale;
        o.review.source = ".ai/repo/project/critiques/x.yaml".into();
        o.review.reviewed_by = "reviewer-one".into();
        o.review.reviewed_at = "0123456789".into();
        o.review.reviewed_with = "majordomus intent stamp".into();
        o.structural = vec![OppositionFinding::structural(
            "FAIL",
            "criterion_uncovered",
            "x#case",
            "no work serves it",
        )];
        o.recorded = vec![
            recorded("took", true, "planned"),
            recorded("declined", true, "rejected"),
            recorded("stuck", true, "open"),
            recorded("minor", false, ""),
            recorded("odd", false, "deferred"),
        ];
        o.disposition = OppositionDisposition::Reject;
        o.rejecting = vec!["structural criterion_uncovered x#case".into()];
        o.gap = vec!["case missing".into()];
        let html = review_card(&ctx, &Ok(o.clone())).render();
        for expected in [
            review_badge(OppositionReviewState::Stale).render(),
            review_sentence(OppositionReviewState::Stale).to_string(),
            badge("bad", "reject").render(),
            mono("reviewer-one").render(),
            mono("0123456789").render(),
            mono(".ai/repo/project/critiques/x.yaml").render(),
            "What rejects the plan".to_string(),
            mono("structural criterion_uncovered x#case").render(),
            tag("by changing the plan").render(),
            badge("ok", "planned").render(),
            badge("neutral", "rejected").render(),
            badge("bad", "open").render(),
            badge("warn", "open").render(),
            badge("warn", "deferred").render(),
            badge("bad", "blocking").render(),
            tag("advises").render(),
            tag("missed_requirement").render(),
            "the finding stuck".to_string(),
            "because took".to_string(),
            "author-two".to_string(),
            "no work serves it".to_string(),
            "case missing".to_string(),
        ] {
            assert!(html.contains(&expected), "{expected}\n{html}");
        }
        // the issue a planned finding went into is linked where the index holds it
        assert!(
            html.contains(&object_link(&ctx, "issue", "I0001", "I0001").render()),
            "{html}"
        );
        assert!(!html.contains("No condition of a gap"), "{html}");
        // every finding is one row: the detail is behind a disclosure, not a column each
        assert_eq!(html.matches("<details").count(), 6, "{html}");
    }

    #[test]
    fn a_bar_of_no_criteria_is_empty_rather_than_a_division_by_zero() {
        let html = criteria_bar(0, 0).render();
        assert!(html.contains("width:0%"), "{html}");
        assert!(criteria_bar(1, 2).render().contains("width:50%"));
    }
}
