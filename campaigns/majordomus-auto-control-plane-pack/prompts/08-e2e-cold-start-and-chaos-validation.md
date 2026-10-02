# 08 — E2E Cold Start and Chaos Validation

This stage decides whether the project is actually automatic.

## Build a true E2E harness

The harness MUST start from dead/stale state. It MUST NOT secretly pre-start the service it is testing.

### Test A — dead cold start

1. stop/kill Majordomus control-plane safely,
2. remove or create controlled stale runtime metadata as test fixture,
3. enter/bootstrap enabled repository through the real canonical entry path,
4. assert exactly one control-plane becomes ready,
5. assert endpoint discovery,
6. assert MCP readiness,
7. assert API/OpenAPI availability,
8. assert Cockpit backend state,
9. assert auto-created/attached session,
10. assert context/handover load path executed,
11. assert peer/presence visibility.

No manual `serve`, `mcp start` or `join` between steps.

### Test B — concurrency storm

Start multiple entry/agent attach processes concurrently.

Assert:

- one intended runtime instance,
- no corrupted registry,
- all valid sessions attach,
- deterministic final snapshot.

### Test C — crash recovery

Hard-kill runtime while sessions exist.

Assert defined reconnect/reconcile behavior and stale claim cleanup.

### Test D — worktrees

Use two legal worktrees under repository doctrine.

Assert project grouping + worktree isolation.

### Test E — mixed providers

Exercise two provider adapters through test harness/real integrations available locally.

Assert common peer state and no duplicate session models.

### Test F — drift injection

Intentionally create a duplicate/hardcoded surface registration or stale generated artifact in fixture form and prove repository validation catches it.

## Performance

Measure healthy-path entry/ensure using existing benchmark conventions. Detect egregious regressions.

## Security

Run redaction tests and verify bound interfaces/auth defaults according to local threat model.

## Evidence artifact

Persist machine-readable E2E results according to repository test/report conventions so CI and Cockpit/docs can reference them if appropriate.
