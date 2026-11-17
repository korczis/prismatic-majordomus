# PROMPT 01 — Fresh forensic baseline and truthful PR triage

Goal: establish a reliable dated picture of the current trunk, active PRs, execution machinery, gates and deployed website, using native capabilities. Do not write features yet.

1. Record `git status --porcelain`, current branch, local/remote SHA, relevant version tags, submodules and worktrees; distinguish pre-existing edits.
2. Inspect `AGENTS.md`, `CLAUDE.md`, `.ai` context/rules, `campaigns`, scripts, `.github/workflows`, `justfile`, `.just`, Rust CLI, Pages and site pipeline. Check what is auto-generated; do not change generated files manually.
3. Query GitHub API/gh current open PRs with pagination, not a cached sample. Include number/title/base/head SHA/updated/draft/review/checks/mergeable/conflicts/owner/changed paths/dependencies. Deal with API failures as UNKNOWN.
4. Query latest required checks on master and recent 10–30 workflow runs. Distinguish failure, timeout, queued, cancellation, permission failure and skipped. Determine authoritative required checks from actual branch policy.
5. Query releases, deployed environments, GitHub Pages actual commit and `https://majordomus.dev/` public metadata (HTTP + API/UI snapshot if available). Don't conflate built artifact with successful publishing.
6. Reproduce historical incident traces for PR #1042/#1043/#1046, then check if their invariants are now exercised on master; do not reopen fixed bugs.
7. Record time taken for canonical local gates and a representative derive. Do not run an expensive full build before selecting a sensible measurement strategy.

## Deliverable
One canonical machine-readable baseline plus a human-readable decision note (location chosen per repo convention). Include `observed_at`, `source`, `subject_sha`, `result`, `freshness`, `confidence`, `blocking_reason`. PRs categorized READY, NEEDS_REBASE, NEEDS_FIX, WAITING_CHECKS, WAITING_REVIEW, BLOCKED_EXTERNAL, SUPERSEDED, UNSAFE, UNKNOWN. Report count and scope; track unknowns honestly.

## Acceptance
Re-run data collection and prove traceability for top P0/P1 candidate. Do not implement unless baseline is specific enough to rank priorities.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
