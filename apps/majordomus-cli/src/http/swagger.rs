//! The Swagger UI shell: one HTML page that loads the pinned Swagger UI distribution and
//! points it at `/openapi.json`. It embeds no specification of its own; what it shows is
//! whatever the server generated from the registry at the moment of the request. The
//! page is rendered once per process and served from memory.
//!
//! The UI's own assets are this distribution's, read from `share/swagger/vendor/` and
//! served beside the page at [`ASSET_PREFIX`] with the digest of their bytes in the URL,
//! exactly like the Cockpit's. Nothing on this page comes from anywhere but this process,
//! so the whole HTTP projection now works with no network — which is what every other part
//! of it already did. `scripts/swagger-assets` puts the files there from the pinned npm
//! package; ADR 0031 deferred that decision as one about distribution size, and it is
//! answered in `.ai/repo/adrs/0039-*`.
//!
//! And when they are not there — a distribution packed without `share/swagger/`, a proxy
//! that ate the script — the page says so. The explanation is markup the server already
//! rendered, not something a script has to draw: a script that has to run in order to
//! report that no script ran is not a report. Swagger UI, when it does load, takes the
//! notice off the page as its first act.
//!
//! The frame around it is this repository's. The type stack and the accent come from
//! `share/design/tokens.yaml` like every other surface, compiled in through `tokens.css`,
//! so the API viewer reads as part of the same tool rather than as a stock installation of
//! somebody else's. What is inside the widget — the operation blocks, the schema tables,
//! the try-it form — is Swagger UI's own stylesheet and is left alone: restyling a third
//! party's component tree against a pinned version is a maintenance bill this page does not
//! need to take on.
//!
//! The page pins itself light. Swagger UI 5 ships no dark theme, so a dark frame around a
//! permanently light widget is worse than a light one; `class="light"` is the escape hatch
//! the generated token block provides for exactly this, and it also stops the browser from
//! rendering dark form controls inside a light panel.

use std::path::Path;
use std::sync::{LazyLock, OnceLock};

use crate::cockpit::assets::Assets;
use crate::http::router::{Request, Response};

/// The Swagger UI distribution version the page pins.
///
/// The one place the version is written. `package.json` pins the same string so that
/// `npm ci` fetches what this constant promises, and `scripts/swagger-assets` refuses to
/// vendor anything else: a half-upgraded pin used to mean a stylesheet from one version
/// and a bundle from another, which fails in the browser and nowhere else.
pub const SWAGGER_UI_VERSION: &str = "5.17.14";

/// The path the page loads the specification from.
pub const SPEC_PATH: &str = "/openapi.json";

/// The path the page is served at.
///
/// It is `/swagger` and not `/docs`: `/docs` is where this repository's own documentation
/// is served, and a viewer for the API is not the documentation. The rule that holds the
/// two apart is `project.web-surface-declared-once`.
pub const SWAGGER_PATH: &str = "/swagger";

/// The one prefix served under [`SWAGGER_PATH`]: the viewer's own files, and nothing else.
///
/// The surface is still one page; this is that page's asset directory, the way
/// `/cockpit/assets/` is the Cockpit's. Every other path under `/swagger` is a 404 that
/// says so, because answering one would invent a route nothing declared.
pub const ASSET_PREFIX: &str = "/swagger/assets/";

/// The directory of the distribution's `share/` the viewer's files are read from.
pub const DIR: &str = "swagger";

/// The stylesheet, under [`DIR`].
pub const STYLESHEET: &str = "vendor/swagger-ui.css";

/// The widget, under [`DIR`]. `swagger-ui-bundle.js` is the standalone build: React, Redux
/// and the client are inside it, so the page loads one file and no module graph.
pub const BUNDLE: &str = "vendor/swagger-ui-bundle.js";

/// The page's own script: it mounts the widget, and it removes the notice that says the
/// widget is not here. Both happen only if the bundle loaded, which is the point — if it
/// did not, this never runs and the notice is what a reader sees.
static INIT: LazyLock<String> = LazyLock::new(|| {
    format!(
        r##"
if (window.SwaggerUIBundle) {{
  const notice = document.getElementById("swagger-unavailable");
  if (notice) notice.remove();
  window.ui = SwaggerUIBundle({{ url: "{SPEC_PATH}", dom_id: "#swagger-ui", deepLinking: true }});
}}
"##
    )
});

