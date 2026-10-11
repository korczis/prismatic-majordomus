//! CI/CD & Releases: what was released, where it is deployed, and what the CI model runs.
//! Read from `distribution.releases`, `release.version`, `deploy.list` and `gates.model`.
//!
//! Per-run CI timings are not recorded by any capability yet, so no build-time chart is
//! drawn: the CI model says which gates run and when, and that is what is shown.

use crate::capability::Context;
use crate::cockpit::html::el;
use serde_json::Value;

use super::dashboard::releases_panel;
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{hero, kpi, list, nothing, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/delivery";

const RELEASES: Source = Source::of("distribution.releases");
const VERSION: Source = Source::of("release.version");
const DEPLOYMENTS: Source = Source::of("deploy.list");
const GATES: Source = Source::of("gates.model");

/// The CI/CD & Releases screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[RELEASES, VERSION, DEPLOYMENTS, GATES]);
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
            .map(|d| {
                el("tr")
                    .child(
                        el("td")
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(d, "id")))
                            .child(el("span").class("mj-ui-cell-title").text(text(d, "title"))),
                    )
                    .child(el("td").class("mj-ui-mono").text(text(d, "application")))
                    .child(el("td").child(state(d, "status")))
            })
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
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(g, "id")))
                    .child(el("td").text(text(g, "job")))
                    .child(el("td").class("mj-ui-dim").text(when))
                    .child(el("td").class("mj-ui-dim").text(text(g, "summary")))
            })
            .collect::<Vec<_>>();
        table(
            &["Gate", "Job", "Runs", "What it holds"],
            rows,
            "The CI model declares no gate.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(releases_panel(releases, version))
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
        .child(panel(
            "Build timings",
            "How long each run took.",
            nothing("No capability records per-run CI timings yet, so none are drawn. The CI model above is what runs."),
            &[gates],
        ));

    View::new(
        "CI/CD & Releases",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}
