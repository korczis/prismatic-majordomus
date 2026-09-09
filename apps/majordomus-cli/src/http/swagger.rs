//! The Swagger UI shell: one HTML page that loads the pinned Swagger UI distribution and
//! points it at `/openapi.json`. It embeds no specification of its own; what it shows is
//! whatever the server generated from the registry at the moment of the request. The
//! page is rendered once per process and served from memory.
//!
//! The UI's own assets are fetched by the browser from the unpkg CDN. That is the one
//! part of the HTTP projection that is not available offline; the OpenAPI document is.

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
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Majordomus API</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@{v}/swagger-ui.css">
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@{v}/swagger-ui-bundle.js" crossorigin></script>
<script>
window.ui = SwaggerUIBundle({{ url: "{spec}", dom_id: "#swagger-ui", deepLinking: true }});
</script>
</body>
</html>
"##,
        v = SWAGGER_UI_VERSION,
        spec = SPEC_PATH
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
/// ```
pub fn page() -> &'static str {
    PAGE.as_str()
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
        // which fails in the browser and nowhere else
        let shell = page();
        assert_eq!(
            shell
                .matches(&format!("swagger-ui-dist@{SWAGGER_UI_VERSION}"))
                .count(),
            2,
            "the stylesheet and the bundle both come from the pinned version"
        );
    }
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
