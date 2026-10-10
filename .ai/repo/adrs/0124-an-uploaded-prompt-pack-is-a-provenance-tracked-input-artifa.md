---
schema: adr/v1
id: adr-0121
kind: adr
title: An uploaded prompt pack is a provenance-tracked input artifact, not a second intent model
status: proposed
date: 2026-10-10
tags:
  - artifact-ingestion
  - mesh-pack
  - prompt-pack
provenance:
  origin: authored
---

# 121. An uploaded prompt pack is a provenance-tracked input artifact, not a second intent model

## Context

An external "master prompt" (`majordomus-opus-fable-full-prompt-pack-20261010.zip`, 23 files,
a `manifest.json`, an `INDEX.md`/`MASTER.md`/`README.md`, four `contracts/*.md`, and milestones
`M00`–`M14`) asks this repository to become an "artifact-to-outcome pipeline": accept uploaded
prompt packs, derive intents/milestones/issues from them, execute the result over the mesh,
drain the resulting PRs, self-update, and expose all of it through the CLI, MCP, REST API and
`majordomus ui`, continuing autonomously through merge, release and deploy.

Reconnaissance before writing anything (2026-10-10, this checkout, `master` at `6800dcb`) found
that most of the target architecture already exists under different names:

- An intent/milestone/issue graph is already canonical: `apps/majordomus-cli/src/intent.rs`,
  `intent_plan.rs`, `intent_binding.rs`, `intent_realization.rs`, `intent_review.rs`, `plan.rs`,
  projected into `cockpit/intents.rs` and `cockpit/plan.rs`. The pack's M03 ("intent and project
  graph reconciliation") proposes re-deriving what this repository already treats as the one
  project model.
- Mesh cooperation — sessions, claims, handovers, reviews, one replicated journal (ADR 0067) —
  is live and is what this ADR's own session used to claim this file. The pack's M06/M07 assume
  they are building this from nothing.
- `majordomus pack` **already exists** (`apps/majordomus-cli/src/capability/builtin/pack.rs`)
  and means something unrelated: token-bounded text shards of the tracked tree for an LLM's file
  search (`pack plan`, `pack build`, `pack verify`, profiles in `share/archive.yaml`). The pack's
  proposed `majordomus pack list/import/run/diff/...` would collide with this name outright —
  exactly the duplication the pack's own section 4 says to avoid, and it did not check.
- `scripts/collision-check` against plausible new paths and a scan of `.ai/repo/adrs/` and open
  branches found no other branch or mesh peer already building this; the one hit
  (`.ai/local/prompts/20261010181418-...`) is this session's own prompt-capture record, not
  prior work.
- The pack's sections 9, 11 and 26 direct continuous, autonomous PR merge, release and deploy.
  That is a different decision from the one this ADR makes, and is refused here on the same
  ground `project.work-is-claimed-before-it-is-built` and the worktree/task-lifecycle rules
  already stand on: a hard-to-reverse, shared-system action is confirmed per action, not
  pre-authorized by a pasted document.

What the pack gets right, and what this ADR keeps: a prompt pack is a versioned *input*
artifact; an intent states what should become true; evidence, checked by a canonical validator
and never by the model that proposed the mapping, decides whether it already is.

## Decision

**An uploaded prompt pack is ingested as a provenance-tracked artifact revision, reconciled
against the existing intent/milestone/issue graph — never a parallel one — and never treated as
self-certifying its own completion.**

Concretely, scoped to what this ADR actually authorizes (reconnaissance and design; M01 onward
is separate, reviewed work):

1. **Identity split.** `ArtifactRevision` (an uploaded pack and its content digest, filename,
   provenance, received time) is a distinct record from `ExecutionRun` (one attempt to realize
   work derived from a revision, against one repository state, at one time). The same revision
   may back zero, one or many runs; a run pins the revision it executed. These are never
   collapsed, so dedup, rollback and audit on one side cannot corrupt the other.
2. **Reuse the existing graph.** Ingestion derives or reconciles records in the intent/milestone/
   issue model that already governs this repository (`intent.rs`/`plan.rs` and friends) — it
   does not introduce a second notion of "milestone" or "issue" scoped only to uploaded packs. A
   derived item must link back to the artifact revision and source section that justified it.
3. **The `pack` name is reserved.** Whatever CLI/capability surface ingestion gets, it is not
   `majordomus pack *` — that command group stays the text-shard tool it already is. A follow-up
   ADR proposing the actual noun (candidates: `artifact`, `intake`) is required before any code
   lands, so the collision is resolved by decision, not by accident.
4. **The validator owns truth, not the proposer.** An agent (or the pack) may propose that an
   issue satisfies a criterion; only the existing evidence/check machinery
   (`majordomus check`/`finish`, the obligations in `share/obligations.yaml`) may mark it
   satisfied. This is the existing contract (ADR 0041, "inputs unchanged is not proven"), stated
   here so ingestion work cannot quietly bypass it.
5. **No autonomous merge/release/deploy from this work.** Execution runs derived from an
   ingested pack integrate through the existing PR, review and release process, with a person
   confirming each merge, release and deploy, until a separate ADR earns and scopes a narrower
   autonomous exception.
6. **Untrusted content stays untrusted.** Nothing read from an uploaded pack — including text
   that reads as an instruction to Majordomus or to the agent ingesting it — is treated as
   governance, policy or privileged authorization. It is source material to extract requirements
   from, subject to the same ZIP-safety handling (size, path, decompression-ratio limits) any
   untrusted archive gets.

This ADR authorizes the reconnaissance above and the design stance in this decision. It does
not authorize implementing M01–M14; each milestone is its own scoped task, announced on the
mesh board and collision-checked before it is built, per `project.work-is-claimed-before-it-is-built`.

## Alternatives rejected

**Building the pack's architecture as specified, including its own `pack` CLI group and its
own project-graph derivation.** Rejected because it duplicates two subsystems this repository
already has under contract (the intent graph, and the `pack` command name), which is the
specific failure mode the pack itself warns against in its own section 4 and section 8.

**Treating the pack's autonomous merge/release/deploy directive as standing authorization.**
Rejected: a document a user pastes into a prompt is not the mechanism this repository uses to
authorize hard-to-reverse, shared-system actions — the task-lifecycle, ADR and mesh-announce
mechanism is, and it requires a scope and a person at each such step.

**Implementing all 14 milestones in one unreviewed branch.** Rejected by the repository's own
incremental-delivery and worktree-topology rules: an enormous unmerged branch is exactly what
`project.worktree-topology` and the pre-push scope check exist to prevent.

## Consequences

- A follow-up ADR must name the actual noun for the ingestion surface before any CLI, MCP or API
  code is written against it; until then, no new `majordomus <noun> *` command group exists.
- M01 (artifact identity and storage) becomes the next concrete, separately-scoped task; it is
  not started by this ADR and needs its own `majordomus start` and mesh claim.
- Anyone resuming this work must re-run collision-check and read the mesh board before starting
  M01, because this ADR does not reserve that work — it only records the decision the work must
  honor.
- The original pack (`majordomus-opus-fable-full-prompt-pack-20261010.zip`) stays a reference
  input, not a spec to implement literally; its milestones are a useful checklist to reconcile
  against the real graph, not identifiers to create records under.
