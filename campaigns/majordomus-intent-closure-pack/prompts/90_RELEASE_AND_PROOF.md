# Milestone 90: End-to-End Proof, Versioning, Release and Evidence

## Mission

Prove that the repository now implements a closed-loop intent-driven development lifecycle, then execute the repository's canonical version/release/deploy ceremony.

Do not paper over failing checks to get a release badge.

## 1. Full clean validation

Run the canonical repository equivalents of:

- formatting;
- compile/build;
- unit tests;
- integration/fixture tests;
- schema/model validation;
- governance checks;
- docs checks;
- generated-artifact drift checks;
- API/OpenAPI tests;
- MCP tests;
- Cockpit/UI tests;
- GitHub sync/reconciliation tests;
- performance checks required by policy;
- package/distribution tests.

Use the actual project commands discovered in repo (`just`, CLI, CI plan, etc.).

## 2. Required end-to-end intent scenario

Create a realistic isolated fixture/test and prove, through supported public/canonical interfaces, this flow:

```text
A. submit desired outcome
B. derive or create canonical intent
C. produce criteria + plan + issue coverage
D. run opposition
E. reject premature execution before valid opposition
F. start an intent-bound session after acceptance
G. execute/record work and evidence
H. refuse premature issue/intent completion while evidence is insufficient
I. add remaining valid evidence
J. derive criterion satisfaction
K. derive intent completion
L. reconcile GitHub projection idempotently
M. observe identical canonical state via CLI structured output/API/MCP/Cockpit data
N. finish session with no unresolved required intent obligations
```

The test may use fakes for external providers/GitHub but must exercise real Majordomus domain/lifecycle code.

## 3. Negative proof

Add or run tests that demonstrate all of these do **not** falsely complete an intent:

- all issues closed but criterion evidence missing;
- GitHub milestone closed manually;
- critique file exists but opposition disposition invalid/stale;
- evidence belongs to old revision;
- criterion has no covering work after plan mutation;
- frontend/API caller sends a manual `complete=true` style mutation if any such path exists.

## 4. Documentation truth pass

Search docs/site/README for statements describing intent functionality as future/conditional/partial. Update only those actually implemented.

Also search for overclaims. Every major claim should map to executable evidence/tests or clearly state limitations.

## 5. Versioning

Follow the repository's canonical versioning policy exactly. If completed sessions/merges trigger automatic version changes, use that system rather than editing versions manually.

Validate version consistency across all exposed surfaces.

## 6. Release/package/deploy

Execute the repository-prescribed release path where credentials/environment allow:

- package/build artifacts;
- release validation;
- publish/tag/release if policy permits;
- deploy site/Cockpit/docs if part of normal release;
- verify deployed endpoints using canonical checks.

If external credentials or infrastructure block a step, record the exact blocked command/status and complete everything else. Never report a deployment you could not verify.

## 7. Repository cleanup

Before final report:

- inspect git diff/status;
- remove temporary/debug files;
- ensure generated files are current;
- ensure no secrets/local absolute paths were added;
- ensure no abandoned branches/worktrees/PRs were created by this run beyond policy;
- reconcile planning artifacts/issues/milestones;
- ensure the pack's work is represented as completed evidence, not just prose.

## 8. Final evidence report

Provide:

### Starting state verified
What was truly missing versus already implemented.

### Closed-loop architecture
Exact canonical owners for intent, plan, opposition, session binding, evidence, satisfaction, completion and reconciliation.

### Proof
The end-to-end scenario and its test/command results.

### Enforcement
Which validators/rules/doctrines/tests prevent regression.

### Surfaces
CLI/API/OpenAPI/MCP/Cockpit/docs consistency.

### Migration
Legacy paths/data removed or migrated.

### Release
Version, commit/tag/release/deploy verification as applicable.

### Remaining debt
Only genuine external blockers or deliberately deferred nonessential enhancements.

## Final acceptance criteria

The work is finished only if this statement is demonstrably true:

> Majordomus can take a desired outcome, bind it to a canonical intent, derive or validate a plan, execute an opposition pass, run intent-bound work, collect admissible evidence, derive criterion and intent satisfaction, refuse false completion, reconcile external projections, and expose the same state across its supported surfaces under executable governance.
