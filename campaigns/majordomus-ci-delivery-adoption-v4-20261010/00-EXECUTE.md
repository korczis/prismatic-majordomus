# MASTER EXECUTION PROMPT — V4

You are Claude Code operating in the live Majordomus repo. The user has already authorized implementing the CI/CD acceleration, updating docs/tooling/`.ai/`, using the deployed mesh, and releasing/deploying within the repository's existing authorization and protection policies. Do not repeatedly request routine `proceed` confirmations.

Read `README.md`, all milestone files, `manifest.json`, then relevant repo sources. Use the current repository state over stale examples. Start implementation, do not stop after advice. Every milestone is realized as canonical intent-linked issues using current project conventions. Do not copy these files into `.ai/` verbatim if that breaks schema. Interpret each `## Issue` as one idempotent work unit and map by stable V4 issue ID to current canonical issues; dedupe with V2/V3/CI previous packages.

### Execution algorithm

1. Bootstrap: inspect status, head SHA, default branch, remote PRs, ongoing releases and active mesh jobs, current `.ai/` authoritative schema and gates. Detect collisions before changing files.
2. Reconcile **one canonical intent** `Fast verified PR delivery and adoption` with existing intent if present. Its acceptance: correct p95 target, no integrity regression, tools/docs/adoption, evidence-backed release/deploy.
3. Materialize milestones and issues with explicit dependencies, acceptance, tests, ownership and evidence. Sync GitHub with existing tooling. Do not invent CLI syntax if commands differ; inspect help first.
4. Scheduler: execute ready, nonconflicting issues; allow multiple mesh workers only if actual runtime supports dispatch/auth and scope conflict proof. Otherwise use safe local serial execution while preserving handovers.
5. Checkpoint after each passing issue; re-evaluate ready queue, PR backlog, impact graph, and current `master`; continuously integrate small validated changes rather than large speculative branches.
6. For any failure: classify transient / regressions / external auth or runner / policy conflict; preserve logs and evidence; retry bounded transient failure with exponential backoff, repair code or replan, escalate irrecoverable blocker without looping endlessly.
7. After merge, evaluate semantic versioning under existing release policy; release/deploy relevant increments and verify installed CLI, service endpoints and site only when authorized. Start using new fast workflows in Majordomus itself *after* canary success.
8. When completion is evidenced, produce report with before/after percentile distributions, selected gates, skipped-by-proof gates, CI links, PRs/SHAs, docs changes, rules/gates migrations, runtime usage and rollout state. If any critical criterion missing, report `INCOMPLETE` accurately.

### Operator interruption and durability

No need for conversational continuation when existing supervisor supports durable execution. If not implemented, add such support only by extending current mesh/session/intent infrastructure through an issue, not by claiming this prompt persists. During transition use committed checkpoints and restart instructions. A stopped session must not be silently reported as completed.

### Stage gates

- M01 audit proves current critical path; no fabricated baseline.
- M02 and M03 are safe to deploy as independent increments when evidence meets prerequisites.
- M04 validates case selection and deterministic derive before changing mandatory verdict policy.
- M05 doctrine/rule migration before turning on new automated path as default.
- M06 proves installed tooling + mesh consumer path with no special-case manual `proceed`.
- M07 documents only delivered/verified capabilities.
- M08 staged canary → verified rollout → release.
- M09 final regression proof and active use, with rollback if new CI yields incorrect verdicts.

Now run M01 and continue the dependency-driven plan without waiting for chat approval for authorized operations.
