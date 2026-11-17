# 06 — Provider deploy and independently observed served revision

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Extend current .ai/repo/deployments/majordomus.yaml, ADR 0219/0220, provider.yml and generated Gigalixir Procfile/buildpack only. Prefer Rust binary deployment with no separate app server. Verify current secrets presence only (no printing); if missing produce explicit BLOCKED credentials instructions. Build slug from exact validated SHA, release health/readiness and rendezvous endpoint; ensure producer persists stable node identity from injected secret, no node.json leak. Live publication gate must compare actual /api/v1/live commit to intended SHA, test TLS hostname, negative remote unauthorized routes and mesh registration. Show status declared/configured/deployed/served_verified separately. Stage rollback and resilience tests, provider restart behavior and zero leaked keys. Never claim provider operational based on workflow file alone.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