/// The content-security policy the page carries.
///
/// Everything comes from this origin: the stylesheet, the bundle, and the document the
/// widget fetches. The one inline script is allowed by the digest of its own bytes,
/// computed from [`INIT`] so that the policy cannot drift from the script it allows, and
/// no other inline script can be added to this page without the browser refusing it.
///
/// `style-src` allows `unsafe-inline` and `script-src` does not, the same asymmetry the
/// Cockpit's policy carries and for the same reason: the page's token block is inline and
/// Swagger UI sets `style` attributes on the nodes it creates, while arbitrary inline
/// script is the exposure that stays shut. `img-src data:` is Swagger UI's own icons,
/// which its stylesheet embeds.
///
/// ```
/// use majordomus_cli::http::swagger;
/// let policy = swagger::csp();
/// assert!(policy.starts_with("default-src 'none'"));
/// assert!(!policy.contains("unsafe-eval"));
/// assert!(!policy.contains("unpkg"), "the page names no third-party origin");
/// ```
pub fn csp() -> &'static str {
    static POLICY: OnceLock<String> = OnceLock::new();
    POLICY.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(INIT.as_bytes());
        let digest = crate::cockpit::base64(&hasher.finalize());
        format!(
            "default-src 'none'; script-src 'self' 'sha256-{digest}'; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; \
             font-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"
        )
    })
}

/// The API viewer: the shell, and the files it loads.
pub struct Swagger {
    assets: Assets,
    page: OnceLock<String>,
}

impl Swagger {
    /// The viewer over a distribution's `share/swagger/`, when the process located one.
    /// Without it the page still renders and says what is missing.
    pub fn new(share_dir: Option<&Path>) -> Self {
        let assets = Assets::at(share_dir.map(|d| d.join(DIR)).as_deref(), ASSET_PREFIX);
        // only a process that located a distribution and found no viewer in it has anything
        // to report. `None` is a router built without one at all, which the Cockpit already
        // says once; saying it twice for every such router is how a log stops being read.
        if share_dir.is_some() && !assets.present() {
            tracing::warn!(
                "the distribution has no share/{DIR}/ directory: {SWAGGER_PATH} will explain itself rather than draw the API viewer; run `scripts/swagger-assets` to vendor it"
            );
        }
        Swagger {
            assets,
            page: OnceLock::new(),
        }
    }

    /// Whether the viewer's own files are in this distribution.
    pub fn complete(&self) -> bool {
        self.assets.has(STYLESHEET) && self.assets.has(BUNDLE)
    }

    /// Answer one request for [`SWAGGER_PATH`] or something under it.
    pub fn handle(&self, req: &Request) -> Response {
        if let Some(name) = req.path.strip_prefix(ASSET_PREFIX) {
            let version = req
                .query
                .iter()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.as_str());
            return self.assets.respond(name, version);
        }
        let mut response = Response::new(200, "text/html; charset=utf-8", self.page().to_string());
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

    /// The shell, rendered once. The asset URLs carry the digests of the files this
    /// distribution actually holds, so the render waits until there is a distribution to
    /// ask — which is why this is not a `static`.
    pub fn page(&self) -> &str {
        self.page.get_or_init(|| render(&self.assets))
    }
}

/// The notice a reader sees when the widget is not on the screen: what did not happen, and
/// what is readable anyway. Two situations, told apart by the server, which knows which one
/// it is in — a page that guessed would send half its readers to the wrong fix.
fn notice(present: bool) -> String {
    let cause = if present {
        format!(
            "This distribution has the viewer and serves it at <code>{ASSET_PREFIX}{BUNDLE}</code>, \
             so the browser did not run it: a proxy, an extension or a content-security policy \
             stopped it on the way in. The browser's console says which."
        )
    } else {
        format!(
            "This distribution has no <code>share/{DIR}/vendor/</code>, so there is no viewer to \
             draw. In a checkout: <code>npm ci &amp;&amp; scripts/swagger-assets</code>. In an \
             installed copy the archive was incomplete — reinstall it."
        )
    };
    format!(
        r#"<div id="swagger-unavailable">
<h1>The API viewer did not load</h1>
<p>{cause}</p>
<p>Nothing on this page is fetched from the network, and nothing is missing from the API
itself: the document this viewer would draw is generated by this process and readable as it
is, at <a href="{SPEC_PATH}"><code>{SPEC_PATH}</code></a>. Every surface this process serves
is listed at <a href="/"><code>/</code></a>.</p>
</div>"#
    )
}

