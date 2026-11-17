# Architecture and safety contracts

## Control loop
```text
EVENT / TIMER → observe GH + git + .ai → typed snapshot → dependency DAG → canonical next action → policy+effect authorization → exclusive lease → freshness+CI+approval recheck → one mutation → observe again → post-action proof → derived projections
```

## Single writer
The merge executor owns one exclusive, recoverable lease. Every mutation binds base and head SHAs; repeat protection/CI checks immediately before merge. An external advance of master invalidates the plan. No 40-operation speculative command batch. The scheduler can prepare candidates in parallel; merge is serialized or a formally verified atomic batch.

## Security / authorization
The current source policy intentionally observes rather than merges on a timer. Preserve that policy until an explicit authorized transition establishes autonomous-safe actions. Repository owner consent, branch protection approvals and status checks are independent. Lack of protection visibility = unknown and no merge. GitHub's remote conflict result cannot substitute for a local derived-driver merge probe, and a successful local probe cannot substitute for GitHub approval/status authority. Fork code must never execute with write token. Remote deploy and fleet update remain governed remote-mutation effects, never implicitly projected over HTTP/MCP.

## Evidence
Candidates have source, trust level, stable identity, branch/base/head/tree, CI verdict bound to SHA/merge tree, dependencies/stack edges, freshness, blocker+actor, next action, disposition, mutation intent/outcome and provenance. Error/unknown is not empty/success. Report source age and cache staleness. Data model is canonical, frontend is renderer.

## Idempotence
GitHub event duplicates, workflow retries, actor restart, crash after merge API succeeds, partial retarget/cleanup, branch deletion, race and rollback are expected. Re-query state instead of repeating blindly. Only deletes proven disposable branches/worktrees. Journal/recovery via existing project patterns; no new independent sqlite database unless justified by current architecture.

## Exit semantics
Inspect existing exit codes and preserve 0/10/12 meaning when applicable; no silent success on error. Blocked by approval is not executable failure. Counters across UI/CLI are derived from the same snapshot and include timestamps.

## Delivery
Version uses `release analyze` and repository writer, not `sed` or hypothetical minor bump. Tags/releases/Pages/installer/fleet require actual policy and test proof. Post-release smoke and rollback strategy required. No fictitious current version or deployed fleet members.
