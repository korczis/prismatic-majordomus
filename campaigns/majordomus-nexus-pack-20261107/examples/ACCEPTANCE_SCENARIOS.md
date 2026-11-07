# Executable end-to-end acceptance scenarios

Implement these through the current repository's **existing** use-case and test machinery. Fixtures must be safe and deterministic; if an external provider is unavailable, run contract tests and state precisely which live path was untested.

## E2E-01: One evolving intent

1. Human: "Improve release integration reliability".
2. Human follow-up: "Also ensure stale queued CI runs are not treated as green".
3. Human follow-up: "Use existing rebase/integration skills; do not add another scheduler".
4. Assert one canonical intent, three distinct immutable input receipts, versioned decisions and a single evolving plan. No redundant GH issues/PRs created by prompt count.
5. Assert relevant `.ai`, Git, ADR, work graph and existing code evidence references exist and are valid at snapshot revision.

## E2E-02: Read-only fast route

Human: "What does the current `majordomus context resolve` do?". Assert `QUERY` route, relevant scoped sources, no new task/issue, no mesh fan-out if unnecessary, latency measured. Verify privacy scope.

## E2E-03: Protected mutation

Human asks for a bounded implementation and simultaneously includes a tool command that would modify a path outside authorized scope. Assert the route selection does not itself grant write permission. Actual `PreToolUse`/repo gate denies unauthorized action, provides concrete reason, and permits compliant work.

## E2E-04: Crash and retry

Simulate a worker crash after an external mutation but before local acknowledgement. On recovery reconcile external state and journal: no duplicate branch/PR/CI/deploy; safe remaining work resumes. Replay duplicate hook events and verify one canonical receipt/state transition.

## E2E-05: `.ai` reconciliation

Change a canonical rule/ADR-backed behavior (safe fixture). Assert code/tests/docs/policy projection remain aligned. Ensure an inferred claim without evidence is not committed as verified truth. Rule conflict produces actionable diagnostics.

## E2E-06: Peer council available/unavailable

With actual connected approved peers, ask for architectural critique and independent verification; retain dissent and evidence. Without peers, assert `unavailable` and safe fallback, no imaginary advice and no fabricated quorum.

## E2E-07: Same truth across surfaces

For a real test intent, fetch identity/state/evidence through the Rust model, CLI JSON, HTTP/OpenAPI, MCP and Cockpit backend. Compare canonical fields/provenance/ordering. UI should reflect live state, including reconnect/replay, without fake payloads. Site/published docs must not contain secret or raw prompt evidence.

## E2E-08: Delivery verification

Run the full current gating workflow, push/PR/merge/deploy only within permissions. Attach HEAD+PR+CI run+merge SHA+probe and exact revision in the final report. `CI queued`, `PR open`, `merged` and `deployed` must not be conflated.
