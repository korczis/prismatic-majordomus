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

/// The Governance screen; `query` may pick one rule to inspect (`?rule=<id>`).
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let answers = ask_all(ctx, &[RULES]);
    let rules = &answers[&RULES];
    let inspected = super::rule::picked(query)
        .map(|id| super::rule::inspector(&super::rule::ask(ctx, &id), &id));

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
                // the report gives the rule as an object: its id and title, a link to it
                let rule = r.get("rule").cloned().unwrap_or(serde_json::Value::Null);
                let id = super::rule::id_of(&rule);
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", super::rule::href(&id))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                                .child(
                                    el("span")
                                        .class("mj-ui-cell-title")
                                        .text(text(&rule, "title")),
                                ),
                        ),
                    )
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

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(inspector) = inspected {
        grid = grid.child(wide(inspector));
    }
    let grid = grid
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// The screen reads the report it names: its verdict and coverage, the proof states and
    /// every rule as a row that leads to the rule's own inspector.
    #[test]
    fn every_rule_is_a_row_that_leads_to_its_inspector() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx, &[]);
        assert_eq!(view.title, "Governance");
        let html = view.main.render();
        for want in ["Proof states", "Coverage", "Rules", "rules.report"] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        let report = ctx
            .execute("rules.report", serde_json::json!({}))
            .expect("the report");
        let ids: Vec<String> = report["rules"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| {
                        super::super::rule::id_of(r.get("rule").unwrap_or(&serde_json::Value::Null))
                    })
                    .filter(|id| !id.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            assert!(html.contains("No rule is declared."), "{html}");
        }
        for id in &ids {
            let href = super::super::rule::href(id);
            assert!(
                html.contains(&format!("href=\"{href}\"")),
                "{id} leads nowhere: {html}"
            );
        }
        assert!(
            !html.contains("id=\"rule\""),
            "no rule is inspected unless asked"
        );
    }

    /// `?rule=` opens the inspector above the table; a value that is no rule id opens none and
    /// is never asked.
    #[test]
    fn a_picked_rule_opens_its_inspector_and_a_bad_one_none() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let unknown = screen(&ctx, &q(&[("rule", "no.such-rule")])).main.render();
        assert!(
            unknown.contains("rules.show"),
            "the inspector asked: {unknown}"
        );
        assert!(!unknown.contains("id=\"rule\""), "{unknown}");
        let hostile = screen(&ctx, &q(&[("rule", "x\" onclick=\"y")]))
            .main
            .render();
        assert!(
            !hostile.contains("rules.show"),
            "a hostile id is never asked: {hostile}"
        );
        assert!(!hostile.contains("onclick=\"y"), "{hostile}");
    }
}
