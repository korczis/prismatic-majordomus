# Majordomus Automatic Control Plane & Agent Cooperation Specification

Status: normative draft for repository implementation
Scope: `prismatic-majordomus` and Majordomus-enabled repositories

## 1. Purpose

Majordomus MUST make repository entry converge automatically from an arbitrary local runtime state to a usable, observable and self-consistent development control plane.

The system is not considered automatic merely because MCP, an HTTP server, Swagger, WebSocket code or Cockpit pages exist independently. Automatic means that a human or supported LLM agent entering an enabled repository does not need to remember or manually execute service-start, registration, discovery, session-load or UI-repair commands.

The canonical invariant is:

> Entering a Majordomus-enabled repository or starting a supported agent session MUST converge, idempotently and within bounded time, to a healthy local control plane with repository identity, session identity, service discovery, MCP/API/OpenAPI/Cockpit availability, agent registration, peer presence and collaboration state derived from canonical repository/runtime data.

## 2. Non-negotiable design principles

1. Single source of truth. Repository facts, service metadata, agent/session state and capability descriptions MUST originate from canonical typed models or registries.
2. Derived surfaces. CLI, JSON, REST API, OpenAPI/Swagger, MCP, Cockpit, documentation and completion MUST derive from the same canonical definitions where they represent the same domain facts.
3. No redundant manual registration. Adding a capability, service, command, agent provider or discoverable entity MUST NOT require editing multiple consumer-specific inventories.
4. Idempotent convergence. Running bootstrap/ensure repeatedly MUST be safe and should result in the same desired state.
5. Self-healing runtime. Missing, stale or crashed local services MUST be detected and repaired where safe.
6. Explicit desired vs actual state. Runtime orchestration MUST reconcile desired state against observed state instead of assuming startup succeeded.
7. Local-first. Repository entry MUST NOT depend on network access. Remote integrations may enrich state asynchronously after the local control plane is available.
8. Bounded entry latency. Shell entry and `direnv` MUST remain responsive. Heavy reconciliation may be delegated to an already installed Majordomus binary/process, but startup semantics must remain deterministic and observable.
9. Provider-neutral collaboration. Claude Code, Codex, Gemini and future providers SHOULD participate through adapters over a common session/peer protocol rather than provider-specific collaboration semantics.
10. Observable and explainable. Every auto-start, registration, discovery, recovery and failure MUST be inspectable through canonical diagnostics.
11. Enforced, not aspirational. Rules/doctrines MUST be backed by executable validation and E2E tests.
12. No secret leakage. Credentials/tokens MUST never be rendered in banner, Cockpit, logs, diagnostics, API payloads or MCP resources.

## 3. Canonical lifecycle

A Majordomus-enabled repository lifecycle is:

```text
repository / worktree entry
        ↓
minimal entry hook
        ↓
resolve canonical repository identity
        ↓
resolve worktree + branch identity
        ↓
runtime ensure / reconcile
        ↓
control-plane ready
        ↓
MCP + API + OpenAPI + WebSocket + Cockpit discoverable
        ↓
resolve/create session identity
        ↓
agent/provider adapter registers session + capabilities
        ↓
load session context + handover
        ↓
peer discovery / presence / claims
        ↓
collaboration ready
        ↓
Cockpit and CLI expose live canonical state
```

The entry hook MUST NOT contain repository business logic. It invokes canonical Majordomus behavior.

## 4. Required canonical model

Exact names are repository-dependent, but the architecture MUST contain canonical typed representations equivalent to:

```text
RepositoryIdentity
WorktreeIdentity
RuntimeDesiredState
RuntimeObservedState
ServiceDefinition
ServiceInstance
EndpointDescriptor
HealthState
SessionIdentity
AgentIdentity
ProviderAdapter
CapabilityDescriptor
PeerPresence
WorkClaim
HandoverDescriptor
Diagnostic
ControlPlaneSnapshot
```