/// The whole page, for one distribution's assets.
fn render(assets: &Assets) -> String {
    format!(
        r##"<!doctype html>
<html lang="en" class="light">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Majordomus API</title>
<link rel="stylesheet" href="{css}">
<style>
{tokens}
/* The frame: the page around the widget, in this repository's type and colour. */
body {{ margin: 0; background: var(--bg); color: var(--fg); font-family: var(--font-sans); }}
.swagger-ui, .swagger-ui .info .title, .swagger-ui .opblock-tag {{ font-family: var(--font-sans); }}
.swagger-ui .microlight, .swagger-ui code, .swagger-ui pre {{ font-family: var(--font-mono); }}
/* Swagger UI's topbar is its own branding and a form for choosing a specification. This
   page serves one specification and the process advertises its surfaces on its home page,
   so the bar is a logo and a field that must not be used. */
.swagger-ui .topbar {{ display: none; }}
.swagger-ui .info .title small.version-stamp {{ background: var(--accent); }}
.swagger-ui a {{ color: var(--accent); }}
/* The notice, on the page from its first byte and taken off it by the script that the
   widget's own arrival runs. It is styled here and not in the widget's stylesheet, because
   the case it exists for is the one where that stylesheet did not arrive either. */
#swagger-unavailable {{ max-width: 46rem; margin: 4rem auto; padding: 0 1.5rem; line-height: 1.6; }}
#swagger-unavailable h1 {{ font-size: 1.5rem; margin: 0 0 1rem; }}
#swagger-unavailable code {{ font-family: var(--font-mono); font-size: 0.9em; }}
#swagger-unavailable a {{ color: var(--accent); }}
</style>
</head>
<body>
<div id="swagger-ui"></div>
{notice}
<script src="{bundle}"></script>
<script>{init}</script>
</body>
</html>
"##,
        css = assets.url(STYLESHEET),
        bundle = assets.url(BUNDLE),
        notice = notice(assets.has(BUNDLE)),
        init = &*INIT,
        tokens = crate::web::html::TOKENS
    )
}

