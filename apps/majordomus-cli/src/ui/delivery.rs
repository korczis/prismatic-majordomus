//! CI/CD & Releases: what was released, where it is deployed, and what the CI model runs.
//! Read from `distribution.releases`, `release.version`, `deploy.list` and `gates.model`.
//!
//! How long CI takes is `integration.timings`, measured from the recorded forge observation
//! (`super::timings`): the CI model says which gates run and when, the timings how long the
//! runs on the open pull requests took.

use crate::capability::Context;
use crate::cockpit::html::el;
use serde_json::Value;

use super::dashboard::releases_panel;
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{
    anchor, code_link, hero, kpi, list, none, panel, table, text, unanswered, value, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/delivery";

/// Where one gate of the CI model is: its row on this screen. A rule's proof and a pull
/// request's checks name gates by id, and lead here.
pub fn gate_href(id: &str) -> String {
    format!("{ROUTE}#gate-{}", anchor(id))
}

const RELEASES: Source = Source::of("distribution.releases");
const VERSION: Source = Source::of("release.version");
const DEPLOYMENTS: Source = Source::of("deploy.list");
const GATES: Source = Source::of("gates.model");

/// The CI/CD & Releases screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(
        ctx,
        &[
            RELEASES,
            VERSION,
            DEPLOYMENTS,
            GATES,
            super::timings::TIMINGS,
        ],
    );
    let (releases, version, deployments, gates) = (
        &answers[&RELEASES],
        &answers[&VERSION],
        &answers[&DEPLOYMENTS],
        &answers[&GATES],
    );

    let top = hero(
        "layers",
        "orange",
        "CI/CD & Releases",
        "From a green check to a published release.",
        "What was released, what the tree declares and what its commits imply next, where it is deployed, and every gate the CI model can run.",
        vec![
            kpi("Released", &releases.read("/latest/version")),
            kpi("Declared", &version.read("/declared")),
            kpi("Next", &version.read("/next")),
            kpi("Gates", &gates.read("/count")),
        ],
    );

    let deployed = unanswered(deployments).unwrap_or_else(|| {
        let rows = list(deployments, "/deployments")
            .iter()
            .map(|d| super::deployments::summary_row(ctx, d))
            .collect::<Vec<_>>();
        table(
            &["Deployment", "Application", "State"],
            rows,
            "No deployment is declared.",
        )
    });

    let model = unanswered(gates).unwrap_or_else(|| {
        let rows = list(gates, "/gates")
            .iter()
            .map(|g| {
                let when = if g.get("always").and_then(Value::as_bool) == Some(true) {
                    "every plan"
                } else if g.get("on-demand").and_then(Value::as_bool) == Some(true) {
                    "on demand"
                } else {
                    "when its paths change"
                };
                let id = text(g, "id");
                el("tr")
                    .attr("id", format!("gate-{}", anchor(&id)))
                    .child(el("td").child(code_link(gate_href(&id), value(g, "id"))))
                    .child(el("td").text(value(g, "job")))
                    .child(el("td").class("mj-ui-dim").text(when))
                    .child(el("td").class("mj-ui-dim").text(value(g, "summary")))
                    .child(el("td").child(gates_named(g, "implies")))
            })
            .collect::<Vec<_>>();
        table(
            &["Gate", "Job", "Runs", "What it holds", "Implies"],
            rows,
            "The CI model declares no gate.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(releases_panel(releases, version, None))
        .child(panel(
            "Deployments",
            "Where the product is declared to run.",
            deployed,
            &[deployments],
        ))
        .child(wide(panel(
            "CI model",
            "Every gate the model declares, the job it runs in and when it is selected.",
            model,
            &[gates],
        )))
        .child(wide(super::timings::panel_of(
            &answers[&super::timings::TIMINGS],
        )));

    View::new(
        "CI/CD & Releases",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The gates an entry names under `key`, each a link to its own row; `none` when it names none.
fn gates_named(g: &Value, key: &str) -> crate::cockpit::html::El {
    let ids: Vec<String> = g
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if ids.is_empty() {
        return none();
    }
    el("span")
        .class("mj-ui-badges")
        .children(ids.iter().map(|id| code_link(gate_href(id), id.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::deployments::tests::furnished;

    /// Every declared deployment is a row, and every gate of the CI model says when it
    /// runs: in every plan, on demand, or when its paths change.
    #[test]
    fn every_deployment_and_every_gate_is_a_row_that_says_when_it_runs() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(
            html.contains("The Majordomus layer, served from its container image"),
            "{html}"
        );
        assert!(!html.contains("No deployment is declared."), "{html}");
        for (gate, when) in [
            ("every-plan", "every plan"),
            ("when-asked", "on demand"),
            ("by-path", "when its paths change"),
        ] {
            assert!(html.contains(gate), "{gate} is missing: {html}");
            assert!(html.contains(when), "{gate} does not say {when}: {html}");
        }
        assert!(
            html.contains("runs in every plan"),
            "the gate's own summary: {html}"
        );
        assert!(!html.contains("The CI model declares no gate."), "{html}");
    }

    /// A gate is a row a link can lead to, and the gates it implies lead to theirs.
    #[test]
    fn a_gate_is_a_row_links_lead_to() {
        assert_eq!(gate_href("shell-suite"), "/ui/delivery#gate-shell-suite");
        let implied = gates_named(
            &serde_json::json!({ "implies": ["rust-integration", "rust-quality"] }),
            "implies",
        )
        .render();
        assert!(
            implied.contains("href=\"/ui/delivery#gate-rust-integration\""),
            "{implied}"
        );
        assert!(
            implied.contains("href=\"/ui/delivery#gate-rust-quality\""),
            "{implied}"
        );
        assert!(gates_named(&serde_json::json!({}), "implies")
            .render()
            .contains(">none<"));

        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(html.contains("id=\"gate-every-plan\""), "{html}");
        assert!(
            html.contains("href=\"/ui/delivery#gate-by-path\""),
            "{html}"
        );
        assert!(
            html.contains(&format!(
                "href=\"{}\"",
                super::super::deployments::href("majordomus")
            )),
            "a deployment leads to its row on Deployments: {html}"
        );
    }
}
