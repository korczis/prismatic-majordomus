# The OpenAPI document is inferred: the tags are the modules, the examples are the benchmark cases, the responses are the router's statuses by kind, the prose is the one text every projection shares, and the site's API reference is rendered from the committed document

## What it means

Nothing in `docs/generated/openapi.json` is written by hand and nothing in it is written for OpenAPI alone. Every tag is a module of the registry, described in the module's own words. Every example on a query parameter or a request body is one of the capability's benchmark cases, by name: the inputs the benchmarks time and the crate's suite replays over a real socket. Every response is a status the router can answer for that kind of capability: 200, 400 `invalid_input`, 404 `not_found`, 500 `internal` for every operation, 422 `refused` for a command only. A query parameter is never declared nullable and never defaults to `null`, because a query string is text and `required: false` already says the rest. `info.summary`, `info.description`, the licence, the contact and `externalDocs` come from `about.rs` and the crate manifest, the same text the MCP `initialize` instructions open with and the HTTP index carries. The documentation site projects the committed document into its data, renders `/docs/api/` from that projection and serves the document raw at `/openapi.json`; `externalDocs.url` in the document is that route, and the site's checks refuse a build where the page, the copy or the route is missing.

## How it works

`http/openapi.rs` walks the registry: for each capability with an HTTP exposure it asks the registry for the capability's case provider, the same `BenchmarkCases` implementation the `capability!` macro requires of every input type, and evaluates it against the repository's index, so an example names an object the repository holds. The tags are `registry.modules()` filtered to the modules that put something on the wire. The responses are built from one table of statuses, keyed by `CapabilityKind`. `about.rs` is the one module the OpenAPI `info`, `mcp/protocol.rs` and the HTTP index read. `Request::bind` inverts the router's binding so that the cases reach the routes as a client would send them, in `tests/http_serve.rs` and `benches/routes.rs` alike. `scripts/generate-site-data` projects the committed document through `scripts/lib/openapi-site.jq` into `site/data/generated/openapi.json`, refusing an operation without a tag, a response or an example, and writes the derived page; `site/templates/api.html` renders it; `scripts/site-check` requires every operation and tag on the page and the raw document at `/openapi.json`.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus generate openapi --check     # in sync
jq '.tags, .paths["/api/v1/object"].get.parameters[0].examples' docs/generated/openapi.json
jq '.paths["/api/v1/peers/announce"].post.responses | keys' docs/generated/openapi.json   # 200, 400, 404, 422, 500, default
jq '.paths["/api/v1/object"].get.responses | keys' docs/generated/openapi.json            # no 422: a query has nothing to refuse
cargo bench --manifest-path apps/majordomus-cli/Cargo.toml --bench routes                # every route, every case
```

## What it does not cover

Response examples: producing one would mean executing the capability while the document is built, and the document is a projection, not a run. The committed document carries the examples of this repository's index, so a new object that sorts first under a kind changes `docs/generated/openapi.json` and `generate --check` asks for a regeneration, as it does for any other change to the registry. Swagger UI on the site: the site serves no third-party script (`scripts/site-check`, rule 8); the running server's `/docs` is Swagger UI over the same document.

## Why it exists

The operator's rule for the Rust executable: what can be inferred from code, data and annotations is inferred, documented, tested, benchmarked, checked automatically and kept in sync; a hand-written example drifts, a benchmark case is replayed. `apps/majordomus-cli/tests/http_serve.rs` replays every route's cases over a socket and asserts the document shows them; `test/cases/92_openapi_reference.sh` generates the document inside a repository `init` wrote, inspects the tags, the examples, the statuses and the prose, and checks the committed copies in this tree against the registry, each other and the site.
