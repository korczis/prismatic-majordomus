# PHASE A: AUDIT AND BOTTLENECK BASELINE

Role: measurement engineer / independent repository cartographer. Read the master charter.


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

## Investigate
1. Git graph: 7/14/30-day PR arrivals, merges, time-to-merge, stale PRs, queue depth and WIP by dependency chain. Deduplicate stacked/batch PRs. Distinguish PR opened-to-merged from ready-to-merged.
2. CI: required check names, job-level durations, queue wait, setup, cache fetch, build, test execution, derivation, attest/publish, deploy; identify top critical path and repeated runs triggered by merge-only changes.
3. Derive: list canonical inputs, generators and artifacts, per-generator times, expensive global scans, invalidation keys, concurrency/locks, ownership and process lifetimes, git diff churn, repeated identical output.
4. Worktrees: live vs stale, dirty vs merged, claimed paths, branch topology and cleanup policy. Never delete merely on age.
5. Product claims: compare docs/CLAIMS.yaml, generated SITE_CLAIMS, main landing copy, code references and recorded test verdicts; do not equate a test pathname with executed evidence.
6. Intent: canonical identity, version pins, criteria, task binding, evidence ledger, satisfaction verdict, completion policy, deploy receipt.
7. Mesh: actual discovery, auth/trust, event durability, peer assignment, startup, reconnection and health; mark plan-only items.
8. UI: canonical backend fields, cockpit navigation, mobile widths, browser tests, missing paths, user actions that actually work.

## Required report
- Facts (with SHA, paths, commands, timestamps), hypotheses and unknowns separated.
- Top 10 bottlenecks ranked by impact = observed frequency × added delay × failure cost; mark estimated values as estimates.
- Map `old behavior -> proposed invariant -> existing owner -> regression test -> measured target`.
- Preserve baseline snapshots in established evidence location (not newly invented ad-hoc Markdown databases).
- Negative control: compare a deliberately unchanged working tree across two runs; explain variable environmental costs.
- Disprove at least two attractive but unsupported optimization hypotheses before prioritizing.

Stop/go: no claiming a speedup without baseline; do not block independent low-risk fixes when full historical observability is inaccessible.
