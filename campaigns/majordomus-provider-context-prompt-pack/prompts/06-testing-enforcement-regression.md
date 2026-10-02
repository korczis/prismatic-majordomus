# Prompt 06: Regression-Proof Testing, Enforcement and Evidence

Use MAXIMUM AVAILABLE CONTEXT. Treat this as hardening the architecture so it cannot quietly decay in three sessions, which software is extremely talented at doing.

Audit all new/modified provider/session/handover/context code and close testing/enforcement gaps.

Add/reuse executable rules/doctrines/policies/gates for at least these invariants:

1. Stateful repository AI work requires a valid Majordomus session.
2. Applicable handover + governance + compiled context must resolve before provider invocation.
3. Provider-native conversation state is never canonical project memory.
4. Provider/model inventories come from one canonical registry.
5. CLI/API/MCP/Cockpit/workflows cannot directly bypass canonical execution orchestration.
6. Routing is deterministic and explainable.
7. Fallback respects governance/locality/capability/pins.
8. Session context consumed for work must be fresh.
9. Incomplete transferred/suspended work must have usable checkpoint/handover state.
10. Secrets never appear in safe serialized/output state.
11. New provider extensions require no consumer-specific registration.
12. Docs claims must not outrun tested implementation where claim verification exists.

Each rule must have positive, negative and regression tests where practical.

Add architecture/call-site tests or validators to detect direct adapter invocations and duplicate inventories.

Mandatory regression scenarios:

- provider A -> provider B continuity
- provider fallback continuity
- peer A -> peer B handover
- process crash/restart
- concurrent sessions/worktrees isolation
- stale HEAD/context refresh
- broken handover predecessor
- duplicate/ambiguous session resolution
- model disappearance
- credentials appear/disappear
- offline/local-only routing
- explicit scope inheritance/removal
- fake secrets redaction across Display/Debug safe forms/JSON/API/MCP/Cockpit/log/session context
- zero-registration fake provider extension
- registry/discovery order randomization

Run and enforce the repository's strongest coverage threshold. Cover meaningful branches, not line-touching theater.

If mutation/property/fuzz infrastructure exists, apply it to precedence, routing, config parsing, lineage and redaction.

Create/update an evidence matrix linking capability -> canonical implementation -> enforcement -> tests -> exposed surfaces, preferably generated from canonical metadata if existing architecture permits.

Integrate all checks into canonical local gates and CI without creating redundant pipelines.