A canonical `ControlPlaneSnapshot` or equivalent SHOULD expose a coherent view of repository + runtime + agent/session state.

Consumers MUST not reconstruct this state independently.

## 5. Repository identity

Repository identity MUST be stable across processes and deterministic for the same checkout/project, while distinguishing unrelated repositories with the same directory name.

It SHOULD be derived from canonical repository metadata such as VCS root/origin identity plus Majordomus project metadata where present.

Worktrees MUST map to the same project identity but distinct worktree identities.

Identity algorithms MUST be documented, schema-versioned and tested.

## 6. Entry/bootstrap contract

A canonical command/API equivalent to the following behavior MUST exist:

```text
majordomus runtime ensure
```

It MUST:

- discover repository root without requiring CWD assumptions beyond being inside the repository,
- resolve project/worktree identity,
- inspect existing control-plane instance(s),
- reject/repair stale PID/socket/port state,
- start required local services if absent,
- avoid duplicate server instances,
- verify readiness using canonical health checks,
- publish/discover endpoint metadata,
- leave an actionable diagnostic if convergence fails,
- be safe when invoked concurrently by multiple shells/agents,
- be safe when invoked repeatedly.

A shell/direnv hook SHOULD call a cheaper entry command such as `majordomus env enter` that internally performs or triggers the canonical ensure semantics without duplicating logic.

## 7. Process supervision and locking

Runtime startup MUST prevent thundering-herd duplicate launches.

Use a robust single-owner mechanism appropriate for the repository/platform, such as an advisory lock, Unix-domain socket ownership, pidfile with process identity validation, or existing supervisor abstraction.

PID existence alone MUST NOT be trusted.

Stale state MUST be recoverable automatically.

Crash recovery MUST be tested.

## 8. Endpoint discovery

Ports and paths MUST NOT be duplicated across `.envrc`, docs, CLI, Cockpit frontend and tests.

A canonical service/endpoint registry MUST define/discover:

- HTTP API base,
- OpenAPI document,
- Swagger UI if present,
- MCP transport endpoint(s),
- WebSocket/event endpoint,
- Cockpit base URL,
- health/readiness endpoints,
- optional docs endpoints.

Consumers SHOULD obtain these through a canonical discovery command/resource/API.

Example conceptual structured output:

```json
{
  "schema_version": "...",
  "project_id": "...",
  "services": [
    {
      "id": "control-plane",
      "state": "ready",
      "endpoints": [
        {"kind": "api", "url": "..."},
        {"kind": "mcp", "url": "..."},
        {"kind": "websocket", "url": "..."},
        {"kind": "cockpit", "url": "..."}
      ]
    }
  ]
}
```

## 9. Health model

Service state MUST distinguish at least:

```text
unknown
starting
ready
degraded
unhealthy
stopped
stale
```

Readiness means the service can satisfy its contract, not merely that a TCP port is open.

Health checks SHOULD be cheap, local and deterministic.

## 10. Agent/session bootstrap

Starting a supported agent in an enabled repository MUST converge to an attached Majordomus session.

The adapter MUST resolve or create:

- project identity,
- worktree identity,
- session identity,
- provider identity,
- agent instance identity,
- declared capabilities,
- current branch/work item context where derivable.

It MUST register with the control plane and renew presence while alive.

Explicit manual `join` MAY exist as a diagnostic/escape hatch, but normal use MUST NOT require it.

## 11. Provider adapter boundary

Provider adapters MUST be thin.

Provider-specific hooks may differ, but they MUST map into common canonical operations such as:

```text
session.attach
session.detach
presence.heartbeat
capabilities.publish
context.load
context.checkpoint
handover.publish
work.claim
work.release
peer.list
message/event publish
```

No provider adapter may invent a second session model.

## 12. Session context and handover

Session context and handovers MUST be discoverable and loaded through one canonical mechanism.

The system SHOULD infer relevant context from repository/worktree/session state and MUST make the result inspectable.

