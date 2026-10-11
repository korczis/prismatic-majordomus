# PHASE G: ENFORCE LESSONS IN `.ai`, RELEASE AND ROLLOUT

Role: policy/validation/release integrator.


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

## Governance integration
- Find canonical doctrine/rule lifecycle and machine validator hooks; do not invent rule filenames or duplicate one policy in multiple tools.
- Add or update ONLY general rules supported by defects and tests: measured completion, no-growing-backlog checks where enforceable; proof-preserving cache reuse; deterministic derived state; bounded queue/backpressure; truthful product claims; explicit remote mutation effects; no credential leaks; single source and cross-surface parity.
- For every rule, demonstrate negative/mutation test that changes a real violation to red; wire through doctor/check/finish/CI according to current architecture. Ensure adopters are not held to repo-specific constraints unless they opt in.
- Do not promote advisory heuristics to blocking without a falsifiable, machine-observable predicate. Unknown must not be smuggled into `pass`.

## Release correctness
- Inspect existing versioning and `scripts/release-record`, release package, install verification, attestation, schema compatibility, changelog and GitHub Pages deploy. Reconcile actual `v0.20.0` history with current head; do not assume latest remains v0.20.0.
- Choose SemVer bump from canonical policy and public/API contract delta. Avoid any ad-hoc extra version source and avoid releases for branches that were never merged.
- Verify release artifacts, checksums, provenance, actual published manifest, install smoke and deployed binary readiness/version SHA; fail closed for mismatch. Document manual approvals if externally required.
- Publish docs and landing projection built from exact released commit, not workspace residue. Verify public URLs and emitted claims, not merely generator success.
- Rollout to configured fleet only if authorized and safe; report per-machine converge/partial/failure, not all-green from majority.

## Closure invariants
No new undocumented debt, no orphaned lease/worktree from THIS campaign, no unsupported marketing language, no missing test or unexecuted blocking check, no dead or duplicated registry. Take before/after counts and identify baseline debt separately from introduced debt.
