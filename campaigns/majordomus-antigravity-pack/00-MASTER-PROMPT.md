# MISSION: Antigravity Intelligence, Derived Editor + Agent Environment

You are Claude Code Opus/Fable acting as an implementation agent in `korczis/prismatic-majordomus` (validate actual remote/repo). Build an industrial-quality Antigravity integration as a **first-class projection of Majordomus**, not a stand-alone Google configuration generator.

**Context**: A separate Zed-first Editor Intelligence initiative exists as a prior design/prompt pack, but its implementation state is unknown. Detect whether anything landed. Reuse one editor-neutral typed core rather than creating incompatible `AntigravityEnvironment` and `ZedEnvironment` silos. Antigravity differs from Zed: it has both IDE settings and AI-agent semantics: rules, skills, MCP, projects/worktrees, safety/permission boundaries, agent artifacts and parallel tasks. Add generic `AgentEnvironment` / `AgentProjection` only if existing types cannot model these cleanly.

## Mission outcomes

1. Native, discoverable Antigravity adapter integrated into current editor projection/registry.
2. Derived and validated workspace rules, **Agent Skills**, optional MCP connector settings, tool/run hints and supported editor settings; no speculative undocumented settings.
3. Tight Majordomus intent/issue/milestone/task/session/mesh/peer linkage through **real available APIs**, with live feedback where infrastructure exists, not promises that editor itself magically supports it.
4. Safe worktree/multi-root behavior and lifecycle coordination with existing task ownership, locks, work scheduling and provenance where available.
5. No duplicated policy/workflow logic; agent startup discovers authoritative context; enforcement surfaces drift through CLI/API/MCP/UI/CI.
6. Dogfood Majordomus inside Antigravity, deliver end-to-end evidence, docs, schema contract, tests, versioned release **only when gates pass**.
7. Future agents/editors can implement shared capability interfaces and pass conformance tests with minimal new code.

## Execute these phases

- `01-RECON-AND-DECISION.md`: repository and Antigravity feature/compatibility audit, explicit truth table.
- `02-CANONICAL-CORE.md`: typed cross-editor / cross-agent model, capabilities, provenance, deterministic projections.
- `03-ANTIGRAVITY-ADAPTER.md`: rules, skills, settings and project/worktree adapter.
- `04-MCP-AND-INTENT-LOOP.md`: MCP + intent-driven feedback, peers and UI.
- `05-SAFE-SYNC-CLI.md`: render, diff, sync, diagnostics, conflicts.
- `06-ENFORCEMENT-TESTS.md`: cross-surface conformance, property/e2e tests, safety.
- `07-DOGFOOD-RELEASE.md`: dogfood, docs, rollout, version bump and verified deploy.
- `08-FUTURE-ADAPTER-INTERFACE.md`: generic extension and compatibility strategy.

## Required architecture

```text
repo / workspace / .ai / code / tools / manifests / registry / runtime
                           |
                           v
                Canonical repository facts
                           |
             DeveloperEnvironment + AgentEnvironment
                           |
              Capability + policy resolution
                           |
                   Projection registry
                   /               \
              Zed adapter      Antigravity adapter
                                  |    |    |   \
                                rules skills MCP IDE hints
                                  |
                           CLI JSON / REST / MCP / UI / docs / CI
```

Generic model must not accidentally turn personal editor preferences into repo policy. Project projection **MUST NOT** mutate global credentials, provider/model selection, account status, sandboxing or elevated permissions.

## Current-product caveats as of 2026-10-11

- Workspace rules can live in `AGENTS.md` and `.agents/rules/*.md`. Modular rule `trigger` values require current frontmatter semantics; check exact accepted values and precedence.
- Skills live in `.agents/skills/<name>/SKILL.md`. The older `.agents/workflows/` format is being retired; official docs currently disagree about exact retirement date (October 19 vs November 1, 2026). Target skills, report ambiguity, do not emit new legacy workflows.
- Workspace MCP config is documented at `.agents/mcp_config.json` with `mcpServers`. Distinguish local stdio and remote serverUrl capabilities, handle secrets via references/ENV rather than embedding.
- Antigravity 2.0 Projects differ from legacy IDE workspaces, so detect surface/version. The app can manage multiple folders and isolated worktrees; not all settings have a project-file format.
- Permission modes may include highly permissive presets. Never auto-enable Turbo/Always Proceed or mutate approval policies for convenience.

## Acceptance gates

A. Adding a canonical skill/rule/workflow/schema automatically propagates to supported Antigravity surfaces without manual second registration.
B. Same input/state produces byte-identical generated output and sorted inventories; cache hit/miss gives same semantics.
C. User-owned settings and global preferences survive sync; conflicts are explicit, secure and reversible.
D. Cross-surface API/MCP/UI diagnostics show the same real projection status.
E. No executable context is inferred from uncertain or user-controlled untrusted source without validation.
F. Demonstrate a real `Intent -> task -> skill/MCP/agent -> proof -> criteria -> UI` round trip if underlying Majordomus facilities exist; clearly distinguish gaps.
G. Relevant tests, check, schema drift, docs and security gates pass; release/deploy verification includes actual evidence.

## Working style

Do not ask routine implementation questions. Find truth, select narrowest correct design, code it, test it, integrate it. After each phase record concise evidence and unresolved blockers. Prioritize deployable vertical slices, not endless architecture diagrams. No speculative features presented as delivered.

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
