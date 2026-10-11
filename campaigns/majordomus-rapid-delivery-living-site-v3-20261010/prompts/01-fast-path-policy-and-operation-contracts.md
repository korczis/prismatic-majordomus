# Milestone RDV3-01: Fast path policy and operation contracts

**Intent:** Shorten the actual verified delivery cycle while keeping the public product story current, useful, and demonstrably true.

**Prior art:** Existing Autonomous Delivery V2 prompts, actual current PRs, existing .ai and site derived features.

## Absolute architectural constraints
- Start with live repository discovery: read AGENTS.md, .ai rules/doctrines, canonical intents, milestone/issue schemas, CI manifests, derived generators, site stack and deployment configs. Do not assume paths or interfaces.
- This pack EXTENDS the existing Autonomous Delivery V2 program. Reconcile intent/milestone/issue identities and never duplicate a functioning subsystem or open PR.
- One canonical source; typed schemas; inferred/derived projections; deterministic output; generated docs and executable gates. Delete replaced legacy paths.
- Operate through existing mesh and PR lifecycle; persist check-pointed evidence; avoid relying on a chat session to continue. Never simulate worker completion or claim deployments without live verification.
- Keep published release integrity, branch protection, secrets, delegated repository/environment permissions and required quality gates. Runtime production deploy is allowed only inside pre-authorized environment scope. Pause that side effect on missing authority, continue unrelated work.
- Minimize unnecessary full builds and redundant derives, but do not weaken risk-appropriate required tests; machine-check cache validity and trigger full suite for contract/release changes.
- Follow repo's actual version analyzer and bump procedure. Never hardcode a version, never bump without eligibility, never pretend a release was published.
- Acceptance criteria are objectively evidenced via exact commit SHA, command and result, generated artifacts, CI links, deployment revision and health checks.

## Milestone exit gates
- All issues verified and integrated or explicitly externally blocked, dependencies reconciled.
- Release/deploy eligibility evaluated per integrated slice, without waiting for other milestones unnecessarily.
- Actual p50/p95 or reproducible timing evidence attached where performance is relevant.
- Live site updated and verified only for truthful deployed capabilities; no fabricated product claims.

## Issue RDV3-01-01: Identify governance rules that cause reauthorization or redundant work

**Outcome:** Identify governance rules that cause reauthorization or redundant work. Deliver the verified result in the current repository, not a design-only writeup.

**Dependency:** live M00 evidence and current V2 dependencies

### Implementation sequence
1. Discover current source-of-truth types, consumers, existing tests, gates, execution and deployment procedures relevant to this issue.
2. Capture a failing characterization test, evidence or measured baseline before changing behavior; reproduce rather than theorize.
3. Choose the minimal canonical owner and typed contract, record migration from old rules/models if any.
4. Implement an end-to-end vertical slice, reusing existing registries, derive machinery, mesh supervisor and validation gates.
5. Add unit, integration, adversarial/failure and cross-surface tests proportionate to risk; verify negative paths fail closed.
6. Generate or update docs and any landing-page projections from canonical data, not pasted manual inventories.
7. Run targeted validation, then relevant release gates. Integrate PR with latest base, rebase safely, merge only with verified approvals/checks.
8. Analyze version eligibility, bump via canonical tool if warranted, release/deploy authorized changed surfaces and verify deployed SHA/version; otherwise record why version-neutral.
9. Attach actual evidence and performance delta to canonical issue, advance next ready task autonomously.

### Machine-checkable acceptance contract
- Before and after behavior is demonstrated on representative inputs, including a failure or stale-data condition.
- No newly duplicated source of truth, no orphan issue, no unverifiable endpoint or hardcoded inventory.
- All affected CLI/API/MCP/UI/site/documentation projections are either tested or explicitly proved unaffected.
- The issue is only complete after tests/gates, code integration, traceable evidence, docs impact and release/deploy decision are recorded.
- Where physical rollout is unavailable, status is `blocked_external` with exact failed action and prerequisites, never `done`.

