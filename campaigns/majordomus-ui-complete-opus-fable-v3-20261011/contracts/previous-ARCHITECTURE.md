# Architecture and explicit anti-goals

Verified existing contracts: `Cockpit` in `apps/majordomus-cli/src/cockpit/mod.rs` is a server-rendered projection of capabilities; `/cockpit` is the existing prefix, `SHELL_SCRIPTS` contains `palette.js`, assets load per page, and JavaScript only enhances complete HTML. `apps/majordomus-cli/src/web/mod.rs` owns topology/discovery/manifest/validation; `docs/COCKPIT.md` lists server-side sources for routes. Design source is `share/design/tokens.yaml`; built Rust copy is generated.

Architecture: canonical repository/worker/evidence state → existing typed capability executor & registries → typed read projections (with provenance and authorization) → reusable SSR components → optional lazy-loaded libraries. Write actions return through canonical executor and durable evidence. Static site consumes versioned publication projections and never simulates connected live peers.

Do NOT: recreate HTML shell as SPA; hand-maintain route list twice; hardcode scores/dates; depend on a network CDN at runtime; alter generated CSS/token copies directly; mirror fake peers/events; add p5 canvas where simple CSS suffices; duplicate domain logic in Alpine; pull every high-cardinality entity at each refresh; make partial backend failures look green; confuse advertised mesh discovery with remote execution.

UI state model: available(value, source_revision, observed_at), loading, empty, unknown, stale, unsupported, forbidden, failed(reason,retry). Source/version/duration fields are optional; no coercion to 0. Stable deep links and canonical IDs everywhere. Selection/filter/sort may be local presentation state, but domain statuses and permissions come from server.

Security: no secrets in serialized projections, escape all content, CSP-compatible assets, CSRF and auth on mutation, server-validated inputs, auditable execution, request cancellation semantics, no surprise deploy or rollback.
