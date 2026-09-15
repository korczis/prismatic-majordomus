# 09 — Legacy Migration, Hardening and Final Proof

Do the uncomfortable cleanup. Do not leave the new architecture sitting beside the old one.

## Repository-wide legacy search

Search for and classify:

- old MCP startup scripts,
- manual `join` flows that should no longer be primary,
- duplicated port constants,
- stale `.envrc` logic,
- duplicate session directories/formats,
- obsolete WebSocket message models,
- frontend hardcoded service/capability lists,
- duplicated REST/MCP/CLI implementations,
- dead runtime pid/socket logic,
- old docs instructing manual startup,
- tests that bypass bootstrap,
- generated files that became authoritative accidentally.

Delete/migrate obsolete paths.

Compatibility shims may remain only where there is a documented user-facing compatibility reason and a planned/defined deprecation policy.

## Full validation

Use canonical repo commands to run:

- format/lint,
- unit tests,
- integration tests,
- E2E tests,
- schema validation,
- generated drift checks,
- docs build/link checks,
- API/OpenAPI validation,
- MCP tests,
- Cockpit tests,
- security/redaction tests,
- performance checks where present.

Inspect `git diff` and search for new duplication.

## Final proof sequence

Demonstrate, with exact commands/output summarized:

```text
control plane stopped
→ enter enabled repo
→ runtime self-starts
→ health ready
→ MCP/API/OpenAPI/Cockpit discovered
→ agent session auto-attaches
→ session context/handover loaded
→ second agent attaches
→ peers see one another
→ claim/handover visible
→ Cockpit reflects state
→ crash runtime
→ system recovers according to spec
→ all gates pass
```

## Final report

Produce:

### Root causes
What specifically made prior “automatic” integration incomplete.

### Canonical architecture
Which modules/types/registries now own each fact.

### Deleted legacy
What was removed and why.

### Runtime convergence
Desired/observed/reconcile behavior and scope.

### Collaboration protocol
Identity, presence, claims, handovers, reconnect/resync.

### Surfaces
CLI/API/OpenAPI/MCP/Cockpit/docs and how each derives from canonical logic.

### Provider adapters
What is automatic for each supported provider and what is genuinely impossible/limited.

### Enforcement
Rules/doctrines/gates/E2E that make regression fail.

### Evidence
Exact commands/tests and results.

### Remaining debt
Only real remaining external blockers or deliberately deferred work. No vague TODO fog.

The final sentence of the report must state whether the cold-start invariant in `SPEC.md` is PROVEN, PARTIALLY PROVEN, or NOT PROVEN, with one-line justification.
