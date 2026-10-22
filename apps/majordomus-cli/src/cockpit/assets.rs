//! The Cockpit's static files, served from the distribution's `share/cockpit/`.
//!
//! Read once and kept: an asset is loaded from disk the first time it is asked for and
//! answered from memory afterwards, with the digest of its bytes as its cache key. The
//! digest is in the URL the pages emit, so a changed file is a changed URL and the
//! browser's cache never has to be told anything.
//!
//! Only the URL a page emits is immutable: a request whose `v` is anything but the file's
//! current version is answered `no-cache`, and every answer carries the digest as its
//! `ETag`, so a revalidation that still holds is a `304` with no body. A script module's
//! relative imports are rewritten, when it is read, to the digested URLs of the modules
//! they name: a module's digest then covers what it imports, so a change to `cockpit.js`
//! is a new URL for every module that imports it, and every importer names the one URL.
//!
//! Nothing is compiled into the executable. `share/` is where this tool keeps its data —
//! the kinds, the schemas, the allow-lists — and the Cockpit's stylesheet and scripts are
//! more of the same, locatable per invocation and replaceable without a rebuild.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use crate::http::router::{Request, Response};

/// The directory inside the share directory that holds the Cockpit's files.
pub const DIR: &str = "cockpit";

/// The URL prefix every asset is served under.
pub const PREFIX: &str = "/cockpit/assets/";

/// Media types by file extension. An extension not named here is not served: the Cockpit
/// ships text, and a directory that grows a binary by accident should not become a file
/// server.
const MEDIA_TYPES: &[(&str, &str)] = &[
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("svg", "image/svg+xml"),
    ("txt", "text/plain; charset=utf-8"),
];

/// How long a browser may keep an asset whose URL carries the right digest. A year: the
/// URL changes when the bytes change, so the entry can never be wrong.
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// How many hex digits of the digest a URL carries as its `v`.
const VERSION_LEN: usize = 16;

/// One file, read. Kept behind an `Arc`, so that naming its URL or answering its
/// revalidation copies nothing but a pointer.
#[derive(Debug)]
struct Loaded {
    body: String,
    media_type: &'static str,
    digest: String,
}

impl Loaded {
    /// The `v` a URL carries: the first [`VERSION_LEN`] hex digits of the digest.
    fn version(&self) -> &str {
        &self.digest[..VERSION_LEN.min(self.digest.len())]
    }

    /// The validator every answer carries.
    fn etag(&self) -> String {
        format!("\"{}\"", self.digest)
    }
}

/// A surface's asset directory, with what has been read from it: the Cockpit's
/// `share/cockpit/` under `/cockpit/assets/`, or another surface's beside it (ADR 0135).
#[derive(Debug)]
pub struct Assets {
    root: Option<PathBuf>,
    dir: &'static str,
    prefix: &'static str,
    loaded: Mutex<BTreeMap<String, Option<Arc<Loaded>>>>,
}

impl Assets {
    /// The assets under `share_dir/cockpit`, when that directory exists. A distribution
    /// without one serves no assets and every page still renders: the stylesheet link and
    /// the module scripts simply resolve to 404, and the Cockpit degrades to unstyled
    /// server-rendered HTML rather than failing.
    pub fn new(share_dir: &Path) -> Self {
        Assets::under(share_dir, DIR, PREFIX)
    }

    /// The assets under `share_dir/<dir>`, served under `prefix`: one surface's files, read
    /// and digested exactly as the Cockpit's are. A surface does not get a second file
    /// server; it gets this one pointed at its own directory.
    ///
    /// ```
    /// use majordomus_cli::cockpit::assets::Assets;
    /// let dir = tempfile::tempdir().unwrap();
    /// std::fs::create_dir_all(dir.path().join("ui")).unwrap();
    /// std::fs::write(dir.path().join("ui/ui.css"), "body{}").unwrap();
    /// let ui = Assets::under(dir.path(), "ui", "/ui/assets/");
    /// assert!(ui.url("ui.css").starts_with("/ui/assets/ui.css?v="));
    /// assert!(!Assets::new(dir.path()).present(), "the Cockpit's directory is another one");
    /// ```
    pub fn under(share_dir: &Path, dir: &'static str, prefix: &'static str) -> Self {
        let root = share_dir.join(dir);
        Assets {
            root: root.is_dir().then_some(root),
            dir,
            prefix,
            loaded: Mutex::new(BTreeMap::new()),
        }
    }

