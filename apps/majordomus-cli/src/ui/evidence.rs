//! Evidence: which claims of the product hold, judged at the presented revision. Read from
//! `evidence.report`: its totals, its claims with their state, and its findings.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::ROWS;
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{
    counts, donut, hero, kpi, kpi_count, list, panel, state, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/evidence";

const EVIDENCE: Source = Source::of("evidence.report");

/// The Evidence screen; `query` may pick one claim to inspect (`?claim=<id>`).
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let answers = ask_all(ctx, &[EVIDENCE]);
    let evidence = &answers[&EVIDENCE];
    let picked = super::claim::picked(query);
    let inspected = picked
        .as_deref()
        .map(|id| super::claim::inspector(ctx, &super::claim::ask(ctx, id), id));

    let top = hero(
        "file-check",
        "green",
        "Evidence",
        "Truth over assumptions: each claim with the test that settles it.",
        "Every claim the product makes, judged against the revision presented, with the findings that keep a claim from holding.",
        vec![
            kpi_count("Claims", &evidence.read("/claims")),
            kpi_count("Findings", &evidence.read("/findings")),
            kpi("Revision", &evidence.read("/presented/revision")),
            kpi("Ledger runs", &evidence.read("/ledger/executions")),
        ],
    );

    let findings = unanswered(evidence).unwrap_or_else(|| {
        let rows = list(evidence, "/findings")
            .iter()
            .take(ROWS * 2)
            .map(|f| {
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-mono")
                                .attr("href", super::claim::href(&text(f, "claim")))
                                .text(text(f, "claim")),
                        ),
                    )
                    .child(el("td").child(state(f, "state")))
                    .child(el("td").class("mj-ui-dim").text(text(f, "reason")))
            })
            .collect::<Vec<_>>();
        table(
            &["Claim", "State", "Why it does not hold"],
            rows,
            "Every claim holds.",
        )
    });

    let claims = unanswered(evidence).unwrap_or_else(|| {
        let rows = list(evidence, "/claims")
            .iter()
            .take(ROWS * 2)
            .map(|c| {
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", super::claim::href(&text(c, "id")))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(c, "id")))
                                .child(el("span").class("mj-ui-cell-title").text(text(c, "claim"))),
                        ),
                    )
                    .child(el("td").child(state(c, "state")))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(text(c, "test_path")),
                    )
            })
            .collect::<Vec<_>>();
        table(&["Claim", "State", "Settled by"], rows, "No claim is made.")
    });

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(inspector) = inspected {
        grid = grid.child(wide(inspector));
    }
    let grid = grid
        .child(panel(
            "Claims without a verdict",
            "The claims the report could not settle, by why.",
            unanswered(evidence).unwrap_or_else(|| donut("claims", &counts(evidence, "/totals"))),
            &[evidence],
        ))
        .child(panel(
            "Findings",
            "What keeps a claim from holding, as the report found it.",
            findings,
            &[evidence],
        ))
        .child(wide(panel(
            "Claims",
            "The product's claims with the test that settles each.",
            claims,
            &[evidence],
        )));

    View::new(
        "Evidence",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::deployments::tests::furnished;

    /// A guarantee no test supports is a finding with its reason, and every claim is a row
    /// with its state and the test that would settle it.
    #[test]
    fn every_claim_is_a_row_and_an_unsupported_guarantee_is_a_finding() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert!(
            html.contains("the claim guarantees a behaviour and names no test at all"),
            "{html}"
        );
        assert!(html.contains("synthetic-guarantee"), "{html}");
        assert!(html.contains("A guarantee no test supports"), "{html}");
        assert!(html.contains("An advisory claim"), "{html}");
        assert!(html.contains("test/cases/1_synthetic.sh"), "{html}");
        assert!(
            html.contains("no_test") || html.contains("no test"),
            "{html}"
        );
        assert!(!html.contains("Every claim holds."), "{html}");
        assert!(!html.contains("No claim is made."), "{html}");
    }
}
