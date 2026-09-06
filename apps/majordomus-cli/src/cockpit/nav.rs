//! Navigation, derived. The sidebar's catalogues are not a list in this file: the
//! capability modules come from the registry, the object kinds come from the index, and
//! the graphs come from the derivations. A module added to `compose_modules!`, a kind
//! added to `share/kinds.yaml`, a graph added to the derivation table — each appears here
//! with no edit to the Cockpit.
//!
//! The Cockpit's own pages are declared once, in [`PAGES`]: the path each answers, the
//! label navigation shows for it, the area it belongs to and what has to be running for it
//! to answer. They were written twice — a dispatch table in `mod.rs` and a list of
//! navigation items here — and the second copy drifted exactly as a second copy does:
//! `search`, `activity` and the topology graph were reachable and unnamed. One declaration
//! and a case that walks it is why that cannot recur.
//!
//! Whether an entry is offered as a destination is not a decision a template makes. It
//! comes from the page's [`Availability`] against the [`Environment`] doing the rendering:
//! a running process answers what it declares, a published build answers only what needs
//! no process, and a page it cannot answer is named without being linked.

use crate::capability::registry::ModuleSource;
use crate::capability::{Availability, Context};
use crate::graph;
use crate::http::router::percent_encode;

/// One of the Cockpit's areas. A page declares the area it belongs to and the navigation
/// marks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// The landing page.
    Overview,
    /// The capability explorer and the runner.
    Capabilities,
    /// The declarative objects of the layer.
    Objects,
    /// The layer's directory contracts and their hierarchy.
    Directories,
    /// The derived graphs.
    Graphs,
    /// What this checkout's lifecycle is holding.
    Continuity,
    /// The health report.
    Health,
    /// The HTTP and MCP surfaces.
    Api,
    /// A page that belongs to no area (search results, an error).
    None,
}

/// Where the navigation is being rendered, and therefore what it can offer.
///
/// The distinction is not cosmetic. The published site has no process behind it, so a
/// link to a page only a process answers is a promise the site cannot keep; the Cockpit
/// served by that process can keep every one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// A running process is rendering, and answers what it declares.
    Served,
    /// A build is rendering, and there is no process behind the result.
    Published,
}

impl Environment {
    /// Can this environment answer a page of that availability?
    ///
    /// ```
    /// use majordomus_cli::capability::Availability;
    /// use majordomus_cli::cockpit::nav::Environment;
    /// assert!(Environment::Served.offers(Availability::Runtime));
    /// assert!(!Environment::Published.offers(Availability::Runtime));
    /// assert!(Environment::Published.offers(Availability::Always));
    /// ```
    pub fn offers(self, availability: Availability) -> bool {
        match (self, availability) {
            // a caller this process has authenticated is not something either environment
            // can promise from the navigation alone
            (_, Availability::Authenticated) => false,
            (Environment::Served, _) => true,
            (Environment::Published, Availability::Always | Availability::BuildTime) => true,
            (Environment::Published, Availability::Runtime) => false,
        }
    }
}

/// One page the Cockpit serves.
#[derive(Debug, Clone, Copy)]
pub struct PageEntry {
    /// The path it answers, without a trailing slash.
    pub path: &'static str,
    /// What navigation calls it, or `None` for a page that is reachable and not offered:
    /// a search result, an error, a view reached from the thing it is about.
    pub label: Option<&'static str>,
    /// The area it belongs to.
    pub area: Area,
    /// What has to be running for it to answer.
    pub availability: Availability,
}

/// Every page the Cockpit serves, declared once. The dispatch in
/// [`Cockpit::route`](super::Cockpit) answers these paths and a case walks this table
/// against it, so a page that is served and unnamed, or named and unserved, fails rather
/// than quietly existing.
///
/// The order is the order the navigation shows: it is the order of the Cockpit's own
/// argument, from what exists to what it is doing, and it is a constant rather than a
/// sort so that it is the same in every build.
pub const PAGES: &[PageEntry] = &[
    PageEntry {
        path: "/cockpit",
        label: Some("Overview"),
        area: Area::Overview,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/capabilities",
        label: Some("Capabilities"),
        area: Area::Capabilities,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/objects",
        label: Some("Objects"),
        area: Area::Objects,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/directories",
        label: Some("Directories"),
        area: Area::Directories,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/graphs",
        label: Some("Graphs"),
        area: Area::Graphs,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/continuity",
        label: Some("Continuity"),
        area: Area::Continuity,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/health",
        label: Some("Health"),
        area: Area::Health,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/api",
        label: Some("API"),
        area: Area::Api,
        availability: Availability::Runtime,
    },
    // the page says Overview and the page is the authority: the case that compares the two
    // caught this table claiming Continuity, which was a guess about where it belongs
    PageEntry {
        path: "/cockpit/activity",
        label: Some("Activity"),
        area: Area::Overview,
        availability: Availability::Runtime,
    },
    // reached from the thing it is about, never from a menu
    PageEntry {
        path: "/cockpit/object",
        label: None,
        area: Area::Objects,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/graphs/topology",
        label: None,
        area: Area::Graphs,
        availability: Availability::Runtime,
    },
    PageEntry {
        path: "/cockpit/search",
        label: None,
        area: Area::None,
        availability: Availability::Runtime,
    },
];