    /// No asset directory at all: for a router built without a distribution.
    pub fn none() -> Self {
        Assets {
            root: None,
            dir: DIR,
            prefix: PREFIX,
            loaded: Mutex::new(BTreeMap::new()),
        }
    }

    /// No asset directory, for a surface whose files would live under `dir` and `prefix`:
    /// nothing is servable, and every URL keeps its plain path under that surface's prefix,
    /// so a missing file is a 404 that names where it was looked for.
    ///
    /// ```
    /// use majordomus_cli::cockpit::assets::Assets;
    /// let ui = Assets::none_under("ui", "/ui/assets/");
    /// assert!(!ui.present());
    /// assert!(!ui.has("ui.css"));
    /// assert_eq!(ui.url("ui.css"), "/ui/assets/ui.css");
    /// ```
    pub fn none_under(dir: &'static str, prefix: &'static str) -> Self {
        Assets {
            dir,
            prefix,
            ..Assets::none()
        }
    }

    /// Is there an asset directory?
    pub fn present(&self) -> bool {
        self.root.is_some()
    }

    /// Is this file there and servable?
    pub fn has(&self, name: &str) -> bool {
        self.load(name).is_some()
    }

    /// The URL a page emits for an asset: the path with the digest of its bytes, so the
    /// URL changes exactly when the file does. An asset that is not there keeps its plain
    /// path, so the 404 names the file a reader should look for.
    pub fn url(&self, name: &str) -> String {
        match self.load(name) {
            Some(a) => format!("{}{name}?v={}", self.prefix, a.version()),
            None => format!("{}{name}", self.prefix),
        }
    }

    /// Answer a request for `prefix + name`. Immutable only when the request's `v` is
    /// exactly the version the URL a page emits carries; anything else (no `v`, an empty
    /// one, a prefix, a stale one) revalidates. An `If-None-Match` naming the current
    /// `ETag` is answered `304 Not Modified` with no body: the 200's validator, caching
    /// and length, and nothing else crosses the wire.
    pub fn respond(&self, name: &str, req: &Request) -> Response {
        let Some(asset) = self.load(name) else {
            return Response::error(
                404,
                "not_found",
                &format!(
                    "no asset '{name}'; the distribution's share/{}/ holds the ones there are",
                    self.dir
                ),
            );
        };
        let version = req
            .query
            .iter()
            .find(|(k, _)| k == "v")
            .map(|(_, v)| v.as_str());
        let cache = if version == Some(asset.version()) {
            IMMUTABLE
        } else {
            "no-cache"
        };
        let etag = asset.etag();
        let response = if req
            .header("If-None-Match")
            .is_some_and(|candidates| none_match_holds(candidates, &etag))
        {
            // the length a 304 announces is the 200's (RFC 9110 section 8.6), never the
            // empty body's; the server writes no body for this status
            Response::new(304, asset.media_type, String::new())
                .with_header("Content-Length", asset.body.len().to_string())
        } else {
            Response::new(200, asset.media_type, asset.body.clone())
        };
        response
            .with_header("Cache-Control", cache)
            .with_header("ETag", etag)
    }

    fn load(&self, name: &str) -> Option<Arc<Loaded>> {
        self.load_from(name, &mut Vec::new())
    }

    /// Load `name`, with `visiting` the modules whose reading is under way above it: a
    /// module that imports one of them is a cycle, and that one import keeps its plain
    /// URL rather than recursing forever.
    fn load_from(&self, name: &str, visiting: &mut Vec<String>) -> Option<Arc<Loaded>> {
        if let Some(cached) = self.loaded.lock().ok()?.get(name) {
            return cached.clone();
        }
        visiting.push(name.to_string());
        let loaded = self.read(name, visiting).map(Arc::new);
        visiting.pop();
        if let Ok(mut map) = self.loaded.lock() {
            map.insert(name.to_string(), loaded.clone());
        }
        loaded
    }

