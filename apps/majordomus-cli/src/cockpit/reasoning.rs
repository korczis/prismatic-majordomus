//! The Cockpit's Reasoning page (ADR 0098): the advisors as `reasoning.advisors` reports
//! them and the open task's reasoning as `reasoning.status` derives it. The page holds no
//! advisor list and no status of its own — what it shows is what every other surface
//! shows — and with no advisor it shows no advisor card, only the local review path.

use serde_json::json;

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, failed, Page};
use super::view::{card, facts, mono, row, table, text_cell, word_badge};
use crate::capability::builtin::reasoning::{AdvisorsReport, ReasoningStatus};
use crate::capability::Context;

/// `/cockpit/reasoning`.
pub fn page(ctx: &Context) -> Page {
    let advisors: AdvisorsReport = match ask(ctx, "reasoning.advisors", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Reasoning, "Reasoning", e),
    };
    let status: ReasoningStatus = match ask(ctx, "reasoning.status", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Reasoning, "Reasoning", e),
    };

    let summary = card(
        "Reasoning",
        el("div")
            .child(facts(vec![
                ("Operational", Node::Element(el("span").text("yes — with or without advisors"))),
                (
                    "Mode",
                    Node::Element(el("span").text(format!(
                        "{} (decided by {})",
                        advisors.mode.mode.as_str(),
                        advisors.mode.source
                    ))),
                ),
                (
                    "Independent review capacity",
                    Node::Element(el("span").text(format!(
                        "{} advisor(s) available, {} optional unavailable",
                        advisors.available, advisors.unavailable
                    ))),
                ),
            ]))
            .child(el("p").class("mj-prose").text(
                "Advisors are optional. Material uncertainty with no suitable advisor gets a structured local review, and the session still concludes, implements and validates.",
            )),
    );

    let advisor_rows: Vec<El> = advisors
        .advisors
        .iter()
        .map(|a| {
            row(vec![
                super::view::cell(mono(a.id.clone())),
                super::view::cell(word_badge(a.status.as_str())),
                text_cell(word_of(&a.transport)),
                text_cell(if a.recovering {
                    format!("{} (recovering)", a.reason)
                } else {
                    a.reason.clone()
                }),
                text_cell(a.capabilities.join(", ")),
            ])
        })
        .collect();
    let advisor_card = card(
        format!("Advisors ({})", advisors.advisors.len()),
        if advisor_rows.is_empty() {
            el("p")
                .class("mj-prose")
                .text("No advisor is declared. share/advisors.yaml is the one place to add one.")
        } else {
            table(
                &["Advisor", "Status", "Transport", "Why", "Capabilities"],
                advisor_rows,
            )
        },
    );

    let capacity_rows: Vec<El> = advisors
        .capacity
        .iter()
        .map(|c| {
            row(vec![
                super::view::cell(mono(c.capability.clone())),
                text_cell(if c.available.is_empty() {
                    "local review".to_string()
                } else {
                    c.available.join(", ")
                }),
            ])
        })
        .collect();
    let capacity_card = card(
        "Capacity",
        table(&["Capability", "Who can provide it now"], capacity_rows),
    );

    let state = &status.state;
    let timeline_rows: Vec<El> = state
        .timeline
        .iter()
        .map(|e| {
            row(vec![
                text_cell(e.at.clone()),
                super::view::cell(word_badge(&word_of(&e.phase))),
                super::view::cell(mono(e.id.clone())),
                text_cell(e.summary.clone()),
            ])
        })
        .collect();
    let session_card = card(
        format!("Session reasoning — task {}", state.task),
        if timeline_rows.is_empty() {
            el("p").class("mj-prose").text(
                "No reasoning was recorded for this task. `majordomus reasoning record` writes an assessment.",
            )
        } else {
            el("div")
                .child(facts(vec![
                    (
                        "Records",
                        Node::Element(el("span").text(state.totals.records.to_string())),
                    ),
                    (
                        "Consultations",
                        Node::Element(el("span").text(format!(
                            "{} completed, {} failed",
                            state.totals.consultations_completed, state.totals.consultations_failed
                        ))),
                    ),
                    (
                        "Independent reviewers",
                        Node::Element(
                            el("span").text(state.totals.independent_reviewers.to_string()),
                        ),
                    ),
                ]))
                .child(table(&["At", "Phase", "Record", "Summary"], timeline_rows))
        },
    );

    let conclusion_rows: Vec<El> = state
        .conclusions
        .iter()
        .map(|c| {
            row(vec![
                super::view::cell(mono(c.id.clone())),
                text_cell(c.decision.clone()),
                text_cell(if c.reviewed_by.is_empty() {
                    "local reasoning (no advisor)".to_string()
                } else {
                    c.reviewed_by.join(", ")
                }),
                text_cell(format!(
                    "{} passed, {} failed",
                    c.validations_passed, c.validations_failed
                )),
            ])
        })
        .collect();

    let mut main = el("div")
        .class("mj-grid")
        .child(summary)
        .child(advisor_card)
        .child(capacity_card)
        .child(session_card);
    if !conclusion_rows.is_empty() {
        main = main.child(card(
            "Conclusions",
            table(
                &["Record", "Decision", "Reviewed by", "Validation"],
                conclusion_rows,
            ),
        ));
    }
    if !advisors.diagnostics.is_empty() {
        let mut list = el("ul").class("mj-list");
        for d in &advisors.diagnostics {
            list = list.child(el("li").text(d));
        }
        main = main.child(card("Findings", list));
    }
    Page::new(Area::Reasoning, "Reasoning", main)
        .subtitle("Optional advisors, and how this session's uncertainties became decisions")
}

/// The wire word of a serialised enum, as every other surface spells it.
fn word_of<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}
