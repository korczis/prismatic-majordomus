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
