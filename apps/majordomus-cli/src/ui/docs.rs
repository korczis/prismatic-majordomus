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
use super::view::{hero, kpi, kpi_count, list, panel, table, text, unanswered, wide};

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
                el("li")
                    .class("mj-ui-card")
                    .child(
                        el("span")
                            .class("mj-ui-card-kicker mj-ui-mono")
                            .text(text(f, "identity")),
                    )
                    .child(
                        el("strong")
                            .class("mj-ui-card-title")
                            .text(text(f, "title")),
                    )
                    .child(
                        el("span")
                            .class("mj-ui-card-text")
                            .text(text(f, "description")),
                    )
            }))
    });

    let documents_body = unanswered(documents).unwrap_or_else(|| {
        let rows = list(documents, "/objects")
            .iter()
            .filter(|d| text(d, "path").starts_with("docs/"))
            .take(ROWS * 6)
            .map(|d| {
                el("tr")
                    .child(el("td").class("mj-ui-cell-title").text(text(d, "title")))
                    .child(el("td").class("mj-ui-mono mj-ui-dim").text(text(d, "path")))
            })
            .collect::<Vec<_>>();
        table(
            &["Document", "Where it lives"],
            rows,
            "The repository holds no document under docs/.",
        )
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
                    .child(el("td").class("mj-ui-dim").text(text(s, "availability")))
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