/// Whether this shell may be offered as a link a reader can follow, in an environment
/// where the surfaces are what [`crate::web::projection_routes`] resolved.
///
/// The console is served by a running process and nothing publishes it, so a published
/// page must name it rather than link it. The answer comes from the surface's declared
/// availability and from nowhere else: not from the page's own address, not from a build
/// flag, not from a template that happens to know which site it is rendering.
///
/// ```
/// use majordomus_cli::http::swagger;
/// assert!(!swagger::offered_by_a_publication(), "a console needs the server behind it");
/// ```
pub fn offered_by_a_publication() -> bool {
    crate::web::projection_routes()
        .iter()
        .find(|r| r.path == SWAGGER_PATH)
        .is_some_and(|r| r.linkable())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A distribution whose `share/swagger/vendor/` holds the two files, with the bytes
    /// given: enough to render the page the way a real one does.
    fn distribution(css: &str, bundle: &str) -> (tempfile::TempDir, Swagger) {
        let dir = tempfile::tempdir().expect("tempdir");
        let vendor = dir.path().join(DIR).join("vendor");
        std::fs::create_dir_all(&vendor).expect("mkdir");
        std::fs::write(vendor.join("swagger-ui.css"), css).expect("write");
        std::fs::write(vendor.join("swagger-ui-bundle.js"), bundle).expect("write");
        let swagger = Swagger::new(Some(dir.path()));
        (dir, swagger)
    }

    fn get(path: &str) -> Request {
        Request::parse_target("GET", path, Vec::new())
    }

    #[test]
    fn the_shell_points_at_the_generated_document_and_carries_none_of_its_own() {
        // the whole claim this module makes: what a reader sees is whatever the server
        // generated from the registry at the moment of the request, never a copy
        let (_dir, swagger) = distribution(".swagger-ui{}", "window.SwaggerUIBundle=1");
        let shell = swagger.page();
        assert!(shell.contains(&format!("url: \"{SPEC_PATH}\"")));
        for embedded in ["\"paths\"", "\"openapi\"", "\"components\""] {
            assert!(
                !shell.contains(embedded),
                "the shell embeds {embedded}, so it can disagree with the registry"
            );
        }
        // and it is rendered once: two calls hand back the same allocation, not two
        assert!(std::ptr::eq(shell.as_ptr(), swagger.page().as_ptr()));
    }

    #[test]
    fn the_page_names_no_origin_but_this_one() {
        // the defect this module was changed to fix: with no network, the CDN version of
        // this page was HTTP 200 and nothing at all — no widget, and no word about why
        let (_dir, swagger) = distribution(".swagger-ui{}", "window.SwaggerUIBundle=1");
        let shell = swagger.page();
        assert!(!shell.contains("unpkg.com"), "{shell}");
        assert!(!shell.contains("https://"), "the page loads nothing remote");
        assert!(shell.contains(&format!("src=\"{ASSET_PREFIX}{BUNDLE}?v=")));
        assert!(shell.contains(&format!("href=\"{ASSET_PREFIX}{STYLESHEET}?v=")));
    }

    #[test]
    fn the_asset_urls_carry_the_digest_of_the_bytes_this_distribution_holds() {
        let (_one_dir, one) = distribution(".a{}", "one");
        let (_two_dir, two) = distribution(".a{}", "two");
        let url = |s: &Swagger| {
            s.page()
                .split(&format!("src=\"{ASSET_PREFIX}{BUNDLE}"))
                .nth(1)
                .and_then(|s| s.split('"').next())
                .expect("a bundle URL")
                .to_string()
        };
        assert_ne!(
            url(&one),
            url(&two),
            "two distributions, two bundles, two URLs: the cache is never told anything"
        );
        assert!(one.complete() && two.complete());
    }

    #[test]
    fn an_absent_viewer_is_explained_on_the_page_rather_than_left_blank() {
        let swagger = Swagger::new(None);
        assert!(!swagger.complete());
        let shell = swagger.page();
        assert!(shell.contains("The API viewer did not load"));
        assert!(shell.contains("scripts/swagger-assets"), "it names the fix");
        assert!(shell.contains(SPEC_PATH), "and what is readable without it");
        // the explanation is markup, not something a script has to draw: everything a
        // reader needs is between the tags before any script is fetched
        let body = shell.split("<body>").nth(1).expect("a body");
        let before_scripts = body.split("<script").next().expect("markup");
        assert!(before_scripts.contains("The API viewer did not load"));
    }

    #[test]
    fn a_present_viewer_still_carries_the_notice_and_the_script_that_removes_it() {
        // the other half: when the bundle does load, the reader must not see both
        let (_dir, swagger) = distribution(".a{}", "window.SwaggerUIBundle=1");
        let shell = swagger.page();
        assert!(shell.contains("id=\"swagger-unavailable\""));
        assert!(INIT.contains("notice.remove()"));
        assert!(INIT.contains("if (window.SwaggerUIBundle)"), "{}", &*INIT);
        // and the notice names the situation the server is actually in
        assert!(shell.contains("so the browser did not run it"), "{shell}");
    }

    #[test]
    fn the_policy_allows_this_origin_and_the_one_script_it_ships() {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(INIT.as_bytes());
        let expected = crate::cockpit::base64(&h.finalize());
        let policy = csp();
        assert!(
            policy.contains(&format!("'sha256-{expected}'")),
            "the policy names the digest of the script it ships"
        );
        assert!(!policy.contains("unsafe-eval"), "{policy}");
        assert!(!policy.contains("http"), "{policy}");
        let script_src = policy
            .split("script-src ")
            .nth(1)
            .and_then(|s| s.split(';').next())
            .expect("a script-src directive");
        assert!(!script_src.contains("unsafe-inline"), "{script_src}");
    }

    #[test]
    fn the_viewer_serves_its_own_files_and_nothing_else_under_its_prefix() {
        let (_dir, swagger) = distribution(".a{}", "bundle bytes");
        let bundle = swagger.handle(&get(&format!("{ASSET_PREFIX}{BUNDLE}")));
        assert_eq!(bundle.status, 200);
        assert_eq!(bundle.body, "bundle bytes");
        // the traversal the asset server refuses, asked through this surface
        assert_eq!(
            swagger
                .handle(&get(&format!("{ASSET_PREFIX}../../Cargo.toml")))
                .status,
            404
        );
        // and a file with no extension the asset server serves: the licence is for a
        // reader of the repository, not for the browser
        assert_eq!(
            swagger
                .handle(&get(&format!("{ASSET_PREFIX}vendor/LICENSE")))
                .status,
            404
        );
    }

    #[test]
    fn the_page_carries_the_policy_and_the_two_headers_that_go_with_it() {
        let (_dir, swagger) = distribution(".a{}", "b");
        let response = swagger.handle(&get(SWAGGER_PATH));
        let header = |name: &str| {
            response
                .headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(header("Content-Security-Policy").as_deref(), Some(csp()));
        assert_eq!(header("X-Content-Type-Options").as_deref(), Some("nosniff"));
        assert_eq!(header("Referrer-Policy").as_deref(), Some("no-referrer"));
    }
}
