# PHASE 01: Forensic audit, official capabilities, contract

Read the master prompt and repo policies. Inventory `editor` commands, config generation, shared language/toolchain schemas, existing Zed support, `.ai` projection mechanisms, current agent adapters, Claude/Gemini outputs, rules/skills, MCP server and schema exports, UI views, GitHub issues, worktree management and releases. Discover paths rather than assuming them.

## Evidence matrix (mandatory)

For every prospective feature record:

| Requirement | Canonical owner | Existing API/type | Antigravity surface/version | Verified format | Security/compatibility | Consumer | Test | Action |
|---|---|---|---|---|---|---|---|---|
| rules | policy registry | discovered | IDE/CLI/2.0 | documented | precedence/frontmatter | agent | fixtures | derive |
| skills | skill registry | discovered | IDE/CLI/2.0 | SKILL.md | progressive disclosure | agent | fixtures | derive |
| MCP | MCP registry | discovered | IDE/CLI/2.0 | mcp_config | auth | agent | integration | derive |
| editor settings | dev env | discovered | IDE/2.0 | verified | personal vs repo | editor | fixture | selective |
| projects/worktrees | repo/mesh | discovered | 2.0/IDE | verified | isolation | agent | e2e | bridge |
| permissions | user/app | not repo-owned | OS/version | verified | high risk | user | no-mutation | inform only |

Do not assume Antigravity is just VS Code JSON. Verify which VS Code settings/extensions/configuration are actually supported and applicable at project level. Never generate undocumented paths/keys.

## Official docs to inspect on execution day

- https://antigravity.google/docs/rules/
- https://antigravity.google/docs/skills
- https://antigravity.google/docs/mcp
- https://antigravity.google/docs/ide/workflows/
- https://antigravity.google/docs/migration/workflows-to-skills/
- https://antigravity.google/docs/projects/
- https://antigravity.google/docs/agent-settings
- https://antigravity.google/docs/settings
- https://antigravity.google/docs/plugins

Record version, last-checked date, contradictory specs and target surface. Documentation isn't proof of actual installed-app behavior. Use capability probes or explicit unknown status as appropriate. Watch the legacy workflows sunset conflict.

## Required deliverables

1. Current repo baseline and tested commands/results.
2. Implementation gap table ranked P0/P1/P2 by actual user value.
3. Decisions: reuse/extend core; config ownership; migration policy; formats; CLI UX; fail-closed behavior.
4. Conflict-risk analysis (global settings, user overrides, generated files, worktrees, multiple roots).
5. A short implementation contract with acceptance criteria and no manual duplicate inventories.

Do not stop after an audit; proceed into phase 02.

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
