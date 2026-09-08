//! What this process serves, resolved once: the topology narrowed to this router, with a
//! handler bound to every surface in it.
//!
//! The router does not name surfaces. It asks this table who owns a path, and the table was
//! built from the resolved topology — so a static surface that a producer declared is
//! served because it was discovered, and nothing is served that was not.
//!
//! A native surface is the one thing a topology cannot supply on its own: a path the
//! executable answers needs code behind it. That binding lives here, in one match, and a
//! native surface with no arm refuses the router at construction rather than answering 404
//! at run time. `every_native_surface_has_a_handler` is the test that keeps the two halves
//! of that match in step.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::web::discover::{self, Runtime};
use crate::web::files::Files;
use crate::web::model::{Surface, SurfaceKind, Topology};
use crate::web::validate::{self, Artifacts};

/// The code behind a route the executable answers itself.
///
/// One value per native surface, matched from its declared id. The variants are what this
/// executable can do, not what a path is called: renaming a mount changes the declaration
/// and nothing here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Native {
    /// The page this process is entered through.
    Home,
    /// The capability registry's own routes.
    Api,
    /// The OpenAPI document.
    OpenApi,
    /// The Swagger UI shell.
    Swagger,
    /// MCP over HTTP.
    Mcp,
    /// The Cockpit.
    Cockpit,
    /// The live channel over which executions report what they are doing.
    Events,
}

impl Native {
    /// The handler a native surface's id binds to, when this executable has one.
    pub fn of(id: &str) -> Option<Native> {
        Some(match id {
            discover::HOME => Native::Home,
            "api" => Native::Api,
            "openapi" => Native::OpenApi,
            "swagger" => Native::Swagger,
            "mcp" => Native::Mcp,
            "cockpit" => Native::Cockpit,
            "events" => Native::Events,
            _ => return None,
        })
    }
}

/// What a resolved surface is answered by.
#[derive(Debug)]
pub enum Bound {
    /// A route this executable answers.
    Route(Native),
    /// A directory a producer generated. Boxed: a static surface carries its resolved root
    /// and its cache, and a table of them should not pay that for the routes beside them.
    Directory(Box<Files>),
}

/// The surfaces one router serves, with what answers each.
#[derive(Debug)]
pub struct Served {
    topology: Topology,
    bound: BTreeMap<String, Bound>,
}

impl Served {
    /// Narrow `topology` to what a process with these runtime capabilities serves, bind a
    /// handler to every surface in it, and refuse anything that cannot be served coherently.
    ///
    /// The refusal is the point: a collision, a nested mount or a native surface with no
    /// handler stops the router being built, so it is found by whoever changed a
    /// declaration rather than by whoever sent the first request.
    pub fn resolve(topology: &Topology, root: &Path, runtime: Runtime) -> Result<Self> {
        let topology = topology.served(runtime);
        let findings = validate::validate(&topology, root, Artifacts::Ignore);
        if validate::blocking(&findings) {
            let reasons: Vec<String> = findings
                .iter()
                .filter(|f| f.severity == validate::Severity::Error)
                .map(|f| format!("{} ({}): {}", f.rule, f.surface, f.message))
                .collect();
            return Err(Error::InvalidSurface {
                surface: "topology".into(),
                reason: format!(
                    "this process cannot serve the surfaces it discovered: {}; `majordomus web validate` reports the same findings with their remedies",
                    reasons.join("; ")
                ),
            });
        }
        let mut bound = BTreeMap::new();
        for surface in &topology.surfaces {
            let handler = match surface.kind {
                SurfaceKind::StaticDirectory => {
                    Bound::Directory(Box::new(Files::new(surface, root)))
                }
                SurfaceKind::NativeRoute => {
                    let Some(native) = Native::of(&surface.id) else {
                        return Err(Error::InvalidSurface {
                            surface: surface.id.clone(),
                            reason: format!(
                                "is declared as a route this executable answers and no handler is bound to it; add an arm to http::surfaces::Native::of, or declare {} as a generated directory",
                                surface.id
                            ),
                        });
                    };
                    Bound::Route(native)
                }
                SurfaceKind::Redirect => {
                    return Err(Error::InvalidSurface {
                        surface: surface.id.clone(),
                        reason: "a redirect surface is declared and this executable serves none"
                            .into(),
                    })
                }
            };
            bound.insert(surface.id.clone(), handler);
        }
        Ok(Served { topology, bound })
    }

    /// The topology this serves, narrowed and validated.
    pub fn topology(&self) -> &Topology {
        &self.topology
    }

    /// The same topology behind an `Arc`, for a context that hands it to a capability.
    pub fn shared(&self) -> Arc<Topology> {
        Arc::new(self.topology.clone())
    }

    /// The surface that owns a request path, with what answers it.
    pub fn owner(&self, path: &str) -> Option<(&Surface, &Bound)> {
        let surface = self.topology.owner(path)?;
        let bound = self.bound.get(&surface.id)?;
        Some((surface, bound))
    }

    /// Is a surface's producer's output there? A route the executable answers always is.
    pub fn ready(&self, id: &str) -> bool {
        match self.bound.get(id) {
            Some(Bound::Directory(files)) => files.available(),
            Some(Bound::Route(_)) => true,
            None => false,
        }
    }

