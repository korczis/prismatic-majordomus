# Phase 14: End-to-End, Property, Chaos, Performance and Evidence Gates

## Testing pyramid
- Unit: reducers, policies, source classification, ordering, readiness, evidence predicates, retries.
- Property: shuffled discovery, duplicate events, idempotency, cycle detection, stale version.
- Contract: canonical capability -> CLI/API/OpenAPI/MCP/UI/docs shape.
- Integration: local journal, fake provider, GitHub simulator, mesh pair, worktree lifecycle.
- E2E: sandbox repo and authorized sandbox deployment target.
- Chaos: crash/restart, partition, leader/worker loss, request succeeded but acknowledgement dropped.
- Performance: route/stream latency, cold/warm views, scheduler fairness, event replay time.
- Security: deny/permission matrix and redaction.

## Scenario catalog
S01 single trivial intent -> plan -> one issue -> one PR -> sandbox deploy -> evidence.
S02 intent with 3 criteria and DAG where only truly ready work dispatches.
S03 zero-ready state correctly blocks and explains.
S04 conflicting path scopes on two mesh peers.
S05 partial peer board with unreachable worktree, uncertainty represented.
S06 worker crash immediately before remote effect.
S07 worker crash after remote effect but before local receipt.
S08 duplicate task dispatch and duplicate GitHub webhook.
S09 PR conflicts with newer master; rebase safely or block with evidence.
S10 required review missing; no bypass.
S11 CI failure followed by authorized bounded repair.
S12 evidence stale because new commit invalidates earlier test proof.
S13 release created but deploy failed; intent remains unsatisfied.
S14 deployment succeeded but health smoke failed; safe rollout halt.
S15 mesh disconnect and lease expiry with old worker returning.
S16 cancellation at multiple stages including irreversible phase.
S17 UI websocket/SSE disconnect, reconnect with resume token.
S18 permission denied from CLI/API/MCP/UI consistently.
S19 synthetic new capability propagates automatically to supported surfaces.
S20 schema upgrade loads old event fixture.
S21 parallel workers produce stable deterministic output.
S22 user changes intent mid-execution; plan revision pinned and re-evaluated.
S23 GitHub rate-limit/backoff; no hot loop or duplicate effects.
S24 no workers available; queued plan survives restart.
S25 release bump policy change has no duplicate tag.
S26 automatic worktree cleanup preserves foreign/unmerged changes.
S27 deployment target reports version different from expected; no false positive.
S28 evidence revocation flips satisfaction when valid policy says so.
S29 event subscriber lag with compaction/snapshot resync.
S30 end-to-end golden-path repeated in clean disposable checkout.

## Definition of proof
Every test has:
- scenario ID mapped to acceptance criterion,
- fixtures and deterministic seed where applicable,
- system under test, inputs and authorization setup,
- expected event sequence and negative conditions,
- exact commands executed, source commit, versions and CI run URL,
- resultant artifact/event IDs and evidence hashes,
- failure diagnostics and rerun instructions.
Never substitute screenshots for actual event and external effect verification.

## Benchmark targets
Establish baseline before choosing hard thresholds.
Propose targets, validate real environment:
- UI navigation with cached initial projection should appear instant to a human.
- event propagation p95 measured end-to-end with explicit sample size.
- no scheduler busy loop while all work blocked.
- bounded journal replay memory and time with tens of thousands of events.
- pagination must keep payload bounded regardless of historical run count.
- CLI env/banner performance budgets may already exist: do not regress them.
- CI feedback loop optimize based on measured critical path, not blanket promise <1 min where service overhead makes it impossible.

## Chaos harness constraints
Inject failures only in disposable sandbox fixtures with isolated credentials.
Record whether external side effects were real/fake and which target was used.
An end-to-end success in a mocked deployment is sandbox proof, not production rollout proof.
