---
id: github-workgraph-phase-13
phase: 13
depends_on: [github-workgraph-phase-12]
goal: adversarial final review and proof
---

# Phase 13 — Adversarial Final Review, De-duplication, and Proof

Read the entire campaign contract and handovers.

## Objective

Assume the implementation is wrong until proven otherwise.

Do not add shiny features. Attack architecture, bypasses, drift, duplication and false claims.

## 1. Repository-wide duplicate search

Search for repeated:
- GitHub endpoint logic;
- issue/milestone DTOs;
- status enums;
- authority maps;
- route registries;
- CLI command registries;
- MCP tool registries;
- OpenAPI fragments;
- Cockpit category/status arrays;
- docs command tables;
- branch/issue parsing;
- external-ID mapping;
- reconciliation diff logic.

Delete or generate duplicates.

## 2. Bypass analysis

Try to:
- close a GitHub issue without acceptance evidence;
- mark canonical status manually;
- create managed feature work without issue mapping;
- mutate GitHub through an alternate code path that skips reconciler/audit;
- call API/MCP operation that bypasses application service;
- edit generated projection manually without drift detection;
- give one external ID to two canonical objects;
- create a dependency cycle;
- supply stale green check evidence;
- reconcile against the wrong repository.

The system should reject or explicitly classify each case.

## 3. Zero-registration proof

Add a representative test entity/capability according to project convention and prove it propagates through every relevant derived surface without manually editing equivalent inventories.

Remove the temporary fixture afterward unless it belongs as a permanent test fixture.

## 4. Determinism

Run relevant commands repeatedly and diff machine output.

Randomize fixture insertion/discovery order.

Ensure graph/reconciliation/serialization remains stable.

## 5. Idempotence

Run:
- generate twice;
- reconciliation plan twice;
- apply to converged fixture;
- migration twice.

Second run should produce no semantic changes.

## 6. Offline behavior

Disconnect/mock GitHub failure and run local development checks.

Ensure unrelated local work remains usable and remote unavailability is not reported as clean sync or empty remote.

## 7. Documentation truth audit

For every major docs claim, point to:
- implementation;
- test;
- capability;
- schema.

Remove aspirational claims that are not true.

## 8. Public surface audit

Verify:
- CLI help;
- JSON;
- REST;
- OpenAPI/Swagger;
- MCP;
- Cockpit;
- GitHub Pages/docs;
- completion if relevant.

No surface should have a contradictory state model or duplicate inventory.

## 9. Performance/security

Run relevant benchmarks/checks.
Search logs/tests/docs for secret-like patterns.
Verify redaction.

## 10. Full repository gates

Run canonical:
- format;
- lint;
- build;
- unit;
- integration;
- use-case impact/coverage;
- schema/generation drift;
- docs/site;
- Cockpit;
- E2E;
- doctor/check/finish-equivalent validation.

Use actual repository commands.

## Final report

Produce a concise but evidence-rich report:

### Root causes found
List architectural causes, not symptoms.

### Canonical architecture
Show:
`Work Graph → provider observation/projection → reconciler → application capabilities → projections`

### Authority table
Summarize field/entity authority.

### Traceability proof
Demonstrate both:
`milestone → issue → branch/worktree → commits → PR → evidence → completion`
and
`commit/PR → issue → milestone`.

### Reconciliation proof
Show dry-run, apply, verify, idempotent second run.

### Enforcement
Name rules/doctrines/policies/gates and tests that make regression fail.

### Surfaces
Show how one canonical capability appears through CLI/API/OpenAPI/MCP/Cockpit/docs.

### Legacy migration
What was adopted, removed, left unmanaged/ambiguous.

### Validation evidence
Exact commands and outcomes.

### Remaining debt
Only genuine constraints, with reason and next concrete step.

## Final acceptance checklist

- [ ] One typed canonical Work Graph exists.
- [ ] Milestones/outcomes and issues/execution contracts are related through typed edges.
- [ ] Readiness/blocking/completion are derived.
- [ ] Acceptance criteria have machine-addressable evidence.
- [ ] Git facts are derived from Git.
- [ ] GitHub facts are typed observations.
- [ ] External identities are stable and unambiguous.
- [ ] Field authority/provenance is explicit.
- [ ] Reconciliation is dry-runnable, deterministic, idempotent and explainable.
- [ ] Plan/apply are separate.
- [ ] Failure cannot masquerade as empty remote state.
- [ ] Branch/worktree/commit/PR traceability works in both directions.
- [ ] Green checks are bound to the correct SHA.
- [ ] Remote close/merge alone does not imply canonical completion.
- [ ] Rules/doctrines/policies are executable, not decorative.
- [ ] `check`/`doctor`/`finish` integrate appropriate invariants.
- [ ] CLI/API/OpenAPI/MCP/Cockpit consume the same application/domain semantics.
- [ ] No manual transport registries were introduced.
- [ ] Generated docs/site are synchronized.
- [ ] Legacy data in scope is migrated or explicitly classified.
- [ ] Offline deterministic tests cover core logic.
- [ ] Cross-surface/E2E tests exist.
- [ ] Secrets are redacted.
- [ ] CI runs the canonical gates.
- [ ] Release traceability is integrated where supported.
- [ ] Zero-registration extensibility is proven.
- [ ] Repository diff contains no obsolete duplicate machinery.
- [ ] Final handover/session state is complete.

Do not declare victory because the UI looks coherent. Prove the graph, the reconciliation, the enforcement and the reverse traceability.
