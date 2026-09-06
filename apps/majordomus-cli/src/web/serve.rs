//! Serving the static surfaces of a resolved topology.
//!
//! One arm in the router asks this: does a static surface own the path? Every discovered
//! `StaticDirectory` is answered here, and a surface that did not exist when this file was
//! written is served because it was discovered, not because an arm was added for it. The
//! routes the executable answers itself — the JSON index, the OpenAPI document, the Swagger
//! shell, MCP, the Cockpit — are not dispatched here: they differ in behaviour rather than
//! in data, and a variant per existing arm would be the match again with more ceremony
//! (ADR 0013).
//!
//! The boundary this module defends: only a resolved surface's own directory is reachable,
//! a request may not walk out of it, and only a closed set of media types is answered.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::model::{SurfaceKind, Topology};
use crate::http::router::Response;

/// The media types a generated report may be served as. A file whose extension is not here
/// is not answered: a static surface is a directory of documents, never a file server.
const MEDIA_TYPES: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("svg", "image/svg+xml"),
    ("txt", "text/plain; charset=utf-8"),
    ("xml", "application/xml"),
];

/// The static surfaces of a topology, ready to answer.
///
/// Built once at startup from the resolved topology: the router holds it and asks it before
/// its own routes, so a mount that a surface owns is never shadowed by a later arm.
#[derive(Debug)]
pub struct StaticSurfaces {
    mounts: Vec<Mounted>,
    /// The mounts the executable answers itself. The application is mounted at `/` and
    /// therefore owns every path nothing else claims — including the ones the router
    /// answers — so precedence is resolved from the topology here rather than left to the
    /// order the router happens to consult its arms in.
    native: Vec<String>,
    cache: Mutex<BTreeMap<PathBuf, Option<(&'static str, String)>>>,
}

#[derive(Debug)]
struct Mounted {
    id: String,
    prefix: String,
    root: PathBuf,
    index: String,
}

impl StaticSurfaces {
    /// Take every static surface of `topology` whose directory exists, resolved against
    /// `root`. A surface whose producer has not run is not mounted: the validator is where
    /// that becomes a finding, and a router that answered 500 for it would be worse.
    pub fn new(topology: &Topology, root: &Path) -> Self {
        let mut mounts = Vec::new();
        let mut native = Vec::new();
        for surface in &topology.surfaces {
            if surface.kind != SurfaceKind::StaticDirectory {
                native.push(surface.mount.as_str().to_string());
                continue;
            }
            let Some(artifact) = surface.artifact.as_ref() else {
                continue;
            };
            let dir = root.join(artifact);
            let Ok(canonical) = dir.canonicalize() else {
                continue;
            };
            mounts.push(Mounted {
                id: surface.id.clone(),
                prefix: surface.mount.prefix(),
                root: canonical,
                index: surface.index.clone().unwrap_or_else(|| "index.html".into()),
            });
        }
        // most specific first, so a surface inside another's prefix is consulted first; the
        // validator refuses that arrangement, and the order does not depend on it
        mounts.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()).then(a.id.cmp(&b.id)));
        native.sort();
        StaticSurfaces {
            mounts,
            native,
            cache: Mutex::new(BTreeMap::new()),
        }
    }

    /// The ids mounted, in the order they are consulted.
    pub fn ids(&self) -> Vec<&str> {
        self.mounts.iter().map(|m| m.id.as_str()).collect()
    }

    /// Is anything mounted?
    pub fn is_empty(&self) -> bool {
        self.mounts.is_empty()
    }

    /// Does a static surface own this path — and is it not one the executable answers?
    pub fn owns(&self, path: &str) -> bool {
        !self.reserved(path) && self.mounts.iter().any(|m| owns(m, path))
    }

    /// Is this path one a native route of the topology owns? Those are answered by the
    /// router itself, and the application's catch-all mount may not take them.
    fn reserved(&self, path: &str) -> bool {
        self.native.iter().any(|mount| {
            let bare = mount.trim_end_matches('/');
            !bare.is_empty() && (path == bare || path.starts_with(&format!("{bare}/")))
        })
    }

    /// Answer a request a static surface owns, or `None` when none does.
    ///
    /// A directory answers with its index; a path that walks out of the surface's root, or
    /// names a media type the set does not hold, is a 404 rather than a hint about what is
    /// on the disk.
    pub fn handle(&self, method: &str, path: &str) -> Option<Response> {
        if self.reserved(path) {
            return None; // the executable answers this one itself
        }
        let mounted = self.mounts.iter().find(|m| owns(m, path))?;
        if method != "GET" && method != "HEAD" {
            return Some(Response::new(
                405,
                "application/json",
                r#"{"error":"method_not_allowed","message":"a static surface answers GET"}"#,
            ));
        }
        // exactly one separator is removed: `//` is a different URL, and collapsing it here
        // would hide an empty segment that safe_relative exists to refuse
        let after = path
            .strip_prefix(mounted.prefix.trim_end_matches('/'))
            .unwrap_or("");
        let relative = after.strip_prefix('/').unwrap_or(after);
        let relative = if relative.is_empty() || relative.ends_with('/') {
            format!("{relative}{}", mounted.index)
        } else {
            relative.to_string()
        };
        let Some(safe) = safe_relative(&relative) else {
            return Some(not_found(&mounted.id));
        };
        let file = mounted.root.join(&safe);
        match self.read(&file, &mounted.root) {
            Some((media_type, body)) => Some(Response::new(200, media_type, body)),
            None => Some(not_found(&mounted.id)),
        }
    }

    /// Read a file once and keep it: a generated report is rebuilt by its producer, not by
    /// a request, so the bytes cannot change under a running server without one.
    fn read(&self, file: &Path, root: &Path) -> Option<(&'static str, String)> {
        if let Ok(cache) = self.cache.lock() {
            if let Some(hit) = cache.get(file) {
                return hit.clone();
            }
        }
        let loaded = load(file, root);
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(file.to_path_buf(), loaded.clone());
        }
        loaded
    }
}

