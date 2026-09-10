//! The page `/` answers with: this process, and everything it serves.
//!
//! Every entry on it is a [`crate::web::Surface`] of the resolved topology the router is serving from.
//! There is no list of links in this file and no template with paths written into it: the
//! sections are the categories a surface declares, the entries are the public surfaces of
//! each, and a surface that appears in the topology tomorrow appears here with no edit.
//! What cannot be derived — whether a static surface's producer has run — is asked of the
//! surface at render time and shown as its state rather than as a link to a certain 404.
//!
//! Every link is relative to the origin, so the page is correct on whatever host, port or
//! reverse-proxy prefix the process was reached through.

use crate::web::html;
use crate::web::model::{Category, SurfaceKind, Topology, Visibility};

/// What the page says about the process, beside what it serves.
///
/// Only facts a reader can act on. The repository root is a filesystem path and a
/// deliberate omission: this page is served to whoever can reach the socket, and where the
/// repository sits on the host is of no use to them and of some use to somebody else.
///
/// What replaces it is [`Identity::id`], which answers the question the path was reached
/// for: *which checkout is this*. Several sessions on one machine each bind their own
/// server, and a page open on the wrong port is indistinguishable from a broken tool
/// unless the page says whose it is. The name is readable and not unique; the id is
/// unique and not readable; a process that knows a root computes the same id and can tell
/// whether a server is its own.
#[derive(Debug, Clone)]
pub struct Identity<'a> {
    /// The executable's version.
    pub version: &'a str,
    /// The one-line description of what this is.
    pub summary: &'a str,
    /// The repository's name, as the index read it.
    pub repository: &'a str,
    /// The repository's identity: the root hashed, never the root itself.
    pub id: &'a str,
    /// The revision the repository is at, when version control could say.
    pub revision: Option<&'a str>,
    /// How many capabilities the registry holds.
    pub capabilities: usize,
}

/// How much of an identity hash a reader is shown: enough to tell two checkouts apart at
/// a glance, and short enough to read out loud.
pub const ID_SHOWN: usize = 12;

/// Whether a surface is ready to answer, decided by asking it rather than by assuming.
pub type Readiness<'a> = &'a dyn Fn(&str) -> Availability;

/// What a surface can be, from the page's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// It answers now.
    Ready,
    /// It is part of the topology and its producer has not run.
    NotBuilt,
}

/// Render the home page for a topology and the process serving it.
///
/// `ready` is asked about every static surface; a native route is always ready, because a
/// process that serves the page serves its own routes.
pub fn page(topology: &Topology, identity: &Identity<'_>, ready: Readiness<'_>) -> String {
    let mut body = String::new();
    body.push_str(&html::summary(&[
        ("Version", identity.version.to_string()),
        ("Repository", identity.repository.to_string()),
        (
            "Identity",
            identity.id[..ID_SHOWN.min(identity.id.len())].to_string(),
        ),
        ("Capabilities", identity.capabilities.to_string()),
        ("Surfaces served", topology.surfaces.len().to_string()),
        (
            "Revision",
            identity
                .revision
                .map(|r| r[..12.min(r.len())].to_string())
                .unwrap_or_else(|| "unknown".into()),
        ),
    ]));

    for category in Category::ALL {
        let surfaces = topology.public_in(category);
        if surfaces.is_empty() {
            continue;
        }
        body.push_str(&format!("<h2>{}</h2>", html::escape(category.title())));
        let rows: Vec<Vec<String>> = surfaces
            .iter()
            .map(|surface| {
                let target = link_target(surface.mount.as_str(), surface.kind);
                let state = match surface.kind {
                    SurfaceKind::StaticDirectory => ready(&surface.id),
                    _ => Availability::Ready,
                };
                let name = match state {
                    Availability::Ready => format!(
                        "<a href=\"{}\">{}</a>",
                        html::escape(&target),
                        html::escape(&surface.id)
                    ),
                    Availability::NotBuilt => html::escape(&surface.id),
                };
                vec![
                    name,
                    format!("<code>{}</code>", html::escape(&target)),
                    html::escape(&surface.title),
                    match state {
                        Availability::Ready => String::from("<span class=\"pass\">ready</span>"),
                        Availability::NotBuilt => format!(
                            "<span class=\"fail\">not built</span> — run <code>{}</code>",
                            html::escape(&surface.producer)
                        ),
                    },
                ]
            })
            .collect();
        body.push_str(&html::table(
            &["Surface", "Path", "What it is", "State"],
            &rows,
        ));
    }

    body.push_str(&format!(
        "<footer>{} — {}{}<br>This page is rendered from the resolved web topology; \
every surface above was discovered, not listed. <code>majordomus web explain</code> says \
where each value came from.</footer>",
        html::escape(identity.repository),
        html::escape(identity.summary),
        identity
            .revision
            .map(|r| format!(
                "<br>revision <span class=\"mono\">{}</span>",
                html::escape(&r[..12.min(r.len())])
            ))
            .unwrap_or_default(),
    ));

    html::page(
        "Majordomus",
        &format!(
            "{} — {} of {} served surface(s) are offered here; the rest are spoken to by a program",
            html::escape(identity.repository),
            topology
                .surfaces
                .iter()
                .filter(|s| s.visibility == Visibility::Public)
                .count(),
            topology.surfaces.len()
        ),
        &body,
    )
}

