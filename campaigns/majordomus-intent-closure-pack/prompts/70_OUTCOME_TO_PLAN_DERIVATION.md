# Milestone 70: Derive a Reviewable Intent and Plan from a Desired Outcome

## Mission

Add the layer above the typed intent record: accept a desired outcome, discover relevant repository context, and produce a canonical *draft* intent plus plan that must pass validation and opposition before execution.

This is assisted derivation, not permission for an LLM to mutate the repository with an unreviewed hallucinated backlog.

## 1. Audit reusable context/planning machinery

Reuse existing:

- context discovery/compiler;
- knowledge/ADR/session history;
- issue/milestone schemas;
- DAG/dependency/wave planner;
- provider/capability routing;
- prompt/workflow system;
- intent validation/coverage;
- opposition pass.

## 2. Desired outcome input

Define/reuse a typed request carrying the minimum necessary information:

- desired outcome/statement;
- optional constraints/invariants from caller;
- scope/project/workspace;
- optional explicit evidence expectations;
- provenance/requester/session.

Keep the natural-language outcome intact as source material; derived structure is separate.

## 3. Context discovery

Before planning, compile relevant facts from canonical repository sources. The planner must reason from current code/tests/docs/ADRs/issues/session knowledge rather than only the user's sentence.

Record which context sources materially informed derivation.

## 4. Draft intent derivation

Produce a typed draft containing:

- normalized statement;
- invariants;
- measurable acceptance criteria;
- evidence expectations where inferable;
- identified gaps/assumptions;
- provenance and confidence/uncertainty only if repository schemas support such concepts.

Do not weaken criteria into vague "works correctly" statements when stronger repository-derived criteria are available.

## 5. Plan decomposition

Derive/reuse typed planning structure:

- milestones;
- issues/work items;
- criterion coverage;
- dependencies;
- execution waves;
- likely verification/evidence work.

Every work item must explain which criterion/gap it serves. Reject orphan work.

## 6. Validation before persistence/execution

The draft must pass canonical:

- schema/model validation;
- link validation;
- criterion coverage;
- dependency/DAG validation;
- scope/invariant checks;
- opposition pass.

Only then can it become an executable accepted plan.

## 7. Human/autonomous modes

Respect existing execution profiles. In interactive mode, present the draft/review cleanly. In autonomous mode, allow policy-approved acceptance after opposition and validation, with full provenance.

Do not require user confirmation for every trivial generated issue when autonomous profile explicitly authorizes execution.

## 8. Revision loop

When opposition rejects the plan, feed required changes back into plan derivation and revalidate. Bound loops according to existing agent/orchestration safety limits and fail with useful diagnostics rather than infinite self-argument.

## 9. Tests

Use deterministic/fake provider fixtures where appropriate to prove:

- desired outcome → valid draft intent;
- context sources are included;
- generated work covers criteria;
- orphan issue rejected;
- invalid DAG rejected;
- opposition rejection causes revision;
- accepted plan becomes executable;
- provider absence/failure is handled according to configured profile;
- existing manually-authored intents still work.

## 10. CLI/API/MCP surface

Integrate derivation through existing canonical interaction surfaces. Avoid duplicating separate planning semantics per surface.

Names should follow current command/API conventions rather than literal prompt wording.

## Acceptance criteria

- a desired outcome can become a validated draft intent and plan using repository context;
- every issue traces to criteria/gaps;
- the plan passes opposition before execution;
- autonomous and interactive profiles behave according to existing policy;
- manually authored intents remain supported;
- derivation is observable, testable and auditable.