fn owns(mounted: &Mounted, path: &str) -> bool {
    let bare = mounted.prefix.trim_end_matches('/');
    if bare.is_empty() {
        return path.starts_with('/');
    }
    path == bare || path.starts_with(&mounted.prefix)
}

fn load(file: &Path, root: &Path) -> Option<(&'static str, String)> {
    let media_type = media_type(file)?;
    // the relative path is already known to hold no `..`; canonicalising both ends closes
    // the door a symbolic link inside the directory would otherwise open
    let resolved = file.canonicalize().ok()?;
    if !resolved.starts_with(root) {
        tracing::warn!(path = %file.display(), "a static surface's file resolves outside its root; refused");
        return None;
    }
    let body = std::fs::read_to_string(&resolved).ok()?;
    Some((media_type, body))
}

/// The media type of a file, or `None` when the set does not hold it.
///
/// ```
/// use majordomus_cli::web::serve::media_type;
/// use std::path::Path;
/// assert_eq!(media_type(Path::new("a/index.html")), Some("text/html; charset=utf-8"));
/// assert_eq!(media_type(Path::new("a/results.json")), Some("application/json"));
/// assert_eq!(media_type(Path::new("a/binary.wasm")), None);
/// assert_eq!(media_type(Path::new("noextension")), None);
/// ```
pub fn media_type(file: &Path) -> Option<&'static str> {
    let extension = file.extension()?.to_str()?.to_ascii_lowercase();
    MEDIA_TYPES
        .iter()
        .find(|(e, _)| *e == extension)
        .map(|(_, t)| *t)
}

/// A request path's relative part, refused before the filesystem is touched when it holds
/// anything but the characters a generated report uses, an empty segment, `.` or `..`.
///
/// ```
/// use majordomus_cli::web::serve::safe_relative;
/// assert_eq!(safe_relative("index.html").as_deref(), Some("index.html"));
/// assert_eq!(safe_relative("coverage/index.html").as_deref(), Some("coverage/index.html"));
/// assert_eq!(safe_relative("../../etc/passwd"), None);
/// assert_eq!(safe_relative("a//b"), None);
/// assert_eq!(safe_relative("a/./b"), None);
/// ```
pub fn safe_relative(relative: &str) -> Option<String> {
    if relative.is_empty() || relative.len() > 300 {
        return None;
    }
    if !relative
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
    {
        return None;
    }
    let segments: Vec<&str> = relative.split('/').collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return None;
    }
    Some(segments.join("/"))
}

