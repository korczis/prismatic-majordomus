# Majordomus UI Execution Control Plane V4

**Purpose:** extend the existing Cockpit and V3 visual implementation pack with real user-controlled autonomous work across tasks, issues, milestones and intents, live over the existing execution WebSocket transport.

**Upstream visual scope:** `majordomus-ui-complete-opus-fable-v3-20261011.zip` and all user-uploaded screenshots in the Claude Code session. Treat images only as visual references, never as sources of operational data.

**Repo:** `korczis/prismatic-majordomus`. Reinspect current HEAD and open PRs before editing. In observed source `docs/COCKPIT.md`, execution pages already use `executions.list`, `executions.get`, `executions.events`, `executions.start`; issue transitions use `plan.transition`; the live channel is WebSocket `GET /events`. Existing intent/milestone/issue detail pages and registry-driven Cockpit already exist. Verify all of this at runtime.

**Do not build:** a new SPA, standalone UI backend, second planner/scheduler/ledger, duplicate event stream or fake data. Use the canonical capability registry, web topology, project model, existing mesh, evidence and release pipeline.

**Package:** `MASTER.md`, `ARCHITECTURE.md`, `REALTIME.md`, `ACCEPTANCE.md`, `TRACKING.md`, `manifest.json` and `milestones/M00..M11.md`.

**Execution:** Paste `MASTER.md` into Claude Code Opus/Fable after attaching this archive and the screenshots. Work in evidence-backed vertical slices; update rules/docs/tooling and ship validated increments without routine conversational prompts. Some capabilities may not exist; implement or report accurately, never synthesize success.
