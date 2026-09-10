//! Navigation, derived. The sidebar's catalogues are not a list in this file: the
//! capability modules come from the registry, the object kinds come from the index, and
//! the graphs come from the derivations. A module added to `compose_modules!`, a kind
//! added to `share/kinds.yaml`, a graph added to the derivation table — each appears here
//! with no edit to the Cockpit.
//!
//! What *is* written here is the areas: Overview, Capabilities, Commands, Executions,
//! Objects, Directories, Graphs, Continuity, Worktrees, Health, Quality, Artifacts,
//! Design, API. Those are concepts rather than
//! entities, they change when the Cockpit's own shape changes, and deriving them from
//! anything would be deriving them from a list of exactly themselves.
//!
//! Every section is presented in the canonical order (`crate::order`). The order a section
//! is *built* in is an accident of its source — the order the modules were composed in, the
//! order the graph derivations are declared in — and an accident is not a reading order.
//! The order is applied to the sections as a whole, so a section added later is ordered
//! without being told to be.
//!
//! One section is grouped rather than flat: the capability modules sit under the
//! operational area they serve. That area is not written here either. A feature declares
//! the modules it is built from and the areas it serves; the product model resolves the two
//! into an area per module, and this file asks for it. A module no feature places is shown
//! under no heading, which the canonical order puts last, and the product model reports it.

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
    /// Every command of every program here, and where each one is projected.
    Commands,
    /// The declarative objects of the layer.
    Objects,
    /// The layer's directory contracts and their hierarchy.
    Directories,
    /// The derived graphs.
    Graphs,
    /// The executions this process is running and remembers.
    Executions,
    /// What this checkout's lifecycle is holding.
    Continuity,
    /// The branch-to-worktree topology of the repository.
    Worktrees,
    /// The health report.
    Health,
    /// What the crate's own public surface is held to.
    Quality,
    /// What the generator writes.
    Artifacts,
    /// The design system: what every surface of this tool is rendered with.
    Design,
    /// The HTTP and MCP surfaces.
    Api,
    /// A page that belongs to no area (search results, an error).
    None,
}

/// One of the Cockpit's areas as data: what a person sees, where it goes, and the area it
/// marks. The one list of areas there is; [`build`] reads it for the sidebar and the
/// product model reads it to validate the areas a feature names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaInfo {
    /// The id a feature names it by: the last segment of its route.
    pub id: &'static str,
    /// What the reader sees.
    pub label: &'static str,
    /// Where it goes.
    pub href: &'static str,
    /// The area it marks.
    pub area: Area,
}

/// The areas. Written here because they are concepts rather than entities; every catalogue
/// under them is derived.
///
/// Not the order the sidebar shows them in: `build` puts every section through the
/// canonical order, so this sequence reaches no reader. It is the set the product model
/// validates against, and nothing more.
pub fn areas() -> &'static [AreaInfo] {
    &[
        AreaInfo {
            id: "overview",
            label: "Overview",
            href: "/cockpit",
            area: Area::Overview,
        },
        AreaInfo {
            id: "capabilities",
            label: "Capabilities",
            href: "/cockpit/capabilities",
            area: Area::Capabilities,
        },
        AreaInfo {
            id: "commands",
            label: "Commands",
            href: "/cockpit/commands",
            area: Area::Commands,
        },
        AreaInfo {
            id: "objects",
            label: "Objects",
            href: "/cockpit/objects",
            area: Area::Objects,
        },
        AreaInfo {
            id: "directories",
            label: "Directories",
            href: "/cockpit/directories",
            area: Area::Directories,
        },
        AreaInfo {
            id: "graphs",
            label: "Graphs",
            href: "/cockpit/graphs",
            area: Area::Graphs,
        },
        AreaInfo {
            id: "continuity",
            label: "Continuity",
            href: "/cockpit/continuity",
            area: Area::Continuity,
        },
        AreaInfo {
            id: "worktrees",
            label: "Worktrees",
            href: "/cockpit/worktrees",
            area: Area::Worktrees,
        },
        AreaInfo {
            id: "health",
            label: "Health",
            href: "/cockpit/health",
            area: Area::Health,
        },
        AreaInfo {
            id: "quality",
            label: "Quality",
            href: "/cockpit/quality",
            area: Area::Quality,
        },
        AreaInfo {
            id: "artifacts",
            label: "Artifacts",
            href: "/cockpit/artifacts",
            area: Area::Artifacts,
        },
        AreaInfo {
            id: "design",
            label: "Design",
            href: "/cockpit/design",
            area: Area::Design,
        },
        AreaInfo {
            id: "api",
            label: "API",
            href: "/cockpit/api",
            area: Area::Api,
        },
    ]
}

