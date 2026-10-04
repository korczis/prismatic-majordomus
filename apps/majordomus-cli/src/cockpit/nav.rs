//! Navigation, derived. The sidebar's catalogues are not a list in this file: the
//! capability modules come from the registry, the object kinds come from `repository.info`,
//! and the graphs come from the derivations. A module added to `compose_modules!`, a kind
//! added to `share/kinds.yaml`, a graph added to the derivation table — each appears here
//! with no edit to the Cockpit.
//!
//! The kinds are *asked for* rather than read: ADR 0012 says no page reads the index
//! directly, because a reader that steps around the executor is outside the cache, the
//! counters and the validation every other caller passes through.
//!
//! What *is* written here is the areas: Overview, Capabilities, Commands, Executions,
//! Objects, Directories, Graphs, Continuity, Completion, Worktrees, Integration, Health, Quality,
//! Artifacts, Design, API. Those are concepts rather than
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
use crate::capability::Context;
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
    /// Whether the active task may be called finished: the lifecycle stage, every question
    /// of the completion policy and what answered it.
    Completion,
    /// What must become true, how far reality is from it, and the work realising it.
    Intents,
    /// The branch-to-worktree topology of the repository.
    Worktrees,
    /// Who else is working in this repository, gathered from every checkout's board.
    Peers,
    /// The pull-request integration queue and its executor.
    Integration,
    /// The discovered nodes of the mesh, and the machinery that observes them.
    Mesh,
    /// The declared model catalogue and its routing.
    Models,
    /// Reasoning: the optional advisors, and the session's uncertainties and conclusions.
    Reasoning,
    /// Token economics, measured.
    Economics,
    /// The health report.
    Health,
    /// What the crate's own public surface is held to.
    Quality,
    /// What the generator writes.
    Artifacts,
    /// What the public contract did since the last release, and the version it requires.
    Release,
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
/// Kept against the dispatcher by `cockpit::tests::every_area_has_a_route_and_every_route_its_area`:
/// this list and `cockpit::STATIC_ROUTES` describe the same Cockpit, so an area with no route
/// and a route with no area are both failures. Executions and Quality were the second kind —
/// answered by the dispatcher, named by the module documentation above, and absent from here,
/// which left `/cockpit/quality` unreachable by a reader, by the navigation crawl the browser
/// probe derives its routes from, and therefore by every test.
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
            id: "executions",
            label: "Executions",
            href: "/cockpit/executions",
            area: Area::Executions,
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
            id: "completion",
            label: "Completion",
            href: "/cockpit/completion",
            area: Area::Completion,
        },
        AreaInfo {
            id: "intents",
            label: "Intents",
            href: "/cockpit/intents",
            area: Area::Intents,
        },
        AreaInfo {
            id: "worktrees",
            label: "Worktrees",
            href: "/cockpit/worktrees",
            area: Area::Worktrees,
        },
        AreaInfo {
            id: "peers",
            label: "Peers",
            href: "/cockpit/peers",
            area: Area::Peers,
        },
        AreaInfo {
            id: "integration",
            label: "Integration",
            href: "/cockpit/integration",
            area: Area::Integration,
        },
        AreaInfo {
            id: "mesh",
            label: "Mesh",
            href: "/cockpit/mesh",
            area: Area::Mesh,
        },
        AreaInfo {
            id: "models",
            label: "Models",
            href: "/cockpit/models",
            area: Area::Models,
        },
        AreaInfo {
            id: "reasoning",
            label: "Reasoning",
            href: "/cockpit/reasoning",
            area: Area::Reasoning,
        },
        AreaInfo {
            id: "economics",
            label: "Economics",
            href: "/cockpit/economics",
            area: Area::Economics,
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
            id: "release",
            label: "Release",
            href: "/cockpit/release",
            area: Area::Release,
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
    /// How many things are behind it, when the number is a fact and not decoration: `None`
    /// for an entry that carries no count, and [`Count::Unknown`] for one whose count the
    /// capability asked for it did not answer.
    pub count: Option<Count>,
    /// Whether this is the page being shown.
    pub current: bool,
}

/// A count an entry shows: a number the capability answered, or the fact that it did not.
/// A capability that failed is never a count of zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Count {
    /// The number.
    Known(usize),
    /// The capability behind it did not answer.
    Unknown,
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

