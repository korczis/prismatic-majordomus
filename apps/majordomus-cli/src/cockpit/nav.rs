//! Navigation, derived. The sidebar's catalogues are not a list in this file: the
//! capability modules come from the registry, the object kinds come from the index, and
//! the graphs come from the derivations. A module added to `compose_modules!`, a kind
//! added to `share/kinds.yaml`, a graph added to the derivation table — each appears here
//! with no edit to the Cockpit.
//!
//! What *is* written here is the eight areas: Overview, Capabilities, Objects, Graphs,
//! Continuity, Health, Artifacts, API. Those are concepts rather than entities, they
//! change when the Cockpit's own shape changes, and deriving them from anything would be
//! deriving them from a list of exactly themselves.

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
    /// The declarative objects of the layer.
    Objects,
    /// The derived graphs.
    Graphs,
    /// What this checkout's lifecycle is holding.
    Continuity,
    /// The health report.
    Health,
    /// What the generator writes.
    Artifacts,
    /// The HTTP and MCP surfaces.
    Api,
    /// A page that belongs to no area (search results, an error).
    None,
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
    /// How many things are behind it, when the number is a fact and not decoration.
    pub count: Option<usize>,
    /// Whether this is the page being shown.
    pub current: bool,
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

/// Build the navigation for a request: the areas, then the catalogues derived from the
/// registry, the index and the graph derivations. `here` is the request path, so the
/// current entry can be marked without a page saying which it is.
pub fn build(ctx: &Context, here: &str) -> Navigation {
    let summary = ctx.registry.summary();
    let areas = Section {
        title: "Cockpit".into(),
        items: vec![
            item("Overview", "/cockpit", Area::Overview, None, here),
            item(
                "Capabilities",
                "/cockpit/capabilities",
                Area::Capabilities,
                Some(summary.total),
                here,
            ),
            item(
                "Objects",
                "/cockpit/objects",
                Area::Objects,
                Some(ctx.index.objects.len()),
                here,
            ),
            item(
                "Graphs",
                "/cockpit/graphs",
                Area::Graphs,
                Some(graph::ids().len()),
                here,
            ),
            item(
                "Continuity",
                "/cockpit/continuity",
                Area::Continuity,
                None,
                here,
            ),
            item("Health", "/cockpit/health", Area::Health, None, here),
            item(
                "Artifacts",
                "/cockpit/artifacts",
                Area::Artifacts,
                None,
                here,
            ),
            item(
                "API",
                "/cockpit/api",
                Area::Api,
                Some(summary.http_routes),
                here,
            ),
        ],
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

fn item(label: &str, href: &str, area: Area, count: Option<usize>, here: &str) -> Item {
    Item {
        label: label.into(),
        href: href.into(),
        area,
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
}