Storage location and schema MUST follow existing `.ai/` / `.majordomus/` conventions discovered in the repository.

Conflicting legacy session directories/formats MUST be migrated or normalized rather than perpetuated.

## 13. Collaboration protocol

A WebSocket alone is not collaboration.

The protocol MUST define at least:

- peer identity,
- presence and heartbeat,
- connect/reconnect semantics,
- capability advertisement,
- event envelope + schema version,
- work claims/leases or equivalent ownership semantics,
- claim expiration/recovery,
- handover publication,
- conflict/overlap visibility,
- last-known state / resynchronization,
- graceful disconnect,
- stale peer cleanup.

The protocol SHOULD support repository-scoped and worktree-scoped topics/channels.

Ordering/replay semantics MUST be explicit where events affect correctness.

## 14. Work coordination

Majordomus SHOULD reduce duplicate agent work.

At minimum, peers MUST be able to discover active work ownership/claims for a repository/worktree/work item.

Claims MUST include enough metadata to answer:

- who/what agent owns it,
- provider/session,
- repository/worktree/branch,
- subject/work item,
- claim creation/renewal/expiry,
- status,
- optional handover pointer.

Claims MUST not become permanent locks after crashes.

## 15. Cockpit requirements

Cockpit MUST render live state derived from the canonical control-plane model.

At minimum it SHOULD expose:

- control-plane health,
- services/endpoints,
- active sessions,
- connected agents/providers,
- peer presence,
- active claims/work ownership,
- worktrees/branches as available canonically,
- handovers/session context references,
- diagnostics,
- MCP capabilities/resources/tools where appropriate.

Cockpit MUST NOT maintain hardcoded duplicate inventories of services/capabilities.

A freshly loaded Cockpit after cold bootstrap MUST reflect current state without manual refresh/re-registration rituals beyond normal UI behavior.

## 16. MCP requirements

MCP exposure MUST derive from canonical capability definitions.

The implementation MUST audit whether current MCP transport/server lifecycle is actually started and discoverable automatically.

MCP SHOULD expose resources/tools sufficient to inspect:

- repository/control-plane snapshot,
- current session,
- peers/presence,
- claims,
- handovers/context,
- diagnostics,
- service/endpoints.

Mutation tools MUST use the same domain services as CLI/API rather than parallel implementations.

## 17. REST/OpenAPI requirements

REST operations MUST reuse canonical domain services.

OpenAPI MUST be generated/derived from the route/schema source already used by the implementation.

Swagger UI is a consumer, not another registry.

Where an operation exists across CLI, API and MCP, behavior and validation MUST converge on the same core implementation.

## 18. CLI requirements

Canonical CLI capabilities SHOULD include equivalents of:

```text
majordomus runtime status
majordomus runtime ensure
majordomus runtime stop
majordomus runtime doctor
majordomus runtime explain
majordomus session status
majordomus peers
majordomus claims
majordomus env status --json
```

Actual names MUST follow existing repository conventions and avoid duplicate commands.

Structured output MUST be schema-backed and deterministic.

## 19. `.envrc`, AGENTS and agent hooks

`.envrc` MUST stay thin and must not own domain logic.

`AGENTS.md` or provider-specific bootstrap instructions SHOULD only describe/invoke canonical Majordomus entry behavior.

No copied port numbers, command registries or startup logic should appear in multiple hook files.

Generated/managed hook fragments are acceptable where derived from canonical definitions and drift-checked.

## 20. Desired-state reconciliation

The runtime MUST have an explicit reconciliation loop/function:

```text
read desired state
observe actual state
compute delta
apply safe actions
verify
publish diagnostics/snapshot
```

Examples of repairable drift:

- server absent,
- stale pid/socket,
- changed endpoint allocation,
- missing session registration,
- expired presence,
- stale claim,
- changed capability registry,
- worktree/session identity change.

The reconciliation algorithm MUST be idempotent and concurrency-safe.

## 21. Event architecture

