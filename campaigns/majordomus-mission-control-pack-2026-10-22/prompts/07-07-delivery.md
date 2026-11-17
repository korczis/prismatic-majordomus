# 07 — One-click Verify and Deliver through existing release train

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Route actions through current completion, evidence, integration/prs drain, versioning, GitHub Actions, release and deployment capabilities. Implement stage transitions with separate tested states: implemented, verified, PR_open, CI_pending, CI_passed, merged, released, deployed, served_verified, satisfaction_met. Only permitted states drive Deliver; require standing owner approval by narrow policy; queue when no grant. Prove full exact-head validation and pre-merge collision/leases. Never mistake PR merge, artifact publication, GitHub Pages success or skipped provider for live deployment. Browser must show exact commit/version/links and failure reason. No force merge/--admin, no fake-green, no automated credential grant. Exact reproducibility and attested output for each stage.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
