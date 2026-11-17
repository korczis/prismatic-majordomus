# MAJORDOMUS EDITOR INTELLIGENCE — MASTER EXECUTION PROMPT

Target: Claude Code Opus / Fable, operating in `korczis/prismatic-majordomus`.
Mission: implement a production-grade, canonical, derived Editor Intelligence subsystem, with Zed as the first complete adapter, and an extensible editor-neutral core.

## Operating mandate

You have authority to inspect, implement, refactor, test, document, commit, push, integrate, release and deploy changes within the repository's existing authorization, security and branch protection policies. Work autonomously on reasonable implementation choices. Do not ask for filenames, module naming, ordinary design decisions or trivial confirmations. Do not claim a deployment or release unless actually verified. Do not bypass required approvals or branch protections. Keep user data and machine-specific state intact. No secret propagation or destructive worktree cleanup. If blocked, record the exact blocker and complete all independent work.

**This prompt expresses desired end state, not a description of features verified to exist.** Start with the actual repository. Inspect AGENTS.md and relevant project instructions, `.ai/**`, `.majordomus/**`, source, Cargo workspace, CLI, MCP, API/OpenAPI, Cockpit/UI, build and release workflows, tests, docs, existing `.zed/`, editor configs, just recipes, and deployment reality. Identify what exists, what is partial, what is hypothetical. Reuse canonical mechanisms. Avoid another parallel subsystem.

## North star

`repository facts -> canonical typed RepositoryEnvironment / existing models -> EditorEnvironment discovery and policy resolution -> deterministic editor-neutral projection -> capability negotiation -> adapter render -> explain/diff/check/sync -> CLI/API/MCP/UI/docs/CI`.

Do not place editor-specific JSON keys into repository-domain models. Do not infer personal aesthetic preferences from repository facts. Do not force a future editor implementation now. Build a narrow, sound interface with one fully working Zed adapter and conformance fixtures proving second-adapter viability.

## Stage 0: inventory and evidence

1. Read applicable instructions and audit repository status, active issues/PRs/milestones/worktrees, code ownership, release rules.
2. Discover source of truth for languages, toolchains, formatters, linters, tasks, tests, launch commands, generated paths, schema registry, user policy, and services.
3. Map existing discovery and projection primitives; identify reusable traits, serialization, diagnostic types, cache, hashing, provenance, rendering, transaction safe-write and validation infrastructure.
4. Audit current `.zed/settings.json`, `.zed/tasks.json`, user-maintained content, existing editor configuration, tracked vs local files; establish preservation strategy.
5. Read up-to-date official Zed documentation for project settings, tasks, environment inheritance, formatters, LSP, JSON and YAML schema association, project instructions, MCP, external agents, supported semantics and conflicts. Prefer official docs and installed version where available. Cite capability evidence in the implementation ADR/doc.
6. Build an evidence matrix: domain fact, source, current consumer, proposed projection, precedence, whether verified, migration needed, risk, test strategy.
7. Define a small independently releasable vertical slice and sequence further slices based on actual dependencies. Avoid large disconnected redesign.

## Core requirements

### Typed canonical data

Provide (or reuse an existing equivalent of) `DeveloperEnvironment` / `EditorEnvironment` containing typed language capabilities, repository conventions, formatter/linter/check policy, available toolchain, workflow/task descriptors, schema associations, generated directories, editor integration availability, diagnostics, provenance and uncertainty. Separate *observed facts*, *declared canonical policy*, *derived recommendations* and *user overrides*. Add schema version and deterministic stable identities. No untyped catch-all blob as canonical model.

Never claim a language server or executable is present solely because a language is detected. Represent unresolved/unknown distinctly from unavailable and available. Make workflows executable only when valid command resolution succeeds.

### Resolution and precedence

Design a deterministic precedence contract and document it. Suggested conceptual precedence: repository enforced policy > repo canonical explicit declarations > inferred repo convention > editor defaults > user preference when non-policy. This is NOT a blanket order for all settings. Personal settings belong to the user and must be excluded from repo-owned projection; security controls always trump convenience. Conflicts must yield actionable diagnostics rather than arbitrary overrides. Do not merge unknown JSON structures destructively.

### Editor capabilities

Use a typed `EditorAdapter` contract (names per repository style), including editor identity, supported target files, target version/capabilities, feature support, projection, parsing if supported, owned-region/field discovery, diff, validation, migration diagnostics, and deterministic rendering. Model supported/unsupported/partial capabilities explicitly. Do not assume editors share project settings format, LSP configuration, task runners, instruction semantics, or formatters.

