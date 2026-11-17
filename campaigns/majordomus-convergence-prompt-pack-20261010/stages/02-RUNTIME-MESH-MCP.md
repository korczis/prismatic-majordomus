# 02 — Make server, mesh and MCP genuinely robust

## Objective
Prove correctness across transport, durability, identity, discovery, trust, leases, retry and real process lifecycle. Close existing P0 fixes before introducing a new mesh abstraction.

## Audit inputs
Investigate current mesh code and tests plus PR leads #883–#898. Read ADR 0035, 0050, 0067, docs/MESH.md, docs/MCP.md, tests/mesh_cooperation.rs, mesh compaction tests, relevant shell lab and CLI/API/MCP projections. Validate exact paths against current tree.

## Required invariants and red-team probes
1. **Durable acknowledgment**: accepted journal event has completed appropriate durable write before success; torn trailing line is detected/handled explicitly, never silently treated as valid. Failure injection after each write/flush/rename/ack edge.
2. **Event identity & pagination**: `(stream, seq)` deduplication, stable cursor and exactly-once enumeration across page boundaries; replay delivery remains idempotent. Shuffle concurrent arrivals, reorder sync and restart mid-page.
3. **Trust & authorization**: discovery grants zero authority; signed handshake checks runtime/repo identity, nonce/replay, expiry, trust, protocol version. Removal of trust tears down ongoing authorization without restart. Test malicious cross-repo and cross-runtime messages.
4. **Bounded resources**: payload sizes before allocation/parse, queue length, per-peer streams/marks, timeout, slowloris HTTP, MCP request duration, socket/thread count, sync chunking, connection churn. Bounds reflect actual declared configs and report typed diagnostics.
5. **Retry semantics**: transport/client never silently resends a side-effecting tool call whose outcome is ambiguous. Distinguish read-only retry, idempotency tokens when supported, and `outcome_unknown` on uncertain execution.
6. **Lifecycle**: one active elected server per checkout identity, superseded server releases only own lease and transfers eligible sessions, in-use server not collected as idle, stale/retired mesh record removed. Probe 2 processes, 2 worktrees, 2 machines when lab exists.
7. **Partial failure**: crash/restart, partition/heal, duplicate delivery, dropped frames, partial journal, disk full, permissions, unavailable peer, mismatched binary versions. Convergence claims require observed tests, never protocol prose alone.
8. **Confidentiality boundary**: docs clearly state signed HTTP mesh is not TLS; secrets never cross via logs, unsafe host exposure or handover default. Validate repository-configured overlay expectations.

## Integration
Prefer incremental ready PRs and share common test fixtures. Don't merge 15 overlapping fixes in parallel. Each fix must show before failing test, after passing test, and precise SHA. Use existing mesh capabilities for CLI/HTTP/MCP/OpenAPI/Cockpit, not direct UI reads. Diagnostics and health expose `unknown/degraded` honestly.

## Proof
Unit/property tests + real TCP/multi-process + partition/crash lab + fault injection for at-least-once delivery; verify no assertion depends on timing luck. Measure p95 resource response/usage under load and cite lab topology. Block release on unproven critical write or privilege paths.

## Operating contract (mandatory in every phase)
- Work against the repository checked out locally. Treat 2026-10-10 audit as leads, NOT a live truth. Fetch and inspect origin, HEAD, branches, PR checks, CI, tags, manifests, ADRs, .ai/**, docs and actual implementation before changes.
- Repo identity: `korczis/prismatic-majordomus`; detect the actual default branch, do not blindly assume master if it changed.
- Follow AGENTS.md and .ai/ governance; reuse existing capabilities, schemas, rules, registries, evidence ledger and commands. No independent truth in MCP, CLI, HTTP, OpenAPI, Cockpit, website, generated documents or shell wrappers.
- Read applicable existing ADRs and test architecture; amend rather than duplicate. Reject parallel control planes, synthetic statuses, mock production data, optimistic health, stale proof and manual inventories.
- Every issue/step has: invariant, owner/source of truth, scope, tests, diagnostic, acceptance proof, and merge/release impact. Attach work to existing intent/milestone/issue if possible; create only if needed, respecting ID allocation and sync rules.
- Make a reversible baseline: `git status`, `git rev-parse HEAD`, `git worktree list --porcelain`, branches, open PRs, CI runs, version, release, existing runtime state. Never reset, delete worktrees or force push without verifying ownership, merged state and no valuable uncommitted work.
- Implementation sequence: inspect → reproduce → design minimum compatible change → write failing test → implement → targeted checks → cross-surface contract checks → docs/evidence → integration. Never report green when unknown.
- Changes must be reviewed against actual `master`/default branch, use ephemeral branches/worktrees, small semantic deltas, deterministic generated output. De-duplicate generated changes where practical; no bulk unrelated formatting.
- Safety: do not change remote secrets, production deployments, destructive Git refs or branch protection silently. For explicitly authorized autonomous release/deploy actions apply repo policy, verifiable gates, least privilege, atomic rollback and fail closed; stop on missing credentials/permissions rather than bypass safeguards.
- If external advisors/providers are unavailable, continue locally without degrading evidence requirements. Do not promise that a plan/review/test passed unless it actually ran and can be linked to commit/digest.
- No time-box theater: optimize for verified closure, not task count, LOC, number of prompts, or a fabricated speed claim.
- Before final report: compile; focused and impacted tests; model/schema/derive drift checks; smoke clean checkout; CI status and commit SHA; security/resource check if applicable; update documentation; enumerate unresolved blockers precisely.

## Mandatory report per stage
1. Baseline HEAD SHA, release/tag, relevant PR/issues/CI links (actual values, not invented).
2. Root cause with file:function or line references and a reproducible failure; state explicit unknowns.
3. Changes made, boundaries preserved, migrations and deletions of legacy code.
4. Evidence matrix: claim → automated test/gate/run → revision/digest → outcome.
5. Numerical before/after if measured; otherwise `not measured`.
6. Artifacts/commit(s)/PR(s), remaining blockers, next runnable stage.
