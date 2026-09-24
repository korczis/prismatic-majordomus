//! Serving a static surface: a generated directory answered under its mount.
//!
//! The directory is the producer's output and this module never writes to it. What it adds
//! is the three things a directory of files is not: a request path resolved safely into it,
//! a media type, and a validator for the bytes it hands back.
//!
//! Safety is by construction rather than by inspection. A request path is decomposed into
//! segments, every segment is checked against a conservative character set, and any segment
//! that is empty, `.` or `..` refuses the request before the filesystem is touched; the
//! resolved path is then canonicalised and required to still be inside the canonical root,
//! which closes the door a symlink inside the directory would otherwise open. There is no
//! path concatenation of untrusted text anywhere in this file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::http::router::Response;

use super::model::Surface;

/// Media types by file extension. A file whose extension is not here is not served: a
/// generated directory that grows something unexpected must not turn this into a general
/// file server.
const MEDIA_TYPES: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("xml", "application/xml"),
    ("txt", "text/plain; charset=utf-8"),
    ("md", "text/markdown; charset=utf-8"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("ico", "image/x-icon"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
    ("ttf", "font/ttf"),
    ("pdf", "application/pdf"),
];

/// How long a browser may keep a document. Conservative: the pages are rebuilt from a
/// working tree a person is editing, and a cached page that is one build behind is worse
/// than a request.
pub const DOCUMENT_CACHE: &str = "no-cache";

/// How long a browser may keep an asset whose name carries a content hash. Zola writes
/// such names for nothing today, so this is used only where the name proves it is safe.
pub const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

/// The largest file answered from memory. Anything larger is read per request rather than
/// held: a documentation tree is small, and a cache with no ceiling is a leak with a plan.
pub const CACHE_MAX_BYTES: usize = 512 * 1024;

/// A static surface, ready to answer.
///
/// Built once, when the router is: the root is resolved and the surface's own metadata is
/// kept, so answering a request costs one path check and one read at most.
#[derive(Debug)]
pub struct Files {
    /// The surface this serves, for diagnostics and for the page that says it is missing.
    surface: Surface,
    /// The canonical directory, when it exists. Absent means the producer has not run.
    root: Option<PathBuf>,
    /// The repository-relative directory, named in every diagnostic.
    artifact: String,
    /// What has been read, by relative path.
    cached: Mutex<BTreeMap<String, Loaded>>,
}

#[derive(Debug, Clone)]
struct Loaded {
    body: Vec<u8>,
    media_type: &'static str,
}

impl Files {
    /// Prepare to serve `surface` out of `root`, the repository root.
    ///
    /// A surface whose directory is absent is not an error here: it is a surface whose
    /// producer has not run, and every request under it answers with what to run.
    pub fn new(surface: &Surface, root: &Path) -> Self {
        let artifact = surface
            .artifact
            .as_ref()
            .map(|a| a.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let dir = surface.artifact.as_ref().map(|a| root.join(a));
        let canonical = dir.as_ref().and_then(|d| d.canonicalize().ok());
        Files {
            surface: surface.clone(),
            root: canonical,
            artifact,
            cached: Mutex::new(BTreeMap::new()),
        }
    }

    /// Is the producer's output there?
    pub fn available(&self) -> bool {
        self.root.is_some()
    }

    /// The surface being served.
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    /// Answer a request whose path this surface owns.
    pub fn respond(&self, path: &str) -> Response {
        let Some(root) = &self.root else {
            return self.unavailable();
        };
        // a mount reached without its trailing slash resolves relative links one level too
        // high in every browser; send it to the canonical form rather than serving there
        if path == self.surface.mount.as_str() && !self.surface.mount.is_root() {
            return Response::new(
                308,
                "text/plain; charset=utf-8",
                format!("{}\n", self.surface.mount.prefix()),
            )
            .with_header("Location", self.surface.mount.prefix());
        }
        let Some(relative) = self.relative(path).and_then(|r| safe_relative(&r)) else {
            return Response::error(
                400,
                "invalid_input",
                &format!("'{path}' is not a path under {}", self.surface.mount),
            );
        };
        if let Ok(cache) = self.cached.lock() {
            if let Some(hit) = cache.get(&relative) {
                return document(hit.body.clone(), hit.media_type);
            }
        }
        let Some(resolved) = resolve(root, &relative, self.surface.index.as_deref()) else {
            return Response::error(
                404,
                "not_found",
                &format!(
                    "no file for '{path}' in {} ({}); the surface is '{}', built by {}",
                    self.artifact, self.surface.title, self.surface.id, self.surface.producer
                ),
            );
        };
        let Some(media_type) = media_type(&resolved.name) else {
            return Response::error(
                404,
                "not_found",
                &format!(
                    "'{path}' names a file this server does not serve; {} answers the types it generates",
                    self.surface.id
                ),
            );
        };
        self.read(&resolved.path, &relative, media_type)
    }

    /// The path of a request relative to the mount, or none when it is not under it.
    ///
    /// The mount itself and the mount with a trailing slash both resolve to the empty
    /// path, which is the surface's index.
    fn relative(&self, path: &str) -> Option<String> {
        let mount = &self.surface.mount;
        let rest = if mount.is_root() {
            path.strip_prefix('/')?
        } else if path == mount.as_str() {
            ""
        } else {
            path.strip_prefix(&mount.prefix())?
        };
        Some(rest.to_string())
    }

    fn read(&self, path: &Path, name: &str, media_type: &'static str) -> Response {
        let body = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(
                    surface = %self.surface.id,
                    file = name,
                    "a file the surface offers cannot be read: {e}"
                );
                return Response::error(
                    500,
                    "internal",
                    &format!(
                        "'{}/{name}' is listed by {} and cannot be read: {e}",
                        self.artifact, self.surface.id
                    ),
                );
            }
        };
        if body.len() <= CACHE_MAX_BYTES {
            if let Ok(mut cache) = self.cached.lock() {
                cache.insert(
                    name.to_string(),
                    Loaded {
                        body: body.clone(),
                        media_type,
                    },
                );
            }
        }
        document(body, media_type)
    }

    /// What a request under a surface whose producer has not run answers with: the reason,
    /// the directory that is missing, the command that writes it, and that this process
    /// resolved its surfaces once at start.
    ///
    /// Restart-based rediscovery is this executable's contract everywhere — the index, the
    /// registry and the topology are all read at start and immutable for the process — so
    /// building the directory while the server runs does not make it appear, and saying so
    /// is cheaper than a reader wondering why.
    fn unavailable(&self) -> Response {
        Response::error(
            503,
            "unavailable",
            &format!(
                "'{}' ({}) is not built: {} does not exist when this process started. Run: {} — then restart this server",
                self.surface.id, self.surface.title, self.artifact, self.surface.producer
            ),
        )
    }
}

