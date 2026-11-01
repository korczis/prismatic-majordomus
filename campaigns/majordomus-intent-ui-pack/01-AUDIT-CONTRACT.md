# 01 — FORENSIC AUDIT, OWNERSHIP AND CANONICAL CONTRACT

Inspect the repository, not just its README. Start by discovering and reading all applicable context files in `.ai/`, `AGENTS.md`, generated provider bootstrap files, ADRs, rules/doctrines, scopes, relevant schemas, validators and tests. Inventory actual roots `apps/majordomus-cli`, `bin`, `lib`, `site`, `docs`, `.just`, `test`, `deploy` and existing routes. Read `docs/CAPABILITIES.md`, `docs/EXECUTIONS.md`, `docs/COCKPIT.md`, `docs/WEB.md`, `docs/ENVIRONMENT.md` if present. Don't assume file names in older docs remain correct.

Build a concise traceability matrix for each domain: **intent, criterion, gap, critique, plan, milestone, issue, dependency, execution, task, session, peer, worktree, PR, check, release, deploy, evidence, authorization, projection, event**. For each identify canonical owner/format, reader and writer, type/schema, existing CLI action, capability registry member, HTTP exposure, MCP exposure, UI projection, real-time event, test, gate, documentation claim and maturity. Mark `verified-present`, `partial`, `absent`, `stale`, or `unknown`. Never replace unknown with fabricated capability names.

Inspect existing `capability!`, `module!`, `compose_modules!` patterns or their current successors; generated OpenAPI, benchmark catalog, Swagger and docs generation. Identify read-only capability paths versus shell-owned write logic. Trace one real example all the way from command to state mutation, back to derived reads and audit. Locate any duplication in UI navigation, frontend caches, backend registries and GitHub metadata.

Inspect actual UI stack, routes, asset pipeline, target browsers, Rust framework and present performance. Don't pick a new UI framework simply because it is trendy. Inventory execution event transport and semantics: durable event store? sequence? deduplication? SSE? WebSocket? reconnect cursor? backpressure? terminal state? Find mesh registration, discovery, liveness, peer identity, worker authorization and available logs/tools. Probe safely in local test mode.

Deliver architectural decision record(s) only if project ADR rules call for it. Distill exactly these contracts:

1. Ownership and schema of every canonical record.
2. Typed action invocation boundary (read vs write vs long-running vs privileged).
3. Derived relationship graph and stable links.
4. Events and execution replay contracts.
5. Authorization, scope, remote side effects and approvals.
6. Projection and cache invalidation strategy.
7. CI/CD/release readiness and external verification sources.

Produce a prioritized dependency graph. P0 is the first end-to-end vertical. P1 enables broader interactions. P2 is embellishment. Flag any planned but nonexistent substrate and choose a minimal additive implementation. Explicitly reject unnecessary rewrites, new independent stores, speculative protocols and 'global' synchronization state. Acceptance: another agent can locate each canonical owner, follow a specific call path and reproduce the pre-change baseline without guessing.