fn not_found(surface: &str) -> Response {
    Response::new(
        404,
        "application/json",
        format!(
            r#"{{"error":"not_found","message":"the surface '{surface}' does not hold that path"}}"#
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::model::{Availability, Mount, Surface};
    use std::collections::BTreeMap;

    fn fixture() -> (tempfile::TempDir, StaticSurfaces) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("target/web/tests");
        std::fs::create_dir_all(dir.join("coverage")).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>tests</h1>").unwrap();
        std::fs::write(dir.join("results.json"), "{}").unwrap();
        std::fs::write(dir.join("coverage/index.html"), "<h1>coverage</h1>").unwrap();
        std::fs::write(dir.join("secret.pem"), "no").unwrap();
        let topology = Topology::new(vec![Surface {
            id: "tests".into(),
            title: "Tests".into(),
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::parse("/tests").unwrap(),
            producer: "test".into(),
            artifact: Some("target/web/tests".into()),
            index: Some("index.html".into()),
            availability: Availability::Both,
            provenance: BTreeMap::new(),
        }]);
        let surfaces = StaticSurfaces::new(&topology, tmp.path());
        (tmp, surfaces)
    }

    #[test]
    fn the_mount_answers_with_its_index_with_or_without_a_trailing_slash() {
        let (_tmp, s) = fixture();
        for path in ["/tests", "/tests/"] {
            let response = s.handle("GET", path).expect("the surface owns it");
            assert_eq!(response.status, 200, "{path}");
            assert!(response.body.contains("<h1>tests</h1>"), "{path}");
        }
    }

    #[test]
    fn a_file_and_a_nested_index_are_answered_with_their_media_types() {
        let (_tmp, s) = fixture();
        let json = s.handle("GET", "/tests/results.json").unwrap();
        assert_eq!(json.content_type, "application/json");
        let nested = s.handle("GET", "/tests/coverage/").unwrap();
        assert!(nested.body.contains("coverage"));
    }

    #[test]
    fn nothing_outside_the_surface_is_reachable() {
        let (_tmp, s) = fixture();
        for path in [
            "/tests/../../../etc/passwd",
            "/tests/./results.json",
            "/tests//results.json",
            "/tests/secret.pem",
        ] {
            let response = s.handle("GET", path).expect("the surface owns the prefix");
            assert_eq!(response.status, 404, "{path} was answered");
        }
    }

    #[test]
    fn a_path_no_surface_owns_is_left_to_the_rest_of_the_router() {
        let (_tmp, s) = fixture();
        assert!(s.handle("GET", "/api/v1/objects").is_none());
        assert!(s.handle("GET", "/testsomething").is_none());
        assert!(!s.owns("/testsomething"));
    }

    #[test]
    fn the_application_may_not_take_a_path_the_executable_answers() {
        let tmp = tempfile::tempdir().unwrap();
        let public = tmp.path().join("site/public");
        std::fs::create_dir_all(&public).unwrap();
        std::fs::write(public.join("index.html"), "<h1>app</h1>").unwrap();
        let mut surfaces = crate::web::discover::native(crate::web::discover::Runtime::full());
        surfaces.push(Surface {
            id: "app".into(),
            title: "App".into(),
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::root(),
            producer: "site".into(),
            artifact: Some("site/public".into()),
            index: Some("index.html".into()),
            availability: Availability::Both,
            provenance: BTreeMap::new(),
        });
        let s = StaticSurfaces::new(&Topology::new(surfaces), tmp.path());
        // the application answers what nothing else claims
        assert_eq!(s.handle("GET", "/").unwrap().status, 200);
        // and never what the executable answers itself
        for path in [
            "/docs",
            "/openapi.json",
            "/mcp",
            "/cockpit",
            "/api/v1/capabilities",
        ] {
            assert!(
                s.handle("GET", path).is_none(),
                "{path} was taken by the application"
            );
            assert!(!s.owns(path), "{path}");
        }
    }

    #[test]
    fn a_write_to_a_static_surface_is_refused_rather_than_ignored() {
        let (_tmp, s) = fixture();
        assert_eq!(s.handle("POST", "/tests/").unwrap().status, 405);
    }

    #[test]
    fn a_surface_whose_producer_has_not_run_is_not_mounted() {
        let tmp = tempfile::tempdir().unwrap();
        let topology = Topology::new(vec![Surface {
            id: "benchmarks".into(),
            title: "Benchmarks".into(),
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::parse("/benchmarks").unwrap(),
            producer: "test".into(),
            artifact: Some("target/web/benchmarks".into()),
            index: Some("index.html".into()),
            availability: Availability::Both,
            provenance: BTreeMap::new(),
        }]);
        let surfaces = StaticSurfaces::new(&topology, tmp.path());
        assert!(surfaces.is_empty());
        assert!(surfaces.handle("GET", "/benchmarks/").is_none());
    }
}
