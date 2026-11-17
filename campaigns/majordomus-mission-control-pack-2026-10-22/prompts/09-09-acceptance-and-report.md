# 09 — Final integrated acceptance, CI, release, rollout

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Converge all slices into one clean train, run full required gates with exact SHA, CI receipts (no skipped required tests), installed-release smoke, local click-to-execute browser E2E, two-node authenticated mesh E2E, kill/restart and event replay E2E, Gigalixir served-SHA smoke, GH Pages snapshot/version consistency, adversarial remote auth and CSRF tests. Capture wall-clock CI and deploy durations from provider timestamps, avoid estimates. Merge via guarded policy only after green head, release with required assets, verify web + provider and E2E via browser. Record evidence in canonical model for release SHA; if provider credentials unavailable, mark deployment BLOCKED and do not falsely claim full delivery. Produce an operator-ready report with exact commands, URLs, PRs, SHAs, blockers, verified paths and reproducible logs/screenshots.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
