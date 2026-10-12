# Milestone 7: Explainability, Provenance, Evidence, and "Why?" UX

Make important Majordomus states explainable in the UI.

## Principle

A status without derivation is decoration.

Users should be able to understand why Majordomus believes something is:

- blocked
- complete
- failing
- stale
- missing evidence
- healthy
- unavailable

## Canonical explainability

Reuse existing explain/provenance/evidence infrastructure if present.

Otherwise introduce the smallest typed projection necessary.

Conceptually:

```text
Status
  -> derived from criterion
  -> linked issue
  -> evidence requirement
  -> missing executable proof
```

## UI interaction

Provide contextual "Why?" / "Explain" affordances on meaningful states.

The result should show:

- canonical state
- derivation chain
- relevant source entities
- evidence
- missing requirements
- actionable next step where derivable

## Provenance

Expose provenance where supported:

- generated from
- observed from
- linked from
- validated by
- superseded by

Do not invent provenance that the backend cannot prove.

## Evidence UX

Evidence should not be a raw file list.

Where supported, group evidence by criterion/claim and show satisfaction status.

## Tests

Add tests proving explainability output matches canonical decision logic.

If completion logic says blocked, UI explanation must identify the same root cause as CLI/API diagnostics.

## Acceptance criteria

- important statuses expose explainability
- explanation is generated from canonical logic, not frontend text heuristics
- evidence/provenance relations are navigable
- UI and CLI/API agree on causes
