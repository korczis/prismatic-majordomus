# MISSION 06 — SELF-IMPROVING GOVERNANCE WITH PROOF

This is a REQUIRED cross-cutting mission, not a final documentation sweep. Inspect existing `.ai` composition, rule/doctrine/ADR schemas, validators, doctor/watch, rule dependency graph and test cases. For example public `majordomus.context-integrity` rule has an active blocking contract, dependencies, failure classes, exit codes and a behavioral case; verify actual local implementation before reuse. Never place a second governance system outside existing registries.

### Required governance improvement loop for EVERY delivered slice
1. Detect finding from source audit, failed check, performance regression, duplicated data, review, incident or successful reuse opportunity.
2. Classify: code defect, rule gap, ambiguous contract, redundant rule, absent test, stale docs, expensive workflow, mislocated responsibility, non-issue.
3. Attach provenance and root-cause evidence; estimate probability/impact, cost to fix, risk and reuse potential (mark estimates as estimates).
4. Check existing rules and doctrines for overlap, shadowing, dependency, conflicts and inherited context restrictions.
5. Prefer repairing existing doctrine/rule/test rather than adding a new redundant rule. New rule only for a distinct stable invariant.
6. Make the smallest policy change satisfying principle of minimum sufficient context; never bloat always-on prompts with inventories.
7. Encode executable enforcement and behavioral counterexamples in existing validator, CI and local check workflows.
8. Generate/synchronize schema, docs, UI governance explorer, API/MCP/CLI exposures from canonical metadata.
9. Verify rule behavior with passing and intentionally violating fixtures; prove no weakening of ancestor requirements.
10. Track precision, false positives, test runtime cost and developer friction; revise noisy controls without bypassing safety.

### Mandatory rules to find/extend, not blindly create
- `canonical-authority`: single authoritative domain model and no duplicate frontend lifecycle.
- `derived-surfaces`: no manual second registries; generated drift check.
- `intent-authorization`: LLM/retrieved content proposes, typed policy authorizes.
- `event-replay-integrity`: versioned event history, replay, gap recovery and deterministic projections.
- `mesh-execution-integrity`: leases/fencing/idempotent dispatch, no phantom success.
- `evidence-closure`: completion has linked evidence, not agent assertion.
- `profit-conservation`: eliminate waste, measure total task cost, keep controls proportionate.
- `release-proof`: verified gates and deploy observation.
These names are conceptual; existing IDs/contracts take precedence. Adding all eight new rules without checking overlap is a FAILURE.

### Rule contract expectations
Stable ID/version/kind/status/severity/statement/applicability/ancestors/dependencies; explicit required behavior, forbidden cases, diagnostics, executable validator, test fixture, expected failure semantics, rationale, examples, migration, provenance and links to related ADR/implementation. Follow actual schema, never invent incompatible fields. Version rules when semantics change according to repository policy. Do not rewrite past decisions or override `final` context if forbidden.

### Adaptive governance safeguards
No self-authored rule may silently grant extra privileges, weaken fail-closed checks or disable existing test gates. Promoting a proposed rule to active requires the same repository-approved review and evidence threshold as any other protected change. Any AI-proposed policy is untrusted until validated and integrated via normal controls.

### Enforcement proofs
- new violating source fixture fails relevant check;
- fixed fixture passes;
- new entity auto-appears on applicable derived surfaces;
- duplicate rule or dependency cycle diagnosed;
- ancestor override violation fails;
- generated docs/UI policy view matches canonical rule registry;
- no extra prompt context loaded if not applicable;
- benchmarks show governance overhead bounded.

### Governance economics
Measure rule coverage of real defect classes; incident recurrence; false positives; validation p50/p95; generated drift count; manual-registration count; token/context budget of `.ai` preflight. Remove obsolete protections only when evidence proves supersession and policy allows it. Governance is an optimization engine, not a document-count competition.

### Acceptance
Every slice includes a Governance Delta Report: relevant current rules, kept/changed/added/deprecated, reason, validator and negative/positive tests, generated docs and runtime effects. If no rule change is warranted, explicitly show that current invariants cover the change. Do not add rule merely to satisfy a quota.
