# PHASE 03: Antigravity native adapter, rules, skills, project configuration

Implement a production-grade Antigravity adapter through the projection interface. Autodetect supported Antigravity surface, version and config mechanisms where safely possible. Do not use global configuration as an implicit project policy target.

## Rules

- Reuse/derive canonical Majordomus `.ai` governance into native directory-aware rule projections: `AGENTS.md`, `.agents/rules/*.md`, or reference mechanisms supported by Antigravity. Choose existing root canonical policy bridge rather than creating two competing AGENTS files.
- Current docs require YAML frontmatter `trigger: always_on | model_decision | glob | manual` for modular rules; `model_decision` has descriptions, `glob` has glob patterns. Inspect exact schema and test parsing. Plain `AGENTS.md` has different semantics.
- Keep always-on instructions extremely concise; prefer progressive-disclosure model_decision rules and linked canonical content to avoid token-budget bloat.
- Model rule scopes/precedence, including nested modules and `.agents/rules.json` composition if supported. No uncontrolled inheritance cycles or path escapes. Validate that every generated rule is actually discoverable and under size budgets.
- Directory-specific rules should not be flattened into a root global wall of text; preserve activation and applicability.

## Skills instead of deprecated workflows

- Generate native `.agents/skills/<slug>/SKILL.md` with official name/description frontmatter and minimal steps delegating to existing canonical workflow actions (task runner, CLI, MCP) rather than copied instructions.
- Use skills for common real tasks discovered in repo: intent preflight, work claiming, implementation, adversarial review, test/quality, release, deploy, docs and incident follow-up, but **only** when canonical workflows/capabilities support them.
- Support local resources/examples only where needed. Derive skill names from canonical IDs; deterministic collision handling; descriptions suitable for automatic activation without flooding context.
- Add a migration plan for existing `.agents/workflows/`: detect, report, convert only ownership-safe cases to skills, preserve semantics, never create new legacy workflows. Official docs disagree on retirement date; avoid relying on either date.
- Skills must not encode blanket deployment permissions or unreviewed secret-handling shortcuts.

## IDE / project UX

- Where verified, derive workspace project hints/settings/tool integrations; avoid unsafe assumptions that Antigravity accepts every VS Code configuration setting.
- Promote relevant canonical commands as agent skill tasks, not speculative proprietary IDE menus.
- For project multi-root support record constituent repo roots and provenance; for worktrees separate per-worktree ephemeral state from committed repo config.
- Do not force model provider, subscription, theme, fonts, keymaps, terminal auto-execution or external-file permissions.
- Document manual UI actions for features that cannot be safely configured through supported project files.

## Adversarial fixtures

Empty repo; Rust + Elixir monorepo; directory-specific conflicting rules; conflicting AGENTS/GEMINI; unsupported trigger; >24KB rule; too-many-rules context; skill name collisions; nested skills; symlink escapes; accidental high-risk commands; user-owned existing files; project with no agent config; platform-specific editor unavailable.

## Deliverable

A minimal, valid, reproducible native Antigravity config projection plus capability report, unit tests and a representative demo fixture, without changing personal settings.

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