Future examples are VS Code, Neovim and JetBrains; they are architectural fit/conformance examples only. Do NOT implement incomplete pseudo-adapters or add false UI options. A new adapter should reuse discovery, provenance, validation, CLI orchestration, and projection contracts with no change to core model.

### Zed adapter (complete)

Derive `.zed/settings.json` and `.zed/tasks.json` from canonical metadata, emitting only meaningful overrides to Zed defaults. Where project settings are unsupported or unsafe, report limitation instead of fabricating. Project workflows refer to canonical CLI/just commands (whichever is real), not duplicate business logic. Use arguments arrays/escaping when supported; avoid injection and path breakage. Handle multi-root and linked worktrees. Never create secrets or machine-specific absolute paths in tracked files. Preserve unrelated user-owned keys and tasks; establish generated ownership and collision diagnostics before writing. Ensure stable JSON formatting and ordering.

Resolve per-language settings, formatter ownership, format-on-save policy, LSP preferences, generated/cache exclusion patterns and file associations strictly from real repo facts/policy. Do not blindly disable built-in editor behavior. Detect conflicts between editor format-on-save and repo canonical formatting; emit precise diagnostics. Infer JSON/YAML schema associations from existing schema registry/paths and match semantics, not globs invented to satisfy one fixture. Validate actual schema reachability and format compatibility. Be precise about relative paths and worktrees.

Zed AI/agent layer: inspect the actual instructions precedence and supported scopes (`AGENTS.md` and other recognized files), MCP and ACP/external agent support. Reuse canonical generated instructions only when the editor reliably consumes them, with a single source and drift gates. Do not create additional competing `AGENTS.md` files. Treat provider credentials, model selection, paid account and personal agent tool permissions as personal/security preferences: never emit API keys or silently loosen approvals. Offer optional supported integration only where it can be demonstrated end to end.

### Commands and UX

Integrate with existing CLI grammar instead of inventing duplicate commands. Desired conceptual UX:

`majordomus editor status [--json]`
`majordomus editor explain [zed] [--field path] [--json]`
`majordomus editor list [--json]`
`majordomus editor zed render [--format json]`
`majordomus editor zed diff [--json]`
`majordomus editor zed sync [--check|--dry-run]`
`majordomus editor zed check`
`majordomus editor doctor`

Adapt names to existing conventions; avoid command collisions. Default `render/diff/status/check/explain` MUST be read-only. `sync` uses atomic writes, dry-run, diff preview and safe conflict behavior. `sync` must be idempotent and preserve nonowned personal/project customizations. Machine-readable output must be stable, versioned where needed, and ANSI-free. Commands should be useful without Zed installed; only runtime-only checks should require it.

### Projection ownership and migration

Define explicit ownership granularity for files/fields. Avoid assuming ownership of entire existing `.zed/settings.json` or `.zed/tasks.json`. Establish rules for absent files, wholly generated files, partially managed files, legacy unmarked files, unknown keys, comments/JSONC, duplicate labels, manual changes, formatting differences, symlinks and permissions. Use safe three-way reconciliation or a proven alternative for any overlap: derived desired state + previous generated baseline + current file; accept nonconflicting edits and refuse destructive conflicts with targeted diagnostics. Never silently overwrite user edits. Ensure failure leaves files intact and rerun recovery is possible.

### Explain/provenance

For each derived setting/task/schema association, make available: canonical ID, value, source path or registry entry, inference rule, precedence/decision, editor capability, emitted location, confidence/unknown status, version and conflict reason. Keep provenance internal to introspection or sidecar state as appropriate, not invalid JSON editor keys. Redact sensitive metadata. Cross-surface outputs must share the canonical explanation model.

### Cache and performance

Reuse existing repository environment snapshot/cache primitives. Discovery and rendering are local/offline and must not execute builds or network requests. Define cold/warm budgets empirically based on existing standards. Cache keys include relevant toolchains, repo metadata, `.ai`, canonical registries, workflows, schema changes, project config, adapter/schema version and policy. Ensure deterministic invalidation. Different worktrees must not share unsafe mutable target state. Watcher or live updates are optional only if cheap and connected to existing event infrastructure.

### Enforcement