/// One entry.
#[derive(Debug, Clone)]
pub struct Item {
    /// What the reader sees.
    pub label: String,
    /// Where it goes.
    pub href: String,
    /// The area it belongs to.
    pub area: Area,
    /// How many things are behind it, when the number is a fact and not decoration.
    pub count: Option<usize>,
    /// Whether this is the page being shown.
    pub current: bool,
    /// Whether the environment rendering this navigation can answer the page. An entry
    /// that is false is named and not linked; it is never silently dropped, because a
    /// reader who cannot see a surface exists cannot ask for it.
    pub available: bool,
}

/// A heading and its entries.
#[derive(Debug, Clone)]
pub struct Section {
    /// The heading.
    pub title: String,
    /// The entries, in a deterministic order.
    pub items: Vec<Item>,
}

/// The whole navigation for one request.
#[derive(Debug, Clone)]
pub struct Navigation {
    sections: Vec<Section>,
}

impl Navigation {
    /// The sections, in order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }
}

/// Build the navigation for a request served by a running process. `here` is the request
/// path, so the current entry can be marked without a page saying which it is.
pub fn build(ctx: &Context, here: &str) -> Navigation {
    build_in(ctx, here, Environment::Served)
}

/// Build the navigation for one environment: the Cockpit's own pages from [`PAGES`], then
/// the catalogues derived from the registry, the index and the graph derivations.
///
/// A published build calls this with [`Environment::Published`] and gets the same entries
/// with the ones no build can answer marked unavailable — the same model, one field
/// different, rather than a second navigation written for the site.
pub fn build_in(ctx: &Context, here: &str, env: Environment) -> Navigation {
    let areas = Section {
        title: "Cockpit".into(),
        items: PAGES
            .iter()
            .filter_map(|page| {
                page.label.map(|label| Item {
                    label: label.into(),
                    href: page.path.into(),
                    area: page.area,
                    count: count(ctx, page.area),
                    current: here == page.path,
                    available: env.offers(page.availability),
                })
            })
            .collect(),
    };

    // the executable's own modules: what a capability belongs to, from the registry
    let modules = Section {
        title: "Capability modules".into(),
        items: ctx
            .registry
            .modules()
            .filter(|m| m.source != ModuleSource::Declarative)
            .map(|m| Item {
                label: m.title.clone(),
                href: format!(
                    "/cockpit/capabilities?module={}",
                    percent_encode(m.id.as_str())
                ),
                area: Area::Capabilities,
                count: Some(m.capabilities),
                current: false,
                available: env.offers(Availability::Runtime),
            })
            .collect(),
    };

    // the kinds of the layer: what the repository declares, not what this code knows
    let kinds = Section {
        title: "Object kinds".into(),
        items: ctx
            .index
            .kinds()
            .into_iter()
            .map(|(kind, count)| Item {
                label: kind.to_string(),
                href: format!("/cockpit/objects?kind={}", percent_encode(kind)),
                area: Area::Objects,
                count: Some(count),
                current: false,
                available: env.offers(Availability::Runtime),
            })
            .collect(),
    };

    let graphs = Section {
        title: "Graphs".into(),
        items: graph::ids()
            .into_iter()
            .map(|id| Item {
                label: id.to_string(),
                href: format!("/cockpit/graphs/{id}"),
                area: Area::Graphs,
                count: None,
                current: here == format!("/cockpit/graphs/{id}"),
                available: env.offers(Availability::Runtime),
            })
            .collect(),
    };

    Navigation {
        sections: [areas, modules, kinds, graphs]
            .into_iter()
            .filter(|s| !s.items.is_empty())
            .collect(),
    }
}