Where Cockpit, agents and runtime need live updates, events MUST come from one typed event model.

Avoid separate ad-hoc WebSocket messages for each consumer.

Events SHOULD contain:

```text
schema_version
event_id
timestamp
project_id
worktree_id (optional)
session_id (optional)
agent_id (optional)
kind
payload
```

If replay/resume is supported, define cursor/sequence semantics explicitly.

## 22. Failure semantics

Automatic bootstrap MUST fail visibly but gracefully.

A broken optional integration MUST not unnecessarily prevent local repository entry.

Failures MUST classify:

- fatal bootstrap invariant,
- degraded optional feature,
- transient starting state,
- configuration error,
- credential/provider error,
- stale runtime state,
- schema/protocol mismatch.

Diagnostics MUST tell the operator what failed and where the canonical source lives.

## 23. Security

Mandatory:

- bind local control-plane safely by default,
- do not expose unauthenticated control endpoints remotely by accident,
- redact credentials,
- never serialize secrets into session context or diagnostic snapshots,
- validate client/session identity appropriate to the local threat model,
- document trust boundaries,
- prevent arbitrary command execution through generic MCP/API passthrough unless explicitly designed and constrained,
- test credential redaction.

## 24. Performance budgets

Repository entry MUST remain fast.

Suggested targets unless repository evidence dictates better values:

```text
entry hook warm path: target < 100 ms
local status snapshot warm: target < 100 ms
runtime ensure when already healthy: target < 150 ms
network access during entry: forbidden
build during entry: forbidden
```

Cold startup may exceed these values but MUST not block shell usability unnecessarily if the architecture supports safe asynchronous local startup. Readiness still must be observable before agent attachment relies on it.

## 25. Determinism

The same unchanged repository/runtime state MUST produce deterministic structured snapshots.

Do not depend on:

- filesystem enumeration order,
- HashMap iteration,
- concurrent task completion order,
- random port choice without canonical discovery publication,
- process-specific ephemeral ordering.

## 26. Schema/versioning

All durable/external structured contracts MUST have explicit schemas and versioning according to repository conventions:

- session records,
- handovers,
- events,
- control-plane snapshots,
- capability descriptors,
- claims,
- discovery metadata.

Schema definitions SHOULD derive from canonical types where repository tooling supports it.

## 27. Enforcement doctrine

The repository MUST encode an enforceable invariant equivalent to:

> Majordomus-enabled repositories MUST automatically converge to their declared local control-plane desired state on canonical entry. Runtime services, endpoints, capabilities, sessions, peer presence and collaboration state MUST be derived from canonical typed definitions, must not require redundant consumer-specific registration, and must be verifiable through executable health, schema and E2E gates.

Additional invariant:

> A feature is not considered integrated merely because one surface exposes it. Where the canonical capability matrix declares CLI/API/OpenAPI/MCP/Cockpit/docs exposure, drift or missing exposure MUST fail validation.

## 28. Capability matrix

A generated capability matrix SHOULD be derivable from canonical metadata:

| Capability | Domain impl | CLI | REST | OpenAPI | MCP | Cockpit | Docs | E2E |
|---|---|---|---|---|---|---|---|---|
| runtime status | required | derived | derived | derived | derived | derived | derived | required |
| runtime ensure | required | derived | derived if appropriate | derived | derived if safe | action/status | derived | required |
| sessions | required | derived | derived | derived | derived | derived | derived | required |
| peers | required | derived | derived | derived | derived | derived | derived | required |
| claims | required | derived | derived | derived | derived | derived | derived | required |
| diagnostics | required | derived | derived | derived | derived | derived | derived | required |

The exact matrix MUST be inferred from existing architecture and must not force nonsensical surfaces.

## 29. E2E acceptance scenarios

### Scenario A: cold/dead state

Preconditions:

- control-plane stopped,
- no live agent sessions,
- stale runtime files may exist,
- repository enabled.

Action:

- enter repo/start canonical agent bootstrap.

