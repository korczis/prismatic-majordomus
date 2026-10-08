//! The intents area: what must become true, how far reality is from it, which work is making
//! it true, and the test behind every criterion.
//!
//! Two pages, both projections. The list is `intent_realization.work`; one intent is
//! `intent_realization.explain`. Nothing here derives a stage, grades a link or decides
//! whether a criterion is met: every word on the page is a word a capability answered, and
//! the links go to the objects the index already holds — the test a criterion names, the
//! issues serving it, the milestones realising the intent (ADR 0075).

use serde_json::json;

use crate::capability::Context;
use crate::evidence::ProofState;
use crate::http::router::percent_encode;
use crate::intent::{IntentEvidenceState, IntentStage, IntentVerdictState};
use crate::intent_realization::{
    IntentExplanation, IntentLinkProvenance, IntentRealization, IntentRealizedWork,
};

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, failed, Page};
use super::view::{
    alert, badge, card, cell, facts, id_cell, link, mono, nothing, row, statistic, table, text_cell,
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
                    &["Intent", "Stage", "Met", "Work", "Providers", "Findings", "Title"],
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

/// One intent: the statement, the distance to it, each criterion with its test and evidence,
/// the milestones and issues realising it, the work and providers, and why it stands where it
/// stands.
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
    let i = &e.intent;
    let total = i.satisfaction.len() - i.optional;

    let summary = card(
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
                        format!("{}/{total}", i.met),
                        "criteria with current evidence",
                        "intent_realization.explain",
                    ))
                    .child(statistic(
                        e.realization.work.len().to_string(),
                        "units of work",
                        "intent_realization.explain",
                    )),
            )
            .child(criteria_bar(i.met, total))
            .child(facts(vec![
                ("Stage", Node::Element(stage_badge(i.stage))),
                ("Verdict", Node::Element(verdict_badge(i.verdict.state))),
                (
                    "Record",
                    Node::Element(object_link(ctx, "intent", &i.id, &i.source)),
                ),
            ])),
    );

    // each criterion with the test it names and the issues declaring they serve it
    let criteria_rows: Vec<El> = i
        .satisfaction
        .iter()
        .map(|c| {
            let evidence = if c.evidence == "test" {
                object_link(ctx, "test", &c.reference, &c.reference)
            } else {
                mono(format!("{} {}", c.evidence, c.reference))
            };
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
                cell(evidence_badge(c.state, c.proof)),
                cell(evidence),
                cell(served),
                cell(match &c.reproduce {
                    Some(r) => mono(r.clone()),
                    None => el("span").text("—"),
                }),
            ])
        })
        .collect();
    let criteria = card(
        "Criteria and the evidence behind each",
        table(
            &[
                "Criterion",
                "What must be true",
                "Evidence",
                "Test",
                "Served by",
                "Reproduce",
            ],
            criteria_rows,
        ),
    );

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

    let body = el("div")
        .class("mj-grid")
        .child(summary)
        .child(criteria)
        .child(milestones)
        .child(work)
        .child(card("Why it stands where it stands", because))
        .when(!e.realization.findings.is_empty(), |d| {
            d.child(findings_card(&e.realization.findings))
        });
    Page::new(Area::Intents, i.title.clone(), body)
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

    #[test]
    fn a_bar_of_no_criteria_is_empty_rather_than_a_division_by_zero() {
        let html = criteria_bar(0, 0).render();
        assert!(html.contains("width:0%"), "{html}");
        assert!(criteria_bar(1, 2).render().contains("width:50%"));
    }
}
