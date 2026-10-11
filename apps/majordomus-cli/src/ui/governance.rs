//! Governance: the rules that bind this repository and how each is proven. Read from
//! `rules.report`: its verdict, its coverage, the state of each rule's proof and its
//! findings.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{
    counts, donut, hero, kpi, kpi_count, kpi_state, list, panel, state, table, text, unanswered,
    wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/governance";

const RULES: Source = Source::of("rules.report");

/// The Governance screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[RULES]);
    let rules = &answers[&RULES];

    let top = hero(
        "shield-check",
        "rose",
        "Governance",
        "Rules that are proven, not only declared.",
        "Every rule of the repository, what proves it — a validator, a test, a gate — and whether that proof ran and passed.",
        vec![
            kpi_state("Verdict", &rules.read("/verdict")),
            kpi("Rules", &rules.read("/coverage/rules")),
            kpi("Blocking", &rules.read("/coverage/blocking")),
            kpi("Gated", &rules.read("/coverage/gated")),
            kpi_count("Findings", &rules.read("/findings")),
        ],
    );

    let table_body = unanswered(rules).unwrap_or_else(|| {
        let rows = list(rules, "/rules")
            .iter()
            .map(|r| {
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(r, "rule")))
                    .child(el("td").child(state(r, "state")))
                    .child(el("td").class("mj-ui-dim").text(text(r, "meaning")))
            })
            .collect::<Vec<_>>();
        table(
            &["Rule", "Proof", "What that means"],
            rows,
            "No rule is declared.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(panel(
            "Proof states",
            "How far each rule's proof reaches, counted by the report.",
            unanswered(rules).unwrap_or_else(|| donut("rules", &counts(rules, "/states"))),
            &[rules],
        ))
        .child(panel(
            "Coverage",
            "The rules by class and by what proves them; the classes overlap, so they are listed, not summed.",
            unanswered(rules).unwrap_or_else(|| {
                el("dl").class("mj-ui-facts").children(
                    counts(rules, "/coverage")
                        .into_iter()
                        .map(|(name, n)| {
                            el("div")
                                .class("mj-ui-fact")
                                .child(el("dt").text(name.replace('_', " ")))
                                .child(el("dd").text(n.to_string()))
                        }),
                )
            }),
            &[rules],
        ))
        .child(wide(panel(
            "Rules",
            "Every rule with the state of its proof.",
            table_body,
            &[rules],
        )));

    View::new(
        "Governance",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}
