//! The Swagger UI shell: one HTML page that loads the pinned Swagger UI distribution and
//! points it at `/openapi.json`. It embeds no specification of its own; what it shows is
//! whatever the server generated from the registry at the moment of the request. The
//! page is rendered once per process and served from memory.
//!
//! The UI's own assets are fetched by the browser from the unpkg CDN. That is the one
//! part of the HTTP projection that is not available offline; the OpenAPI document is.
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

use std::sync::LazyLock;

/// The Swagger UI distribution version the page pins.
pub const SWAGGER_UI_VERSION: &str = "5.17.14";

/// The path the page loads the specification from.
pub const SPEC_PATH: &str = "/openapi.json";

/// The path the page is served at.
///
/// It is `/swagger` and not `/docs`: `/docs` is where this repository's own documentation
/// is served, and a viewer for the API is not the documentation. The rule that holds the
/// two apart is `project.web-surface-declared-once`.
pub const SWAGGER_PATH: &str = "/swagger";

static PAGE: LazyLock<String> = LazyLock::new(|| {
    format!(
        r##"<!doctype html>
<html lang="en" class="light">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Majordomus API</title>
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
        v = SWAGGER_UI_VERSION,
        spec = SPEC_PATH,
        tokens = crate::web::html::TOKENS
    )
});

/// The page, rendered once.
///
/// ```
/// use majordomus_cli::http::swagger;
/// let page = swagger::page();
/// assert!(page.contains(swagger::SWAGGER_UI_VERSION));
/// assert!(page.contains("url: \"/openapi.json\""));
/// assert!(!page.contains("\"paths\""), "the shell embeds no specification");
/// assert!(page.contains("--font-sans"), "the shell carries the repository's design tokens");
/// ```
pub fn page() -> &'static str {
    PAGE.as_str()
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

    #[test]
    fn the_shell_points_at_the_generated_document_and_carries_none_of_its_own() {
        // the whole claim this module makes: what a reader sees is whatever the server
        // generated from the registry at the moment of the request, never a copy
        let shell = page();
        assert!(shell.contains(&format!("url: \"{SPEC_PATH}\"")));
        for embedded in ["\"paths\"", "\"openapi\"", "\"components\""] {
            assert!(
                !shell.contains(embedded),
                "the shell embeds {embedded}, so it can disagree with the registry"
            );
        }
        // and it is rendered once: two calls hand back the same allocation, not two
        assert!(std::ptr::eq(shell.as_ptr(), page().as_ptr()));
    }

    #[test]
    fn the_pinned_distribution_is_the_one_both_asset_urls_name() {
        // a half-upgraded pin loads a stylesheet from one version and a bundle from another,
        // which fails in the browser and nowhere else.
        //
        // The subject is the asset URLs, not every mention of the distribution: the widget
        // also names it in `data-mj-foreign`, which is a declaration about whose component
        // tree this is and not a thing the browser fetches. Counting bare occurrences
        // conflated the two and made this test refuse a correct page, so it counts the
        // fetched URLs, and separately holds every mention to the pinned version — which
        // is stronger than the count was, and does not break when another mention is added.
        let shell = page();
        let fetched = shell
            .matches(&format!(
                "https://unpkg.com/swagger-ui-dist@{SWAGGER_UI_VERSION}/"
            ))
            .count();
        assert_eq!(
            fetched, 2,
            "the stylesheet and the bundle both come from the pinned version"
        );
        let pinned = shell.matches("swagger-ui-dist@").count();
        assert_eq!(
            shell
                .matches(&format!("swagger-ui-dist@{SWAGGER_UI_VERSION}"))
                .count(),
            pinned,
            "every mention of the distribution names the pinned version, not only the URLs"
        );
    }
}
