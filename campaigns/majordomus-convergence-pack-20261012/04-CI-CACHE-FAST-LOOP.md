# PROMPT 04 — Accelerate dev/CI critical path safely

Goal: cut total time from valid change to verified merge/deploy, not just make a benchmark attractive.

1. Extract durations for checkout, dependency downloads, cargo compilation, quality cases, derive, docs/site, container, upload and deployment. Measure at least warm/cold distinctions and p50/p95 where enough samples. Identify critical path and parallel slack.
2. Inspect existing Cargo target caching, sccache, GH cache, Docker BuildKit, .dockerignore, lockfile invalidation, workflow concurrency, site artifacts, Rust test target grouping. Avoid multiple competing caches or stale artifacts.
3. Make one or two lowest-risk/highest-leverage improvements: restore cache keys, separate dependency compile, parallelize independent test groups, deduplicate redundant builds, reuse derived outputs safely, add concurrency cancellation only where it won't disrupt protected deploy.
4. Ensure cache identity includes toolchain, dependencies, features, generator schema and relevant environment. Test cache misses and hits have identical artifact identities. Never share sensitive cached data across trust boundaries.
5. Test local developer flow and GitHub Actions with actual commands; document cost and storage tradeoffs. Do not reduce required coverage.
6. Preserve a single source of workflow intent: docs and just commands derive or link to actual CI definitions, not copy them.

## Acceptance
Before/after durations from comparable runs, exact workflow links, known cache-hit details, same required checks. If comparison unavailable, report only observed times, not fabricated speedups.

## Non-negotiable Majordomus constraints
Read AGENTS.md, CLAUDE.md, applicable .ai context chain and existing campaign/skill/doctrine definitions before writing code. Existing behavior, tests and actual checked-out HEAD trump this prompt. Reuse repository tools and names discovered, never invent CLI switches. Source-of-truth first; typed, derived, inferable, deterministic, explainable, provenance-carrying where appropriate. No duplicate registries, mock-green tests, weakened checks, force-pushing protected branches, secret leakage, broad destructive cleanup, or unsupported deploy claims. Preserve legitimate branch protections and approvals. Use --admin only where authorized, not as a bypass. No human interaction for routine decisions; if an irreversible action lacks approval, mark BLOCKED and continue independent work. Work in narrow vertical slices: implementation + tests + governance + docs + integration + deployment verification. At the end record exact SHA, commands, status and URLs. Never report PENDING/UNKNOWN as PASS.
