//! Deployments: where the product runs and whether what is live is what was built. Read
//! from `deploy.list`; the delivery report (`delivery.report`), which probes the public site
//! and judges every feature's delivery, is slow enough to be a panel of its own.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask, ask_all, Answer, Source};
use super::view::{
    hero, kpi, kpi_count, list, panel, panel_to, state, strip, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/deployments";

/// The route of the delivery panel.
pub const DELIVERY_PANEL: &str = "/ui/panels/delivery";

const DEPLOYMENTS: Source = Source::of("deploy.list");
const DELIVERY: Source = Source::of("delivery.report");

/// The Deployments screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[DEPLOYMENTS]);
    let deployments = &answers[&DEPLOYMENTS];

    let top = hero(
        "server",
        "green",
        "Deployments",
        "What is live, and whether it is what was built.",
        "Every declared deployment, and — asked on its own — whether the public site serves the trunk and how far each feature has been delivered.",
        vec![
            kpi("Deployments", &deployments.read("/count")),
            kpi_count("Declared", &deployments.read("/deployments")),
        ],
    );

    let declared = unanswered(deployments).unwrap_or_else(|| {
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

    let placeholder = el("section")
        .class("mj-ui-panel")
        .attr("aria-label", "Delivery")
        .attr("data-panel-src", DELIVERY_PANEL)
        .attr("aria-busy", "false")
        .child(
            el("header").class("mj-ui-panel-head").child(
                el("div")
                    .class("mj-ui-panel-heading")
                    .child(el("h2").class("mj-ui-panel-title").text("Delivery"))
                    .child(el("p").class("mj-ui-panel-sub").text(
                        "Asked on its own: delivery.report probes the public site and judges every feature, which would hold this screen back.",
                    )),
            ),
        )
        .child(
            el("p").class("mj-ui-panel-body").child(
                el("a").class("mj-ui-link").attr("href", DELIVERY_PANEL).text("Open the delivery report"),
            ),
        );

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Declared deployments",
            "Where the product is declared to run.",
            declared,
            &[deployments],
        )))
        .child(wide(placeholder));

    View::new(
        "Deployments",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The delivery report as a panel: the publication's verification and every feature's stage.
pub fn delivery_panel(ctx: &Context) -> View {
    View::new("Delivery", delivery(&ask(ctx, DELIVERY)))
}

fn delivery(report: &Answer) -> El {
    let body = unanswered(report).unwrap_or_else(|| {
        let tallies = strip(vec![
            kpi("Features", &report.read("/tallies/features")),
            kpi("On the trunk", &report.read("/tallies/on_master")),
            kpi("Deployed", &report.read("/tallies/deployed")),
            kpi(
                "Publicly verified",
                &report.read("/tallies/publicly_verified"),
            ),
        ]);
        let publication = el("p")
            .class("mj-ui-line")
            .child(state(
                report
                    .ok()
                    .and_then(|v| v.pointer("/publication/verification"))
                    .unwrap_or(&Value::Null),
                "state",
            ))
            .text(format!(
                " {} — {}",
                report
                    .read("/publication/url")
                    .text()
                    .unwrap_or_else(|| "no public address".into()),
                report
                    .read("/publication/verification/detail")
                    .text()
                    .unwrap_or_default()
            ));
        let rows = list(report, "/features")
            .iter()
            .map(|f| {
                let blocking: Vec<String> = f
                    .pointer("/delivery/blocking")
                    .and_then(Value::as_array)
                    .map(|b| {
                        b.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                el("tr")
                    .child(
                        el("td")
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(f, "id")))
                            .child(el("span").class("mj-ui-cell-title").text(text(f, "title"))),
                    )
                    .child(
                        el("td").child(state(f.get("delivery").unwrap_or(&Value::Null), "stage")),
                    )
                    .child(
                        el("td").child(state(f.get("delivery").unwrap_or(&Value::Null), "state")),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(blocking.join(", ")),
                    )
            })
            .collect::<Vec<_>>();
        el("div")
            .class("mj-ui-stack")
            .child(tallies)
            .child(publication)
            .child(table(
                &["Feature", "Stage", "Delivery", "What blocks it"],
                rows,
                "No feature is declared.",
            ))
    });
    panel_to(
        "Delivery",
        "Whether the public site serves the trunk, and how far each feature has been delivered.",
        None,
        body,
        &[report],
    )
}
