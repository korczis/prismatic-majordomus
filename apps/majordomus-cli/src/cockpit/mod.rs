//! The Cockpit: the human-facing projection of the registry, server-rendered.
//!
//! It is a projection and not an application. Every page is laid out from what a
//! capability answered, through the same executor MCP and the HTTP routes call; the
//! navigation's catalogues come from the registry, the index and the graph derivations;
//! the runner's form comes from the input schema. Nothing here holds a list of
//! capabilities, routes, kinds or graphs, and adding one to the registry adds it here.
//!
//! ```text
//!            capability! ─── registry ────┬──── MCP tools and resources
//!                    declarative objects ─┤──── HTTP routes ── OpenAPI ── Swagger UI
//!                                         ├──── the command line
//!                                         └──── Cockpit ── pages, navigation, runner, graphs
//! ```
//!
//! The routes it serves are the projection's own, like `/docs` and `/openapi.json`: they
//! are not capabilities, they declare no schema, and they are listed as infrastructure in
//! the OpenAPI document rather than as operations.
//!
//! What the browser adds is an enhancement in the strict sense. The pages are complete
//! HTML; the scripts add a command palette, a runner, a graph drawing and two optional
//! visual views. With no JavaScript at all, every page still shows everything it knows.

pub mod assets;
pub mod html;
pub mod nav;
pub mod pages;
pub mod view;

use std::sync::Arc;

use crate::capability::Context;
use crate::http::router::{percent_decode, Request, Response};

use assets::Assets;

/// Every Cockpit route starts here.
pub const PREFIX: &str = "/cockpit";

/// The stylesheet the shell links.
pub const STYLESHEET: &str = "cockpit.css";

/// The script modules every page loads: the shell's own behaviour, then the palette.
/// A page adds its own module to this list, which is what keeps a graph library
/// off the overview.
pub const SHELL_SCRIPTS: &[&str] = &["cockpit.js", "palette.js"];

/// The Cockpit over one context. Cheap to clone into every worker thread: the assets are
/// shared and the context is an `Arc`.
pub struct Cockpit {
    ctx: Arc<Context>,
    version: &'static str,
    assets: Assets,
}

impl Cockpit {
    /// A Cockpit over a context, serving its assets from `share_dir/cockpit`.
    pub fn new(
        ctx: Arc<Context>,
        version: &'static str,
        share_dir: Option<&std::path::Path>,
    ) -> Self {
        let assets = match share_dir {
            Some(dir) => Assets::new(dir),
            None => Assets::none(),
        };
        if !assets.present() {
            tracing::warn!(
                "the distribution has no share/cockpit/ directory: the Cockpit will serve unstyled markup; run `just cockpit-assets` to build it"
            );
        }
        Cockpit {
            ctx,
            version,
            assets,
        }
    }

    /// Does this request belong to the Cockpit?
    ///
    /// ```
    /// use majordomus_cli::cockpit::Cockpit;
    /// assert!(Cockpit::owns("/cockpit"));
    /// assert!(Cockpit::owns("/cockpit/capabilities"));
    /// assert!(!Cockpit::owns("/cockpitx"));
    /// assert!(!Cockpit::owns("/api/v1/objects"));
    /// ```
    pub fn owns(path: &str) -> bool {
        path == PREFIX || path.starts_with(&format!("{PREFIX}/"))
    }

    /// Answer one request under [`PREFIX`].
    pub fn handle(&self, req: &Request) -> Response {
        if req.method != "GET" {
            return Response::error(
                405,
                "method_not_allowed",
                "the Cockpit's pages are read with GET; a capability that changes something is called on its own route",
            );
        }
        if let Some(name) = req.path.strip_prefix(assets::PREFIX) {
            let version = req
                .query
                .iter()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.as_str());
            return self.assets.respond(name, version);
        }

