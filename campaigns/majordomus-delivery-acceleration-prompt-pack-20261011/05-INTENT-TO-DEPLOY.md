# PHASE D: INTENT -> EVIDENCE -> DEPLOYMENT, NOT JUST TASK DONE

Role: domain/model & lifecycle integrator.


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

## Validate distinct semantics
Intent statement/criteria/invariants/revision; milestone realization graph; issue/task binding; worker context compilation; opposition/critique; execution evidence; task/issue completion; intent satisfaction; release/deployment verification. These are distinct types/verdicts. Refuse unresolved or stale binding. Inspect issues #875-879 and PR #821 before writing new code.

## Requirements
- At session start infer/load the active intent and exact criterion(s) using canonical task binding; explicit exemptions are typed and explained. Never hallucinate association.
- The execution context includes minimum necessary intent, acceptance criteria, rules and pinned revisions. Missing links fail preflight where policy requires; tests cover both shell and Rust engines if both exist.
- Each criterion has evidence requirements and provenance; test success is associated with exact input revision. Opposition pass raises falsifiable gaps, not empty prose. No work begins when a required critique is missing.
- `finish --outcome completed` judges task contract, with separate issue and intent satisfaction verdicts. Unknown/stale/degraded downstream deployment cannot become `satisfied`.
- Changes to criterion text, dependency state, artifacts or target deployment invalidate only truly affected evidence; explain why.
- E2E controlled sample: (a) simple change with expected accepted deploy, (b) failing test refuses, (c) valid task but unmet intent refuses satisfaction, (d) deployment not observed -> remains unknown, (e) stale evidence -> invalidates.
- Expose same data via existing CLI, MCP, API, Cockpit and docs projections, without frontend-specific semantics.

## Product deliverable
A single trace/replay record answering: what was asked, what did each worker do, what failed, what actually shipped, where, and which criterion is still owed? Include authoritative URLs, commit SHA and deployed revision where available.
