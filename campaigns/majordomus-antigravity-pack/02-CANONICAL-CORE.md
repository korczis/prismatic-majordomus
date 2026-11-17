# PHASE 02: Canonical typed core and projection engine

Implement/reuse shared editor-neutral developer model and augment for agent-aware integrations. Distinguish facts, intent, workflow descriptors, language environment, schema associations, agent policy, skills, provider capability, MCP endpoint and user preferences. Do not equate a policy instruction with a command that the agent may execute.

## Conceptual model (adapt to actual codebase)

```rust
struct DeveloperEnvironment { project: ProjectIdentity, languages: Vec<Language>, toolchains: Vec<Toolchain>, workflows: Vec<Workflow>, schemas: Vec<SchemaAssociation>, diagnostics: Vec<Diagnostic> }
struct AgentEnvironment { policies: Vec<PolicyRef>, skills: Vec<SkillRef>, context_sources: Vec<ContextRef>, mcp: Vec<McpDescriptor>, task_context: Vec<TaskContextRef> }
struct AdapterCapability { id: CapabilityId, surface: Surface, supported: SupportState, source: Provenance, constraints: Vec<Constraint> }
struct ProjectionResult { schema_version: String, artifacts: Vec<ProjectArtifact>, diagnostics: Vec<Diagnostic>, provenance: Vec<Derivation> }
```

These are suggestions, not instructions to duplicate existing Rust schemas.

## Requirements

- Typed adapter trait with `probe`, `capabilities`, `render`, `validate`, `explain`; `apply` belongs to generic atomic sync engine, not adapter-specific scattered writes.
- Explicit support states: supported, unsupported, unknown, deprecated, requires_user_action. No fake booleans where compatibility is uncertain.
- Deterministic total ordering: domain grouping, normalized display name, stable canonical ID; source enumeration and thread order must never leak.
- Versioned schema and migration; no silently incompatible JSON changes.
- Minimal semantic delta from native editor defaults; avoid emitting empty/no-op files.
- Project -> multi-root -> worktree scopes distinct; stable path handling, relative path preference, collision resistance.
- Provenance fields: canonical source, derivation algorithm/version, confidence, visibility/security class, user override information and reasons for omitted unsupported fields.
- Conflict resolution: repository-owned generated entries vs user-owned entries; tracked generated manifest (or equivalent) with ownership/provenance fingerprint. Detect legacy files and avoid destructive takeover.
- Local-only, cheap deterministic discovery. No network calls or builds during folder open. Cache invalidation includes relevant repository manifests, .ai policies and generator/schema versions.
- Create conformance fixtures that apply to Zed (when implemented) and Antigravity, with required/optional capability differences.

## Architectural proof

Demonstrate that an added canonical skill/rule/schema enters the relevant projection without editing renderer inventories. Show two editor adapters consuming one core, or use an existing adapter as fixture if Zed is not implemented. Integrate errors via canonical diagnostics/doctor.

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