        let page = self.route(req);
        let navigation = nav::build(&self.ctx, &req.path);
        let scripts = SHELL_SCRIPTS
            .iter()
            .copied()
            .chain(page.scripts.iter().copied())
            .map(|name| self.assets.url(name))
            .collect();
        let shell = view::Shell {
            title: &page.title,
            subtitle: page.subtitle.clone(),
            area: page.area,
            breadcrumbs: page.breadcrumbs.clone(),
            navigation: &navigation,
            stylesheet: self.assets.url(STYLESHEET),
            scripts,
            assets_present: self.assets.present(),
            version: self.version,
        };
        let body = view::page(&shell, page.main);
        let mut response = Response::new(page.status, "text/html; charset=utf-8", body);
        // a page is derived from the registry and the index, both immutable for the life
        // of the process, but the process is a development server: revalidation is right,
        // and the assets carry the long cache because their URLs carry their digests
        response
            .headers
            .push(("Cache-Control".into(), "no-cache".into()));
        response
            .headers
            .push(("Content-Security-Policy".into(), csp().into()));
        response
            .headers
            .push(("X-Content-Type-Options".into(), "nosniff".into()));
        response
            .headers
            .push(("Referrer-Policy".into(), "no-referrer".into()));
        response
    }

    fn route(&self, req: &Request) -> pages::Page {
        let path = req.path.trim_end_matches('/');
        let query = &req.query;
        match path {
            "" | PREFIX => pages::overview(&self.ctx),
            "/cockpit/capabilities" => pages::capabilities(&self.ctx, query),
            "/cockpit/objects" => pages::objects(&self.ctx, query),
            "/cockpit/object" => match query.iter().find(|(k, _)| k == "uri") {
                Some((_, uri)) => pages::object(&self.ctx, uri),
                None => pages::objects(&self.ctx, query),
            },
            "/cockpit/graphs" => pages::graphs(&self.ctx),
            "/cockpit/graphs/topology" => pages::topology(&self.ctx),
            "/cockpit/continuity" => pages::continuity(&self.ctx),
            "/cockpit/directories" => pages::directories(&self.ctx, query),
            "/cockpit/health" => pages::health(&self.ctx),
            "/cockpit/artifacts" => pages::artifacts(&self.ctx),
            "/cockpit/api" => pages::api(&self.ctx),
            "/cockpit/search" => pages::search(&self.ctx, query),
            "/cockpit/activity" => pages::activity(&self.ctx),
            other => {
                if let Some(id) = other.strip_prefix("/cockpit/capabilities/") {
                    pages::capability(&self.ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/graphs/") {
                    pages::graph(&self.ctx, &percent_decode(id))
                } else {
                    pages::not_found(other)
                }
            }
        }
    }
}

/// The content-security policy every page carries. Scripts come from this origin and from
/// nowhere else; the one inline script is allowed by the digest of its own bytes, so an
/// injected inline script is refused even if one ever got through the escaping. No
/// `unsafe-eval`, no remote origin, and nothing may frame the page.
///
/// `style-src` allows `unsafe-inline` and `script-src` does not, which is the one asymmetry
/// here and it is deliberate. A drawing library sets `style` attributes on the elements it
/// creates — that is how Cytoscape sizes its canvas — and a policy that forbids them makes
/// the optional views silently misrender rather than fail. The exposure is a style
/// injection, which needs the escaping to have already failed and which cannot execute; the
/// exposure `script-src 'unsafe-inline'` would carry is arbitrary code, and that stays shut.
///
/// The digest is computed from [`view::THEME_BOOTSTRAP`] itself, so the policy cannot
/// drift from the script it allows.
///
/// ```
/// let policy = majordomus_cli::cockpit::csp();
/// assert!(policy.starts_with("default-src 'none'"));
/// assert!(!policy.contains("unsafe-eval"));
/// ```
pub fn csp() -> &'static str {
    static POLICY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    POLICY.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(view::THEME_BOOTSTRAP.as_bytes());
        let digest = base64(&hasher.finalize());
        format!(
            "default-src 'none'; script-src 'self' 'sha256-{digest}'; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; \
             font-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"
        )
    })
}

/// Standard base64, for the one digest the policy names. Four characters per three bytes,
/// padded; no dependency for twenty lines.
///
/// ```
/// use majordomus_cli::cockpit::base64;
/// assert_eq!(base64(b""), "");
/// assert_eq!(base64(b"f"), "Zg==");
/// assert_eq!(base64(b"fo"), "Zm8=");
/// assert_eq!(base64(b"foo"), "Zm9v");
/// assert_eq!(base64(b"foobar"), "Zm9vYmFy");
/// ```
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((*chunk.get(1).unwrap_or(&0) as u32) << 8)
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_allows_no_remote_origin_and_no_unsafe_source() {
        let policy = csp();
        assert!(policy.contains("script-src 'self' 'sha256-"), "{policy}");
        assert!(!policy.contains("unsafe-eval"), "{policy}");
        // the asymmetry, asserted rather than assumed: styles may be inline, scripts never
        assert!(
            policy.contains("style-src 'self' 'unsafe-inline'"),
            "{policy}"
        );
        let script_src = policy
            .split("script-src ")
            .nth(1)
            .and_then(|s| s.split(';').next())
            .expect("a script-src directive");
        assert!(!script_src.contains("unsafe-inline"), "{script_src}");
        assert!(!policy.contains("http"), "{policy}");
        assert!(policy.contains("frame-ancestors 'none'"), "{policy}");
    }

    #[test]
    fn the_policy_names_the_digest_of_the_script_it_allows() {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(view::THEME_BOOTSTRAP.as_bytes());
        let expected = base64(&h.finalize());
        assert!(
            csp().contains(&format!("'sha256-{expected}'")),
            "the policy names the digest of the bootstrap it ships"
        );
        assert_eq!(expected.len(), 44, "base64 of 32 bytes");
    }
}
