# Repository Discovery / Gap Audit

Before design hardens, inspect actual HEAD, branch, commit, working tree, repo governance, existing open work and related PRs. Protect uncommitted user work; do not reset/overwrite it. Current repo code is authority, this prompt pack is not a repo snapshot.

## Enumerate and link evidence
- UI routes and route registry, `/ui`, `/ui/mesh`, `/ui/roadmap`, `/ui/issues`, `/ui/prs`, `/ui/evidence`, `/ui/delivery`, server components and browser assets.
- Data plane: `mesh.peers`, `mesh.events`, `monitor.stream`, source executor, `source::ask_all`, peer/session/work/execution/evidence/usage capabilities, actual schemas and field semantics. Determine which names still exist.
- Stream plane: current monitor API e.g. `/api/v1/monitor` if present, cursor semantics, polling/SSE/WebSocket support, retention windows, event IDs, delivery ordering, auth and backpressure.
- Canonical UI metadata e.g. `web.surfaces`, capability registries, OpenAPI/MCP derivation, typed Rust schema generation, import paths.
- Design: current token authority (historically discussed `share/design/tokens.yaml`), UI component conventions, asset bundling, CSP, static file serving, JS dependencies, privacy configurations.
- Build/test: cargo workspace, just recipes, actions, release gates, GH Pages deployment, browser tests, fixtures, benchmark facilities, ADR/governance workflows.
- Infrastructure: mesh identity/security, localhost broker/daemon where present, Jetson/remote kiosk deployment affordances, frontend security model, event storage ownership, resource limits.

## Produce audit matrix
`candidate capability | actual source path/command | authoritative field schema | freshness | privacy classification | stable ID | streaming strategy | scene consumers | missing gap | evidence`.

## Resolve contradictions
If historical discussion asserts a capability that no longer exists, note `absent` and choose compatible current mechanism. Never hardcode paths from this document without verification. Never expose nonexistent CLI/API endpoints simply to match previous prose. Do not make up data during gaps. Keep first vertical slice independent of any speculative economics instrumentation.

## Initial proof of transport
Use minimal sample data from **actual** canonical adapter and demonstrate end-to-end IDs + timestamps + source IDs + reconnect behavior before expanding art. Verify permissions and content security policy. Inspect available replay/log semantics and avoid performing privileged operations when just reading.

## Decision memo
Deliver short `Reuse / Extend / Reject` decisions for p5.js vs native canvas/WebGL, transport, schema, scene registration, derived docs, source storage and public redaction. Record trade-offs, avoid gratuitous frameworks.