### Tracking fields
`intent_id`, `milestone_id`, `issue_id`, `scope`, `dependencies`, `worker_id`, `run_id`, `base_sha`, `head_sha`, `pr`, `tests`, `gate_verdict`, `derive_hash`, `site_baseline_sha`, `site_delta`, `release_analysis`, `version`, `deployment_sha`, `health_proof`, `latency_before_ms`, `latency_after_ms`, `decision`, `blocker`, `next_action`.

## Issue RDV3-01-02: Define intent-backed unattended delivery authority envelope

**Outcome:** Define intent-backed unattended delivery authority envelope. Deliver the verified result in the current repository, not a design-only writeup.

**Dependency:** RDV3-01-01

### Implementation sequence
1. Discover current source-of-truth types, consumers, existing tests, gates, execution and deployment procedures relevant to this issue.
2. Capture a failing characterization test, evidence or measured baseline before changing behavior; reproduce rather than theorize.
3. Choose the minimal canonical owner and typed contract, record migration from old rules/models if any.
4. Implement an end-to-end vertical slice, reusing existing registries, derive machinery, mesh supervisor and validation gates.
5. Add unit, integration, adversarial/failure and cross-surface tests proportionate to risk; verify negative paths fail closed.
6. Generate or update docs and any landing-page projections from canonical data, not pasted manual inventories.
7. Run targeted validation, then relevant release gates. Integrate PR with latest base, rebase safely, merge only with verified approvals/checks.
8. Analyze version eligibility, bump via canonical tool if warranted, release/deploy authorized changed surfaces and verify deployed SHA/version; otherwise record why version-neutral.
9. Attach actual evidence and performance delta to canonical issue, advance next ready task autonomously.

### Machine-checkable acceptance contract
- Before and after behavior is demonstrated on representative inputs, including a failure or stale-data condition.
- No newly duplicated source of truth, no orphan issue, no unverifiable endpoint or hardcoded inventory.
- All affected CLI/API/MCP/UI/site/documentation projections are either tested or explicitly proved unaffected.
- The issue is only complete after tests/gates, code integration, traceable evidence, docs impact and release/deploy decision are recorded.
- Where physical rollout is unavailable, status is `blocked_external` with exact failed action and prerequisites, never `done`.

### Tracking fields
`intent_id`, `milestone_id`, `issue_id`, `scope`, `dependencies`, `worker_id`, `run_id`, `base_sha`, `head_sha`, `pr`, `tests`, `gate_verdict`, `derive_hash`, `site_baseline_sha`, `site_delta`, `release_analysis`, `version`, `deployment_sha`, `health_proof`, `latency_before_ms`, `latency_after_ms`, `decision`, `blocker`, `next_action`.

## Issue RDV3-01-03: Make authorized continuation default across handovers and mesh workers

**Outcome:** Make authorized continuation default across handovers and mesh workers. Deliver the verified result in the current repository, not a design-only writeup.

**Dependency:** RDV3-01-02

### Implementation sequence
1. Discover current source-of-truth types, consumers, existing tests, gates, execution and deployment procedures relevant to this issue.
2. Capture a failing characterization test, evidence or measured baseline before changing behavior; reproduce rather than theorize.
3. Choose the minimal canonical owner and typed contract, record migration from old rules/models if any.
4. Implement an end-to-end vertical slice, reusing existing registries, derive machinery, mesh supervisor and validation gates.
5. Add unit, integration, adversarial/failure and cross-surface tests proportionate to risk; verify negative paths fail closed.
6. Generate or update docs and any landing-page projections from canonical data, not pasted manual inventories.
7. Run targeted validation, then relevant release gates. Integrate PR with latest base, rebase safely, merge only with verified approvals/checks.
8. Analyze version eligibility, bump via canonical tool if warranted, release/deploy authorized changed surfaces and verify deployed SHA/version; otherwise record why version-neutral.
9. Attach actual evidence and performance delta to canonical issue, advance next ready task autonomously.

