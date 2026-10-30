# Majordomus Governance & Discovery

This repository is supervised by Majordomus. Normative AI context and rules live under `.ai/`.

## Core Protocol
1. Read [`.ai/README.md`](../../.ai/README.md) and follow its discovery protocol.
2. Effective project rules live in `.ai/repo/rules/` with their dependencies resolved.
3. Before working under any path, resolve context via `majordomus context resolve <path>` (or read the README chain from `.ai/` down to that directory).
4. Use Majordomus MCP tools (`majordomus_*`) served by `bin/majordomus-mcp`.
5. Announce intent and active paths with `majordomus_announce` and monitor peer workers with `majordomus_peers`.
6. Coordinate cross-machine work using the mesh (`majordomus mesh`).
7. Follow worktree topology (`<repo>-wt/<branch>`) with `majordomus worktree`.