fn document(body: Vec<u8>, media_type: &'static str) -> Response {
    Response::new(200, media_type, body).with_header("Cache-Control", DOCUMENT_CACHE)
}

/// A file inside a surface's directory, resolved.
struct Resolved {
    /// The path on disk.
    path: PathBuf,
    /// The name relative to the root, from which the media type is read.
    name: String,
}

/// Resolve a relative request path inside `root`, applying the index rule.
///
/// The request path is decomposed and every segment checked before anything touches the
/// filesystem; the result is canonicalised and required to still be inside the canonical
/// root, so neither `..` nor a symlink can leave the surface.
fn resolve(root: &Path, relative: &str, index: Option<&str>) -> Option<Resolved> {
    let mut name = safe_relative(relative)?;
    if name.is_empty() || name.ends_with('/') {
        name.push_str(index?);
    }
    let mut candidate = root.join(&name);
    if candidate.is_dir() {
        name = format!("{}/{}", name.trim_end_matches('/'), index?);
        candidate = root.join(&name);
    }
    let resolved = candidate.canonicalize().ok()?;
    if !resolved.starts_with(root) {
        tracing::warn!(
            file = %relative,
            "a request resolved outside the surface's directory and was refused"
        );
        return None;
    }
    if !resolved.is_file() {
        return None;
    }
    Some(Resolved {
        path: resolved,
        name,
    })
}