/// How many things an area holds, when the number is a fact the context already knows.
/// Nothing here counts by walking a page; every number is one the registry or the index
/// answered for its own reasons.
fn count(ctx: &Context, area: Area) -> Option<usize> {
    match area {
        Area::Capabilities => Some(ctx.registry.summary().total),
        Area::Objects => Some(ctx.index.objects.len()),
        Area::Graphs => Some(graph::ids().len()),
        Area::Api => Some(ctx.registry.summary().http_routes),
        Area::Overview | Area::Directories | Area::Continuity | Area::Health | Area::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    fn repository() -> SyntheticRepository {
        SyntheticRepository::small().expect("a synthetic repository")
    }

    #[test]
    fn the_module_catalogue_is_the_registrys_and_not_a_list_here() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit");
        let modules = nav
            .sections()
            .iter()
            .find(|s| s.title == "Capability modules")
            .expect("a module section");
        let labels: Vec<&str> = modules.items.iter().map(|i| i.label.as_str()).collect();
        let expected = ctx
            .registry
            .modules()
            .filter(|m| m.source != ModuleSource::Declarative)
            .count();
        assert_eq!(labels.len(), expected);
        assert!(expected > 0, "the builtin registry composes modules");
    }

    #[test]
    fn the_current_page_is_marked_once() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit/health");
        let current: Vec<&Item> = nav
            .sections()
            .iter()
            .flat_map(|s| &s.items)
            .filter(|i| i.current)
            .collect();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].label, "Health");
    }

    #[test]
    fn the_kind_catalogue_is_the_indexs_and_not_a_list_here() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit");
        let kinds = nav
            .sections()
            .iter()
            .find(|s| s.title == "Object kinds")
            .expect("a kind section");
        let labels: Vec<&str> = kinds.items.iter().map(|i| i.label.as_str()).collect();
        let expected: Vec<&str> = ctx.index.kinds().into_keys().collect();
        assert_eq!(labels, expected);
    }
    #[test]
    fn the_areas_are_the_page_table_and_not_a_list_in_build() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit");
        let areas = nav
            .sections()
            .iter()
            .find(|s| s.title == "Cockpit")
            .expect("the areas section");
        let offered: Vec<&str> = PAGES.iter().filter_map(|p| p.label).collect();
        let shown: Vec<&str> = areas.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(shown, offered);
        let hrefs: Vec<&str> = areas.items.iter().map(|i| i.href.as_str()).collect();
        let paths: Vec<&str> = PAGES
            .iter()
            .filter(|p| p.label.is_some())
            .map(|p| p.path)
            .collect();
        assert_eq!(hrefs, paths);
    }

    /// The table and the dispatch are two readings of one set. This is the case that keeps
    /// them one: every page declared here is a path the Cockpit owns and answers with a
    /// page of the area it claims, so a page served and unnamed, or named and unserved,
    /// fails here rather than existing quietly.
    #[test]
    fn every_declared_page_is_a_page_the_cockpit_actually_serves() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let cockpit = crate::cockpit::Cockpit::new(ctx.clone(), "test", None);
        for page in PAGES {
            assert!(
                crate::cockpit::Cockpit::owns(page.path),
                "{} is declared and the Cockpit does not own it",
                page.path
            );
            let req = crate::http::router::Request::parse_target("GET", page.path, vec![]);
            let rendered = cockpit.route(&req);
            assert_ne!(
                rendered.status, 404,
                "{} is declared and answers 404",
                page.path
            );
            assert_eq!(
                rendered.area, page.area,
                "{} answers with an area the table does not claim",
                page.path
            );
        }
    }

    #[test]
    fn the_order_is_the_same_on_every_build() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let once = build(&ctx, "/cockpit");
        let twice = build(&ctx, "/cockpit");
        let flatten = |n: &Navigation| -> Vec<(String, String)> {
            n.sections()
                .iter()
                .flat_map(|s| {
                    s.items
                        .iter()
                        .map(|i| (s.title.clone(), i.href.clone()))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        assert_eq!(flatten(&once), flatten(&twice));
        assert!(!flatten(&once).is_empty());
    }

    #[test]
    fn a_published_build_names_what_it_cannot_answer_and_does_not_link_it() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let served = build_in(&ctx, "/cockpit", Environment::Served);
        let published = build_in(&ctx, "/cockpit", Environment::Published);

        let labels = |n: &Navigation| -> Vec<String> {
            n.sections()
                .iter()
                .flat_map(|s| s.items.iter().map(|i| i.label.clone()))
                .collect()
        };
        // the same entries in both: a surface is named either way
        assert_eq!(labels(&served), labels(&published));

        assert!(served
            .sections()
            .iter()
            .flat_map(|s| &s.items)
            .all(|i| i.available));
        assert!(published
            .sections()
            .iter()
            .flat_map(|s| &s.items)
            .all(|i| !i.available));
    }
}
