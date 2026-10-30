//! The front door: what `/` answers with when the process serves the UI. It is a screen of
//! the UI, in its frame, so a person who opens the process lands where every other screen
//! is one click away.
//!
//! Nothing on it is written here. The facts about the process are the router's (the version
//! of this executable, the revision the repository is at), the release is
//! `distribution.releases`'s and the capability count `repository.info`'s, the screens are
//! [`super::SCREENS`], and the surfaces are the web topology's, by [`crate::web::home::entries`]
//! — the same reading the plain page draws for a process that does not serve the UI. A static
//! surface whose producer has not run is shown with the command that would make it, never as
//! a link to a certain 404.

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;
use crate::web::home::{Availability, Entry};
use crate::web::model::Category;

use super::icons::icon;
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{hero, kpi, kpi_to};
use super::SCREENS;

/// The route the front door answers.
pub const ROUTE: &str = "/";

const REPOSITORY: Source = Source::of("repository.info");
const RELEASES: Source = Source::of("distribution.releases");

/// What the front door says about the process: facts the router holds and no capability
/// answers.
#[derive(Debug, Clone)]
pub struct Process<'a> {
    /// The version of this executable.
    pub version: &'a str,
    /// The repository's name.
    pub repository: &'a str,
    /// The revision the repository is at, when version control could say.
    pub revision: Option<&'a str>,
    /// The one-line description of what this is.
    pub summary: &'a str,
    /// The line naming the checkout that answered, as the plain page draws it
    /// ([`crate::web::home::Checkout::line`]): markup this process made, every value escaped,
    /// so that a front door read in a browser says whose answers it holds (I1031).
    pub checkout: String,
}

/// A tile of the hero for a fact the process holds itself, cited as such.
fn fact(label: &str, value: &str, source: &str) -> El {
    el("span")
        .class("mj-ui-kpi")
        .attr("title", format!("read from {source}"))
        .child(el("span").class("mj-ui-kpi-label").text(label))
        .child(el("span").class("mj-ui-kpi-value").text(value))
        .child(el("span").class("mj-ui-kpi-source").text(source))
}

/// The tone of a screen's glyph, by its place in the navigation: the hero tones, in turn.
fn tone(i: usize) -> &'static str {
    const TONES: &[&str] = &["blue", "violet", "cyan", "green", "orange", "rose"];
    TONES[i % TONES.len()]
}

/// One card that leads somewhere: a glyph, a kicker, a title and a line.
fn card(href: &str, glyph: &str, tone: &str, kicker: &str, title: &str, line: &str) -> El {
    el("li").child(
        el("a")
            .class("mj-ui-card mj-ui-card--link")
            .attr("href", href)
            .child(
                el("span")
                    .class("mj-ui-card-head")
                    .child(
                        el("span")
                            .class(format!("mj-ui-card-glyph mj-tone--{tone}"))
                            .child(icon(glyph)),
                    )
                    .child(
                        el("span")
                            .class("mj-ui-card-kicker mj-ui-mono")
                            .text(kicker),
                    ),
            )
            .child(el("strong").class("mj-ui-card-title").text(title))
            .child(el("span").class("mj-ui-card-text").text(line)),
    )
}

/// The glyph and the tone of a category of surfaces.
fn category_glyph(category: Category) -> (&'static str, &'static str) {
    match category {
        Category::Interface => ("monitor", "blue"),
        Category::Documentation => ("book", "violet"),
        Category::Api => ("code", "cyan"),
        Category::Protocol => ("nodes", "green"),
        Category::Report => ("file-check", "orange"),
    }
}

/// One surface of the topology: a card that leads to it when it answers, and one that says
/// what would make it when it does not.
fn surface(category: Category, e: &Entry) -> El {
    match e.state {
        Availability::Ready => {
            let (glyph, tone) = category_glyph(category);
            card(&e.href, glyph, tone, &e.href, &e.id, &e.title)
        }
        Availability::NotBuilt => el("li").child(
            el("div")
                .class("mj-ui-card")
                .child(
                    el("span")
                        .class("mj-ui-card-head")
                        .child(
                            el("span")
                                .class("mj-ui-card-glyph")
                                .child(icon(category_glyph(category).0)),
                        )
                        .child(
                            el("span")
                                .class("mj-ui-card-kicker mj-ui-mono")
                                .text(&e.href),
                        )
                        .child(word_badge("not built")),
                )
                .child(el("strong").class("mj-ui-card-title").text(&e.id))
                .child(el("span").class("mj-ui-card-text").text(&e.title))
                .child(
                    el("span")
                        .class("mj-ui-card-text")
                        .text("Build it: ")
                        .child(el("code").class("mj-ui-mono").text(&e.producer)),
                ),
        ),
    }
}