    fn read(&self, name: &str, visiting: &mut Vec<String>) -> Option<Loaded> {
        let root = self.root.as_ref()?;
        let relative = safe_name(name)?;
        let media_type = media_type(&relative)?;
        let path = root.join(&relative);
        // the name is already known to hold no `..` and no absolute segment; canonicalising
        // both ends closes the door a symlink inside the directory would otherwise open
        let resolved = path.canonicalize().ok()?;
        let root = root.canonicalize().ok()?;
        if !resolved.starts_with(&root) {
            tracing::warn!(
                asset = name,
                dir = self.dir,
                "an asset resolves outside its share directory; refused"
            );
            return None;
        }
        let mut body = std::fs::read_to_string(&resolved).ok()?;
        if is_script(&relative) {
            body = rewrite_imports(&body, |specifier| {
                let target = resolve_relative(&relative, specifier)?;
                if visiting.contains(&target) {
                    return None;
                }
                let loaded = self.load_from(&target, visiting)?;
                Some(loaded.version().to_string())
            });
        }
        Some(Loaded {
            digest: digest(body.as_bytes()),
            body,
            media_type,
        })
    }
}

/// Does an `If-None-Match` value name this `ETag`? The weak comparison the header is
/// defined with: `*`, or any member of the list once a `W/` is set aside.
fn none_match_holds(candidates: &str, etag: &str) -> bool {
    candidates.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

/// Is this file a script module, whose relative imports are rewritten to digested URLs?
fn is_script(name: &str) -> bool {
    name.ends_with(".js") || name.ends_with(".mjs")
}

/// Rewrite every relative module specifier a script names, in `import ... from`,
/// `export ... from`, a bare `import '...'` and an `import('...')` with a literal, to the
/// same specifier with `?v=` and the version `version_of` gives it. A specifier
/// `version_of` has no version for (a file that is not there, a cycle) is left as it is,
/// which still works and only revalidates.
///
/// Only a quoted string that starts with `./` or `../`, directly after one of those
/// keywords, is a candidate, so a string elsewhere in a minified library is not touched.
fn rewrite_imports(source: &str, mut version_of: impl FnMut(&str) -> Option<String>) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len() + 64);
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        let quote = bytes[i];
        // a quote is one byte, so the text on either side of it is whole characters
        let quoted = quote == b'\'' || quote == b'"';
        let rest = if quoted { &source[i + 1..] } else { "" };
        if quoted
            && (rest.starts_with("./") || rest.starts_with("../"))
            && follows_an_import(&source[..i])
        {
            if let Some(length) = rest.find(quote as char) {
                let specifier = &rest[..length];
                let end = i + 1 + length;
                if let Some(version) = plain_module_specifier(specifier)
                    .then(|| version_of(specifier))
                    .flatten()
                {
                    out.push_str(&source[copied..end]);
                    out.push_str("?v=");
                    out.push_str(&version);
                    copied = end;
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&source[copied..]);
    out
}

/// Does the text before a quote end in `from`, `import`, or `import(` — as whole words, and
/// not as a property (`x.from(`)?
fn follows_an_import(before: &str) -> bool {
    let before = before.trim_end();
    let (before, keywords): (&str, &[&str]) = match before.strip_suffix('(') {
        Some(call) => (call.trim_end(), &["import"]),
        None => (before, &["from", "import"]),
    };
    keywords.iter().any(|keyword| {
        before.strip_suffix(keyword).is_some_and(|head| {
            !head
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || "_$.".contains(c))
        })
    })
}

/// A specifier this rewrite may version: a path of plain characters to a script, with no
/// query or fragment of its own.
fn plain_module_specifier(specifier: &str) -> bool {
    is_script(specifier)
        && specifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
}

/// The asset a relative specifier names from the module `from`, or `None` when it climbs
/// out of the asset directory.
fn resolve_relative(from: &str, specifier: &str) -> Option<String> {
    let mut segments: Vec<&str> = from.split('/').collect();
    segments.pop();
    for segment in specifier.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    safe_name(&segments.join("/"))
}

/// The hex SHA-256 of some bytes: an asset's identity in its URL and in its `ETag`.
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// The digest cache shared by every router of the process, so two routers over one
/// distribution read each file once between them.
pub fn shared(share_dir: &Path) -> &'static Assets {
    static SHARED: OnceLock<Assets> = OnceLock::new();
    SHARED.get_or_init(|| Assets::new(share_dir))
}

