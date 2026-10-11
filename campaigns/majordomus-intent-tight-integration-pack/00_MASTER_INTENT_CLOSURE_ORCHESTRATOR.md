# Mission: Close the Majordomus intent loop completely

You are working inside `prismatic-majordomus`.

The repository already has a partial intent model. Do not rebuild it.

Your mission is to turn the existing pieces into one tightly integrated, enforced, observable, closed-loop intent-driven development system.

## Starting facts to verify, not blindly trust

The current design is believed to include:

- typed intent record,
- statement,
- invariants,
- milestones,
- criteria,
- evidence links,
- derived stage,
- criterion status derived from evidence,
- `intent validate`,
- `intent preflight`,
- `intent coverage`,
- issues naming criteria served,
- typed gap records,
- typed critique records,
- refusal when work begins before required critique.

Believed missing or incomplete:

- session start loading/resolving intent,
- completion policy owning the "may finish" decision from intent satisfaction,
- first-class intent view in Cockpit,
- GitHub milestone/issue reconciliation with intent,
- desired outcome → draft plan derivation,
- actually executing the opposition pass,
- closed-loop reassessment when evidence/work changes.

Before implementation, prove which of these are still true on the current tree.

---

# Phase 1 — repository archaeology

Build an architecture map before changing behavior.

Map:

```text
concept
canonical type/source
schema
storage
loader
validator
CLI commands
capability declaration
API projection
MCP projection
Cockpit projection
tests
rules/doctrines
generated docs
```

for:

- intent,
- criterion,
- evidence,
- milestone,
- issue,
- gap,
- critique/opposition,
- session/task,
- completion/finish,
- GitHub sync,
- Cockpit,
- release/publication if intent is public.

Then classify every gap as:

- missing wiring,
- missing state transition,
- missing projection,
- duplicated logic,
- missing enforcement,
- missing UX,
- missing orchestration,
- schema deficiency.

Prefer wiring over invention.

---

# Phase 2 — architecture decision

Create a concise internal design note using the repository's current decision mechanism if required.

The architecture should satisfy:

```text
Intent
  ↓
IntentExecutionContext
  ↓
Session
  ↓
Evidence
  ↓
Satisfaction
  ↓
Completion
```

with:

```text
Intent
  ↔ Plan
  ↔ GitHub projection
  ↔ Cockpit/CLI/API/MCP projection
```

and:

```text
DesiredOutcome
  → IntentDraft
  → PlanDraft
  → Opposition
  → AcceptedPlan
```

Do not create parallel stores.

Use existing project-model/plan/session/evidence types where possible.

---

# Phase 3 — execute milestones 20–100

Implement all milestone prompts in order, unless archaeology proves one is already fully complete.

For an already-complete milestone:
- do not rewrite it,
- add missing cross-surface/enforcement proof if necessary,
- mark it closed with evidence.

For a partially complete milestone:
- migrate/finish it rather than layering another mechanism.

For a missing milestone:
- implement the smallest architecture that satisfies the invariant and integrates everywhere.

---

# Phase 4 — hardening

For each concern ensure:

- typed model,
- deterministic derivation,
- schema,
- validation,
- CLI,
- structured output,
- capability/API/OpenAPI/MCP where public,
- Cockpit where human-facing,
- docs,
- tests,
- rules,
- CI/gates,
- migration.

No consumer-specific truth.

---

# Phase 5 — end-to-end proof

Build an executable scenario that proves:

1. user states desired outcome,
2. Majordomus derives or creates draft intent,
3. criteria/invariants are explicit,
4. plan is derived,
5. opposition is executed,
6. plan is revised/accepted,
7. session starts and resolves the intent automatically,
8. issue/criteria mapping is loaded into session context,
9. evidence is produced,
10. one criterion remains unmet,
11. completion refuses,
12. new work/evidence resolves the criterion,
13. satisfaction becomes true,
14. completion succeeds,
15. GitHub projection reconciles,
16. Cockpit/CLI/API/MCP agree,
17. a changed revision can invalidate stale evidence and reopen/reassess.

Use fixtures where external services are not appropriate.
Use live GitHub only where the repository's integration test strategy permits it.

---

# Hard acceptance criteria

The task is incomplete if any of these are false:

- session automatically resolves intent when policy requires one;
- intent context is present in the session without human copy/paste;
- opposition is actually run or deterministically orchestrated;
- satisfaction has one canonical engine;
- completion consumes canonical satisfaction;
- GitHub state is reconciled, not independently authored;
- Cockpit has a useful first-class intent view;
- desired outcome can produce a plan draft;
- reassessment exists when evidence/revision changes;
- all public surfaces agree;
- blocking rules are wired and mutation-tested;
- docs no longer describe shipped functionality conditionally;
- zero-registration extensibility is preserved.

---

# Final report

Return:

## Before
What was really missing.

## Architecture
Canonical types, ownership and flow.

## Implemented
One section per closed milestone.

## Removed
Duplicated/legacy logic deleted.

## Enforcement
Rules/gates and mutation proof.

## Cross-surface
CLI/API/OpenAPI/MCP/Cockpit/docs agreement.

## E2E proof
Exact scenario and outputs.

## Remaining limitations
Only real limitations that still exist.

Do not call it complete if a sentence from the old "Partly built" description still truthfully applies.
