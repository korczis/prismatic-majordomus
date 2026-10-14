# Milestone 2 — Bind sessions to intent at start

Goal: when work is intent-governed, the session automatically resolves and loads the governing intent and criterion context.

## Required behavior

At session/task start:

1. resolve the current issue/work item,
2. resolve whether it serves an intent,
3. resolve the exact criteria served,
4. load the governing intent,
5. load invariants,
6. load current criterion/evidence state,
7. record the resolved intent binding in session/task state,
8. include a bounded intent briefing in the session context,
9. fail preflight if intent linkage is mandatory but unresolved,
10. allow explicit typed maintenance/exemption paths only according to policy.

No human should have to copy the intent into a prompt.

## Canonical data

Do not duplicate the intent record into session state.

Session state should hold a typed binding/reference plus the minimal snapshot/provenance needed to know what was resolved at start.

Conceptually:

```text
SessionIntentBinding
  intent_id
  criteria_served[]
  resolved_at_revision
  resolution_reason
  source_issue
```

Use actual repository naming.

## Drift semantics

If the intent or issue mapping changes after session start:
- preserve what was resolved at start as evidence/history,
- expose current drift,
- decide according to existing session/current-state semantics whether check/finish requires re-resolution.

Do not silently mutate history.

## Context integration

The session briefing should include:
- intent statement,
- relevant invariants,
- criteria served by this work,
- current satisfaction/evidence summary,
- plan/milestone identity,
- explicit blockers.

Respect context budget.
Do not dump the whole intent universe.

## CLI/API/MCP

Expose the binding through existing structured session/task projections where appropriate.

No special transport-only models.

## Cockpit

If session details exist, show the governing intent/criteria as links, but do not build the full intent UI here.

## Tests

Cover:
- one issue → one intent,
- one issue → multiple criteria,
- maintenance exemption,
- missing intent,
- missing criterion,
- changed mapping after start,
- session resume/recovery,
- handover retains intent reference,
- fresh clone/portable behavior.

## Enforcement

Update the appropriate rule/doctrine:
intent-governed work starts only with a resolved intent binding or explicit typed exemption.

Mutation-test the wiring.

## Acceptance criteria

Starting an intent-governed session automatically creates an explainable binding and loads relevant intent context.
