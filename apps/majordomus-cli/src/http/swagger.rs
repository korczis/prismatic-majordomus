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
//! The UI's own assets are fetched by the browser from the unpkg CDN. That is the one
//! part of the HTTP projection that is not available offline; the OpenAPI document is.

use crate::web::home::{Identity, ID_SHOWN};
use crate::web::html;

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
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Majordomus API — {banner}</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@{v}/swagger-ui.css">
<style>
.majordomus-served {{
  font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
  padding: .6rem 1rem; border-bottom: 1px solid #d8dde3; background: #f6f8fa; color: #24292f;
}}
@media (prefers-color-scheme: dark) {{
  .majordomus-served {{ background: #161b22; border-bottom-color: #30363d; color: #c9d1d9; }}
}}
</style>
</head>
<body>
<div class="majordomus-served">Served by {banner}</div>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@{v}/swagger-ui-bundle.js" crossorigin></script>
<script>
window.ui = SwaggerUIBundle({{ url: "{spec}", dom_id: "#swagger-ui", deepLinking: true }});
</script>
</body>
</html>
"##,
        v = SWAGGER_UI_VERSION,
        spec = SPEC_PATH,
        banner = banner
    )
}
