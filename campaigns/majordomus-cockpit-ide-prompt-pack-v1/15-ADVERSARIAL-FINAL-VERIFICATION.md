# Prompt 15 — Adversarial Final Verification and Proof

You are now the skeptical reviewer, not the implementer.

Assume the previous phases overclaimed success.

Your job is to falsify the statement:

> "Majordomus Cockpit is now a real, automatically available development IDE/control plane and every interface is a derived projection of canonical typed state."

## 1. Fresh-start test

From a clean/fresh checkout or realistic disposable clone:
- bootstrap dependencies using documented commands;
- enter repository;
- verify automatic environment integration;
- start Claude/Codex/Gemini-compatible client path where feasible;
- verify shared server;
- verify peer registration;
- open Cockpit;
- verify Swagger;
- verify MCP.

Record friction/manual undocumented steps. Any hidden tribal knowledge is a failure.

## 2. Zero-registration test

Add a disposable representative:
- capability;
- workflow;
- rule/object.

After canonical generation/startup, prove it automatically appears where applicable in:
- CLI;
- API;
- OpenAPI;
- Swagger;
- MCP;
- Cockpit;
- docs/generated;
- GH Pages build model.

If a consumer-specific list must be edited, fail the architecture.

## 3. Drift sabotage test

Deliberately introduce:
- stale generated artifact;
- duplicated route/exposure;
- missing docs obligation;
- missing test/use-case;
- invalid rule/schema;
- stale client bootstrap.

Verify canonical validators fail loudly and explain how to fix each.

## 4. Multi-peer collision test

Simulate two workers:
- same path claim;
- same identifier claim;
- separate scopes;
- reconnect;
- handover.

Verify Cockpit and CLI/MCP agree.

## 5. Worktree test

Test:
- correct feature worktree;
- misplaced worktree;
- dirty worktree;
- branch without worktree;
- migration plan.

Ensure mutation cannot bypass rules merely because it came from browser.

## 6. Execution test

Run representative:
- read-only capability;
- mutating command allowed by governance;
- long/streaming operation;
- failure;
- cancellation;
- reconnect during execution.

Verify stable execution records and consistent state.

## 7. Security test

Attempt:
- HTML/script in object title/content;
- shell metacharacters in typed fields;
- traversal path;
- cross-origin mutation;
- huge output;
- secret-looking env values;
- malformed websocket/event input if relevant.

Confirm safe failure/redaction.

## 8. Performance regression test

Measure:
- cold startup;
- warm server reuse;
- repeated Cockpit home loads;
- large list;
- search;
- event streaming.

Inspect counters to prove repeated requests do not rebuild canonical state.

## 9. Docs/site truth test

Compare current implementation against:
- `AGENTS.md`;
- `CLAUDE.md`;
- Cockpit docs;
- capabilities docs;
- API docs;
- generated reference;
- GH Pages.

Any conflicting claim is a defect. Fix canonical source and regenerate.

## 10. Final transport matrix

Generate from code and include in report:
- every capability;
- its allowed projections;
- actual generated projections;
- justified waivers;
- test coverage.

No manually curated matrix.

## 11. Final "done" report

The final report MUST contain:

### Root causes fixed
Specific architectural/legacy defects.

### Canonical owners
Exact source modules/types/files.

### IDE capabilities delivered
Only features actually verified.

### Automatic startup
Exact path and evidence.

### Transport parity
Generated evidence.

### Governance
Rules/doctrines/policies and executable enforcement.

### Tests
Exact commands and counts/results.

### Security
Tests and remaining threat boundaries.

### Performance
Measured values and baseline comparison.

### Docs/GH Pages
Generation and deployment proof.

### Version/release/deploy
Exact version, commit/tag/build/deploy evidence.

### Git hygiene
Branch/worktree/PR state.

### Remaining debt
Anything not complete.

Forbidden phrases without evidence:
- "fully implemented"
- "production ready"
- "all tests pass"
- "deployed"
- "verified"
- "single source of truth"

Each must be followed by the command/artifact/runtime observation that proves it.

If any acceptance criterion is unmet, report the system as incomplete and fix it where possible before finalizing.