/// The href a mount is reached through: a directory mount keeps its trailing slash so that
/// the page it answers with resolves its own relative links correctly.
fn link_target(mount: &str, kind: SurfaceKind) -> String {
    match kind {
        SurfaceKind::StaticDirectory if !mount.ends_with('/') => format!("{mount}/"),
        _ => mount.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::discover::{self, Runtime};
    use crate::web::model::Mount;

    fn identity() -> Identity<'static> {
        Identity {
            version: "1.2.3",
            summary: "a control plane",
            repository: "prismatic-majordomus",
            revision: Some("abcdef0123456789"),
            capabilities: 42,
        }
    }

    /// A repository whose only relevant fact is that it has a site.
    fn with_a_site() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("site")).expect("mkdir");
        std::fs::write(tmp.path().join(discover::SITE_CONFIG), "base_url = \"/\"\n")
            .expect("write");
        tmp
    }

    fn topology() -> Topology {
        let tmp = with_a_site();
        let mut surfaces = discover::native(Runtime::full());
        surfaces.extend(discover::application(tmp.path()));
        Topology::new(surfaces).served(Runtime::full())
    }

    #[test]
    fn every_public_surface_of_the_topology_is_on_the_page() {
        let topology = topology();
        let html = page(&topology, &identity(), &|_| Availability::Ready);
        for surface in &topology.surfaces {
            if surface.visibility != Visibility::Public {
                continue;
            }
            assert!(
                html.contains(&format!(">{}<", surface.id)),
                "{} is served and not on the page:\n{html}",
                surface.id
            );
        }
    }

    #[test]
    fn an_internal_surface_is_served_and_not_advertised() {
        let topology = topology();
        assert!(
            topology.surfaces.iter().any(|s| s.id == "mcp"),
            "the process serves mcp"
        );
        let html = page(&topology, &identity(), &|_| Availability::Ready);
        assert!(!html.contains(">mcp<"), "{html}");
    }

    #[test]
    fn a_surface_that_leaves_the_topology_leaves_the_page() {
        let full = topology();
        let html = page(&full, &identity(), &|_| Availability::Ready);
        assert!(html.contains(">cockpit<"));
        let without =
            Topology::new(discover::native(Runtime::default())).served(Runtime::default());
        let html = page(&without, &identity(), &|_| Availability::Ready);
        assert!(!html.contains(">cockpit<"), "{html}");
    }

    #[test]
    fn a_static_surface_whose_producer_has_not_run_is_named_and_not_linked() {
        let tmp = with_a_site();
        let surfaces = discover::application(tmp.path());
        assert!(!surfaces.is_empty(), "a repository fixture has no site");
        let topology = Topology::new(surfaces).served(Runtime::full());
        let html = page(&topology, &identity(), &|_| Availability::NotBuilt);
        assert!(html.contains("not built"), "{html}");
        assert!(!html.contains("href=\"/docs/\""), "{html}");
        assert!(html.contains("scripts/site-build --serve"), "{html}");
    }

    #[test]
    fn every_link_is_relative_to_the_origin() {
        let html = page(&topology(), &identity(), &|_| Availability::Ready);
        assert!(!html.contains("href=\"http"), "{html}");
        assert!(!html.contains("127.0.0.1"), "{html}");
    }

    #[test]
    fn a_directory_mount_is_linked_with_its_trailing_slash() {
        assert_eq!(link_target("/docs", SurfaceKind::StaticDirectory), "/docs/");
        assert_eq!(
            link_target("/swagger", SurfaceKind::NativeRoute),
            "/swagger"
        );
        assert_eq!(
            link_target(Mount::root().as_str(), SurfaceKind::NativeRoute),
            "/"
        );
    }

    #[test]
    fn a_title_from_a_producer_cannot_close_a_tag() {
        let mut surfaces = discover::native(Runtime::full());
        surfaces[0].title = "</td><script>alert(1)</script>".into();
        let topology = Topology::new(surfaces).served(Runtime::full());
        let html = page(&topology, &identity(), &|_| Availability::Ready);
        assert!(!html.contains("<script>alert"), "{html}");
    }
}
