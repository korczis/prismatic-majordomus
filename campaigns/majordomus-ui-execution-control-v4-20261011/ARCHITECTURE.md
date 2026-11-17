# Typed architecture and action model

## Source of truth

Canonical project entities (intent, milestone, issue, task) → readiness/intent realization and dependency graph → capability registry + executor → execution ledger and event channel → derived SSR views / HTTP / MCP / CLI. Live mesh peer state is an observed supplement, not a source of task ownership or authority. Evidence ledger decides satisfaction; GitHub state is reconciled with canonical records rather than silently preferred.

## Proposed contract, not assumed implementation

`WorkTargetRef { repo_id, kind, canonical_id, revision }`; `WorkLaunchRequest { target, mode, requested_capability, authority_ref, idempotency_key, expected_revision }`; `WorkRunView { parent_execution_id, target, child_execution_ids, plan_revision, terminal_state, blockers, evidence_refs, cursor, freshness }`. Exact names MUST be discovered/adjusted to existing types, not added alongside equivalents.

Every command/action must declare required roles/scopes, input schema, effects, capabilities, possible refusals, output schema, event types and evidence links. Group operations may fan out to child executions but remain traceable as ONE parent run, without rewriting historical child state.

States: `not_started`, `ready`, `queued`, `running`, `waiting`, `blocked`, `succeeded` (execution), `failed`, `cancelled`, `unknown`, `stale`; satisfaction is a **separate** criterion-level field derived from evidence. Avoid universal progress %. When denominator has a semantic meaning use `completed_required / total_required` and display blocked/excluded counts; otherwise show phase/step with no fake percentage.

Idempotency keys bind target+operation+revision+authority; stale revision rejects with actionable retry/refresh. Readiness and action availability come from backend, not frontend conditionals. Mutating actions must carry CSRF/session guard as applicable and write auditable execution records.

## Routing

Use existing Cockpit paths and navigation; prefer `/cockpit/intents/{id}`, `/cockpit/plan/milestones/{id}`, `/cockpit/plan/issues/{id}`, `/cockpit/executions/{id}` and current task equivalents. Add generic drill-down/contextual controls inside those views rather than duplicate `/ui/...` screens. `majordomus ui` must resolve to the same shared server and Cockpit. Route registry needs tests comparing dispatch, nav, topology, OpenAPI infrastructure and browser crawl.

## Components

`WorkActionBar`, `TargetBreadcrumb`, `ReadinessPanel`, `WorkGraph`, `RunProgress`, `ExecutionTimeline`, `WorkerStatus`, `BlockerInspector`, `EvidenceInspector`, `LiveLogViewer`, `ReplayBadge`, `LaunchPreview`, `ContextualDrawer`, `ActionResult`, `RunHistory`, `RunFilters`, `ReconnectStatus`, `PermissionExplanation`. Reuse existing Cockpit components where functionally equivalent; no JS-owned canonical state.

## Missing/partial/offline

Never show a green tick for a missing backend response. Distinguish no work, no permission, no measured data, insufficient history, stale state, disconnected, and actual failures. Render initial SSR evidence and URLs without JS. On disconnect, freeze last sample with stale label until resync; don't imply live activity.
