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
//! The routes it serves are the projection's own, like `/swagger` and `/openapi.json`: they
//! are not capabilities, they declare no schema, and they are listed as infrastructure in
//! the OpenAPI document rather than as operations.
//!
//! What the browser adds is an enhancement in the strict sense. The pages are complete
//! HTML; the scripts add a command palette, a runner, a graph drawing and two optional
//! visual views. With no JavaScript at all, every page still shows everything it knows.

pub mod assets;
pub mod html;
pub(crate) mod intents;
pub(crate) mod mcp;
pub(crate) mod monitor;
pub(crate) mod nav;
pub(crate) mod pages;
pub(crate) mod plan;
pub(crate) mod reasoning;
pub(crate) mod trains;
pub(crate) mod view;

use std::sync::Arc;

use crate::capability::Context;
use crate::http::router::{percent_decode, Request, Response};
use crate::live::{IntoLive, Live};

use assets::Assets;

/// Every Cockpit route starts here.
pub const PREFIX: &str = "/cockpit";

/// The Cockpit's identity in the web topology (`web.surfaces`).
pub const SURFACE: &str = "cockpit";

/// The stylesheet the shell links.
pub const STYLESHEET: &str = "cockpit.css";

/// The script modules every page loads. `cockpit.js` is not among them: the palette
/// imports it, so it is in the graph of every page anyway. Every module that imports it
/// names it by the one digested URL the server rewrites the import to (`assets.rs`), so a
/// `<script>` tag here would add nothing but a second request.
///
/// A page adds its own module to this list, which is what keeps a graph library off the
/// overview.
pub const SHELL_SCRIPTS: &[&str] = &["palette.js"];

/// The site's own scripts the bar at the top of every page behaves by — its menus, then its
/// theme toggle — served as byte copies of `site/nav.js` and `site/theme.js` that
/// `scripts/cockpit-assets` writes and checks (`project.the-ui-wears-the-sites-navbar`).
pub(crate) const BAR_SCRIPTS: &[&str] = &["nav.js", "theme.js"];

/// The vendored libraries the page modules load at run time, through `vendor()` and
/// `vendorModule()` in `cockpit.js`. A run-time URL is not a static import the server can
/// rewrite, so the page publishes each one's digested URL on its body (`data-vendor`), the
/// way the UI hands its graph library over in `data-graph-lib`; a file not there is left
/// out, and the script falls back to its plain path. A library a module imports itself
/// (`three.core.min.js`, imported by `three.module.min.js`) is versioned by that import.
pub(crate) const VENDOR: &[&str] = &[
    "alpine.csp.min.js",
    "cytoscape.min.js",
    "three.module.min.js",
    "p5.min.js",
];

/// The Cockpit over the repository as it is now. Cheap to clone into every worker thread:
/// the assets are shared and the view of the repository is an `Arc`.
pub struct Cockpit {
    live: Arc<Live>,
    version: &'static str,
    assets: Assets,
}

/// The plain routes the dispatcher answers that no area of the navigation names: pages a
/// reader reaches from another page, which the sidebar does not carry.
const PAGES_WITHOUT_AN_AREA: &[&str] = &[
    "/cockpit/object",
    "/cockpit/graphs/topology",
    "/cockpit/monitor",
    "/cockpit/search",
    "/cockpit/activity",
];

