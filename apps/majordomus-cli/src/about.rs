//! What the executable says about itself, written once. The OpenAPI document's `info`,
//! the MCP `initialize` instructions, the HTTP index and the generated reference all
//! read these sentences from here, so that no projection describes the surface in words
//! of its own and none can drift from the others. Prose only: counts, URLs of a running
//! server and anything else that is measured are added by the projection that measures
//! it.

/// The product name, as every projection titles itself.
pub const NAME: &str = "Majordomus";

/// The source repository, from the crate manifest.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// The licence, from the crate manifest, as an SPDX identifier.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// Where the HTTP projection is explained for a person: the derived API reference on the
/// documentation site, rendered from the OpenAPI document itself.
pub const REFERENCE_URL: &str = "https://korczis.github.io/prismatic-majordomus/docs/api/";

/// One sentence: what is being served.
pub const SUMMARY: &str = "The read-only projection of a repository's AI layer: every rule, prompt, session and project record under .ai/, and the capabilities that read them, served over MCP, HTTP and the command line from one registry.";

/// The paragraphs a reader of any projection should have, in order. CommonMark, as
/// Swagger UI and the site render it; each entry is one paragraph.
pub const PARAGRAPHS: &[&str] = &[
    "**One registry, every interface a projection.** Every operation here is a capability of the registry and nothing is declared for HTTP alone. `operationId` is the capability's canonical id; the same capability answers as the MCP tool `x-majordomus-mcp` names and as the command `x-majordomus-cli` names, where its exposure says so, through one executor with one cache. A change to a capability's description or input type reaches this document, the MCP schema, the command line and the generated reference on the next start; nothing is edited by hand.",
    "**Loopback, unauthenticated, read-only.** The first `majordomus mcp` in a repository binds this server beside its stdio session and logs the URL; `majordomus serve` binds it alone. It listens on 127.0.0.1, asks for no credentials, and never writes to the repository: whoever can reach the socket can read the layer, and nobody can change it through it. `GET /` lists the routes; `/docs` is Swagger UI over this document; `/mcp` is MCP over HTTP for a second client.",
    "**Binding.** `GET` binds every top-level property of the capability's input as a query parameter, coerced by the schema's type (integers, numbers and booleans are parsed, everything else is text); `POST` binds the input as the JSON body. An unknown parameter is an invalid input, not ignored. Every failure is one JSON body, `{ \"error\": { \"code\", \"message\" } }`: 400 `invalid_input`, 404 `not_found`, 422 `refused` (a command's alone), 500 `internal`; 405 `method_not_allowed` names a path that exists under another method.",
    "**Examples are benchmark cases.** Every example in this document is one of the capability's own benchmark cases, the inputs the benchmarks time and the tests replay against a real socket: an operation without an example is an operation without a case, and the executable does not compile in that state. `x-majordomus-benchmark` and `x-majordomus-cache` carry the policies; `x-majordomus-stability` and `x-majordomus-provenance` say how far a capability is proved and where it was declared.",
    "**Generated, committed, checked.** This document is rendered from the registry at every request and committed as `docs/generated/openapi.json`; `majordomus generate --check` derives it again and refuses a stale copy, and CI runs that check on every push, so the committed document, the served one and the reference on the site are the same document.",
];

/// The paragraphs as one CommonMark text.
///
/// ```
/// use majordomus_cli::about;
/// let text = about::description();
/// assert!(text.starts_with(about::SUMMARY));
/// assert!(text.contains("**Binding.**"));
/// ```
pub fn description() -> String {
    let mut text = String::from(SUMMARY);
    for p in PARAGRAPHS {
        text.push_str("\n\n");
        text.push_str(p);
    }
    text
}
