# PROMPT 03 — Autonomous PR drain and integration train

Goal: reduce mergeable backlog while preserving code quality, collaborators' work and branch protection. Read current queue via existing Majordomus PR observation first.

1. Build dependency DAG using linked issues, changed files, base/head commits and observed conflicts. Distinguish genuine dependencies from title similarity. Detect abandoned work, duplicates and superseded work only with direct evidence.
2. Rank unblockers and small valid PRs highest; prioritize repairs unlocking Pages/CI/release. Set limited WIP and a single serialized merge lane.
3. For READY PR, compare SHA and required checks, review approvals, merge policy, generated drift and risk, then merge with existing protected mechanism if permitted. Use queue/auto-merge when it is already configured.
4. For NEEDS_REBASE, check active collaborator and branch owner; rebase safely, preserve work, resolve only understood conflicts, re-run validation and verify new head SHA. No blind force push.
5. For NEEDS_FIX, fix minimal root cause in owning branch or new authorized repair PR. Never merge red PRs. For UNKNOWN, query again or leave unknown. For WAITING_REVIEW, preserve reviewer gate, do not self-approve on behalf of humans.
6. After each merge, assert master advanced as expected and required post-merge checks pass. Re-evaluate queue to avoid stale merge plans. Trigger downstream canonical deploy only if policy permits and observe it.
7. Emit canonical PR evidence and stale queue diagnostics into existing CLI/API/MCP/UI where existing adapters support them. No separate frontend inventory.

## Acceptance
Measure starting and ending counts, median/p95 age if sample permits, merged/blocked/closed counts with exact URLs. Zero unreviewed bypasses and zero lost dirty work. At least one real integrated P0/P1 outcome where actionable.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
