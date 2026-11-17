# PHASE 06: CI enforcement, property testing, security and contracts

Use repository-native `check`/CI/rule enforcement gates and docs generator. The rule must be executable, not ceremonial Markdown.

## Contract invariant

> A supported editor/agent integration is a typed, deterministic, explainable projection of canonical repository facts and policies. It may not introduce a parallel policy/workflow/skill/MCP catalog, mutate personal editor preferences or circumvent execution permissions.

## Test matrix (minimum)

1. Pure render same inputs -> byte identical artifacts.
2. Randomized source order / concurrency / HashMap insertion order -> identical output.
3. Adding/removing a canonical rule/skill/workflow -> projection updates everywhere, zero consumer registration.
4. Valid/invalid modular rule triggers, frontmatter, nested scope, glob association, rule size/token overflow and precedence.
5. Skills activation metadata and migration from legacy workflow; both legacy retirement dates must not affect adapter-generated output.
6. MCP config stdio vs serverUrl, offline, malformed, wrong transport, secret redaction, preexisting servers, auth/permission denial.
7. User-owned files survive sync; generated drift detected; conflicting ownership rejected; changed generated file preserved until explicit remediation.
8. Non-project global configs unchanged, elevated permission modes unchanged, credentials never serialized.
9. Multi-root and worktree collision/canonical paths, symlink traversal defense, empty repo, no Git, missing editor and unsupported Antigravity version.
10. CLI status -> API payload -> MCP representation -> Cockpit projection parity on stable canonical IDs/diagnostics.
11. Unknown support state -> no undocumented settings emitted, actionable diagnostics.
12. Apply idempotency, dry-run no mutation, atomicity under interruption, process concurrency.
13. Regression tests for existing Zed adapter if present; adding Antigravity must not alter other adapter outputs.
14. Generated docs schema drift and verified canonical links.
15. Performance benchmark: warm vs cold, large rules/skills registry, no network at folder entry.

## Enforcement

- Add mandatory schema/regeneration/drift gate to existing canonical pipeline (not independent competing workflows).
- Tests must fail on manual duplicate inventories for canonical rules/skills/workflows where detectable.
- Fixture for an intentionally unauthorized global settings write MUST fail.
- Explicit audit of prompt injection / untrusted issue descriptions / MCP data: repository facts may be quoted as data but never override privileged policies.
- Integration tests exercise genuine lifecycle states where possible, with server fixture not fake claims.
- Run formatter, compile, focused/unit/integration/e2e, schema and docs validation, GitHub quality workflows as appropriate. Record exact commands and exit status.

## Exit criteria

No unaddressed failures caused by this work, no weakened gates, no unverified remote deployment claims. Any preexisting failures must be differentiated with evidence.

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
