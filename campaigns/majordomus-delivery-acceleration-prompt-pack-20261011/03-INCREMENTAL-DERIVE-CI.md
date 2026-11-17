# PHASE B: INCREMENTAL DERIVATION + PROOF-PRESERVING CI

Role: build system & CI performance architect.


## NON-NEGOTIABLE EXECUTION CONTRACT
- Work against the ACTUAL current repository HEAD. Read AGENTS.md, `.ai/**`, governance/doctrines, schemas, ADRs, plan/issue model, commands/capability registry, CI workflows, docs/CLAIMS.yaml, documentation generation, site/cockpit rules, and tests BEFORE proposing changes. Discover paths; do not invent them. The September 2026 bundled archive is historic context, NOT current truth.
- The source of truth is the existing canonical typed repository model and Git. Inferred/derived means no shadow YAML registry, hardcoded frontend inventories, manually synchronized docs or generated facts. Respect existing local shell/Rust boundary; do not reflexively rewrite.
- Establish the state of feature by repo evidence: existing, partial, missing, blocked, obsolete. Explicitly separate implemented, merged, released, deployed, and verified-in-production. No claims of completion by intention or PR description alone.
- Prefer the smallest safe change that corrects a systemic bottleneck. Reuse existing commands, gates, logging, event formats, diagnostics, ownership and effects model. New abstractions need at least two proven consumers or a compelling correctness reason.
- Avoid unsafe auto-merge, branch deletion, release, remote fleet actions, weakened security, ignored exit codes, bypassed branch protection, or suppressed CI. Use established permissions, policy and authorization; never leak credentials. Automated changes only when authorized by existing policy. If blocked, record blocker and continue independent work.
- Repository work must be tracked by intent -> criteria -> milestone -> issue/task -> evidence where supported. Create/fix missing bindings through existing machinery rather than fictional names. Granular PRs, preferably single-purpose; separate generated artifacts where feasible.
- Enforce via actual tests/CI, not merely via a Markdown doctrine. For each new invariant create negative/mutation proof that would fail before the patch; maintain portable adopter behavior. Determinism across machines, worktrees, process schedules and retries is mandatory.
- Do not claim an unknown measurement. Report reproducible commands, revision IDs, expected vs observed results, limitations and exact remaining work. Fail closed on uncertainty for completion, releases and remote writes, without breaking read-only diagnostics.
- Progress autonomously through justified tasks, but stop any destructive/high-impact action outside existing authorization. If a real external approval is required, state the exact blocker and proceed on safe independent tasks.
- Keep PUBLIC docs and UI claims honest and evidence-backed. Changes to contracts require schema/version compatibility review and migrations. Build, test, regenerate, document, review, integrate, release and verify deployment only where permissions and real evidence permit.

## Objective
Fix change amplification and verification critical path. Representative reported PR derives took 265/527/677 seconds; PR #902 described a repeated 85-100-minute required check. These are leads to measure, not universal baselines.

## Implement after tracing existing system
1. Build a typed dependency graph from canonical source inputs to derived artifacts and validators; reuse any existing graph. Normalize paths/identities, account for changes to generators themselves, broad dependencies and deletion/rename effects.
2. Support `derive --affected` / existing equivalent without adding incompatible switches where an API already exists. For known-safe changes regenerate only affected outputs; for uncertain input/graph/version use conservative full derive or fail with actionable reason.
3. Use content-addressable input fingerprints combining generator version, source revisions, schemas/toolchain and relevant environment (never secrets). Ensure cache poisoning, staleness, races and cross-worktree contamination fail safely.
4. Separate expensive presentation/materialized outputs from semantic evidence; avoid committing unnecessary churn only if compatibility and repository policy permit. Preserve deterministic byte-stable generated artifacts.
5. Profile CI by measured historical test durations; balance shards, parallelize independent tests, prevent `stdin` hangs, harden flaky infrastructure classification, and preserve required full checks at appropriate protected boundaries.
6. Prove that reuse is valid for semantically identical trees (including deliberate revert/non-merge-commit cases); do not assume mere tree hash equality is sufficient for historical semantics.
7. Add `explain` to show why a generator or gate reran and which input invalidated it. Expose via existing CLI/API/diagnostic projection if already supported.
8. Add invalidation/mutation tests: modified schema, deleted file, new command, new claim, force push, concurrent worktrees, cache corruption, swapped executable, interrupted derive, nondeterministic enumeration, modified generator, differing OS/toolchain.

## Proof
- Side-by-side cold/warm/repeated runs, median and p95 when enough samples; record machine and build cache conditions.
- Byte equality full derive vs incremental output for each representative change class.
- Deterministic network-disabled local derivation if policy requires it.
- Existing full coverage/mutation/branch protection retains behavior.
- An unknown dependency MUST NOT quietly report up-to-date.

Deliver: small PR slices, timing table, cache key/invalidation specification, trace example, test evidence, rollback path.
