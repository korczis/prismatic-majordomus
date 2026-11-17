# PHASE 08 — END-TO-END DOGFOODING, OBSERVABILITY, ROLLOUT AND FINAL AUDIT

Pick ONE deliberately small, real, reversible improvement in the Majordomus repository with 2–3 acceptance criteria including a code change, test and documentation. Use it as golden intent. State exact baseline branch SHA, available peers, release/deploy targets and permissions. No artificial 'success' events. First run against fixtures/sandbox with deterministic fake GitHub and deploy adapters. Then live on real Majordomus dev environment with bounded auth/policy and real checks. Production deployment only if authorized under canonical policy. Show all mutations through UI and confirm CLI/API/MCP equivalence.

Scenario: input via UI -> canonical intent/criteria -> gap + critique + approved plan -> linked milestone/issues -> dependency scheduling -> mesh dispatch with exclusive claims and worktrees -> actual code/tests -> reviewed PR -> checked merge -> version selection + artifact -> deploy to authorized target -> smoke proof -> evidence ledger -> satisfied verdict, reflected in every surface. Force coordinator restart during one active task, lose a worker during another, inject duplicate event, make one CI check fail before repair, reject one unauthorized production action and test resumption.

Collect metrics into typed canonical observability events (existing telemetry system first): intent lead time, time in each stage, queue wait, work duration, issue throughput, PR WIP/age, merge/deploy frequency, retry/failed attempts, recovery latency, cross-surface projection lag, UI initial render p50/p95, streamed event backlog, cost and provider tokens if available. Instrument selectively; no metrics database parallel to existing architecture. Define SLOs after measurement and use data to set thresholds. Provide dashboards derived from same metrics. Track pre/post comparison.

Rollout strategy: feature flag/config via existing canonical configuration; enabled on project itself first; shadow/dry-run mode; then manual-approval bounded execution; then policy-scoped autonomous mode; no unbounded remote mutation. Version migrations, deprecation plan for duplicated legacy code, rollback if live verification fails. Resolve PR conflicts, sync master, check no untracked/generation drift, build, run focused tests + full gates, verify deploy and public site, inspect real generated docs. Keep partial deliverables mergeable and well-tested. Never declare total completion if remaining P0 blockers exist.

Acceptance checklist:
[ ] Intent entered only once through UI and canonical typed record created.
[ ] Planner produces dependency-correct, critiqued, evidence-aware plan.
[ ] Issues/milestones created/reconciled once under replay.
[ ] Mesh peers/claims/worktrees tracked without conflict or phantom ownership.
[ ] Real workers execute; streams are resumable and correctly correlated.
[ ] Restart/partition/duplicate event neither loses nor doubles work.
[ ] PR reviews and required CI are respected; merge receipt matches default branch.
[ ] Release version/artifact digest/source SHA attributable and reproducible.
[ ] Authorized deploy health/smoke verified; denied side effect stays denied.
[ ] Intent satisfaction derived from current criterion evidence, not button press.
[ ] CLI/REST/OpenAPI/MCP/UI/docs agree on IDs/status/provenance and links.
[ ] New canonical entity propagates without manually updating every consumer.
[ ] `.ai` machine gates and executable use cases reject introduced regressions.
[ ] Local/injected sample repo works; mobile UI tested and documented.
[ ] Release, deployment, docs and public site updated with verifiable claims.

Final report format: BASELINE (SHA/version and deficits); IMPLEMENTED (canonical sources, migrations, deleted duplication); PROOF (test commands/output and relevant artifacts); END-TO-END TRACE (intent ID -> issue IDs -> executions -> PRs -> merge SHA -> release -> deployment -> evidence); UI/DERIVATION matrix; OPERATIONS (recovery, permissions, performance); RELEASE/DEPLOY (exact results); REMAINING BLOCKERS (owner, proof, next safe action). Never substitute a list of files touched for behavioral proof.
