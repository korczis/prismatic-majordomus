# PHASE 08: Future-proof adapters without framework theater

Future targets may include VS Code, JetBrains, Neovim, Cursor, Windsurf, Claude Code and other agent runtimes. Do **not** implement them now. Ensure extension is demonstrably possible by registering one fake/conformance adapter in tests and by articulating provider capabilities, scopes and safety boundaries.

## Capability taxonomy

- Editing: formatting, diagnostics, LSP, schema association, tasks/commands, launch/debug hints.
- Agentic: persistent rules, conditional instructions, skill packs, MCP, environment context, task resume, evidence collection.
- Operational: multi-root, worktrees, agent coordination, event streaming, project-local vs global settings, offline mode.
- Permissions: mutating tools, terminal command approvals, outside-workspace access, data exfiltration controls.
- Outputs: native project files, generated snippets, runtime API resources, manual-configuration guides.

Each adapter declares per-surface supported/unknown/deprecated/unsupported/requires_user_action status; no adapter may silently degrade security. The capability resolver picks only verified, supported project-owned semantics. A missing native feature is a diagnostic, **not** an excuse to invent a config key.

## Conformance protocol

1. Adapter discovery and metadata schema validation.
2. Pure deterministic projection under input permutations.
3. Zero manual registration of canonical entities in consumers.
4. No global/personal setting mutation.
5. Diff/check/sync safely preserving user ownership.
6. Provenance/explain and typed diagnostics.
7. Offline behavior, version compatibility and migration.
8. CLI/API/MCP/UI parity where shared domain applies.
9. New adapter addition does not change output of existing adapters.
10. Security fixture disallowing implicit permission escalation.

## Complexity controls

One real shared domain, two real adapters only if already available, and one fake conformance adapter in tests. Avoid giant plugin SDK, dynamic-language plugin loader, source rewriters or editor-specific core enums masquerading as extensibility. Schema versions and adapter compatibility migration must be documented.

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