/// The front door.
pub fn screen(ctx: &Context, process: &Process<'_>, sections: &[(Category, Vec<Entry>)]) -> View {
    let answers = ask_all(ctx, &[REPOSITORY, RELEASES]);
    let (repo, releases) = (&answers[&REPOSITORY], &answers[&RELEASES]);
    let revision = process
        .revision
        .map(|r| r[..12.min(r.len())].to_string())
        .unwrap_or_else(|| "unknown".into());
    let surfaces: usize = sections.iter().map(|(_, s)| s.len()).sum();
    // the summary is the product's sentence; the hero's line is what this page is for

    let top = hero(
        "home",
        "blue",
        "Majordomus",
        "Every screen of the UI, and everything this process serves, one click away.",
        &format!("{} {} at {revision}.", process.summary, process.repository),
        vec![
            fact("This executable", process.version, "this process"),
            kpi_to(
                kpi("Released", &releases.read("/latest/version")),
                super::delivery::ROUTE,
            ),
            kpi_to(
                kpi("Capabilities", &repo.read("/capabilities/total")),
                super::capabilities::ROUTE,
            ),
            fact("Revision", &revision, "version control"),
            fact("Surfaces", &surfaces.to_string(), "the web topology"),
        ],
    );

    let screens = el("section")
        .class("mj-ui-panel")
        .attr("aria-labelledby", "mj-ui-front-screens")
        .child(
            el("header").class("mj-ui-panel-head").child(
                el("div")
                    .class("mj-ui-panel-heading")
                    .child(
                        el("h2")
                            .attr("id", "mj-ui-front-screens")
                            .class("mj-ui-panel-title")
                            .text("Start here"),
                    )
                    .child(
                        el("p")
                            .class("mj-ui-panel-sub")
                            .text("Every screen of majordomus ui, and the question it answers."),
                    ),
            ),
        )
        .child(
            el("ul").class("mj-ui-cards").children(
                SCREENS
                    .iter()
                    .enumerate()
                    .map(|(i, s)| card(s.route, s.icon, tone(i), s.route, s.title, s.answers)),
            ),
        );

    let served = el("section")
        .class("mj-ui-panel")
        .attr("aria-labelledby", "mj-ui-front-served")
        .child(
            el("header").class("mj-ui-panel-head").child(
                el("div")
                    .class("mj-ui-panel-heading")
                    .child(
                        el("h2")
                            .attr("id", "mj-ui-front-served")
                            .class("mj-ui-panel-title")
                            .text("Everything this process serves"),
                    )
                    .child(el("p").class("mj-ui-panel-sub").text(
                        "Discovered from the web topology, not listed; majordomus web explain says where each came from.",
                    )),
            ),
        )
        .children(sections.iter().map(|(category, entries)| {
            el("div")
                .class("mj-ui-stack")
                .child(
                    el("h3")
                        .class("mj-ui-panel-title")
                        .text(category.title()),
                )
                .child(
                    el("ul").class("mj-ui-cards").children(
                        entries
                            .iter()
                            // this page is one of the surfaces; a card leading to itself is not
                            .filter(|e| e.href != ROUTE)
                            .map(|e| surface(*category, e)),
                    ),
                )
        }));

    View::new(
        "Majordomus",
        el("div")
            .class("mj-ui-stack")
            .child(top)
            .raw(process.checkout.clone())
            .child(screens)
            .child(served),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    fn sections() -> Vec<(Category, Vec<Entry>)> {
        let e = |id: &str, href: &str, state| Entry {
            id: id.into(),
            title: format!("{id} title"),
            href: href.into(),
            state,
            producer: format!("scripts/build-{id}"),
        };
        vec![
            (
                Category::Interface,
                vec![
                    e("ui", "/ui", Availability::Ready),
                    e("cockpit", "/cockpit", Availability::Ready),
                    e("home", "/", Availability::Ready),
                ],
            ),
            (
                Category::Documentation,
                vec![e("docs", "/docs/", Availability::NotBuilt)],
            ),
        ]
    }

    /// The front door leads to every screen of the UI and every surface that answers, says
    /// what would build one that does not, and carries the version it was given.
    #[test]
    fn the_front_door_leads_to_every_screen_and_every_surface() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let process = Process {
            version: "9.8.7",
            repository: "fixture",
            revision: Some("0123456789abcdef"),
            summary: "What this is.",
            checkout: String::from("<p class=\"mj-checkout\">checkout 0123456789ab</p>"),
        };
        let html = screen(&ctx, &process, &sections()).main.render();
        for s in SCREENS {
            assert!(
                html.contains(&format!("href=\"{}\"", s.route)),
                "no link to {}: {html}",
                s.route
            );
        }
        for want in [
            "9.8.7",
            "0123456789ab",
            "href=\"/cockpit\"",
            "scripts/build-docs",
            "Everything this process serves",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(
            !html.contains("href=\"/docs/\""),
            "a surface not built is no link: {html}"
        );
        assert!(
            !html.contains("href=\"/\""),
            "the front door does not lead to itself: {html}"
        );
    }
}
