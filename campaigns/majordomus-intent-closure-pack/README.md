# Majordomus Intent Closure Pack

Purpose: finish the existing intent subsystem into a closed-loop, intent-driven execution model without creating a parallel architecture.

## Premise

The repository already has the important lower layer:

- typed intent records;
- statement, invariants, milestones, acceptance criteria and evidence links;
- derived intent stage;
- `intent validate` link validation;
- `intent preflight` issue-to-intent/criterion checks;
- `intent coverage` work-to-criterion coverage;
- typed gap and critique records;
- refusal of execution that began without a critique.

The missing layer is orchestration above that model: session binding, actual opposition execution, satisfaction/completion semantics, reconciliation, UI projection, plan derivation, and hardening.

This pack deliberately does **not** ask the coding agent to reinvent intent records, session infrastructure, project state, evidence ledgers, execution planes, GitHub synchronization, Cockpit, API, MCP, or governance. It must discover and extend the existing mechanisms.

## End-state invariant

```text
desired outcome
      ↓
canonical Intent
  ├─ statement
  ├─ invariants
  └─ acceptance criteria
      ↓
context + gap discovery
      ↓
derived plan
  ├─ milestones
  ├─ issues
  ├─ dependencies
  └─ execution waves
      ↓
executed opposition / critique
      ↓
accepted or revised plan
      ↓
intent-bound session execution
      ↓
evidence ledger
      ↓
criterion satisfaction
      ↓
intent reassessment
      ├─ insufficient → derive/revise work
      └─ sufficient → intent complete
```

Hard rule:

> No executable work without an intent or an explicit typed exemption; no intent work without a completed opposition pass; no completion without criterion-level evidence; no satisfied intent from issue or milestone status alone.

## Execution modes

### Recommended: master autonomous run

Give `prompts/00_MASTER_AUTONOMOUS_ORCHESTRATOR.md` to Claude Code Opus 5.5 in the repository root. It coordinates the entire pack, treats each numbered prompt as one milestone, each implementation section inside a prompt as one issue/work item, and keeps moving until the acceptance criteria are proven.

### Incremental milestone mode

Run the numbered milestone prompts in order:

1. `10_SESSION_INTENT_BINDING.md`
2. `20_EXECUTED_OPPOSITION_PASS.md`
3. `30_SATISFACTION_ENGINE.md`
4. `40_COMPLETION_POLICY.md`
5. `50_GITHUB_RECONCILIATION.md`
6. `60_COCKPIT_INTENT_PROJECTION.md`
7. `70_OUTCOME_TO_PLAN_DERIVATION.md`
8. `80_CROSS_SURFACE_HARDENING.md`
9. `90_RELEASE_AND_PROOF.md`

Do not skip forward merely because later work is more visually satisfying. A Cockpit graph over undefined completion semantics is just a prettier lie.

## How each prompt must be interpreted

- One prompt file = one milestone.
- Major numbered implementation sections inside the prompt = issue/work-item candidates.
- The agent should use Majordomus' own canonical planning/issue/milestone mechanisms if they exist and are functional.
- If the repository already provides a prompt-to-plan import path, use it instead of hand-creating duplicate metadata.
- If a requested behavior already exists in the current repository, prove it, close the corresponding item, and move on. Do not rewrite working architecture for sport.
- If names/paths/commands in these prompts differ from current repository reality, preserve the intent and adapt to the canonical implementation discovered in-repo.

## Non-negotiable execution discipline

Every milestone must:

1. inspect current repo state and relevant policies before edits;
2. map the request onto existing architecture;
3. update typed models only where truly necessary;
4. implement the narrowest canonical mechanism;
5. migrate legacy state in scope;
6. add focused unit/integration/contract tests;
7. update docs and generated projections;
8. update validation/gates so the behavior is enforced;
9. exercise CLI/API/MCP/Cockpit surfaces where applicable;
10. run the repository's canonical quality commands;
11. produce evidence, not a confidence statement;
12. commit/merge/release only through the repository's own governed workflow.

No bypasses, `--no-verify`, disabled tests, force-pushes, synthetic green checks, or manually edited generated outputs unless repository policy explicitly requires them.

## Definition of done for the entire pack

The project is no longer merely intent-aware. It is intent-driven when all of these are true:

- session start resolves and loads an active intent execution envelope;
- intent-linked work cannot begin without resolved criteria and completed opposition;
- opposition is actually executed by orchestration, not just represented by a record;
- criterion satisfaction is derived from evidence under typed policy;
- milestone/intent state derives from criteria and evidence, not checkbox folklore;
- session completion is blocked when required intent criteria lack evidence;
- GitHub issues/milestones reconcile with canonical intent structure;
- Cockpit exposes intent, criteria, evidence, blockers, progress and critique from canonical data;
- a desired outcome can produce a reviewable draft intent/plan using existing context discovery;
- all relevant surfaces consume the same canonical model;
- regression gates prevent reverting to issue-centric or manually duplicated truth;
- documentation explains the architecture and extension path;
- an end-to-end test demonstrates outcome → plan → critique → execution → evidence → satisfied intent.

## What not to do

Do not create:

- a second intent model;
- a second evidence ledger;
- a second session state format;
- frontend-only intent semantics;
- GitHub as an independent source of truth;
- a new orchestration engine if the existing execution plane can be extended;
- free-form LLM decisions where deterministic policy can decide;
- boolean `complete: true` fields that bypass evidence evaluation;
- manual lists of criteria/issues duplicated across consumers;
- a giant speculative framework before one vertical slice works.

## Suggested stopping test

A fresh fixture should be able to execute this scenario through supported repository commands/APIs:

```text
create desired outcome
→ derive/review intent
→ derive/review plan
→ run opposition
→ start intent-bound session
→ execute issue work
→ record evidence
→ evaluate criteria
→ refuse premature completion
→ satisfy remaining criteria
→ reconcile GitHub projection
→ render same state in Cockpit/API/MCP
→ complete intent
```

If any transition requires editing internal files by hand, the loop is not closed.
