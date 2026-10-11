# PHASE C: PR INTEGRATION, STACKED TRAINS AND BACKPRESSURE

Role: integration queue engineer. Prioritize draining verified work over generating more branches.


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

## Discover actual queue
Read implementation/docs of `prs drain`, `prs repair`, branch relation, integrations, git worktree lifecycle, GitHub branch protection, required status contexts and approval policy. Analyze open PR #902 as a candidate not an assumed installed feature.

## Invariants
- Every integration action has durable reason, exact head, dependency graph, required checks, approvals/authorization, resulting main SHA, and post-merge check. Do not bypass protection or merge failed/unknown revisions.
- Dependency DAG must be acyclic; stacked links explicit and verifiable. Only merge a PR when all dependencies landed and its reviewed diff matches the intended effect.
- Recognize relation `up_to_date`, `behind`, `conflicting`, and only add semantically valid equivalents e.g. proposed `carries_master` when tests support correctness including reverts and empty/nonmerge commits.
- Idempotent drain and restart safety: re-running after crash must not duplicate merges/releases, skip tasks or corrupt state. Concurrent integrators acquire exclusive lease/lock with fencing; stale lease recovery tested.
- Incremental refresh only when necessary. Track what invalidated a check and whether evidence reuse is justified by repository policy.
- Cleanup only after proving merged/redundant/superseded status, no uncommitted unique work, no active owner/lease, and no remote references that must survive. Use dry-run and reversible/archive path where appropriate.
- Backpressure: derive queue WIP limits from measured service rate; prevent new autonomous feature branches when ready-to-merge queue is starved or backlog grows. Preserve emergency/bugfix lanes explicitly.

## Live exercises
1. Independent PR: green -> merge -> verified master -> cleanup.
2. Two-level stacked train: dependency preserved -> correctly ordered integrations.
3. A reverted commit produces same tree but non-equivalent history: reject incorrect reuse.
4. Failed/unknown status, missing branch, force-push, conflict, dirty worktree, duplicate request, crashed integrator, failed release: refuse or recover without false success.
5. Backpressure simulation: queue grows; automatically prioritize draining over new tasks, expose clear diagnostics.

## Reporting
Numbers: PR queue arrival and departure rates, ready-to-merge age, retry counts, repeated check minutes, manually repaired branches. Classify current gaps as verified, partial, blocked. No arbitrary 5-minute claim unless operationally achieved by a representative cohort.
