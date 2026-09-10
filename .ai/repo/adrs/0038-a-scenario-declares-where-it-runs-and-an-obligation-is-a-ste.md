---
schema: adr/v1
id: adr-0038
kind: adr
title: A scenario declares where it runs, and an obligation is a step
status: proposed
date: 2026-09-10
tags:
  - architecture
  - agents
  - governance
related:
  - file:.ai/repo/adrs/0002-canonical-capability-registry.md
  - file:.ai/repo/adrs/0030-a-task-owes-obligations-and-evidence-goes-stale.md
  - file:.ai/repo/adrs/0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md
  - rule:project.no-new-nouns
  - rule:project.optional-complexity
  - file:lib/usecase.sh
  - file:lib/evidence.sh
  - file:share/commands.yaml
  - file:share/obligations.yaml
  - file:.ai/repo/workflows/task-lifecycle.md
provenance:
  origin: authored
---

# 38. A scenario declares where it runs, and an obligation is a step

## Context

A repeated proposal, arriving again in a design note on 2026-09-10, is that `.ai/`
should gain a typed orchestration layer: workflow objects composing capabilities into
a validated DAG, so that `session-start`, `fix-bug`, `merge-ready` and their relatives
stop being prose an agent is asked to remember and become something a machine can run
and refuse. The motivation is right and the repository already agrees with most of it.
What it asks for mostly exists, and the part that does not is smaller than the proposal.

**What exists.** `context` and the `SessionStart` hook are `session-start`. `finish`
with its contract, `handover` and the `PreCompact` checkpoint are `session-finish`.
`continuity.md` is `resume-work`. `doctor` and `watch` are `repo-doctor`. `usecase
impact` is `impact-analysis` and the selector `test-affected` would need. The capability
registry with `capability/closure.rs` is `surface-sync`. `src/execution/` is the run
loop, and ADR 33 already settled that an execution is a watched capability call rather
than a second registry. `share/obligations.yaml` with `evidence` and
`majordomus.obligation-closure` is the `merge-ready` checklist, evaluated live at HEAD
where git can settle it.

**Three reasons the proposal cannot be taken as written.** First, `workflow` is not a
free noun here: it already means the prose context documents under
`.ai/repo/workflows/`, and it already means `just` — `Surface::Workflow`,
`projections.workflow`, the identity `workflow.site-build` of ADR 27. A third meaning is
what `project.no-new-nouns` refuses. Second, a workflow engine with its own step
definitions, schemas and route table is the second subsystem ADR 33 declined by name.
Third, a generic DAG with inference, retries, rollback and compensation is what
`project.optional-complexity` refuses in one sentence: *a mechanism that exists to
define mechanisms ... is the failure this rule refuses*.

**What is genuinely missing** is narrower and is visible in the check output of any
session. `check` evaluates the obligations *the active task declared at `start`*. A
worker who forgot to declare them is told `the task declares no obligations` and passes.
There is nothing that says, for a class of work, what that class owes — that a defect fix
owes a test and a declared invariant whether or not anyone remembered to promise them.
And a scenario, the one executable shape the layer already has, can only run against a
disposable fixture; it cannot ask a question about the repository a worker is standing in.

## Decision

**No new kind and no engine. The scenario gains a place to run and a second class of
step.**

- **`scenario.mode`: `fixture` (default) or `live`.** `fixture` is today, unchanged: a
  setup script, a disposable repository, committed evidence. `live` runs against the
  repository the command was invoked in, has no setup and creates nothing.

- **A live scenario may only run read-only commands.** The class is read from
  `share/commands.yaml`, which already declares it per command and is already the
  authority the reference semantics come from; `usecase validate` refuses a live step
  whose command is not `class: read-only`, exit 10. Safety is a property of the
  declaration, not of the author's care. A live scenario is therefore a *question* about
  this repository, never an action on it: mutation stays with `start`, `check` and
  `finish`, which supervise it, and a second unsupervised path to the same writes is
  exactly what this decision refuses to open.