/// Every route the dispatcher answers without an identifier in the path, and the area each
/// one belongs to — `""` for a page that belongs to none, which the sidebar does not carry.
///
/// Derived from the navigation (I1009): every area of [`nav::areas`] is a route under its
/// own id, so the sidebar and this list cannot name different Cockpits, and only the pages
/// no area names are written down, in `PAGES_WITHOUT_AN_AREA`. The `match` that answers
/// them is not data a program can read back at run time, so what keeps the two honest is
/// that both are compared, in each direction:
/// `every_page_renders_complete_html_with_the_shell_and_the_security_headers` drives these
/// routes over a socket, so a route the dispatcher lost is a failure;
/// `every_plain_arm_of_the_dispatcher_is_a_static_route` reads the `match` from this file's
/// source, so an arm added there and not here is a failure. Integration, Reasoning and
/// Economics were that kind: each arm and area landed after a hand-kept list of these routes
/// was written, and nothing compared the list with the `match` it describes.
/// `/cockpit/quality` was exactly that page: the dispatcher answered it, the navigation did
/// not name it, and no test and no browser sweep could reach it.
///
/// ```
/// let routes = majordomus_cli::cockpit::static_routes();
/// assert!(routes.contains(&("/cockpit/capabilities", "capabilities")));
/// assert!(routes.contains(&("/cockpit/monitor", "")));
/// ```
pub fn static_routes() -> Vec<(&'static str, &'static str)> {
    nav::areas()
        .iter()
        .map(|area| (area.href, area.id))
        .chain(PAGES_WITHOUT_AN_AREA.iter().map(|path| (*path, "")))
        .collect()
}

impl Cockpit {
    /// A Cockpit over a view of the repository, serving its assets from `share_dir/cockpit`.
    pub fn new(
        live: impl IntoLive,
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
            live: live.into_live(),
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
            return self.assets.respond(name, req);
        }