/// A request path made safe to join onto a surface's root, or none.
///
/// The character set is what the producers write and nothing wider. `!` is in it because
/// rustdoc names the redirect stub of every macro `macro.<name>!.html`, and a reference
/// that 404s on its own macros is a reference with holes: it is a legal path character
/// that no browser escapes, it cannot spell `.`, `..` or a separator, and it therefore
/// opens no route out of the directory that the segment checks do not already close.
/// `%` stays out, so an escaped traversal is refused as text rather than decoded into one.
///
/// ```
/// use majordomus_cli::web::files::safe_relative;
/// assert_eq!(safe_relative("cli/index.html").as_deref(), Some("cli/index.html"));
/// assert_eq!(safe_relative("").as_deref(), Some(""));
/// assert_eq!(safe_relative("cli/").as_deref(), Some("cli/"));
/// assert_eq!(safe_relative("m/macro.x!.html").as_deref(), Some("m/macro.x!.html"));
/// assert_eq!(safe_relative("../Cargo.toml"), None);
/// assert_eq!(safe_relative("a/../../b"), None);
/// assert_eq!(safe_relative("/etc/passwd"), None);
/// assert_eq!(safe_relative("a//b"), None);
/// assert_eq!(safe_relative("a\0b"), None);
/// assert_eq!(safe_relative("%2e%2e/b"), None);
/// ```
pub fn safe_relative(path: &str) -> Option<String> {
    if path.len() > 1024 {
        return None;
    }
    if path.is_empty() {
        return Some(String::new());
    }
    if path.starts_with('/') {
        return None;
    }
    let trailing = path.ends_with('/');
    let body = path.trim_end_matches('/');
    if body.is_empty() {
        return Some(String::new());
    }
    for segment in body.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        if !segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-~@+()'!".contains(&b))
        {
            return None;
        }
    }
    Some(if trailing {
        format!("{body}/")
    } else {
        body.to_string()
    })
}

