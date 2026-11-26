# PROMPT 08 — Integrate, deploy, verify, report and hand over

Goal: convert work-in-progress into a durable, verified result before campaign expires. Freeze new features for remaining 20% effective time.

1. Inventory branches/worktrees created by campaign; preserve existing unrelated edits. Summarize each change with base/head SHA and intended impact.
2. Run focused gates for touched components and required canonical integration gates. Check CI required results on exact PR heads. Fix regressions introduced by campaign, otherwise block and document.
3. Ensure generated artifacts and .ai context/doc invariants pass; inspect changed files for secrets, dead code, unrelated formatting changes and manually duplicated source inventories.
4. Merge only authorized green PRs in dependency order. Reconcile remote master and verify expected final SHA. Do not impersonate approvals.
5. Run canonical release/version bump only if release policy requires it. No gratuitous bump per PR unless repository doctrine explicitly demands it.
6. Observe deployment metadata, fetch public Pages UI snapshot and compare commit/version/digest where available. If DNS/CDN is stale, report STALE with bounded retry and actual HTTP evidence, not success.
7. Document regression tests and architectural decisions in established docs/ADR hierarchy, regenerate GH Pages, deploy if authorized, re-verify public output.
8. Collect campaign before/after metrics, method, timestamps, sample sizes and URLs. Highlight any unknowns; do not suppress failures.

## Final report mandatory sections
- Outcome: shipped / merged-only / PR-only / blocked / not started.
- Baseline + end table (master SHA, open PRs by class, CI times/results, deployments, version, drift, verified tasks).
- Implementations and precise PR/commit/release/deploy links.
- Tested commands, exit codes and evidence.
- Measured speedups/costs with methodology, or NOT MEASURED.
- What remains and why.
- Next highest leverage task with concrete acceptance.

Do not write DONE if deployment or verification are unresolved.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