Extend the existing `.ai` rules/doctrines/gates for derived editor projections, schema source truth, user preference boundaries, deterministic output, secure task execution and drift. Do not add unenforced Markdown theater. Wire checks to canonical local quality gates and CI at minimal cost. Detect drift, unsupported semantic mapping, invalid schema paths, hardcoded duplicated workflow bodies, secrets, unsafe shell construction and unauthorized overrides. Diagnostics must be actionable.

### Surfaces

Project onto existing relevant CLI structured output, API/OpenAPI, MCP, Cockpit/UI, docs and environment banner only if the canonical infrastructure truly exists. No parallel catalog, no independent JS ordering, no artificial Swagger-only definitions. Expose editor status, source explanation, generated diffs, compatibility, schema associations, task inventory and safe sync. UI must remain usable, responsive, keyboard-friendly, mobile-friendly, accessible and derived. Do not introduce another frontend stack. For mutating API/UI actions enforce permissions, request validation and CSRF/origin policies where appropriate. Read-only status is a shippable first vertical.

### Testing

Unit: typed normalization and ordering, precedence, inference, unknowns, conflicts, capability matrix, version compatibility, escaping, schema association, field ownership, provenance, redaction.

Property/metamorphic: randomized input order, repeated render byte stability, cache hit vs miss, unrelated changes not perturbing output, idempotent sync, safe parsing and serializing, no secret leakage.

Filesystem: no `.zed/`, existing generated, mixed ownership, edited files, invalid JSON/JSONC, read-only file, symlinks, path spaces, nested workspaces, linked Git worktrees, missing language servers, absent tools, partial schemas, unicode, newline behavior, concurrent sync, interrupted write and rollback.

Cross-surface: canonical IDs/order/evidence align through CLI JSON, adapter output, API, MCP, UI model and docs index where those surfaces exist. Use one canonical fixture instead of golden copies for each surface. Test Zed task executable mapping with real repository workflow IDs rather than theoretical strings.

Security: malicious task labels/paths, shell metacharacters, hostile schema path, path traversal, unsafe overwrite, secrets in env, execution permission, injection and remote URLs. No executing derived tasks during rendering/checking.

Performance: benchmark representative cold/warm render, diff and check; report reproducible measurements and regressions.

### Rollout, release, and maintenance

Work in small coherent slices: (1) discovery and contract, (2) typed model and fixtures, (3) Zed projection read-only, (4) safe sync/migration, (5) validation/enforcement, (6) surface integration and UX, (7) docs/dogfooding/release. Complete working verticals as soon as practical, merge per actual repository policy, bump versions based on existing semver/Elm-like and project-specific policy rather than assuming minor bump for every change. Run relevant CI and verify outputs before claiming released or deployed. Update changelog and `.ai` according to canonical generators. No fabricated deploy status.

### Acceptance gates

A. With no Zed installed, `render/diff/check/explain` operate on the repo accurately.
B. An actual repo language/task/schema addition propagates to `.zed` with zero manual consumer registration.
C. Repeated render is byte-identical; sync twice has no second diff.
D. User-owned settings and credentials remain untouched; deliberately conflicting edits produce clear refusal.
E. Schema associations correspond to real schemas; workflow tasks route to existing canonical commands.
F. `.ai` and existing CI enforce project invariants; intentionally introduced drift is caught.
G. CLI, JSON, UI/API/MCP where implemented agree on status, IDs, provenance and order.
H. Zed adapter is production usable, documented and dogfooded in Majordomus; hypothetical future editors are not advertised as supported.
I. A minimal second-adapter conformance test or fake adapter demonstrates adding a new editor without modifying core discovery and policy resolution.
J. Tests, security checks, docs build and release verification yield recorded exact commands and results.

## Required completion report

Provide a concise table: audit of existing reality / architecture decisions / actual files modified / Zed output before and after / migration safety / tests with exact commands and pass-fail results / cross-surface verification / CI & release artifacts with links / deployed URLs and health evidence if genuinely deployed / remaining blockers. Include explicit evidence of zero-registration discovery and safe user-override preservation. No success claims based only on plan, generated prompt or unexecuted tests.

## Non-goals

No five-editor rewrite; no AI provider key management; no personal theme/font overwrites; no independent editor-owned workflow registry; no installer network dependency on directory entry; no forced IDE runtime; no generating unknown LSP settings from guesses; no hardcoded enum listing all editors in unrelated domain models; no high-cost background polling; no weakening CI; no dangerous git cleanup.

Execute the mission through working code now. Prefer incremental end-to-end delivery and evidence over architectural diagrams with no implementation.