/// A relative path safe to join onto the asset root: no absolute segment, no `..`, no
/// empty segment, and nothing outside a conservative character set.
///
/// ```
/// use majordomus_cli::cockpit::assets::safe_name;
/// assert_eq!(safe_name("js/graph.js").as_deref(), Some("js/graph.js"));
/// assert_eq!(safe_name("../../etc/passwd"), None);
/// assert_eq!(safe_name("/etc/passwd"), None);
/// assert_eq!(safe_name("js//graph.js"), None);
/// assert_eq!(safe_name("js/../../x"), None);
/// assert_eq!(safe_name("a b.css"), None);
/// ```
pub fn safe_name(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > 200 {
        return None;
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
    {
        return None;
    }
    let segments: Vec<&str> = name.split('/').collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return None;
    }
    Some(segments.join("/"))
}

/// The media type of a file name, when it is one the Cockpit serves.
///
/// ```
/// use majordomus_cli::cockpit::assets::media_type;
/// assert_eq!(media_type("cockpit.css"), Some("text/css; charset=utf-8"));
/// assert_eq!(media_type("x.wasm"), None);
/// assert_eq!(media_type("noextension"), None);
/// ```
pub fn media_type(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1;
    MEDIA_TYPES
        .iter()
        .find(|(e, _)| *e == extension)
        .map(|(_, t)| *t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GET for `target`, with further request headers.
    fn get(target: &str, headers: &[(&str, &str)]) -> Request {
        Request::parse_target("GET", target, vec![]).with_headers(
            headers
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        )
    }

    fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
        response
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// A share directory whose `cockpit/` holds `files`.
    fn share(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (name, body) in files {
            let path = dir.path().join(DIR).join(name);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
            std::fs::write(path, body).expect("write");
        }
        dir
    }

    #[test]
    fn an_absent_directory_serves_nothing_and_does_not_panic() {
        let assets = Assets::new(Path::new("/nonexistent-share-directory"));
        assert!(!assets.present());
        assert_eq!(assets.respond("cockpit.css", &get("/", &[])).status, 404);
        assert_eq!(assets.url("cockpit.css"), "/cockpit/assets/cockpit.css");
    }

    #[test]
    fn a_traversal_is_refused_before_the_filesystem_is_touched() {
        for hostile in [
            "../Cargo.toml",
            "../../etc/passwd",
            "/etc/passwd",
            "a/../../b",
            "",
        ] {
            assert_eq!(safe_name(hostile), None, "{hostile}");
        }
    }

    #[test]
    fn a_file_is_read_once_and_answered_with_its_digest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cockpit = dir.path().join(DIR);
        std::fs::create_dir_all(&cockpit).expect("mkdir");
        std::fs::write(cockpit.join("cockpit.css"), "body{color:red}").expect("write");
        let assets = Assets::new(dir.path());
        assert!(assets.present());

        let url = assets.url("cockpit.css");
        assert!(url.starts_with("/cockpit/assets/cockpit.css?v="), "{url}");
        let version = url.split_once("?v=").expect("a version").1.to_string();

        let hit = assets.respond("cockpit.css", &get(&url, &[]));
        assert_eq!(hit.status, 200);
        assert_eq!(hit.body, "body{color:red}");
        assert!(hit
            .headers
            .iter()
            .any(|(k, v)| k == "Cache-Control" && v == IMMUTABLE));

        let unversioned = assets.respond("cockpit.css", &get("/", &[]));
        assert!(unversioned
            .headers
            .iter()
            .any(|(k, v)| k == "Cache-Control" && v == "no-cache"));

        // the file is gone and the answer is the same: it was read once
        std::fs::remove_file(cockpit.join("cockpit.css")).expect("remove");
        assert_eq!(assets.respond("cockpit.css", &get("/", &[])).status, 200);
        assert_eq!(version.len(), VERSION_LEN);
    }

    #[test]
    fn an_extension_the_cockpit_does_not_serve_is_not_served() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cockpit = dir.path().join(DIR);
        std::fs::create_dir_all(&cockpit).expect("mkdir");
        std::fs::write(cockpit.join("secrets.env"), "TOKEN=1").expect("write");
        let assets = Assets::new(dir.path());
        assert_eq!(assets.respond("secrets.env", &get("/", &[])).status, 404);
    }

    #[test]
    fn only_the_exact_current_version_is_immutable() {
        let dir = share(&[("cockpit.css", "body{color:red}")]);
        let assets = Assets::new(dir.path());
        let url = assets.url("cockpit.css");
        let version = url.split_once("?v=").expect("a version").1.to_string();
        let full = digest(b"body{color:red}");
        for target in [
            "/cockpit/assets/cockpit.css?v=".to_string(),
            format!("/cockpit/assets/cockpit.css?v={}", &version[..4]),
            format!("/cockpit/assets/cockpit.css?v={full}"),
            "/cockpit/assets/cockpit.css?v=0000000000000000".to_string(),
            "/cockpit/assets/cockpit.css".to_string(),
        ] {
            let answer = assets.respond("cockpit.css", &get(&target, &[]));
            assert_eq!(answer.status, 200, "{target}");
            assert_eq!(
                header(&answer, "Cache-Control"),
                Some("no-cache"),
                "{target}"
            );
            assert_eq!(
                header(&answer, "ETag").map(str::to_string),
                Some(format!("\"{full}\"")),
                "{target}"
            );
        }
        let exact = assets.respond("cockpit.css", &get(&url, &[]));
        assert_eq!(header(&exact, "Cache-Control"), Some(IMMUTABLE));
    }

    #[test]
    fn a_revalidation_that_still_holds_is_a_304_with_no_body() {
        let dir = share(&[("cockpit.js", "export const a = 1;")]);
        let assets = Assets::new(dir.path());
        let url = assets.url("cockpit.js");
        let fresh = assets.respond("cockpit.js", &get(&url, &[]));
        let etag = header(&fresh, "ETag").expect("an ETag").to_string();

        for (target, candidates, cache) in [
            (url.as_str(), etag.clone(), IMMUTABLE),
            ("/cockpit/assets/cockpit.js", etag.clone(), "no-cache"),
            (
                "/cockpit/assets/cockpit.js",
                format!("\"other\", W/{etag}"),
                "no-cache",
            ),
            ("/cockpit/assets/cockpit.js", "*".to_string(), "no-cache"),
        ] {
            let answer = assets.respond(
                "cockpit.js",
                &get(target, &[("if-none-match", &candidates)]),
            );
            assert_eq!(answer.status, 304, "{target} {candidates}");
            assert!(answer.body.is_empty(), "a 304 carries no body");
            assert_eq!(header(&answer, "ETag"), Some(etag.as_str()));
            assert_eq!(header(&answer, "Cache-Control"), Some(cache));
            assert_eq!(
                header(&answer, "Content-Length"),
                Some(fresh.body.len().to_string().as_str()),
                "the 200's length, not the empty body's"
            );
        }
        for stale in ["\"0000\"", ""] {
            let answer = assets.respond(
                "cockpit.js",
                &get("/cockpit/assets/cockpit.js", &[("If-None-Match", stale)]),
            );
            assert_eq!(answer.status, 200, "{stale:?}");
            assert_eq!(answer.body, "export const a = 1;");
        }
    }

    #[test]
    fn naming_an_asset_shares_its_bytes_rather_than_copying_them() {
        let dir = share(&[("cockpit.css", "body{}")]);
        let assets = Assets::new(dir.path());
        let first = assets.load("cockpit.css").expect("loaded");
        let _ = assets.url("cockpit.css");
        let second = assets.load("cockpit.css").expect("loaded");
        assert!(Arc::ptr_eq(&first, &second), "one body, shared");
    }

    #[test]
    fn a_module_names_its_imports_by_their_digested_urls() {
        let dir = share(&[
            ("cockpit.js", "// the shell — \"its\" helpers\nexport const api = 1;"),
            ("socket.js", "import { api } from './cockpit.js';\nexport const s = api;"),
            (
                "executions.js",
                "import { api } from './cockpit.js';\nimport { s } from \"./socket.js\";\nimport('./missing.js');",
            ),
            ("vendor/three.core.min.js", "export const V=1;"),
            (
                "vendor/three.module.min.js",
                "import{V as a}from\"./three.core.min.js\";export{a}from\"./three.core.min.js\";",
            ),
            ("cycle-a.js", "import './cycle-b.js';"),
            ("cycle-b.js", "import './cycle-a.js';"),
        ]);
        let assets = Assets::new(dir.path());
        let version = |name: &str| {
            assets
                .url(name)
                .split_once("?v=")
                .expect("a version")
                .1
                .to_string()
        };

        let core = version("cockpit.js");
        let socket = assets.respond("socket.js", &get("/", &[]));
        assert!(
            socket
                .body
                .text()
                .contains(&format!("from './cockpit.js?v={core}'")),
            "{}",
            socket.body
        );

        let executions = assets
            .respond("executions.js", &get("/", &[]))
            .body
            .text()
            .into_owned();
        assert!(
            executions.contains(&format!("'./cockpit.js?v={core}'")),
            "{executions}"
        );
        assert!(
            executions.contains(&format!("\"./socket.js?v={}\"", version("socket.js"))),
            "{executions}"
        );
        assert!(
            executions.contains("import('./missing.js')"),
            "an absent file keeps its plain URL"
        );

        // a module's version covers what it imports: its digest is of the rewritten text
        assert_eq!(
            assets.load("socket.js").expect("loaded").digest,
            digest(socket.body.as_bytes())
        );
        assert_ne!(
            assets.load("socket.js").expect("loaded").digest,
            digest(b"import { api } from './cockpit.js';\nexport const s = api;")
        );

        let three = assets
            .respond("vendor/three.module.min.js", &get("/", &[]))
            .body
            .text()
            .into_owned();
        let core3 = version("vendor/three.core.min.js");
        assert_eq!(
            three
                .matches(&format!("\"./three.core.min.js?v={core3}\""))
                .count(),
            2,
            "{three}"
        );

        // a cycle ends, with the import that closes it left plain
        assert!(assets.has("cycle-a.js"));
        assert!(assets.has("cycle-b.js"));
    }

    #[test]
    fn only_an_import_specifier_is_rewritten() {
        let v = |s: &str| (s == "./a.js").then(|| "1234".to_string());
        assert_eq!(
            rewrite_imports("import { x } from './a.js';", v),
            "import { x } from './a.js?v=1234';"
        );
        assert_eq!(
            rewrite_imports("export * from \"./a.js\"", v),
            "export * from \"./a.js?v=1234\""
        );
        assert_eq!(
            rewrite_imports("import './a.js'", v),
            "import './a.js?v=1234'"
        );
        assert_eq!(
            rewrite_imports("await import ( './a.js' )", v),
            "await import ( './a.js?v=1234' )"
        );
        for untouched in [
            "const s = './a.js';",
            "Array.from('./a.js')",
            "x.import('./a.js')",
            "reimport './a.js'",
            "import './b.js';",
            "import './a.js?v=1';",
            "import './a.css';",
            "import 'https://x/a.js';",
            "import './a.js",
            "// a dash — then 'quoted' — text",
        ] {
            assert_eq!(rewrite_imports(untouched, v), untouched);
        }
    }

    #[test]
    fn a_relative_specifier_resolves_inside_the_directory_or_not_at_all() {
        assert_eq!(
            resolve_relative("palette.js", "./cockpit.js").as_deref(),
            Some("cockpit.js")
        );
        assert_eq!(
            resolve_relative("vendor/three.module.min.js", "./three.core.min.js").as_deref(),
            Some("vendor/three.core.min.js")
        );
        assert_eq!(
            resolve_relative("vendor/x.js", "../cockpit.js").as_deref(),
            Some("cockpit.js")
        );
        assert_eq!(resolve_relative("cockpit.js", "../outside.js"), None);
    }

    #[test]
    fn if_none_match_is_compared_weakly_over_its_list() {
        assert!(none_match_holds("\"ab\"", "\"ab\""));
        assert!(none_match_holds(" \"x\" , W/\"ab\"", "\"ab\""));
        assert!(none_match_holds("*", "\"ab\""));
        assert!(!none_match_holds("\"a\"", "\"ab\""));
        assert!(!none_match_holds("", "\"ab\""));
    }
}