    /// The one line a server logs when it starts: how many surfaces it serves and where
    /// each one is, `base` prefixed so the log is clickable.
    ///
    /// Every served surface, not only the public ones: this is an operator's diagnostic,
    /// and the operator attaching a second client needs the MCP mount that the home page
    /// has no reason to advertise to a browser.
    ///
    /// Both halves are read off the resolution. A surface added tomorrow is in this line,
    /// and a route that moves moves here, because nothing below names one.
    ///
    /// ```
    /// use majordomus_cli::http::Served;
    /// use majordomus_cli::web::{discover::{self, Runtime}, Topology};
    /// let topology = Topology::new(discover::native_all());
    /// let served = Served::resolve(&topology, std::path::Path::new("/nonexistent"), Runtime::full()).unwrap();
    /// let line = served.summary("http://127.0.0.1:8741");
    /// assert!(line.contains("http://127.0.0.1:8741/swagger"));
    /// assert!(line.contains("6 surface(s)"));
    /// ```
    pub fn summary(&self, base: &str) -> String {
        let mut parts: Vec<String> = self
            .topology
            .surfaces
            .iter()
            .map(|s| {
                let mount = s.mount.as_str();
                let path = if s.mount.is_root() { "/" } else { mount };
                format!("{} {base}{path}", s.id)
            })
            .collect();
        parts.sort();
        format!(
            "{} surface(s): {}",
            self.topology.surfaces.len(),
            parts.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_native_surface_has_a_handler() {
        for surface in discover::native_all() {
            assert!(
                Native::of(&surface.id).is_some(),
                "the native surface '{}' is declared and nothing answers it",
                surface.id
            );
        }
    }

    #[test]
    fn a_native_surface_with_no_handler_refuses_the_router() {
        let mut surfaces = discover::native_all();
        // a surface at no reserved mount, so what refuses the router is the missing
        // handler and not a reservation
        let cockpit = surfaces
            .iter_mut()
            .find(|s| s.id == "cockpit")
            .expect("the cockpit is declared");
        cockpit.id = "invented".into();
        let topology = Topology::new(surfaces);
        let err = Served::resolve(&topology, Path::new("/nonexistent"), Runtime::full())
            .expect_err("a surface nothing answers is refused")
            .to_string();
        assert!(err.contains("invented"), "{err}");
        assert!(err.contains("Native::of"), "{err}");
    }

    #[test]
    fn a_collision_refuses_the_router_and_names_both_surfaces() {
        let mut surfaces = discover::native_all();
        let mut clone = surfaces
            .iter()
            .find(|s| s.id == "cockpit")
            .expect("the cockpit is declared")
            .clone();
        clone.id = "second".into();
        surfaces.push(clone);
        let topology = Topology::new(surfaces);
        let err = Served::resolve(&topology, Path::new("/nonexistent"), Runtime::full())
            .expect_err("two surfaces on one mount are refused")
            .to_string();
        assert!(err.contains("mount-collision"), "{err}");
        assert!(err.contains("second"), "{err}");
    }

    #[test]
    fn the_reserved_mounts_belong_to_the_surfaces_that_own_them() {
        let topology = Topology::new(discover::native_all());
        let served = Served::resolve(&topology, Path::new("/nonexistent"), Runtime::full())
            .expect("the declared topology is servable");
        assert_eq!(served.owner("/swagger").unwrap().0.id, "swagger");
        assert_eq!(served.owner("/openapi.json").unwrap().0.id, "openapi");
        assert_eq!(served.owner("/api/v1/health").unwrap().0.id, "api");
        assert_eq!(served.owner("/mcp").unwrap().0.id, "mcp");
        assert_eq!(served.owner("/cockpit/graphs").unwrap().0.id, "cockpit");
        // the home page owns what nothing else claims, and nothing else claims it
        assert_eq!(served.owner("/").unwrap().0.id, discover::HOME);
        assert_eq!(served.owner("/whatever").unwrap().0.id, discover::HOME);
    }

    /// A repository whose only relevant fact is that it has a site.
    fn with_a_site() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("site")).expect("mkdir");
        std::fs::write(tmp.path().join(discover::SITE_CONFIG), "base_url = \"/\"\n")
            .expect("write");
        tmp
    }

    #[test]
    fn a_documentation_surface_does_not_swallow_the_routes_beside_it() {
        let tmp = with_a_site();
        let mut surfaces = discover::native_all();
        surfaces.extend(discover::application(tmp.path()));
        let topology = Topology::new(surfaces);
        let served = Served::resolve(&topology, tmp.path(), Runtime::full())
            .expect("the topology is servable");
        assert_eq!(served.owner("/docs/").unwrap().0.id, discover::DOCS);
        assert_eq!(
            served.owner("/docs/cli/index.html").unwrap().0.id,
            discover::DOCS
        );
        for (path, id) in [
            ("/swagger", "swagger"),
            ("/openapi.json", "openapi"),
            ("/api/v1/objects", "api"),
            ("/mcp", "mcp"),
            ("/cockpit", "cockpit"),
        ] {
            assert_eq!(served.owner(path).unwrap().0.id, id, "{path}");
        }
        // the deployment is published, never served: it would otherwise claim `/`
        assert!(!served.topology().ids().contains(&discover::APPLICATION));
    }

    #[test]
    fn a_process_without_a_feature_serves_none_of_its_surfaces() {
        let topology = Topology::new(discover::native_all());
        let served = Served::resolve(&topology, Path::new("/nonexistent"), Runtime::default())
            .expect("the topology is servable");
        assert!(served.owner("/mcp").is_none_or(|(s, _)| s.id != "mcp"));
        assert!(!served.topology().ids().contains(&"cockpit"));
        assert!(served
            .summary("http://x")
            .contains("swagger http://x/swagger"));
    }
}
