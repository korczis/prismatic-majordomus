# PHASE 05: Safe rendering, diffing, apply and CLI ergonomics

Build the Antigravity UX on top of existing `majordomus editor` infrastructure. The commands below are intended semantics; preserve actual CLI naming/conventions if different:

```text
majordomus editor list
majordomus editor antigravity status [--json]
majordomus editor antigravity render [--output DIR]
majordomus editor antigravity diff [--json]
majordomus editor antigravity sync [--dry-run]
majordomus editor antigravity check
majordomus editor antigravity explain <key-or-artifact>
majordomus editor doctor
```

## Artifact ownership contract

- Define generated artifact ownership at file/field/section level and safe update mechanics. If a JSON file mixes managed and unmanaged fields, merge only managed parts and reject ambiguous ownership collisions.
- For Markdown `AGENTS.md` and skills, never overwrite user-authored content without proven ownership. If a marker-based managed region is appropriate, implement strict parser, fingerprint, source control diff, conflict diagnostics and safe rollback.
- Render is pure. Diff is read-only. Check exits nonzero on actionable drift. Sync writes atomically only after validating proposed output and user-owned conflicts.
- Canonical semantic JSON normalization independent of incidental key/insertion order, stable UTF-8/line endings; avoid reformatting unrelated user content.
- Locking for concurrent processes, temporary files and atomic rename on same filesystem. Avoid committing machine-specific paths; honor symlink policies.
- `--dry-run` summarizes write/change/delete/conflict and no-effect operations; no silent deletion of obsolete hand-owned files.
- Include source/why/override on `explain`. No secret leakage in debug mode.
- New users get small, usable generated project config. Existing users get clean migration with reviewed diffs and no surprise preference changes.

## Cache and performance

Target cheap warm projections after cached discovery. Measure on representative repo; no network, build, agent startup or npm/cargo invocation on folder entry. Cache invalidation must include repo manifests, rule/skill registries, schemas, adapter version and environment scope. CLI may provide explicit refresh command, but normal check must catch drift.

## UX

Explain errors with `path`, canonical source, expected, actual, remediation and whether manual UI action is required. Structured output versioned and consumed by API/MCP/UI rather than separately invented.

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