/// The media type of a file name, when it is one a static surface serves.
///
/// ```
/// use majordomus_cli::web::files::media_type;
/// assert_eq!(media_type("index.html"), Some("text/html; charset=utf-8"));
/// assert_eq!(media_type("logo.png"), Some("image/png"));
/// assert_eq!(media_type("secrets.env"), None);
/// assert_eq!(media_type("Makefile"), None);
/// ```
pub fn media_type(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    MEDIA_TYPES
        .iter()
        .find(|(e, _)| *e == extension)
        .map(|(_, t)| *t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::model::{Availability, Category, Mount, SurfaceKind, Visibility};

    fn surface(mount: &str, artifact: &str) -> Surface {
        Surface {
            id: "docs".into(),
            title: "Documentation".into(),
            category: Category::Documentation,
            visibility: Visibility::Public,
            kind: SurfaceKind::StaticDirectory,
            mount: Mount::parse(mount).unwrap(),
            producer: "scripts/site-build --serve".into(),
            feature: None,
            artifact: Some(artifact.into()),
            index: Some("index.html".into()),
            availability: Availability::ServedOnly,
            built_from: None,
            provenance: Default::default(),
        }
    }

    fn tree() -> (tempfile::TempDir, Files) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("target/web/docs");
        std::fs::create_dir_all(dir.join("cli")).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>Documentation</h1>").unwrap();
        std::fs::write(dir.join("cli/index.html"), "<h1>CLI</h1>").unwrap();
        std::fs::write(dir.join("style.css"), "body{}").unwrap();
        std::fs::write(dir.join("secrets.env"), "TOKEN=1").unwrap();
        let files = Files::new(&surface("/docs", "target/web/docs"), tmp.path());
        (tmp, files)
    }

    #[test]
    fn the_mount_answers_with_the_index_and_its_bare_form_redirects() {
        let (_tmp, files) = tree();
        assert!(files.available());
        let index = files.respond("/docs/");
        assert_eq!(index.status, 200);
        assert_eq!(index.content_type, "text/html; charset=utf-8");
        assert!(index.body.text().contains("Documentation"));

        let bare = files.respond("/docs");
        assert_eq!(bare.status, 308);
        assert!(bare
            .headers
            .iter()
            .any(|(k, v)| k == "Location" && v == "/docs/"));
    }

    #[test]
    fn a_nested_page_and_an_asset_are_served_with_their_types() {
        let (_tmp, files) = tree();
        assert!(files.respond("/docs/cli/").body.text().contains("CLI"));
        assert!(files.respond("/docs/cli").body.text().contains("CLI"));
        let css = files.respond("/docs/style.css");
        assert_eq!(css.content_type, "text/css; charset=utf-8");
        assert!(css
            .headers
            .iter()
            .any(|(k, v)| k == "Cache-Control" && v == DOCUMENT_CACHE));
    }

    #[test]
    fn nothing_escapes_the_surface() {
        let (tmp, files) = tree();
        std::fs::write(tmp.path().join("secret.html"), "<b>outside</b>").unwrap();
        for hostile in [
            "/docs/../secret.html",
            "/docs/../../etc/passwd",
            "/docs/cli/../../secret.html",
            "/docs/%2e%2e/secret.html",
            "/docs//etc/passwd",
        ] {
            let response = files.respond(hostile);
            assert!(
                response.status == 400 || response.status == 404,
                "{hostile} answered {}",
                response.status
            );
            assert!(!response.body.text().contains("outside"), "{hostile}");
        }
    }

    #[test]
    fn a_symlink_out_of_the_surface_is_refused() {
        let (tmp, files) = tree();
        std::fs::write(tmp.path().join("outside.html"), "<b>outside</b>").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            tmp.path().join("outside.html"),
            tmp.path().join("target/web/docs/link.html"),
        )
        .unwrap();
        let response = files.respond("/docs/link.html");
        assert_eq!(response.status, 404, "{}", response.body);
    }

    #[test]
    fn an_extension_the_surface_does_not_generate_is_not_served() {
        let (_tmp, files) = tree();
        assert_eq!(files.respond("/docs/secrets.env").status, 404);
    }

    #[test]
    fn a_surface_whose_producer_has_not_run_says_what_to_run() {
        let tmp = tempfile::tempdir().unwrap();
        let files = Files::new(&surface("/docs", "target/web/docs"), tmp.path());
        assert!(!files.available());
        let response = files.respond("/docs/");
        assert_eq!(response.status, 503);
        assert!(response.body.text().contains("scripts/site-build --serve"));
        assert!(response.body.text().contains("target/web/docs"));
    }

    #[test]
    fn a_surface_serves_only_paths_it_owns() {
        let (_tmp, files) = tree();
        // the mount's own subtree, and nothing else
        assert_eq!(files.respond("/docs/index.html").status, 200);
        assert_eq!(files.respond("/elsewhere/index.html").status, 400);
        assert_eq!(files.respond("/docsx").status, 400);
        assert_eq!(files.surface().id, "docs");
    }

    #[test]
    fn a_root_mount_answers_from_the_top_of_its_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("target/web/docs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>root</h1>").unwrap();
        let files = Files::new(&surface("/", "target/web/docs"), tmp.path());
        assert_eq!(files.respond("/").status, 200);
        assert!(files.respond("/").body.text().contains("root"));
        // the root mount has no bare form to redirect to
        assert_ne!(files.respond("/").status, 308);
    }

    #[test]
    fn a_surface_with_no_index_answers_nothing_for_its_mount() {
        let (tmp, _files) = tree();
        let mut without = surface("/docs", "target/web/docs");
        without.index = None;
        let files = Files::new(&without, tmp.path());
        assert_eq!(files.respond("/docs/").status, 404);
        // a named file is still served: only the mount itself has nothing to answer with
        assert_eq!(files.respond("/docs/style.css").status, 200);
    }

    #[test]
    fn a_media_type_is_read_from_the_name_whatever_its_case() {
        assert_eq!(media_type("LOGO.PNG"), Some("image/png"));
        assert_eq!(media_type("a.b.json"), Some("application/json"));
        assert_eq!(media_type("index.html"), media_type("INDEX.HTML"));
        assert_eq!(media_type(".hidden"), None);
        assert_eq!(safe_relative(&"a".repeat(2000)), None);
        assert_eq!(
            safe_relative("///"),
            None,
            "a relative path never starts at the root"
        );
        assert_eq!(safe_relative("a//"), Some("a/".to_string()));
    }

    /// A rustdoc tree as the producer leaves it, reduced to one file of every kind a
    /// browser asks for when it opens the reference: the landing page, the crate's page,
    /// the hashed stylesheet, script and font rustdoc writes under `static.files/`, and the
    /// redirect stub rustdoc names after a macro with its `!`.
    fn rustdoc_tree() -> (tempfile::TempDir, Files) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("target/web/rustdoc");
        std::fs::create_dir_all(dir.join("static.files")).unwrap();
        std::fs::create_dir_all(dir.join("majordomus_cli")).unwrap();
        std::fs::write(dir.join("index.html"), "<h1>landing</h1>").unwrap();
        std::fs::write(dir.join("static.files/a.css"), "body{}").unwrap();
        std::fs::write(dir.join("static.files/b.js"), "var b;").unwrap();
        // not UTF-8, as a font is not: the bytes must come back untouched
        std::fs::write(
            dir.join("static.files/c.woff2"),
            [0x77, 0x4f, 0x46, 0x32, 0xff],
        )
        .unwrap();
        std::fs::write(dir.join("majordomus_cli/index.html"), "<h1>crate</h1>").unwrap();
        std::fs::write(
            dir.join("majordomus_cli/macro.m!.html"),
            "<meta http-equiv=\"refresh\" content=\"0;URL=macro.m.html\">",
        )
        .unwrap();
        let mut surface = surface("/rustdoc", "target/web/rustdoc");
        surface.id = "rustdoc".into();
        surface.availability = Availability::Both;
        let files = Files::new(&surface, tmp.path());
        (tmp, files)
    }

    #[test]
    fn a_macros_redirect_stub_is_served_under_the_name_rustdoc_gives_it() {
        let (_tmp, files) = rustdoc_tree();
        let stub = files.respond("/rustdoc/majordomus_cli/macro.m!.html");
        assert_eq!(stub.status, 200, "{}", stub.body);
        assert_eq!(stub.content_type, "text/html; charset=utf-8");
        assert!(stub.body.text().contains("URL=macro.m.html"));
        // a `!` names a file; it does not find one that is not there
        assert_eq!(
            files
                .respond("/rustdoc/majordomus_cli/macro.x!.html")
                .status,
            404
        );
    }

    #[test]
    fn every_file_a_rustdoc_tree_needs_is_answered_with_its_type() {
        let (_tmp, files) = rustdoc_tree();
        for (path, media) in [
            ("/rustdoc/", "text/html; charset=utf-8"),
            ("/rustdoc/index.html", "text/html; charset=utf-8"),
            ("/rustdoc/majordomus_cli/", "text/html; charset=utf-8"),
            (
                "/rustdoc/majordomus_cli/index.html",
                "text/html; charset=utf-8",
            ),
            ("/rustdoc/static.files/a.css", "text/css; charset=utf-8"),
            (
                "/rustdoc/static.files/b.js",
                "text/javascript; charset=utf-8",
            ),
            ("/rustdoc/static.files/c.woff2", "font/woff2"),
        ] {
            let response = files.respond(path);
            assert_eq!(response.status, 200, "{path}: {}", response.body);
            assert_eq!(response.content_type, media, "{path}");
        }
        let font = files.respond("/rustdoc/static.files/c.woff2");
        assert_eq!(font.body.as_bytes(), [0x77, 0x4f, 0x46, 0x32, 0xff]);
        let bare = files.respond("/rustdoc");
        assert_eq!(bare.status, 308);
        assert!(bare
            .headers
            .iter()
            .any(|(k, v)| k == "Location" && v == "/rustdoc/"));
        // and every extension a rustdoc build carries, whether or not this tree has one:
        // measured over a real `cargo doc` output (css, html, js, md, png, svg, txt, woff2),
        // with the favicon and search types a newer rustdoc may add
        for (name, media) in [
            ("a.html", "text/html; charset=utf-8"),
            ("a.css", "text/css; charset=utf-8"),
            ("a.js", "text/javascript; charset=utf-8"),
            ("a.woff2", "font/woff2"),
            ("a.svg", "image/svg+xml"),
            ("a.png", "image/png"),
            ("a.ico", "image/x-icon"),
            ("a.json", "application/json"),
            ("a.txt", "text/plain; charset=utf-8"),
            ("a.md", "text/markdown; charset=utf-8"),
        ] {
            assert_eq!(media_type(name), Some(media), "{name}");
        }
    }

    #[test]
    fn allowing_a_bang_opens_no_way_out_of_the_surface() {
        let (tmp, files) = rustdoc_tree();
        std::fs::write(tmp.path().join("secret.html"), "<b>outside</b>").unwrap();
        std::fs::write(tmp.path().join("target/web/secret!.html"), "<b>outside</b>").unwrap();
        for hostile in [
            "/rustdoc/../secret.html",
            "/rustdoc/../secret!.html",
            "/rustdoc/!/../../secret.html",
            "/rustdoc/majordomus_cli/../../secret!.html",
            "/rustdoc/%2e%2e/secret.html",
            "/rustdoc/%2E%2E/secret!.html",
            "/rustdoc/..%2fsecret.html",
            "/rustdoc/%21/../../secret.html",
            "/rustdoc//secret.html",
            "/rustdoc/./index.html",
            "/rustdoc/..!/secret.html",
        ] {
            let response = files.respond(hostile);
            assert!(
                response.status == 400 || response.status == 404,
                "{hostile} answered {}",
                response.status
            );
            assert!(!response.body.text().contains("outside"), "{hostile}");
        }
        // a symbolic link named like a macro stub is still a link out, and still refused
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                tmp.path().join("secret.html"),
                tmp.path().join("target/web/rustdoc/macro.leak!.html"),
            )
            .unwrap();
            assert_eq!(files.respond("/rustdoc/macro.leak!.html").status, 404);
        }
    }

    #[test]
    fn a_file_is_read_once_and_answered_from_memory_afterwards() {
        let (tmp, files) = tree();
        assert_eq!(files.respond("/docs/").status, 200);
        std::fs::remove_file(tmp.path().join("target/web/docs/index.html")).unwrap();
        assert_eq!(files.respond("/docs/").status, 200);
    }
}
