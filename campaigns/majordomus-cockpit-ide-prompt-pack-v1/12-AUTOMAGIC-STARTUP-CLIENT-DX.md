# Prompt 12 — Automagic Startup, Repo Entry, Client Attachment, Zero-Friction DX

## Mission

Entering or opening the repository should automatically make the Majordomus control plane available without turning `.envrc` into a shell-program landfill.

## Preserve the repo-entry principle

`.envrc` is a trigger/adapter only.

Canonical runtime/environment facts belong to typed Majordomus state and are projected into:
- CLI banner/status;
- API;
- OpenAPI/Swagger;
- MCP;
- Cockpit.

## Verify and harden startup paths

Audit:
- `.envrc`;
- `bin/majordomus-mcp`;
- `.mcp.json`;
- `.gemini/settings.json`;
- `.codex/config.toml`;
- any Claude settings/hooks;
- server lease/reuse;
- binary bootstrap/build;
- port selection;
- stale server replacement;
- version/fingerprint checks.

## Desired behavior

On repository entry / supported client startup:

1. repository root is discovered;
2. environment snapshot is resolved cheaply;
3. shared server is found or started;
4. outdated/stale server is repaired according to canonical policy;
5. MCP client connects;
6. peer registers;
7. context bootstrap is available;
8. Cockpit URL is discoverable;
9. Swagger URL is discoverable;
10. status banner is shown according to configured verbosity;
11. no remote network access blocks directory entry;
12. no unnecessary rebuild occurs.

## Performance budgets

Respect current measured budgets. If none exist, establish realistic repository-specific targets and benchmarks.

At minimum:
- warm repo entry must feel instantaneous;
- server reuse must avoid rebuild;
- read-only context/status should use cached immutable startup state;
- no registry/index/OpenAPI rebuild per Cockpit request.

## Generated client config

Client configuration should be generated from canonical policy/template data if the repo architecture supports generation.

Changes to shared bootstrap semantics must update:
- canonical `.ai` policy;
- generated `AGENTS.md`;
- generated `CLAUDE.md`;
- MCP/client configs;
- docs;
- tests.

## Doctor

`majordomus doctor` or canonical equivalent should diagnose:
- missing hooks;
- missing generated config;
- stale client config;
- server state;
- version mismatch;
- missing executable;
- bad worktree;
- unpushed branch;
- docs/generated drift;
- peer connectivity.

Cockpit should show the same diagnoses, not reimplement them.

## Acceptance

Test fresh checkout, warm checkout, stale server, changed binary, missing generated config, multiple clients, disconnected/reconnected client, and no-network conditions.
