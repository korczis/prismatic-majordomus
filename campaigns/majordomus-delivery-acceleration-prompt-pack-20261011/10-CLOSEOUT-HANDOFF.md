# FINAL CLOSURE / AUTO-HANDOVER PROMPT

Review the master objective and evidence gathered in this same session. Do not produce a success story. Produce an independently reviewable closeout.


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

## Mandatory artifacts
1. Executive delta in human-facing value and measured latency, with numeric confidence/sample counts.
2. Table: intent criterion -> implementation SHA -> proof command + run artifact -> PR -> protected merge -> released version -> deployed version -> satisfaction verdict.
3. Before/after: open PRs, ready PR age, stale branches/worktrees, cache hits, derive time, required check p50/p95, manual touches, failed/unknown runs, generated diff count.
4. Refused scenarios and red/mutation tests, including negative-control proof.
5. Explicit SKIPPED/NOT IMPLEMENTED/NOT MERGED/NOT DEPLOYED/UNKNOWN items and reasons.
6. Security and regression coverage summary; exact surviving pre-existing failures.
7. Documentation, `.ai`, schema, site/CLI/API/MCP/UI cross-surface parity outcomes.
8. Handover suitable for next session: canonical intent, currently ready task, blockers, dependency links, no duplicated state, exact next command.
9. Safe cleanup inventory and what was deliberately kept.

## Gate
If any criterion cannot be backed by a revision-specific proof, mark it incomplete. 'Code authored' is not 'merged', 'merged' is not 'released', 'released' is not 'deployed', and 'deployed' is not 'intent satisfied'.