/// What the layer holds, as `repository.info` answers it: the object count and the kinds
/// with theirs.
///
/// A capability that cannot answer leaves the navigation without its object counts rather
/// than without a navigation: the sidebar is how a person reaches the page that would say
/// what went wrong, so it is the last thing that should fail with the thing it reports on.
/// The two fields of `repository.info` this file reads; the rest of the report is the
/// health page's subject, and taking only these keeps the navigation's dependency on the
/// capability to what it actually shows.
#[derive(serde::Deserialize)]
struct Held {
    objects: usize,
    kinds: std::collections::BTreeMap<String, usize>,
}

/// `None` when `repository.info` did not answer, or answered something this file cannot
/// read: the object count is then unknown, never zero.
fn held(ctx: &Context) -> Option<Held> {
    let value = ctx.execute("repository.info", serde_json::json!({})).ok()?;
    serde_json::from_value::<Held>(value).ok()
}

/// Build the navigation for a request: the areas, then the catalogues derived from the
/// registry, the index and the graph derivations. `here` is the request path, so the
/// current entry can be marked without a page saying which it is.
pub fn build(ctx: &Context, here: &str) -> Navigation {
    let summary = ctx.registry.summary();
    // What the layer holds is asked for, not opened. `repository.info` answers how many
    // objects there are and which kinds they have, and asking it is what keeps the
    // navigation a projection instead of a second reader of the index (ADR 0012). One
    // call answers both catalogues below.
    let held = held(ctx);
    build_with(ctx, here, held.as_ref(), &summary)
}

/// [`build`], over what `repository.info` answered (`None`: it did not).
fn build_with(
    ctx: &Context,
    here: &str,
    held: Option<&Held>,
    summary: &crate::capability::registry::Summary,
) -> Navigation {
    // the counts are facts of this context, decided per area: how many things are behind
    // an entry is not part of what an area is
    let count = |a: Area| -> Option<Count> {
        match a {
            Area::Capabilities => Some(Count::Known(summary.total)),
            Area::Objects => Some(held.map_or(Count::Unknown, |h| Count::Known(h.objects))),
            Area::Graphs => Some(Count::Known(graph::ids().len())),
            Area::Api => Some(Count::Known(summary.http_routes)),
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
                count: Some(Count::Known(m.capabilities)),
                current: false,
            })
            .collect(),
    };

    // the kinds of the layer: what the repository declares, not what this code knows
    let kinds = Section {
        title: "Object kinds".into(),
        items: held
            .into_iter()
            .flat_map(|h| &h.kinds)
            .map(|(kind, count)| Item {
                label: kind.to_string(),
                href: crate::entity::kind_route(kind),
                area: Area::Objects,
                group: None,
                count: Some(Count::Known(*count)),
                current: false,
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

fn item(label: &str, href: &str, area: Area, count: Option<Count>, here: &str) -> Item {
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

    /// A `repository.info` that did not answer leaves the object count unknown and the kind
    /// catalogue empty: never a count of zero, which reads as a repository that holds nothing.
    #[test]
    fn a_failed_repository_info_is_an_unknown_count_and_not_zero() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let nav = build_with(&ctx, "/cockpit", None, &ctx.registry.summary());
        let objects = nav
            .sections()
            .iter()
            .flat_map(|s| &s.items)
            .find(|i| i.area == Area::Objects && i.href == "/cockpit/objects")
            .expect("the Objects entry");
        assert_eq!(objects.count, Some(Count::Unknown));
        assert!(
            nav.sections().iter().all(|s| s.title != "Object kinds"),
            "no kind is listed from an answer that did not come"
        );
        // and the answer, when it comes, is the number
        let nav = build(&ctx, "/cockpit");
        let objects = nav
            .sections()
            .iter()
            .flat_map(|s| &s.items)
            .find(|i| i.area == Area::Objects && i.href == "/cockpit/objects")
            .expect("the Objects entry");
        assert!(
            matches!(objects.count, Some(Count::Known(_))),
            "{:?}",
            objects.count
        );
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
        // asked for the same way the navigation asks, so that the assertion cannot pass
        // by reading a source the page is not allowed to read (ADR 0012)
        let reported = ctx
            .execute("repository.info", serde_json::json!({}))
            .expect("repository.info answers");
        let expected: Vec<&str> = reported["kinds"]
            .as_object()
            .expect("kinds")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(labels, expected);
    }
}
