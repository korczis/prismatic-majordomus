# Majordomus Automatic Control Plane Prompt Pack

This pack turns the recurring “MCP/Cockpit/agents should start and cooperate automatically” requirement into a normative spec plus staged implementation prompts.

## Recommended execution order

1. `SPEC.md` — keep open as normative contract.
2. `prompts/00-master-orchestrator.md` — give this to Claude Code Fable/Opus first.
3. Run `01` through `09` in order unless the master prompt and repository evidence justify merging stages.
4. Do not advance past a stage with known failures caused by that stage.
5. Require real commits/checkpoints according to repository doctrine, but do not force artificial commit boundaries if current repository rules define another workflow.

## Why staged prompts

The problem crosses repository bootstrap, runtime supervision, typed schemas, MCP, HTTP, WebSocket, session context, provider adapters, Cockpit, rules/doctrines and E2E. A single prompt invites partial success to masquerade as completion. Staging creates explicit evidence gates.

## Pack contents

- `01-repository-audit-and-gap-map.md`
- `02-canonical-control-plane-model.md`
- `03-runtime-bootstrap-and-reconciliation.md`
- `04-session-peer-collaboration-protocol.md`
- `05-mcp-api-openapi-cockpit-integration.md`
- `06-agent-provider-auto-attach.md`
- `07-rules-doctrines-docs-and-generated-surfaces.md`
- `08-e2e-cold-start-and-chaos-validation.md`
- `09-legacy-migration-hardening-and-final-proof.md`

## Global execution rule

Every prompt inherits `SPEC.md`. Repository evidence overrides guessed file/module names. Existing abstractions must be reused or extended rather than shadowed by parallel systems.
