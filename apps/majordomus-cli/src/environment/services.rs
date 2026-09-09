//! The local services of a repository: what this executable serves, where, and whether
//! anything is answering there now.
//!
//! Paths are not written here. Each descriptor names the constant that already decides its
//! route — [`crate::cockpit::PREFIX`], [`crate::capability::HttpExposure::PREFIX`],
//! [`crate::http::swagger::SWAGGER_PATH`], [`crate::web::discover::DOCS_MOUNT`] and the
//! rest of what [`crate::http::openapi::infrastructure_routes`] lists — so a route that
//! moves moves here too, and `every_infrastructure_route_is_described` fails the build if
//! a new one is added without a name for it. The base address is the shared server's lease, which is
//! where a running server publishes it and the only place it is true.
//!
//! Availability is decided by a connection attempt with a hard budget and no name
//! resolution ([`super::probe`]); a service that did not answer in time is `unknown`,
//! never "down".

use std::path::Path;
use std::time::Duration;

use crate::capability::HttpExposure;
use crate::cockpit;
use crate::http::{mcp, swagger};
use crate::web::discover::DOCS_MOUNT;

use super::{ServiceAvailability, ServiceState};

/// How long the connection attempt to a published address may take. Loopback either
/// answers in microseconds or is not there; anything slower is a machine under load, and
/// the snapshot says `unknown` rather than making a shell prompt wait for it.
pub const PROBE_BUDGET: Duration = Duration::from_millis(80);

/// One service this executable serves, and the constant that decides where.
struct Descriptor {
    id: &'static str,
    title: &'static str,
    path: &'static str,
}

/// Every service, in the order a snapshot reports them: the human surfaces first, the
/// machine ones after.
fn descriptors() -> [Descriptor; 7] {
    [
        Descriptor {
            id: "cockpit",
            title: "Cockpit",
            path: cockpit::PREFIX,
        },
        Descriptor {
            id: "docs",
            title: "Documentation",
            path: DOCS_MOUNT,
        },
        Descriptor {
            id: "swagger",
            title: "Swagger UI",
            path: swagger::SWAGGER_PATH,
        },
        Descriptor {
            id: "api",
            title: "HTTP API",
            // The prefix every capability route lives under; the routes themselves are the
            // registry's and are not repeated anywhere. Without the trailing slash, because
            // that is how the surface is mounted (`web::discover::native_all` trims it) and
            // this list is compared against what the server actually serves.
            path: HttpExposure::PREFIX.trim_end_matches('/'),
        },
        Descriptor {
            id: "openapi",
            title: "OpenAPI document",
            path: swagger::SPEC_PATH,
        },
        Descriptor {
            id: "mcp",
            title: "MCP over HTTP",
            path: mcp::PATH,
        },
        Descriptor {
            id: "index",
            title: "Home page",
            path: "/",
        },
    ]
}

/// The services of the repository at `root`, with the address a running server published.
///
/// `probe` decides whether the address is contacted at all: a resolution that may not
/// spend the time reports every service `unknown`, which is what it knows.
pub fn resolve(root: &Path, local_half: &str, probe: bool) -> Vec<ServiceState> {
    let base = published_url(root, local_half);
    // One connection attempt for the whole server, not one per route: they are all the
    // same socket, and seven probes would cost seven times as much to learn one thing.
    let availability = match (&base, probe) {
        (Some(url), true) => super::probe::reachable(url, PROBE_BUDGET),
        (Some(_), false) => ServiceAvailability::Unknown,
        (None, _) => ServiceAvailability::NotRunning,
    };
    descriptors()
        .into_iter()
        .map(|d| ServiceState {
            id: d.id.into(),
            title: d.title.into(),
            path: d.path.into(),
            url: base.as_ref().map(|base| join(base, d.path)),
            availability,
        })
        .collect()
}

/// The address the repository's shared server published, from its lease. Reads one small
/// file and contacts nothing; [`crate::lease::probe`] is the version that checks whether
/// the server is really there, and it costs an HTTP round trip.
pub fn published_url(root: &Path, local_half: &str) -> Option<String> {
    let path = root.join(local_half).join(crate::lease::LEASE_PATH);
    let text = std::fs::read_to_string(path).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    if document.get("schema").and_then(serde_json::Value::as_str)? != crate::lease::SCHEMA {
        return None;
    }
    document
        .get("url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// A base address and a route, joined without a doubled or missing slash.
///
/// ```
/// use majordomus_cli::environment::services::join;
/// assert_eq!(join("http://127.0.0.1:8741", "/cockpit"), "http://127.0.0.1:8741/cockpit");
/// assert_eq!(join("http://127.0.0.1:8741/", "/"), "http://127.0.0.1:8741/");
/// assert_eq!(join("http://127.0.0.1:8741", "/api/v1/"), "http://127.0.0.1:8741/api/v1/");
/// ```
pub fn join(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::openapi::infrastructure_routes;

    /// The reason the paths are constants rather than strings: a route added to the
    /// server's own infrastructure list without a name here would be a service the
    /// Cockpit, the banner and the documentation never mention. This is what notices.
    #[test]
    fn every_infrastructure_route_is_described() {
        let described: Vec<&str> = descriptors().iter().map(|d| d.path).collect();
        for route in infrastructure_routes() {
            assert!(
                described.contains(&route.path.as_str()),
                "the server serves {} and no service describes it",
                route.path
            );
        }
    }

    #[test]
    fn every_service_has_a_distinct_id_and_a_path_the_router_could_serve() {
        let mut ids: Vec<&str> = descriptors().iter().map(|d| d.id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count, "two services share an id");
        for d in descriptors() {
            assert!(d.path.starts_with('/'), "{} has a relative path", d.id);
            assert!(!d.title.is_empty());
        }
    }

    #[test]
    fn without_a_lease_nothing_is_running_and_no_url_is_invented() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let services = resolve(dir.path(), ".ai/local", true);
        assert!(!services.is_empty());
        for s in &services {
            assert_eq!(s.availability, ServiceAvailability::NotRunning);
            assert_eq!(s.url, None, "{} invented an address", s.id);
        }
    }

    #[test]
    fn a_lease_of_another_schema_is_not_read() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join(".ai/local").join(crate::lease::LEASE_PATH);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(
            &path,
            r#"{"schema":"something/else","url":"http://127.0.0.1:1"}"#,
        )
        .expect("a lease");
        assert_eq!(published_url(dir.path(), ".ai/local"), None);
    }

    #[test]
    fn a_published_address_becomes_every_services_url() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join(".ai/local").join(crate::lease::LEASE_PATH);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(
            &path,
            format!(
                r#"{{"schema":"{}","url":"http://127.0.0.1:8741"}}"#,
                crate::lease::SCHEMA
            ),
        )
        .expect("a lease");
        // No probe: this asserts the addresses, not what answers at them.
        let services = resolve(dir.path(), ".ai/local", false);
        let cockpit = services
            .iter()
            .find(|s| s.id == "cockpit")
            .expect("the cockpit");
        assert_eq!(
            cockpit.url.as_deref(),
            Some("http://127.0.0.1:8741/cockpit")
        );
        assert_eq!(
            cockpit.availability,
            ServiceAvailability::Unknown,
            "a resolution that did not probe knows nothing, and says so"
        );
    }
}
