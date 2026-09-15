# Prompt 04 — Import Doctrines, Connect Rules, Make Them Enforceable

Read the audit and previous implementation.

Import/adapt approved doctrines.

A doctrine that exists only as prose and is silently violated by the repository is decorative mythology. Do not ship decorative mythology.

## For every selected doctrine

Determine:

- normative statement;
- scope;
- rationale;
- enforceable invariants;
- non-enforceable guidance;
- related skills;
- existing target rules/gates;
- needed schema constraints;
- needed validator/lint;
- migration required;
- exceptions, if legitimate;
- explainability/diagnostics.

## Doctrine vs rule

Respect Majordomus' existing semantics for doctrine and rule.

Do not collapse them if the repository treats them differently.

Where appropriate:

```text
doctrine
  ↓ defines architectural invariant
rule(s)
  ↓ make concrete checks
validator/gate
  ↓ executes checks
diagnostic
  ↓ explains failures
docs/UI
  ↓ expose the same canonical relation
```

## Highest-value donor ideas

Pay special attention to reusable donor governance around:

- single source of truth;
- derived/generated inventories;
- documentation completeness;
- schema/front matter requirements;
- session/handover continuity;
- ADR/knowledge capture;
- test/documentation requirements;
- public API documentation;
- deterministic generation;
- cross-surface parity;
- repository hygiene;
- ownership/provenance;
- anti-duplication;
- agent coordination;
- worktree discipline;

but import only what Prompt 01 proved exists and is relevant.

## Enforcement

Connect each enforceable invariant to the target's canonical check pipeline.

Prefer:

- typed validation;
- schema validation;
- structural checks;
- compile-time guarantees;
- repository-aware lints;
- drift checks.

Avoid:

- regex-only enforcement where parsing/schema exists;
- independent shell scripts;
- duplicate CI workflows;
- checks that only run manually.

## Retrospective correctness

Do not grandfather current violations forever.

Fix existing in-scope legacy state or add an explicit migration with a bounded compatibility path if immediate repair is technically unsafe.

All new rules must have positive and negative tests.

Error messages must tell a developer what to fix.
