# PHASE 07: Dogfood, docs, rollout, release and evidence

Apply the Antigravity projection to Majordomus itself using the same user command that downstream projects will run. Prove it also works in a second representative fixture/repo without hand-written hardcoded entries.

## Dogfooding

- Inspect current `.agents/` state. Do not overwrite existing custom skills/rules/MCP config. Migrate only generated/owned artifacts safely.
- Produce a minimal useful set of native rules/skills directly linked to canonical intent preflight, checks, docs, review, release only if underlying workflows exist.
- Validate project discovery, disabled/offline MCP degradation, worktree isolation and actual ability to access appropriate repo context.
- Demonstrate adding a new `.ai` canonical rule or workflow and that generated Antigravity configuration changes automatically and predictably.
- Document IDE setup for unsupported non-file settings via precise UI steps; no fake in-app API automation.

## Documentation

Derive human-oriented docs from canonical schema where possible: architecture, setup, ownership/merge, rules vs skills vs MCP, offline behavior, plugin capabilities, limits, security defaults, permissions, troubleshooting, migration from old `.agents/workflows/`, test matrix and future adapter guide. Respect existing README/frontmatter hierarchy, site, GitHub Pages. ADR only if existing policy warrants.

## Delivery and release

- Land small verified changes through repository's normal branch/PR/merge workflow; respect protection and approvals.
- Version bump per **actual** `.ai` / Elm-like / semantic release policy; do not assume 'every PR minor bump' is implemented until validated.
- Update changelog/releases from evidence; regenerate docs/site/OpenAPI and code artifacts, run mandatory gates and CI.
- Deploy only using established pipelines. Verify published package/tag, CLI help, documented URLs and runtime state from real results.
- If deployment or permissions unavailable, state exact blocker and stop claiming completion. Do not repeatedly spin uncontrolled release jobs.

## Final evidence report

1. Before/after capabilities + actual code files changed.
2. Canonical model and adapters reused/introduced.
3. Exact generated files and their ownership.
4. Tests + gates executed, passed/failed, CI links if available.
5. One measured end-to-end intent/skill/MCP/quality/evidence cycle; what could not be proven.
6. Release/tag/version/deployment facts, or precise blockers.
7. UX screen/view routes and API/MCP consumers, or gaps.
8. Remaining debt ranked, with no fabricated status.

## Non-negotiable execution protocol

- Inspect the **actual checked-out repository**, `AGENTS.md`, `.ai/**`, registries, schemas, existing editor integrations, quality gates, just/task metadata, release policy and GitHub integration before changing code. Paths/names in this pack are conceptual unless confirmed.
- Respect existing `RepositoryEnvironment`, `DeveloperEnvironment` and `EditorProjection` if they exist; if absent, implement the minimum typed abstractions. Never announce an existing implementation without finding it.
- Single canonical discovery and model, derived adapters, deterministic ordering, typed diagnostics, provenance (`value`, `source`, `confidence`, `reason`, `override`), versioned contracts and idempotent generation.
- No duplicate copies of `.ai` policies, workflow command catalogs, or MCP tool inventories. Prefer thin pointers / references / generated indices. Rules are constraints, skills are procedural instructions, MCP exposes executable/introspectable capabilities.
- Strictly separate project-owned semantics from personal user settings, credentials, model choice, account identity, UI layout and permission escalation. Never auto-enable permissive agent modes or overwrite global configs.
- All writes must have render/diff/check/sync semantics, safe preservation of user-owned fields, backup or reversible plan where appropriate, atomic replacement and explicit conflict diagnostics. Never silently destroy unmanaged files.
- Security: no key/token material written, no secret display in diagnostics, no arbitrary command execution during discovery, no implicit network during editor opening, no unrestricted workspace escape. No auto-approval of dangerous actions.
- Cross-platform path normalization and executable discovery, Unicode, unusual filenames, empty/missing tools, worktrees, multi-root projects, symlinks and non-Git projects need explicit treatment.
- Use existing CLI/API/MCP/UI/docs/gates; do not build independent parallel backends or frontend truths.
- Fail closed on unknown/unsupported Antigravity config semantics. Cite official docs and record tested version/capability, not guesses. On unsupported features expose diagnostic and actionable manual instructions.
- Work autonomously in small vertical slices; validate each before continuing. Do not merge/release/deploy without repo-native required checks; do not claim success when unverified. Do not bypass branch protections or security approvals.
- Keep implementation scope focused on Antigravity as the next editor/agent adapter and generic abstractions it actually needs. No premature implementation of VS Code, JetBrains or Neovim.