Expected:

- one control-plane instance starts,
- health becomes ready,
- endpoints are discoverable,
- session registers automatically,
- context/handover load runs,
- Cockpit reflects session,
- MCP is reachable and exposes canonical capabilities,
- no manual startup/join command required.

### Scenario B: already healthy

Multiple new shells/agents enter concurrently.

Expected:

- no duplicate control-plane processes,
- all sessions register,
- peers see each other,
- ensure is fast/idempotent.

### Scenario C: runtime crash

Kill control-plane ungracefully.

Expected:

- stale state detected,
- next reconciliation recovers,
- clients reconnect/resync as designed,
- stale claims eventually expire or recover.

### Scenario D: worktree isolation

Open two worktrees of same project.

Expected:

- same project identity,
- distinct worktree/session identity,
- peers can distinguish scope,
- Cockpit groups correctly,
- claims do not accidentally collide unless intentionally project-global.

### Scenario E: two providers

Start two supported providers in same project.

Expected:

- common collaboration model,
- both publish provider-specific capabilities through adapters,
- peer presence visible,
- no provider-specific duplicate session system.

### Scenario F: schema/capability change

Add a representative canonical capability.

Expected:

- relevant CLI/API/OpenAPI/MCP/Cockpit/docs projections update through derivation/generation,
- no repeated manual registration.

## 30. Required test classes

- unit tests for identity/reconciliation/protocol logic,
- property tests for idempotence where useful,
- concurrency tests for ensure/register,
- stale process/socket recovery tests,
- protocol reconnect/resync tests,
- schema tests,
- generated-artifact drift tests,
- cross-surface contract tests,
- Cockpit integration tests,
- MCP integration tests,
- true cold-start E2E test,
- multi-agent E2E test,
- worktree isolation E2E test,
- security/redaction tests,
- performance regression checks where infrastructure supports it.

## 31. Definition of Done

The implementation is complete only when:

- [ ] cold repository entry converges automatically,
- [ ] runtime ensure is idempotent and concurrency-safe,
- [ ] stale runtime state self-recovers,
- [ ] endpoint discovery is canonical,
- [ ] MCP starts/discovers automatically,
- [ ] supported agent sessions auto-attach,
- [ ] peer presence works,
- [ ] collaboration claims/handovers have defined semantics,
- [ ] session context is automatically loaded through canonical machinery,
- [ ] Cockpit shows live canonical runtime/session/peer state,
- [ ] CLI/API/OpenAPI/MCP/Cockpit share domain implementations,
- [ ] no redundant manual inventories remain in scope,
- [ ] rules/doctrines and executable gates enforce the architecture,
- [ ] docs explain operation, extension and recovery,
- [ ] E2E tests prove dead-state bootstrap without manual commands,
- [ ] adding a representative capability demonstrates zero/near-zero consumer registration,
- [ ] repository-level canonical checks pass.

## 32. Explicit anti-goals

Do not solve this with:

- a giant `.envrc`,
- `nohup` shell spaghetti,
- fixed ports copied into five files,
- one-off Claude startup scripts,
- a provider-specific peer protocol,
- Cockpit-only fake status,
- frontend hardcoded service lists,
- MCP tools implemented separately from REST/CLI domain services,
- PID-only health checks,
- silent recovery failures,
- unbounded network calls on `cd`,
- generated source churn on every directory entry,
- a rule Markdown file with no executable enforcement,
- tests that start the server before testing “automatic startup”.

## 33. Architectural proof

The final implementation must be able to demonstrate this sequence from a deliberately dead state:

```text
kill/remove live runtime
        ↓
enter enabled repo
        ↓
no manual Majordomus command
        ↓
one healthy control-plane
        ↓
agent attached
        ↓
MCP/API/OpenAPI/WS/Cockpit discoverable
        ↓
peer/session visible
        ↓
context/handover loaded
        ↓
E2E PASS
```

Anything less is partial integration, not automatic cooperation.
