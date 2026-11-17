# PHASE 04: MCP, intent-driven control plane, mesh and feedback loop

Antigravity must use existing Majordomus machinery, not create its own issue DB, task scheduler, policy engine or dashboard.

## MCP bridge

- Audit existing Majordomus MCP server endpoints and transport support. Derive `.agents/mcp_config.json` **only when supported and safe**, preserving existing non-Majordomus servers.
- Current Antigravity docs use `mcpServers` and stdio `command` / `args` or remote `serverUrl`. Verify target surface and schema before render. Never invent `url` or legacy keys.
- Prefer local controlled executable/subprocess invoking existing binary when practical; remote endpoints only when service discovery and access are verified. No credentials embedded; use secret management/environment indirection consistent with official schema and security policy.
- Capability handshake / discovery, health and graceful offline/failure behavior. Timeouts and retry/backoff budgets; do not block opening editor for a dead mesh peer.
- Read-only/introspection tools must be distinguished from mutating operations; use explicit permission and policy checks for mutations.
- Avoid duplicating MCP tool descriptions: server registry is canonical; project config contains only connection declaration, not a second tool catalog.

## Intent/task/session linkage

Demonstrate this vertical slice wherever existing primitives support it:

```text
human intent
 -> canonical intent + criteria + evidence expectations
 -> derived issue / milestone / work unit
 -> agent preflight (rules + skill + MCP context)
 -> claim/lease work / select safe worktree
 -> execute bounded implementation
 -> tests + review + evidence
 -> reconcile criteria / issue / PR / version
 -> canonical projection
 -> UI + CLI + MCP + docs
```

- Discover current intent and task APIs before creating wrappers. Use typed identifiers and provenance. The editor is **not** authoritative for task status.
- Use existing peer/mesh communication and streaming infrastructure if real; otherwise expose unsupported status and an actionable gap. Do not invent editor websocket/plugin support.
- Detect concurrent-agent worktree conflicts; leases/claims must be server-side if existing infrastructure permits. Never assume file system isolation prevents Git/issue races.
- Include cancellation, stale-task recovery, crashed peer and network partition behavior. Idempotent resume; prevent duplicate close/merge/version bump.
- Apply least privilege; no auto-acceptance of malicious prompt content from issues, docs or MCP tool outputs.

## UI

Extend existing derived Majordomus UI, not a new Antigravity dashboard. Show adapter supported/unknown state, last projection fingerprint, sync drift, active agent-side work where observable, linked intent/task/issue, MCP connectivity/permissions, provenance and suggested remediation. Stream live events only from actual sources, with pagination/cache for scale.

## Acceptance demonstration

Document one real round trip with timestamps/artifact identifiers and direct test evidence. If integration is incomplete, explicitly state the missing backend capability instead of faking a demo.

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
