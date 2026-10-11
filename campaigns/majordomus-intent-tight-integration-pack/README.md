# Majordomus Intent Tight Integration Pack

Purpose: take the existing **partly built intent model** and finish the architecture above it without creating a parallel universe.

This pack is for Claude Code Opus 5.5 or another strong repository-capable coding agent working inside `prismatic-majordomus`.

The source statement this pack closes is:

> An intent is already a typed project record with statement, invariants, milestones, criteria and evidence. Its stage is derived from the plan; criterion state is derived from the evidence ledger; links validate; preflight can name which intent an issue serves; coverage maps work to criteria; gaps and critiques are typed; execution before critique can be refused.
>
> Missing: session intent loading, completion-driven satisfaction, first-class Cockpit intent view, GitHub milestone reconciliation, desired-outcome-to-plan derivation, and an actually executed opposition pass.

The mission is NOT to redesign intent from scratch.
The mission is to **close the loop**.

---

# Target end state

```text
desired outcome
    ↓
typed intent
    ├─ statement
    ├─ invariants
    ├─ criteria
    └─ evidence contract
    ↓
gap/context discovery
    ↓
derived draft plan
    ├─ milestones
    ├─ issues
    ├─ criterion mappings
    └─ dependencies / execution waves
    ↓
executed opposition pass
    ↓
accepted/revised plan
    ↓
intent-bound session
    ↓
work / verification
    ↓
typed evidence ledger
    ↓
criterion satisfaction
    ↓
intent satisfaction
    ↓
completion policy
    ↓
GitHub + Cockpit + CLI/API/MCP projections
    ↓
reassessment / new gap if unsatisfied
```

One model, one loop, many projections.

---

# Non-negotiable architectural invariants

1. **Intent is canonical for why.**
2. **Milestones/issues are execution structures, not a second source of truth.**
3. **Evidence decides satisfaction.**
4. **Session state binds to intent; it does not invent one.**
5. **Completion cannot declare an intent satisfied independently.**
6. **Opposition is executed, not merely represented by a required file.**
7. **GitHub is a synchronized projection/integration surface, not canonical intent storage.**
8. **Cockpit renders canonical backend state and does not reimplement intent semantics in JavaScript.**
9. **CLI/API/OpenAPI/MCP/docs are projections of the same typed model.**
10. **No new discoverable intent entity requires registration in multiple consumers.**
11. **No generated/manual duplicate inventories.**
12. **No handwritten `satisfied: true` or `completed: true` that can drift from evidence.**
13. **No bypass through maintenance paths without explicit typed exemption semantics.**
14. **Every blocking architectural rule must have executable proof that it is wired.**
15. **Legacy behavior must be migrated, not merely grandfathered silently.**

---

# Pack structure

Run `00_MASTER_INTENT_CLOSURE_ORCHESTRATOR.md` for a one-shot autonomous execution.

Or execute milestones in order:

1. `10_ARCHAEOLOGY_AND_ARCHITECTURE_MAP.md`
2. `20_SESSION_INTENT_BINDING.md`
3. `30_EXECUTED_OPPOSITION_PASS.md`
4. `40_SATISFACTION_ENGINE.md`
5. `50_COMPLETION_POLICY_INTEGRATION.md`
6. `60_GITHUB_RECONCILIATION.md`
7. `70_COCKPIT_AND_CROSS_SURFACE_PROJECTION.md`
8. `80_DESIRED_OUTCOME_TO_PLAN.md`
9. `90_REASSESSMENT_AND_CLOSED_LOOP.md`
10. `100_RULES_TESTS_DOCS_ENFORCEMENT.md`
11. `110_END_TO_END_PROOF_AND_RELEASE_READINESS.md`

Each file is one milestone. Major sections inside a milestone should be turned into issues/work items using the repository's current plan tooling where appropriate.

---

# Mandatory working method

Before changing code, the agent MUST:

- read `AGENTS.md`,
- read `.ai/README.md`,
- resolve context for the paths it will touch,
- load rules/doctrines and their dependencies,
- inspect relevant ADRs,
- inspect current intent implementation,
- inspect session lifecycle,
- inspect finish/completion policy,
- inspect GitHub integration,
- inspect Cockpit,
- inspect capability/transport derivation,
- inspect schemas,
- inspect current tests and cases,
- inspect existing plan/issues serving the work,
- inspect current branches/peers/collisions.

Do not assume filenames or type names from this pack.
Discover them.

---

# Definition of done

The architecture is complete only if the following flow is executable and test-proven:

```text
desired outcome
  → draft intent
  → criteria/invariants
  → draft plan
  → opposition executed
  → plan accepted/revised
  → session starts bound to intent
  → work produces evidence
  → satisfaction is derived
  → completion refuses while criteria remain unmet
  → completion succeeds only when evidence supports it
  → GitHub projection reconciles
  → Cockpit shows the same state
  → CLI/API/MCP agree
  → changed reality can reopen/reassess the intent
```

And this must be true without:
- manually editing a satisfaction boolean,
- manually keeping GitHub milestones in sync,
- frontend-specific business logic,
- consumer-specific registries,
- second plan/intent models.

---

# Explicit anti-goals

Do not:
- rewrite the whole planning subsystem,
- replace the existing intent record unless proven structurally insufficient,
- create `intent2`,
- introduce a second evidence ledger,
- make GitHub the authority,
- create hand-authored Cockpit-only state,
- hardcode intent behavior into frontend templates,
- create another session model,
- add an LLM requirement to core correctness,
- require Gemini/Codex/Claude to be present for correctness,
- hide missing capability behind prose,
- weaken blocking rules to warnings,
- mark future behavior as shipped.

---

# Autonomy

The agent should proceed autonomously through:
inspect → design → implement → migrate → test → document → enforce → validate.

Do not ask the human for routine file/module naming choices.
Use repository conventions.

Where multiple implementations are possible:
1. prefer extension of existing canonical types,
2. prefer derived projections,
3. prefer deterministic typed state,
4. prefer deletion of duplicated legacy logic,
5. prefer repository-local evidence over model opinion.

---

# Final proof expected

The final report must include:

- root cause of the previously open loop,
- architectural ownership of each concern,
- exact data flow,
- exact commands/tests run,
- cross-surface proof,
- mutation/refusal proof,
- legacy migration,
- zero-registration extensibility proof,
- remaining true limitations only.

If any "conditional" statement from the old partly-built description remains true, name it explicitly.
