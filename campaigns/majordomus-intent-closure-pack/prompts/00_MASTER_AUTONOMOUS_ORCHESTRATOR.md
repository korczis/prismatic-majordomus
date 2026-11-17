# MASTER: Close the Majordomus Intent Loop End-to-End

You are Claude Code Opus 5.5 operating inside the current `prismatic-majordomus` repository.

Your job is to finish the existing intent subsystem into a production-quality, closed-loop intent-driven development system. Work autonomously until the end-state is implemented, migrated, tested, documented, enforced and proven.

Do not ask the user for routine implementation choices. Discover repository conventions and decide accordingly. If a genuine product ambiguity exists, choose the narrowest behavior consistent with existing doctrines and leave explicit evidence in the final report.

## 1. Ground truth before edits

Read and obey all applicable repository governance, including discovered equivalents of:

- `AGENTS.md` / agent instructions;
- `.ai/**` and `.majordomus/**`;
- doctrines, rules, ADRs, schemas and generated registries;
- session lifecycle and execution-plane code;
- issue/milestone/project model;
- intent model, intent validation, intent preflight and intent coverage;
- gap/critique records;
- evidence ledger;
- completion/finish policy;
- GitHub sync/reconciliation;
- API/OpenAPI/MCP;
- Cockpit/site/docs;
- tests and canonical quality gates;
- versioning/release/deploy rules.

Audit the **current branch**, not historical assumptions. The user-provided starting observation is:

- typed intent record exists;
- stage and criterion state are at least partly derived;
- broken intent links are rejected;
- issues name criteria they serve;
- coverage can explain which work carries which criterion;
- uncovered criterion situations are rejected in at least some cases;
- gaps and critiques are typed;
- execution that starts before a critique can be rejected;
- but sessions do not yet load intent as execution context;
- completion policy does not fully decide satisfaction;
- Cockpit lacks a complete intent projection;
- GitHub milestones are not fully reconciled against intents;
- desired outcomes do not yet derive plans;
- opposition is represented but not actually orchestrated/executed.

Verify every bullet against the current repository. If the repo has advanced beyond it, treat existing functionality as done only after tests/proof.

## 2. Use this prompt pack as the plan

Treat each sibling file as one milestone and execute them in this order:

1. `10_SESSION_INTENT_BINDING.md`
2. `20_EXECUTED_OPPOSITION_PASS.md`
3. `30_SATISFACTION_ENGINE.md`
4. `40_COMPLETION_POLICY.md`
5. `50_GITHUB_RECONCILIATION.md`
6. `60_COCKPIT_INTENT_PROJECTION.md`
7. `70_OUTCOME_TO_PLAN_DERIVATION.md`
8. `80_CROSS_SURFACE_HARDENING.md`
9. `90_RELEASE_AND_PROOF.md`

Use Majordomus itself to represent these as milestones/issues if the current repository supports doing so cleanly. Do not create duplicate planning state merely to satisfy this instruction.

## 3. Autonomous execution protocol

For each milestone:

1. inspect relevant code/tests/docs;
2. write or update the canonical plan/issue records;
3. identify the minimal vertical slice;
4. perform the opposition/critique step required by the repository;
5. implement;
6. run focused tests immediately;
7. integrate affected projections;
8. add regression gates;
9. update documentation/ADRs only where policy requires;
10. run broader validation;
11. close/reconcile issue state only when evidence exists;
12. continue to the next milestone without waiting for user approval.

When an implementation choice is uncertain, consult available local context first. If Majordomus supports multi-provider consultation and providers are available, use it under existing policy. The solution must still work when optional providers are absent.

## 4. Architectural law

The intended control loop is:

```text
desired outcome
      ↓
Intent
      ↓
context/gap discovery
      ↓
plan (milestones/issues/dependencies/waves)
      ↓
executed opposition
      ↓
accepted/revised plan
      ↓
intent-bound session execution
      ↓
evidence
      ↓
criterion satisfaction
      ↓
intent reassessment
      ├─ incomplete → revise/derive work
      └─ satisfied → complete
```

Do not let GitHub, Cockpit, CLI or Markdown become a separate authority. They are projections/interaction surfaces over canonical typed project state.

## 5. Hard invariants to land and enforce

By the end, enforce equivalents of:

> No executable work without an intent or a typed policy-approved exemption.

> Work under an intent must name the criteria it advances.

> Intent work must not begin before a real opposition/critique pass has completed and its disposition has been accepted.

> Criteria are satisfied from evidence under canonical evaluation policy, not issue closure or a hand-edited boolean.

> Intent completion is derived from required criteria, invariants and admissible evidence.

> Session completion cannot claim success while required intent obligations remain unsatisfied unless the session is explicitly partial and hands over the remaining obligations.

> GitHub issues/milestones reflect canonical intent planning state and drift is detectable/reconcilable.

> All surfaces expose the same intent facts.

## 6. Scope control

Do not wander into unrelated refactors. Fix adjacent architecture only when it blocks canonical integration or leaves a bypass around the new invariant.

Prefer deleting duplicate mechanisms over adding adapters.

Prefer extending current typed models and registries over creating new configuration formats.

Prefer deterministic code for lifecycle decisions; use LLMs for semantic derivation/critique where appropriate, always recording provenance and allowing validation.

## 7. Testing strategy

Require at least:

- model/unit tests;
- invalid-state tests;
- lifecycle integration tests;
- repository fixture tests;
- cross-surface contract tests where surfaces expose intent state;
- migration/legacy compatibility tests where needed;
- one full end-to-end scenario proving the closed loop.

Tests must exercise failure paths, especially premature work start and premature completion.

## 8. Completion

Do not call this finished because code compiles.

The run is complete only when `90_RELEASE_AND_PROOF.md` passes and the repository's canonical release/version/deploy policy has been followed for the resulting version, to the extent credentials/infrastructure are available.

If an external dependency prevents publication/deploy, finish every local/repository step, record exact blocker evidence, and do not fabricate deployment success.

## 9. Final report

Return one concise report with:

- verified starting state;
- milestones completed;
- canonical architecture after changes;
- tests/gates executed with results;
- migrations performed;
- GitHub reconciliation behavior;
- Cockpit/API/MCP behavior;
- exact proof of closed-loop intent execution;
- version/release/deploy result;
- genuine remaining debt only.

Do not stop after analysis. Implement the system.
