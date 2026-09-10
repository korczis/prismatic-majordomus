//! A distribution's static files, served from a directory of `share/` under a URL prefix.
//!
//! Read once and kept: an asset is loaded from disk the first time it is asked for and
//! answered from memory afterwards, with the digest of its bytes as its cache key. The
//! digest is in the URL the pages emit, so a changed file is a changed URL and the
//! browser's cache never has to be told anything.
//!
//! Nothing is compiled into the executable. `share/` is where this tool keeps its data —
//! the kinds, the schemas, the allow-lists — and the Cockpit's stylesheet and scripts are
//! more of the same, locatable per invocation and replaceable without a rebuild.
//!
//! Two surfaces read from here: the Cockpit's `share/cockpit/` at [`PREFIX`], and the API
//! viewer's `share/swagger/` at [`crate::http::swagger::ASSET_PREFIX`]. Whose directory it
//! is, is a constructor argument; the reading, the digest, the traversal refusal and the
//! cache header are decided once, here, because two answers to one question drift.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::http::router::Response;

/// The directory inside the share directory that holds the Cockpit's files.
pub const DIR: &str = "cockpit";

/// The URL prefix the Cockpit's assets are served under.
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

/// One file, read.
#[derive(Debug, Clone)]
struct Loaded {
    body: String,
    media_type: &'static str,
    digest: String,
}

/// One served asset directory, with what has been read from it.
#[derive(Debug)]
pub struct Assets {
    root: Option<PathBuf>,
    prefix: &'static str,
    loaded: Mutex<BTreeMap<String, Option<Loaded>>>,
}

impl Assets {
    /// The Cockpit's assets under `share_dir/cockpit`, when that directory exists. A
    /// distribution without one serves no assets and every page still renders: the
    /// stylesheet link and the module scripts simply resolve to 404, and the Cockpit
    /// degrades to unstyled server-rendered HTML rather than failing.
    pub fn new(share_dir: &Path) -> Self {
        Assets::at(Some(&share_dir.join(DIR)), PREFIX)
    }

    /// No asset directory at all: for a router built without a distribution.
    pub fn none() -> Self {
        Assets::at(None, PREFIX)
    }

    /// Any directory of static files, served under `prefix`. `None`, or a path that is not
    /// a directory, is an `Assets` that serves nothing and says so: absence is answered
    /// with a 404 that names the file, never with a panic and never with a blank page.
    ///
    /// ```
    /// use majordomus_cli::cockpit::assets::Assets;
    /// let none = Assets::at(None, "/swagger/assets/");
    /// assert!(!none.present());
    /// assert_eq!(none.url("vendor/x.js"), "/swagger/assets/vendor/x.js");
    /// assert_eq!(none.respond("vendor/x.js", None).status, 404);
    /// ```
    pub fn at(root: Option<&Path>, prefix: &'static str) -> Self {
        Assets {
            root: root.filter(|r| r.is_dir()).map(Path::to_path_buf),
            prefix,
            loaded: Mutex::new(BTreeMap::new()),
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
            Some(a) => format!(
                "{}{name}?v={}",
                self.prefix,
                &a.digest[..16.min(a.digest.len())]
            ),
            None => format!("{}{name}", self.prefix),
        }
    }

    /// Answer a request for `PREFIX + name`. `versioned` is whether the request carried a
    /// `v` parameter matching the file's digest, which is what makes the answer immutable.
    pub fn respond(&self, name: &str, version: Option<&str>) -> Response {
        let Some(asset) = self.load(name) else {
            return Response::error(
                404,
                "not_found",
                &format!(
                    "no asset '{name}' under {}; the distribution's share/ directory holds the ones there are",
                    self.prefix
                ),
            );
        };
        let immutable = version.is_some_and(|v| asset.digest.starts_with(v));
        let mut response = Response::new(200, asset.media_type, asset.body.clone());
        response.headers.push((
            "Cache-Control".into(),
            if immutable {
                IMMUTABLE.into()
            } else {
                "no-cache".into()
            },
        ));
        response
            .headers
            .push(("ETag".into(), format!("\"{}\"", asset.digest)));
        response
    }

    fn load(&self, name: &str) -> Option<Loaded> {
        if let Some(cached) = self.loaded.lock().ok()?.get(name) {
            return cached.clone();
        }
        let loaded = self.read(name);
        if let Ok(mut map) = self.loaded.lock() {
            map.insert(name.to_string(), loaded.clone());
        }
        loaded
    }

    fn read(&self, name: &str) -> Option<Loaded> {
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
                prefix = self.prefix,
                "an asset resolves outside the directory it is served from; refused"
            );
            return None;
        }
        let body = std::fs::read_to_string(&resolved).ok()?;
        Some(Loaded {
            digest: digest(body.as_bytes()),
            body,
            media_type,
        })
    }
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

    #[test]
    fn an_absent_directory_serves_nothing_and_does_not_panic() {
        let assets = Assets::new(Path::new("/nonexistent-share-directory"));
        assert!(!assets.present());
        assert_eq!(assets.respond("cockpit.css", None).status, 404);
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

        let hit = assets.respond("cockpit.css", Some(&version));
        assert_eq!(hit.status, 200);
        assert_eq!(hit.body, "body{color:red}");
        assert!(hit
            .headers
            .iter()
            .any(|(k, v)| k == "Cache-Control" && v == IMMUTABLE));

        let unversioned = assets.respond("cockpit.css", None);
        assert!(unversioned
            .headers
            .iter()
            .any(|(k, v)| k == "Cache-Control" && v == "no-cache"));

        // the file is gone and the answer is the same: it was read once
        std::fs::remove_file(cockpit.join("cockpit.css")).expect("remove");
        assert_eq!(assets.respond("cockpit.css", None).status, 200);
    }

    #[test]
    fn an_extension_the_cockpit_does_not_serve_is_not_served() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cockpit = dir.path().join(DIR);
        std::fs::create_dir_all(&cockpit).expect("mkdir");
        std::fs::write(cockpit.join("secrets.env"), "TOKEN=1").expect("write");
        let assets = Assets::new(dir.path());
        assert_eq!(assets.respond("secrets.env", None).status, 404);
    }
}