- **A step is either a command or an obligation.** An obligation step carries
  `obligation: <token>` from `share/obligations.yaml` and no `run:`. It is not executed;
  it asserts that the obligation is discharged, through the same `lib/evidence.sh` path
  `check` uses — established live where git or the site can settle it, and read from the
  ledger where it cannot, with the same stale-versus-old judgement. It resolves to
  `pass`, `unmet` or `stale`. Obligation steps are valid only in `live` mode: a fixture
  has no task to owe anything.

- **A live scenario's verdict is `unmet`, not `fail`.** A failing fixture scenario is a
  defect in the tool. A live scenario whose obligation is undischarged is work not done,
  and the two must not be reported in the same word. The runner exits 10 — *contract
  unmet*, the code the tool already uses for this — never 1.

- **Live evidence is never committed.** Fixture evidence stays under
  `.ai/local/evidence/use-cases/<id>.json` and remains a derived, committed artifact.
  A live result depends on the machine, the tree and the minute, and a generated file
  whose content depends on who generated it is the one thing derivation may not produce
  (ADR 5, and the reason `mj_uc_run_one` pins the fixture's branch name). Live evidence
  is written under `.ai/local/evidence/live/` and is ignored.

- **`usecase run` still runs fixtures by default.** Live scenarios are selected by
  `--live` or by naming their id, so CI's meaning does not change under it. `usecase
  coverage` counts a live scenario as `partial`, never `covered`: a guarantee CI cannot
  reproduce is not a guarantee.

This composes rather than centralises, which is the shape ADR 27 chose for commands and
ADR 33 for executions. A live scenario declares an ordered set of questions and
obligations; every one of them is answered by a capability that already exists, and the
scenario implements none of them.

## Alternatives rejected

**A `workflow` kind under `.ai/repo/workflows/` with its own schema and runner.** The
proposal as written. It adds a third meaning to an occupied noun, a second declaration
of "an ordered list of commands with expectations" beside the scenario, and a second
runner beside `usecase run` and `src/execution/`. Every argument ADR 33 made against an
action registry applies unchanged, and the drift would start at the first edit that
updated one of the two.

**Letting a live scenario mutate, guarded by the task's scope claim.** More expressive,
and it would let `feature-start` be a scenario end to end. It also creates a second way
to write to a supervised repository, one that no profile, no scope check and no finish
contract sits in front of. The mutating half of `feature-start` already has an owner —
`worktree create`, `start`, and `.ai/repo/workflows/task-lifecycle.md` — and what was
missing from it was never the mutation. Reconsider only with a concrete case that the
read-only half plus obligations cannot express.

**A new obligation vocabulary for scenarios.** `share/obligations.yaml` already carries
the tokens, the inputs each is hashed over, and which of them git settles without asking.
A scenario-local vocabulary would be a second list to keep in step, and `doctor` already
proves the existing one in both directions.

**Adding `established_by` logic to the scenario runner.** The runner asks
`lib/evidence.sh` and reports what it says. An obligation that is settled live at HEAD
is settled the same way for `check`, for `finish` and for a scenario, or the three
disagree about whether the same tree is finished.

## Consequences

- A class of work can now state what it owes, independently of what a worker remembered
  to declare at `start`. `check` remains the task's own verdict; a live scenario is the
  class's, and the two are read together.
- `usecase validate` gains two refusals — a live step that is not read-only, an
  obligation step outside live mode — and both are decided from declarations that
  already exist. Nothing new is hand-maintained.
- The scenario schema is now read in two places with two meanings for `steps`. The cost
  is one branch in `mj_uc_run_one` and one in the validator; the alternative was a second
  file format, a second parser and a second runner.
- `usecase coverage` numbers do not move: live scenarios cannot raise a target to
  `covered`. A live scenario that also happens to be the only thing naming a command
  leaves that command `partial`, which is the honest reading.
- What this does not deliver: `review-change`, and any plan whose steps must mutate. Both
  remain open, and neither is blocked by this decision.
