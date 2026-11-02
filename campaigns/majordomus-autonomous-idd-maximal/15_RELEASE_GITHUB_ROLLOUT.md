# Phase 15: GitHub Integration, PR Queue, Versioning, Release and Rollout

## Preflight
Inspect actual current PRs, branches, worktrees, workflow checks, default-branch protections, auto-merge configuration and release tagging policy. Compare docs/DELIVERY.md, RELEASE.md, CI.md, WORKTREES.md and `.ai` policy. Don't trust stale counts from an older audit.

## Queue admission and WIP
- Limit new branches by configurable per-repo/per-domain WIP.
- Pull conflict graph from file overlap and dependency edges, with caveats for semantic conflicts.
- Group trivially compatible changes only if provenance/acceptance remains intact.
- Prioritize aged ready PRs and blockers on critical path.
- Compute queue health: PR age, READY time, CI queue time, merge throughput, revert rate and deploy latency.
- Distinguish external contributors and bot-owned PRs; never rewrite unknown human branches.
- Never force push a shared or externally owned branch without explicit policy.

## PR state machine
`draft -> ready_for_review -> checks_running -> review_blocked|conflicted|mergeable -> merge_requested -> merged -> integration_verified`.
Determine and use GitHub-native actual states.
Revalidate required checks, base SHA, signed/protected branch policy and review immediately before merge.
When base SHA changes, recalculate compatibility; stale verdicts are not guarantees.
Idempotent merge operation reconciles GitHub remote `merged_at`/merge_commit_sha after timeout.
Never report merged from a submitted API request without observing final GitHub state.
After merge, attach issue/milestone/criterion provenance and verify included diff.
Cleanup only branches/worktrees proved safe; preview or quarantine unknown state.

## Versioning
- Read existing version policy and any Elm-like semantic API change classification before touching tags.
- Diff canonical public API/schema/CLI contracts and apply established bump policy.
- One version bump per policy-defined release unit, NOT necessarily each issue or PR.
- Derive changelog and release notes from accepted work and canonical evidence.
- Avoid tag collisions; release is keyed by source SHA + version + target channel.
- Check whether version already published before writing; reconcile partial release after crash.
- Do not rewrite released artifacts or history unless explicit policy.

## Deploy
- Confirm authorized deployment target and environment; make no assumption that `majordomus.dev` static site is the same as a mutable production control plane.
- Build immutable artifacts from specific commit SHA; record build provenance.
- Deploy to sandbox/staging first where available.
- Verify actual observed version and functional smoke endpoint, not just upload complete.
- Promote only through policy-approved environment gates.
- On failed smoke, pause next promotion, publish diagnostics and invoke approved rollback if reversible.
- Mark result `verified` only after checks at target environment and within freshness window.
- UI must display target commit, version, route, last smoke and rollback eligibility.

## Tracing
Intent -> plan revision -> issue -> branch/worktree -> commits -> CI -> PR -> merge SHA -> release -> artifact -> deployment -> smoke -> criterion evidence.

## Acceptance
Prove both success and adverse path: merge timeout with completed merge; release request repeated; deploy success with failed smoke; partial permission; stale base; branch cleanup safety.