### Machine-checkable acceptance contract
- Before and after behavior is demonstrated on representative inputs, including a failure or stale-data condition.
- No newly duplicated source of truth, no orphan issue, no unverifiable endpoint or hardcoded inventory.
- All affected CLI/API/MCP/UI/site/documentation projections are either tested or explicitly proved unaffected.
- The issue is only complete after tests/gates, code integration, traceable evidence, docs impact and release/deploy decision are recorded.
- Where physical rollout is unavailable, status is `blocked_external` with exact failed action and prerequisites, never `done`.

### Tracking fields
`intent_id`, `milestone_id`, `issue_id`, `scope`, `dependencies`, `worker_id`, `run_id`, `base_sha`, `head_sha`, `pr`, `tests`, `gate_verdict`, `derive_hash`, `site_baseline_sha`, `site_delta`, `release_analysis`, `version`, `deployment_sha`, `health_proof`, `latency_before_ms`, `latency_after_ms`, `decision`, `blocker`, `next_action`.

## Issue RDV3-01-04: Replace rigid scope locks with auditable bounded scope evolution

**Outcome:** Replace rigid scope locks with auditable bounded scope evolution. Deliver the verified result in the current repository, not a design-only writeup.

**Dependency:** RDV3-01-03

### Implementation sequence
1. Discover current source-of-truth types, consumers, existing tests, gates, execution and deployment procedures relevant to this issue.
2. Capture a failing characterization test, evidence or measured baseline before changing behavior; reproduce rather than theorize.
3. Choose the minimal canonical owner and typed contract, record migration from old rules/models if any.
4. Implement an end-to-end vertical slice, reusing existing registries, derive machinery, mesh supervisor and validation gates.
5. Add unit, integration, adversarial/failure and cross-surface tests proportionate to risk; verify negative paths fail closed.
6. Generate or update docs and any landing-page projections from canonical data, not pasted manual inventories.
7. Run targeted validation, then relevant release gates. Integrate PR with latest base, rebase safely, merge only with verified approvals/checks.
8. Analyze version eligibility, bump via canonical tool if warranted, release/deploy authorized changed surfaces and verify deployed SHA/version; otherwise record why version-neutral.
9. Attach actual evidence and performance delta to canonical issue, advance next ready task autonomously.

### Machine-checkable acceptance contract
- Before and after behavior is demonstrated on representative inputs, including a failure or stale-data condition.
- No newly duplicated source of truth, no orphan issue, no unverifiable endpoint or hardcoded inventory.
- All affected CLI/API/MCP/UI/site/documentation projections are either tested or explicitly proved unaffected.
- The issue is only complete after tests/gates, code integration, traceable evidence, docs impact and release/deploy decision are recorded.
- Where physical rollout is unavailable, status is `blocked_external` with exact failed action and prerequisites, never `done`.

### Tracking fields
`intent_id`, `milestone_id`, `issue_id`, `scope`, `dependencies`, `worker_id`, `run_id`, `base_sha`, `head_sha`, `pr`, `tests`, `gate_verdict`, `derive_hash`, `site_baseline_sha`, `site_delta`, `release_analysis`, `version`, `deployment_sha`, `health_proof`, `latency_before_ms`, `latency_after_ms`, `decision`, `blocker`, `next_action`.

## Issue RDV3-01-05: Preserve immutable safety, secrets, provenance and release constraints

**Outcome:** Preserve immutable safety, secrets, provenance and release constraints. Deliver the verified result in the current repository, not a design-only writeup.

**Dependency:** RDV3-01-04

