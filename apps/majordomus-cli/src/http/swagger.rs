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
pub const DOCS_PATH: &str = "/docs";

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
        .find(|r| r.path == DOCS_PATH)
        .is_some_and(|r| r.linkable())
}
