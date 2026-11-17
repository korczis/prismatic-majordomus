# 06 — Canonical projections: CLI / UI / MCP / API

Reuse existing Majordomus typed entity graph, event transport, OpenAPI schemas, MCP tools and UI/cockpit projections. Never create a second manual queue or front-end truth. Derive one operational snapshot and event stream with schema version, initial/current PR counts, ready/blocked/integrating queue, dependency paths, worker/lease ownership, gate/evidence state, completion events, version obligations, releases/deploys, current target revision, active error and retry status.

UI should render navigable PR/issue/milestone/intent/peer/release relationships, with current live stream and reconnection/resume support. Use existing design/Flowbite/Alpine/graph/chart facilities if present, don't rewrite the application. Support drill-down to evidence, blockers and operation history. Add only legitimate actions supported by canonical capability/effect/authz model. Mobile friendly, paginated/streamed, bounded and quick; status surfaces remain read-only by default, no secrets or privilege elevation.

Document operation procedures and restart logic using existing derived docs. Add enforceable `.ai` invariants only to established rule and gate mechanisms. Confirm cross-surface consistency with fixtures or contracts rather than screenshot-only assertions.