        // One reading of the repository for the whole page: the navigation and the page
        // itself must be two views of one generation, never two generations side by side.
        let ctx = self.live.current();
        let page = self.route(&ctx, req);
        let navigation = nav::build(&ctx, &req.path);
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
            favicon: self.assets.url("favicon.svg"),
            scripts,
            bar_scripts: BAR_SCRIPTS.iter().map(|s| self.assets.url(s)).collect(),
            vendor: self.vendor_urls(),
            assets_present: self.assets.present(),
            version: self.version,
        };
        let body = view::page(&shell, page.main);
        let mut response = Response::new(page.status, "text/html; charset=utf-8", body);
        // a page is derived from the registry and the index, which follow the repository
        // as it moves under this process: revalidation is right, and the assets carry the
        // long cache because their URLs carry their digests
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

    /// The digested URL of every vendored library that is there, by the file name
    /// `vendor()` is called with, as the JSON object the page publishes in `data-vendor`.
    fn vendor_urls(&self) -> String {
        let urls: std::collections::BTreeMap<&str, String> = VENDOR
            .iter()
            .map(|file| (*file, format!("vendor/{file}")))
            .filter(|(_, path)| self.assets.has(path))
            .map(|(file, path)| (file, self.assets.url(&path)))
            .collect();
        serde_json::to_string(&urls).unwrap_or_else(|_| "{}".into())
    }

    fn route(&self, ctx: &Arc<Context>, req: &Request) -> pages::Page {
        let path = req.path.trim_end_matches('/');
        let query = &req.query;
        match path {
            "" | PREFIX => pages::overview(ctx),
            "/cockpit/capabilities" => pages::capabilities(ctx, query),
            "/cockpit/commands" => pages::commands(ctx, query),
            "/cockpit/objects" => pages::objects(ctx, query),
            // the address an object was reachable at before it had one of its own; kept
            // so that a link written against it still lands on the entity it named
            "/cockpit/object" => match query.iter().find(|(k, _)| k == "uri") {
                Some((_, uri)) => pages::object(ctx, uri),
                None => pages::objects(ctx, query),
            },
            "/cockpit/executions" => pages::executions(ctx, query),
            "/cockpit/graphs" => pages::graphs(ctx),
            "/cockpit/graphs/topology" => pages::topology(ctx),
            "/cockpit/continuity" => pages::continuity(ctx),
            "/cockpit/intents" => intents::list(ctx),
            "/cockpit/plan" => plan::plan(ctx, query),
            "/cockpit/worktrees" => pages::worktrees(ctx),
            "/cockpit/peers" => pages::peers(ctx),
            "/cockpit/integration" => pages::integration(ctx),
            "/cockpit/trains" => trains::page(ctx),
            "/cockpit/mesh" => pages::mesh(ctx),
            "/cockpit/monitor" => monitor::page(ctx, query),
            "/cockpit/models" => pages::models(ctx),
            "/cockpit/reasoning" => reasoning::page(ctx),
            "/cockpit/economics" => pages::economics(ctx),
            "/cockpit/directories" => pages::directories(ctx, query),
            "/cockpit/health" => pages::health(ctx),
            "/cockpit/quality" => pages::quality(ctx),
            "/cockpit/artifacts" => pages::artifacts(ctx),
            "/cockpit/release" => pages::release(ctx),
            "/cockpit/design" => pages::design(ctx),
            "/cockpit/api" => pages::api(ctx),
            "/cockpit/mcp" => mcp::page(ctx, query),
            "/cockpit/search" => pages::search(ctx, query),
            "/cockpit/activity" => pages::activity(ctx),
            other => {
                if let Some(id) = other.strip_prefix("/cockpit/executions/") {
                    pages::execution(ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/capabilities/") {
                    pages::capability(ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/commands/") {
                    pages::command(ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/plan/milestones/") {
                    plan::milestone(ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/plan/issues/") {
                    plan::issue(ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/intents/") {
                    intents::intent(ctx, &percent_decode(id))
                } else if let Some(id) = other.strip_prefix("/cockpit/graphs/") {
                    pages::graph(ctx, &percent_decode(id), query)
                } else if let Some(rest) = other.strip_prefix("/cockpit/objects/") {
                    // one kind per segment, one entity per two. No kind and no entity is
                    // named here: both are read from the index, which is what makes adding
                    // an object add its page.
                    let rest = percent_decode(rest);
                    match rest.split_once('/') {
                        Some((kind, slug)) => pages::entity(ctx, kind, slug),
                        None => pages::objects_of_kind(ctx, &rest, query),
                    }
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
/// The digest is computed from `view::theme_bootstrap()` itself, so the policy cannot
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
        hasher.update(view::theme_bootstrap().as_bytes());
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

    /// Every library a script loads at run time is one the page publishes a digested URL
    /// for: a `vendor('x')` that `VENDOR` does not name would still load, from its plain
    /// path, and be downloaded again on every navigation without anything saying so.
    #[test]
    fn every_vendored_library_a_script_loads_is_published_with_its_digest() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/cockpit");
        let mut named = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("share/cockpit") {
            let path = entry.expect("an entry").path();
            if path.extension().is_none_or(|e| e != "js") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("a script");
            for call in ["vendor('", "vendorModule('"] {
                for rest in source.split(call).skip(1) {
                    let file = rest.split('\'').next().unwrap_or_default();
                    named.push((path.display().to_string(), file.to_string()));
                }
            }
        }
        assert!(
            named.iter().any(|(_, f)| f == "alpine.csp.min.js"),
            "the scan found the shell's own Alpine load: {named:?}"
        );
        for (script, file) in &named {
            assert!(
                VENDOR.contains(&file.as_str()),
                "{script} loads {file}, which VENDOR does not name"
            );
        }
    }

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
        h.update(view::theme_bootstrap().as_bytes());
        let expected = base64(&h.finalize());
        assert!(
            csp().contains(&format!("'sha256-{expected}'")),
            "the policy names the digest of the bootstrap it ships"
        );
        assert_eq!(expected.len(), 44, "base64 of 32 bytes");
    }

    /// A page the dispatcher answers and the navigation does not name is a page a reader
    /// cannot reach, a crawl cannot find and therefore no test covers. The reverse — an area
    /// the sidebar offers and no route answers — is a link to nothing. The routes are derived
    /// from the areas (I1009), so what is left to go wrong is one address twice: a page
    /// written down as area-less that an area also names, or two areas at one address.
    #[test]
    fn every_area_has_a_route_and_every_route_its_area() {
        let routes = static_routes();
        for area in nav::areas() {
            assert!(
                routes.contains(&(area.href, area.id)),
                "the navigation offers {} and no route answers it",
                area.href
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        let twice: Vec<&str> = routes
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| !seen.insert(*path))
            .collect();
        assert!(
            twice.is_empty(),
            "one address is two routes, an area's and an area-less page's: {twice:?}"
        );
    }

    /// `static_routes()` is a claim about the `match` in `Cockpit::route`, and a claim checked
    /// only from its own side drifts: three arms were added to the dispatcher after the list
    /// was written, and every test that read the list went on passing. This reads the arms
    /// from the source the dispatcher is compiled from and compares both directions.
    #[test]
    fn every_plain_arm_of_the_dispatcher_is_a_static_route() {
        let src = include_str!("mod.rs");
        let start = src
            .find("fn route(&self")
            .expect("cockpit/mod.rs carries the dispatcher as fn route");
        let body = &src[start..];
        let end = body
            .find("other =>")
            .expect("the dispatcher ends its plain arms with a catch-all");
        let mut arms: Vec<&str> = body[..end]
            .lines()
            .map(str::trim_start)
            .filter_map(|line| line.strip_prefix('"'))
            .filter_map(|line| line.split_once("\" =>").map(|(path, _)| path))
            .filter(|path| path.starts_with("/cockpit"))
            .collect();
        if body[..end].contains("\"\" | PREFIX =>") {
            arms.push(PREFIX);
        }
        assert!(
            arms.len() > 10,
            "the dispatcher's arms were not found, so nothing was compared: {arms:?}"
        );

        let unlisted: Vec<&str> = arms
            .iter()
            .copied()
            .filter(|arm| !static_routes().iter().any(|(path, _)| path == arm))
            .collect();
        assert!(
            unlisted.is_empty(),
            "the dispatcher answers plain routes static_routes() does not list: {unlisted:?}"
        );
        let unanswered: Vec<&str> = static_routes()
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| !arms.contains(path))
            .collect();
        assert!(
            unanswered.is_empty(),
            "static_routes() lists routes the dispatcher has no arm for: {unanswered:?}"
        );
    }

    /// The public route table is a projection of the dispatcher too. docs/COCKPIT.md was
    /// written by hand and fell eight routes behind, so a reader of the documentation could
    /// not learn that Commands, Mesh, Models, Directories, Quality, Artifacts, Release or
    /// Design existed. Every plain route the dispatcher answers has a row, and the table
    /// names no plain route the dispatcher does not answer.
    #[test]
    fn the_documented_route_table_is_the_dispatchers() {
        let doc = include_str!("../../../../docs/COCKPIT.md");
        let start = doc
            .find("## What is on it")
            .expect("docs/COCKPIT.md carries the route table under 'What is on it'");
        let section = &doc[start..];
        let end = section[3..].find("\n## ").map_or(section.len(), |i| i + 3);
        let documented: Vec<&str> = section[..end]
            .lines()
            .filter_map(|line| line.strip_prefix("| `"))
            .filter_map(|line| line.split('`').next())
            .filter(|route| !route.contains('<'))
            .map(|route| route.split('?').next().unwrap_or(route))
            .collect();

        let undocumented: Vec<&str> = static_routes()
            .iter()
            .map(|(path, _)| *path)
            .filter(|path| !documented.contains(path))
            .collect();
        assert!(
            undocumented.is_empty(),
            "docs/COCKPIT.md has no row for routes the dispatcher answers: {undocumented:?}"
        );

        let unrouted: Vec<&str> = documented
            .iter()
            .copied()
            .filter(|route| route.starts_with("/cockpit") && !route.starts_with("/cockpit/assets"))
            .filter(|route| !static_routes().iter().any(|(path, _)| path == route))
            .collect();
        assert!(
            unrouted.is_empty(),
            "docs/COCKPIT.md documents routes the dispatcher does not answer: {unrouted:?}"
        );
    }
}
