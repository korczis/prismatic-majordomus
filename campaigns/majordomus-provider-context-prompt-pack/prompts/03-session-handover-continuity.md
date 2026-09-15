# Prompt 03: Automatic Session Context + Handover Continuity

Use MAXIMUM AVAILABLE CONTEXT. This prompt is a hard architectural requirement, not an optional integration.

Majordomus must own continuity. Providers are disposable execution engines.

Implement/fix the canonical execution lifecycle so normal stateful repository work follows:

```
repo/task
 -> session resolver
 -> current/predecessor session
 -> handover resolver
 -> governance preflight
 -> context discovery
 -> context compiler
 -> task requirements
 -> provider/model router
 -> invocation envelope
 -> execution events/provenance
 -> session update
 -> handover/checkpoint update
 -> validation
```

Requirements:

## Automatic session bootstrap

Every supported stateful execution entrypoint must automatically resolve or create the correct Majordomus session. This includes applicable CLI, Cockpit, MCP, workflow, skill, provider CLI, peer/mesh and automation paths.

No human must have to remember to load the session manually.

## Automatic handover discovery

Resolve relevant predecessor/handover using typed canonical metadata, including task/issue/milestone/worktree/branch/session lineage as supported by the project.

Do not rely only on filename chronology.

## Session schema

Reuse/extend canonical typed, versioned session schema. It should represent/reference enough state for continuity, including repository/worktree/branch/HEAD/task/profile/workflow/provider provenance/handover lineage/progress/validation/deployment state where project conventions support them.

Derived facts should be derived, not duplicated manually.

## Handover schema

Reuse/extend canonical typed handover structure for objective/current state/decisions/files/tests/failures/remaining work/risks/provider history/commits/deployment/verification.

Structured data is primary; narrative may complement it.

## Context hydration

Before provider routing, automatically hydrate relevant context from sessions, handovers, rules, doctrines, policies, ADRs, knowledge, issues, milestones, Git, source/tests, profiles/workflows/skills and prior decisions.

Use canonical context discovery/compiler. Do not create surface-specific prompt assemblers.

## Context freshness

Before invocation, verify context against current HEAD, working tree, issue/milestone state, governance and provider policy. Recompute stale derived context automatically.

## Provider switching

Switching Codex <-> Claude <-> Gemini <-> Ollama must preserve canonical session/context continuity. Provider-native conversation IDs are optional optimization metadata only.

## Peer handover

Transferring work peer A -> peer B must automatically checkpoint and hydrate the receiving peer from canonical handover/context.

## Recovery

Persist enough state so crash/restart/resume works without hidden in-memory dependencies.

## Invocation envelope

Create/reuse a typed canonical invocation envelope carrying validated session/task/context/governance/provider-selection/provenance data. Consumers must invoke provider runtime through this path.

## Architecture enforcement

Prevent direct adapter calls from CLI/Cockpit/MCP/workflows that bypass canonical session/context orchestration, using module visibility, architecture tests, types, validators or equivalent repository conventions.

Add hard regression tests for:

- first task auto-creates/resolves session
- existing session auto-resumes
- relevant handover auto-loads
- provider switch preserves context
- provider fallback preserves context
- peer transfer preserves context
- crash/restart recovery
- concurrent worktree/session isolation
- stale context refresh
- broken lineage detection
- token-budget/context-priority preservation
- direct provider bypass rejection

Update all relevant docs and session/issue/milestone/handover state.
