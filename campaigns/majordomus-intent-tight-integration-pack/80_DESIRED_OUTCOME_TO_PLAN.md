# Milestone 8 — Derive a plan draft from a desired outcome

Goal: Majordomus can turn a desired outcome into a typed draft intent/plan that is reviewed before execution.

This is NOT an autonomous magic planner.
It is a structured derivation pipeline.

## Input

Introduce/reuse a typed `DesiredOutcome` concept only if current intent creation has no equivalent.

Input should minimally capture:
- desired outcome statement,
- optional constraints/invariants,
- scope/context,
- evidence/acceptance hints,
- provenance.

Avoid giant prompt blobs as the canonical record.

## Derivation pipeline

```text
DesiredOutcome
  ↓
context discovery
  ↓
gap analysis
  ↓
draft intent
  ↓
draft criteria/invariants
  ↓
draft milestones/issues/dependencies
  ↓
coverage check
  ↓
opposition pass
  ↓
accepted/revised plan
```

## Planner implementation

If an LLM/provider is available:
- use existing reasoning/advisor/provider architecture,
- require structured output validated against typed schema,
- retain provenance.

If no provider is available:
- deterministic/manual mode must still allow a human-authored draft to enter the same validation/opposition pipeline.

Majordomus correctness must not depend on one model vendor.

## Validation

Derived plan must satisfy existing:
- link validation,
- criterion coverage,
- issue purpose,
- dependency graph,
- scope,
- critique/opposition requirement.

No direct execution from unvalidated generated text.

## Incrementality

Support revision:
- draft v1,
- opposition findings,
- revised v2,
- accepted revision.

Do not overwrite history silently.

## CLI

Prefer commands conceptually like:
- intent propose/from-outcome,
- intent plan/draft,
but use existing command architecture and vocabulary.

Do not create redundant verbs if current commands can be extended.

## Cockpit

Expose draft/review state only after backend semantics exist.
Do not build a wizard that owns planning logic.

## Tests

Cover:
- deterministic/manual draft,
- structured provider output,
- malformed provider output,
- missing criterion coverage,
- cycle in dependencies,
- opposition revision,
- provider absent,
- idempotent regeneration where appropriate,
- provenance.

## Acceptance criteria

A desired outcome can enter Majordomus and emerge as a validated, critiqued, typed execution plan without manual creation of every low-level record.
