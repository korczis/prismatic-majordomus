# Execution Instructions

## 1. Agent posture

Act as a repository owner completing a migration, not as a feature contractor satisfying isolated prose. Inspect first. Use the repository's own governance. If `AGENTS.md`, `.ai/**`, ADRs, schemas, generated registries, CI scripts or canonical command inventories contradict this pack, determine the correct current invariant and document the discrepancy.

Never fabricate success. Distinguish:

```text
implemented
locally tested
repository-gate verified
committed
pushed
CI verified
deployed
runtime/site verified
```

These are separate states.

## 2. Beginning every run

At the start of every stage:

1. identify repository root and current branch/HEAD,
2. inspect `git status`, worktrees, unmerged branches/PR-related local state if available,
3. read `AGENTS.md` and any applicable nested agent instructions,
4. load relevant `.ai/rules`, doctrines, policies, ADRs, session context, handover and project plan,
5. inspect generated/canonical registries rather than trusting prose inventories,
6. run the narrowest relevant existing diagnostics before editing,
7. record the current failing obligations,
8. identify the canonical owner for every concept touched.

Do not ask the human for filenames or obvious architecture choices that can be derived from the repository.

## 3. Work loop

For every obligation use this loop:

```text
OBSERVE
  ↓
REPRODUCE
  ↓
ROOT CAUSE
  ↓
DEFINE CANONICAL OWNER
  ↓
IMPLEMENT
  ↓
MIGRATE / DELETE LEGACY PATH
  ↓
TEST UNIT + INTEGRATION + E2E + REGRESSION
  ↓
INTEGRATE CLI / API / OPENAPI / MCP / COCKPIT / DOCS AS APPLICABLE
  ↓
ADD/UPDATE ENFORCEMENT
  ↓
RUN GATES
  ↓
RE-INSPECT FOR DUPLICATION / DRIFT
  ↓
LAND + VERIFY
```

If a gate fails, fix the cause. Do not increase a debt baseline unless the repository's explicit policy proves that the increase is intentional and the user requested it. For the core convergence tasks in this pack, the target is generally to remove the baseline/debt, not normalize a higher number.

## 4. Scope discipline

Foundation first. During these stages do not add unrelated product features. New supporting abstractions are allowed only when they remove duplication or make a required invariant enforceable.

If you discover attractive adjacent work, record it as debt/issue unless it blocks closure.

## 5. Testing standard

For every changed/new production path:

- 100% changed-line coverage,
- 100% changed-branch coverage where instrumentation can measure it,
- behavioral test for every public contract,
- failure-mode tests,
- edge-case tests,
- regression test for each bug/root cause,
- cross-surface contract tests when the same canonical object is exposed in multiple surfaces,
- mutation/adversarial tests for enforcement wiring where practical.

Do not game coverage with implementation-coupled assertions. Tests should prove obligations.

For legacy code outside the touched surface, maintain or raise the repository-wide floor. Do not reduce quality to hit a number.

## 6. Rule/enforcement standard

A rule that can block work must have machine-readable enforcement metadata and executable evidence. The exact schema should follow repository conventions, but the semantic graph must be discoverable:

```text
rule / doctrine / policy
    ↓
validator(s)
    ↓
commands / gates that invoke validator
    ↓
test obligations
    ↓
last execution evidence
    ↓
affected surfaces
```

If some invariant genuinely requires human judgment, classify it honestly. Do not call it fully machine-enforced. The user's requested target for core repository invariants is no human-only blocking rule.

## 7. Surface standard

Canonical behavior must be accessible through all surfaces for which it is relevant. This does not mean every capability needs an identical UI button. It means there must be a single model/implementation with appropriate adapters.

As applicable verify:

- CLI invocation and structured output,
- HTTP API,
- generated OpenAPI/Swagger,
- MCP tools/resources/prompts,
- Cockpit inspection, invocation, validation and management,
- documentation/GitHub Pages,
- shell completion/just bridge,
- diagnostics/doctor,
- generated indexes.

No downstream manual registration when the repository's architecture can derive it.

## 8. Generated data

Generated files are outputs, not alternate authorities. Verify generated drift checks and regenerate only using canonical project commands.

Do not hand-edit generated artifacts unless repository policy explicitly requires it.

## 9. Git and landing

Keep commits cohesive and evidence-rich. Before pushing:

- repository gates pass,
- generated state clean,
- `git diff` reviewed,
- no secrets/local paths,
- no unrelated modifications,
- worktrees understood.

When credentials/network are available, push and verify the actual remote branch/CI/site/runtime. When they are unavailable, state the exact unverified boundary. Never claim “deployed” based only on a local build.

## 10. Stopping conditions

Do not stop at a convenient partial implementation.

A stage can stop only when:

- its acceptance criteria pass, or
- a genuine external blocker exists that cannot be solved from the repository/environment.

If externally blocked, complete every non-blocked part and write an exact handover using `templates/HANDOVER.md`, including reproduction commands and the single unresolved external dependency.

## 11. Evidence

Every stage must update an evidence report based on `templates/EVIDENCE_REPORT.md`. Prefer generated machine-readable evidence if the repo already has a mechanism for it.

The final stage must prove that no core convergence debt is merely hidden behind a baseline, disabled check, stale generated artifact, or unverified documentation claim.
