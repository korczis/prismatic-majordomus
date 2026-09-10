//! The Swagger UI shell: one HTML page that loads the pinned Swagger UI distribution and
//! points it at `/openapi.json`. It embeds no specification of its own; what it shows is
//! whatever the server generated from the registry at the moment of the request.
//!
//! It also says whose server it is. A browser holds a page long after the process that
//! served it has gone, and several sessions on one machine each bind their own server; a
//! shell that named no checkout let a page from an exited server, and then a page from
//! another session's tree, both read as this repository — with the UI blaming CORS for
//! what was a closed socket. The banner is server-rendered HTML, so it still says which
//! checkout answered even when every request from the page fails.
//!
//! The identity is the repository's name and the hash of its root, never the root: this
//! page is served to whoever can reach the socket, and `web::home` withholds the path for
//! the same reason.
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
//! The frame around it is this repository's, and so is what the frame owes a reader: a
//! `<main>` landmark and a level-one heading, which the page had neither of until the UI
//! audit was first allowed to look at this surface. The widget itself carries
//! `data-mj-foreign`, the declaration a page makes when a subtree is a third party's to
//! answer for: the accessibility engine skips it and the audit's report says it was
//! skipped and why, rather than reporting a clean page or an unfixable one.
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

use crate::web::home::{Identity, ID_SHOWN};
use crate::web::html;

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

/// The shell for one process, naming the repository it answers for.
///
/// ```
/// use majordomus_cli::http::swagger;
/// use majordomus_cli::web::home::Identity;
/// let page = swagger::page(&Identity {
///     version: "0.2.0",
///     summary: "a summary",
///     repository: "prismatic-majordomus",
///     id: "aceb47bab0d2cab647ce994dcc003157",
///     revision: Some("0123456789abcdef"),
///     capabilities: 7,
/// });
/// assert!(page.contains(swagger::SWAGGER_UI_VERSION));
/// assert!(page.contains("url: \"/openapi.json\""));
/// assert!(page.contains("prismatic-majordomus"), "the shell names its checkout");
/// assert!(page.contains("aceb47bab0d2"), "and the identity that distinguishes it");
/// assert!(!page.contains("\"paths\""), "the shell embeds no specification");
/// ```
pub fn page(identity: &Identity<'_>) -> String {
    let revision = identity
        .revision
        .map(|r| format!(" · revision {}", &r[..12.min(r.len())]))
        .unwrap_or_default();
    let banner = format!(
        "{} · {} · majordomus {}{}",
        html::escape(identity.repository),
        html::escape(&identity.id[..ID_SHOWN.min(identity.id.len())]),
        html::escape(identity.version),
        html::escape(&revision),
    );
    format!(
        r##"<!doctype html>
<html lang="en" class="light">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Majordomus API — {banner}</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@{v}/swagger-ui.css">
<style>
{tokens}
/* The frame: the page around the widget, in this repository's type and colour. */
body {{ margin: 0; background: var(--mj-bg); color: var(--mj-fg); font-family: var(--font-sans); }}
.swagger-ui, .swagger-ui .info .title, .swagger-ui .opblock-tag {{ font-family: var(--font-sans); }}
.swagger-ui .microlight, .swagger-ui code, .swagger-ui pre {{ font-family: var(--font-mono); }}
/* Swagger UI's topbar is its own branding and a form for choosing a specification. This
   page serves one specification and the process advertises its surfaces on its home page,
   so the bar is a logo and a field that must not be used. */
.swagger-ui .topbar {{ display: none; }}
.swagger-ui .info .title small.version-stamp {{ background: var(--mj-accent-fill); }}
.swagger-ui a {{ color: var(--mj-accent); }}
/* The frame owes the page a landmark and a heading; the widget renders its own title as an
   h2 beneath this one, so the order holds. Without them the page has neither, which is what
   the audit found the first time it was allowed to look at this surface. */
.mj-api-title {{ margin: 0; padding: 1rem 1.25rem 0; color: var(--mj-fg); }}
</style>
</head>
<body>
<h1 class="mj-api-title">Majordomus API</h1>
<main id="swagger-ui" data-mj-foreign="swagger-ui-dist@{v}"></main>
<script src="https://unpkg.com/swagger-ui-dist@{v}/swagger-ui-bundle.js" crossorigin></script>
<script>
window.ui = SwaggerUIBundle({{ url: "{spec}", dom_id: "#swagger-ui", deepLinking: true }});
</script>
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
