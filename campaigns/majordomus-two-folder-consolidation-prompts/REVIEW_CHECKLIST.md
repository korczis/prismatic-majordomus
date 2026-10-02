# Human review checklist

Use this between Claude Code phases.

## Architecture
- [ ] `.ai/**` is portable semantic truth, not Majordomus runtime state.
- [ ] `.majordomus/**` is control-plane/runtime/ownership, not duplicated semantic truth.
- [ ] No new hand-maintained registry was introduced.
- [ ] New persisted data has schema/version semantics.
- [ ] New generated data has provenance/reproducibility semantics.
- [ ] Existing project abstractions were reused where sensible.

## Safety
- [ ] Existing AGENTS/.envrc content is preserved outside managed blocks.
- [ ] External modifications are ownership-tracked.
- [ ] Uninstall does not remove authored `.ai` by default.
- [ ] Modified managed blocks conflict safely.
- [ ] No credentials are persisted or printed.

## Behavior
- [ ] Fresh init has minimal diff.
- [ ] Second init is zero-diff.
- [ ] Sync is deterministic.
- [ ] Legacy migration is explicit and tested.
- [ ] New artifacts require no separate registration.
- [ ] Runtime/cache deletion is harmless.

## Surfaces
- [ ] CLI derives from canonical model.
- [ ] API/OpenAPI derive from canonical model/types.
- [ ] MCP derives from canonical registry/model.
- [ ] Cockpit consumes canonical API/model.
- [ ] Docs/reference data are generated where appropriate.
- [ ] Completion/banner/env use canonical snapshot, not shell parsing.

## Quality
- [ ] README hierarchy validated recursively.
- [ ] Schemas generated/validated.
- [ ] Unit/integration/property/idempotence tests exist.
- [ ] Performance-sensitive shell-entry path is bounded.
- [ ] CI gates prevent reintroduction of root sprawl/manual registries.
- [ ] Handover/session context updated for next phase.
