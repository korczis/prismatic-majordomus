//! Documentation: what the product does and where it is written down. Read from
//! `objects.list` (the product features and the documents of the layer) and `web.surfaces`
//! (where each surface is served or published).
//!
//! A document is named by its path, which is where it lives; the published site renders the
//! same files, and the surface that serves them is listed beside them.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::ROWS;
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{
    hero, kpi, kpi_count, list, more, object_or, panel, table, text, unanswered, value, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/docs";

const FEATURES: Source = Source::with("objects.list", r#"{"kind":"feature"}"#);
const DOCUMENTS: Source = Source::with("objects.list", r#"{"kind":"document"}"#);
const SURFACES: Source = Source::of("web.surfaces");

/// The Documentation screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[FEATURES, DOCUMENTS, SURFACES]);
    let (features, documents, surfaces) = (
        &answers[&FEATURES],
        &answers[&DOCUMENTS],
        &answers[&SURFACES],
    );

    let top = hero(
        "book",
        "cyan",
        "Documentation",
        "What the product does, and where it is written down.",
        "Every product feature as the layer declares it, every document of the repository, and every surface that serves or publishes them.",
        vec![
            kpi("Features", &features.read("/count")),
            kpi("Documents", &documents.read("/count")),
            kpi_count("Surfaces", &surfaces.read("/surfaces")),
        ],
    );

    let feature_list = unanswered(features).unwrap_or_else(|| {
        el("ul")
            .class("mj-ui-cards")
            .children(list(features, "/objects").iter().map(|f| {
                // a feature objects.list names is an object of the index: its technical view
                let identity = text(f, "identity");
                el("li")
                    .class("mj-ui-card")
                    .attr("id", format!("feature-{}", super::view::anchor(&identity)))
                    .child(object_or(&text(f, "uri"), &identity).class("mj-ui-card-kicker"))
                    .child(
                        el("strong")
                            .class("mj-ui-card-title")
                            .text(value(f, "title")),
                    )
                    .child(
                        el("span")
                            .class("mj-ui-card-text")
                            .text(value(f, "description")),
                    )
            }))
    });

    let documents_body = unanswered(documents).unwrap_or_else(|| {
        let under_docs: Vec<serde_json::Value> = list(documents, "/objects")
            .into_iter()
            .filter(|d| text(d, "path").starts_with("docs/"))
            .collect();
        let rows = under_docs
            .iter()
            .take(ROWS * 6)
            .map(|d| {
                // a document objects.list names is an object of the index: its technical view
                el("tr")
                    .child(el("td").class("mj-ui-cell-title").text(value(d, "title")))
                    .child(el("td").child(object_or(&text(d, "uri"), &text(d, "path"))))
            })
            .collect::<Vec<_>>();
        el("div")
            .child(table(
                &["Document", "Where it lives"],
                rows,
                "The repository holds no document under docs/.",
            ))
            .children(more(
                (ROWS * 6).min(under_docs.len()),
                under_docs.len(),
                None,
            ))
    });

    let surfaces_body = unanswered(surfaces).unwrap_or_else(|| {
        let rows = list(surfaces, "/surfaces")
            .iter()
            .filter(|s| text(s, "visibility") == "public")
            .map(|s| {
                let mount = text(s, "mount");
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-link")
                                .attr("href", mount.clone())
                                .text(text(s, "title")),
                        ),
                    )
                    .child(el("td").class("mj-ui-mono").text(mount))
                    .child(el("td").class("mj-ui-dim").text(value(s, "availability")))
            })
            .collect::<Vec<_>>();
        table(
            &["Surface", "Mount", "Where"],
            rows,
            "No public surface is declared.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "What the product does",
            "Each feature as one file of the layer declares it.",
            feature_list,
            &[features],
        )))
        .child(panel(
            "Documents",
            "The project's documents under docs/, by title.",
            documents_body,
            &[documents],
        ))
        .child(panel(
            "Surfaces",
            "Every public surface of the topology and where it is served.",
            surfaces_body,
            &[surfaces],
        ));

    View::new(
        "Documentation",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::deployments::tests::furnished;

    /// Every feature the layer declares is a card with its identity and its title, as the
    /// one file that declares it says.
    #[test]
    fn every_declared_feature_is_a_card() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(html.contains("mj-ui-card"), "{html}");
        assert!(html.contains(">synthetic<"), "{html}");
        assert!(html.contains("A synthetic feature"), "{html}");
        assert!(
            !html.contains("objects.list could not answer"),
            "the furnished repository has the kind: {html}"
        );
    }

    /// A feature and a document lead to their technical views by the URI the index gives
    /// them, and a feature's card is a place a link can lead to.
    #[test]
    fn a_feature_and_a_document_lead_to_their_technical_views() {
        let repo = furnished();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(html.contains("id=\"feature-synthetic\""), "{html}");
        assert!(
            html.contains("href=\"/cockpit/object?uri=majordomus%3A%2F%2Ffeature%2Fsynthetic\""),
            "{html}"
        );
        assert!(
            html.contains("href=\"/cockpit/object?uri=majordomus%3A%2F%2Fdocument%2Fdocs%2F"),
            "a document under docs/ leads to its technical view: {html}"
        );
    }
}