### Implementation sequence
1. Discover current source-of-truth types, consumers, existing tests, gates, execution and deployment procedures relevant to this issue.
2. Capture a failing characterization test, evidence or measured baseline before changing behavior; reproduce rather than theorize.
3. Choose the minimal canonical owner and typed contract, record migration from old rules/models if any.
4. Implement an end-to-end vertical slice, reusing existing registries, derive machinery, mesh supervisor and validation gates.
5. Add unit, integration, adversarial/failure and cross-surface tests proportionate to risk; verify negative paths fail closed.
6. Generate or update docs and any landing-page projections from canonical data, not pasted manual inventories.
7. Run targeted validation, then relevant release gates. Integrate PR with latest base, rebase safely, merge only with verified approvals/checks.
8. Analyze version eligibility, bump via canonical tool if warranted, release/deploy authorized changed surfaces and verify deployed SHA/version; otherwise record why version-neutral.
9. Attach actual evidence and performance delta to canonical issue, advance next ready task autonomously.

### Machine-checkable acceptance contract
- Before and after behavior is demonstrated on representative inputs, including a failure or stale-data condition.
- No newly duplicated source of truth, no orphan issue, no unverifiable endpoint or hardcoded inventory.
- All affected CLI/API/MCP/UI/site/documentation projections are either tested or explicitly proved unaffected.
- The issue is only complete after tests/gates, code integration, traceable evidence, docs impact and release/deploy decision are recorded.
- Where physical rollout is unavailable, status is `blocked_external` with exact failed action and prerequisites, never `done`.

### Tracking fields
`intent_id`, `milestone_id`, `issue_id`, `scope`, `dependencies`, `worker_id`, `run_id`, `base_sha`, `head_sha`, `pr`, `tests`, `gate_verdict`, `derive_hash`, `site_baseline_sha`, `site_delta`, `release_analysis`, `version`, `deployment_sha`, `health_proof`, `latency_before_ms`, `latency_after_ms`, `decision`, `blocker`, `next_action`.

## Issue RDV3-01-06: Implement policy migration, negative tests and canonical documentation

**Outcome:** Implement policy migration, negative tests and canonical documentation. Deliver the verified result in the current repository, not a design-only writeup.

**Dependency:** RDV3-01-05

### Implementation sequence
1. Discover current source-of-truth types, consumers, existing tests, gates, execution and deployment procedures relevant to this issue.
2. Capture a failing characterization test, evidence or measured baseline before changing behavior; reproduce rather than theorize.
3. Choose the minimal canonical owner and typed contract, record migration from old rules/models if any.
4. Implement an end-to-end vertical slice, reusing existing registries, derive machinery, mesh supervisor and validation gates.
5. Add unit, integration, adversarial/failure and cross-surface tests proportionate to risk; verify negative paths fail closed.
6. Generate or update docs and any landing-page projections from canonical data, not pasted manual inventories.
7. Run targeted validation, then relevant release gates. Integrate PR with latest base, rebase safely, merge only with verified approvals/checks.
8. Analyze version eligibility, bump via canonical tool if warranted, release/deploy authorized changed surfaces and verify deployed SHA/version; otherwise record why version-neutral.
9. Attach actual evidence and performance delta to canonical issue, advance next ready task autonomously.

### Machine-checkable acceptance contract
- Before and after behavior is demonstrated on representative inputs, including a failure or stale-data condition.
- No newly duplicated source of truth, no orphan issue, no unverifiable endpoint or hardcoded inventory.
- All affected CLI/API/MCP/UI/site/documentation projections are either tested or explicitly proved unaffected.
- The issue is only complete after tests/gates, code integration, traceable evidence, docs impact and release/deploy decision are recorded.
- Where physical rollout is unavailable, status is `blocked_external` with exact failed action and prerequisites, never `done`.

### Tracking fields
`intent_id`, `milestone_id`, `issue_id`, `scope`, `dependencies`, `worker_id`, `run_id`, `base_sha`, `head_sha`, `pr`, `tests`, `gate_verdict`, `derive_hash`, `site_baseline_sha`, `site_delta`, `release_analysis`, `version`, `deployment_sha`, `health_proof`, `latency_before_ms`, `latency_after_ms`, `decision`, `blocker`, `next_action`.