/// One entry.
#[derive(Debug, Clone)]
pub struct Item {
    /// What the reader sees.
    pub label: String,
    /// Where it goes.
    pub href: String,
    /// The area it belongs to.
    pub area: Area,
    /// The heading it sits under inside its section, when the section is grouped. Derived,
    /// never written here: the capability modules are grouped by the operational area the
    /// features that name them serve.
    pub group: Option<String>,
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
    let summary = ctx.registry.summary();
    // the counts are facts of this context, decided per area: how many things are behind
    // an entry is not part of what an area is
    let count = |a: Area| -> Option<usize> {
        match a {
            Area::Capabilities => Some(summary.total),
            Area::Objects => Some(ctx.index.objects.len()),
            Area::Graphs => Some(graph::ids().len()),
            Area::Api => Some(summary.http_routes),
            _ => None,
        }
    };
    let areas = Section {
        title: "Cockpit".into(),
        items: areas()
            .iter()
            .map(|a| item(a.label, a.href, a.area, count(a.area), here))
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
                // The module's own area, derived by the product model from the features
                // that name it, shown under the name the Why catalogue gives it. Neither
                // the grouping nor the heading is written in this file.
                group: ctx
                    .product
                    .module_area(m.id.as_str())
                    .and_then(|id| ctx.why.areas().iter().find(|a| a.id == id))
                    .map(|a| a.title.clone()),
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
                group: None,
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
                group: None,
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
            .map(alphabetical)
            .collect(),
    }
}

/// One section's entries in the canonical order: by label, case-folded so that `API` sits
/// with `Artifacts` rather than ahead of every lowercase kind, digit runs by value, and the
/// href behind it to break a tie between two entries a reader would call the same.
///
/// The comparator is `crate::order`'s, not this file's. It was this file's, it was the only
/// case-folded comparator in the crate, and every other surface sorted by raw bytes instead.
fn alphabetical(mut section: Section) -> Section {
    crate::order::canonical(&mut section.items);
    section
}

impl crate::order::Ordered for Item {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        match &self.group {
            Some(group) => crate::order::OrderKey::grouped(group, &self.label, &self.href),
            None => crate::order::OrderKey::plain(&self.label, &self.href),
        }
    }
}

fn item(label: &str, href: &str, area: Area, count: Option<usize>, here: &str) -> Item {
    Item {
        label: label.into(),
        href: href.into(),
        area,
        group: None,
        count,
        current: here == href,
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
    fn a_grouped_section_keeps_its_groups_contiguous_and_the_ungrouped_last() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit");
        for section in nav.sections() {
            let mut seen: Vec<&str> = Vec::new();
            let mut ungrouped = false;
            for item in &section.items {
                match item.group.as_deref() {
                    Some(g) => {
                        assert!(
                            !ungrouped,
                            "a grouped entry follows an ungrouped one in '{}': the canonical \
                             order puts the ungrouped tail last",
                            section.title
                        );
                        if seen.last().copied() != Some(g) {
                            assert!(
                                !seen.contains(&g),
                                "the group '{g}' appears twice in '{}': its members are not \
                                 contiguous, so one pass cannot render its heading once",
                                section.title
                            );
                            seen.push(g);
                        }
                    }
                    None => ungrouped = true,
                }
            }
        }
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
    fn every_section_reads_alphabetically() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit");
        assert!(!nav.sections().is_empty(), "there is something to order");
        for section in nav.sections() {
            let labels: Vec<String> = section
                .items
                .iter()
                .map(|i| i.label.to_lowercase())
                .collect();
            let mut sorted = labels.clone();
            sorted.sort();
            assert_eq!(labels, sorted, "section {} is out of order", section.title);
        }
    }

    #[test]
    fn the_areas_are_ordered_by_name_and_not_by_the_order_they_are_written_in() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build(&ctx, "/cockpit");
        let areas = nav
            .sections()
            .iter()
            .find(|s| s.title == "Cockpit")
            .expect("the area section");
        let labels: Vec<&str> = areas.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels.first(), Some(&"API"));
        assert_eq!(labels.last(), Some(&"Worktrees"));
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
