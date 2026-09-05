---
schema: skill/v1
id: implement
version: 1
title: Implement a change
description: Carry a requested change through the repository evidence-first, from the invariant it must keep to a verified, reconciled, documented and committed tree, without widening its scope.
status: active
tags: [implementation, evidence, invariants, drift]
related: [repo-review]
inputs:
  - the requested change, in the requester's words, with any issue or specification it names
  - a checkout with git history and the effective context for every path the change will touch
  - the repository's verification, generation and drift-check commands
outputs:
  - the change, as coherent commits whose messages name what each proves
  - behavioural evidence that distinguishes the new behaviour from the old
  - regenerated projections and a clean drift check
  - an implementation record naming the invariant, the evidence, what was reconciled and what remains unsupported
---

# Purpose

Make an engineering change the way this repository asks for one: establish what is true
before touching anything, name the invariant the change keeps or repairs, decide what a
caller will be able to observe, prove it from the outside, make the smallest correct
change, and leave every derived surface reconciled. The order is the point. A worker that
writes the mechanism first and finds the invariant afterwards ships whatever the
mechanism happened to do.

This is not a style guide, not a licence to refactor what is near the change, and not a
task manager. It is the protocol between "please change X" and "X is changed, here is
the evidence".

# When to use

Any change to the tree that is more than a typo: a defect, a feature, a refactor, a
performance change, a generated-data or catalogue addition, a migration, a provider
adapter, a CI or tooling change. Before starting, run `repo-review` over the scope when
the state of that part of the tree is not already known; the review's findings are the
first input here.

Not this skill: reviewing without changing (`repo-review`), or deciding whether the
change should be made at all.

# Procedure

Work through the phases in order; each one produces something the next one reads.
Record the invariant, the observable behaviour and the evidence plan before writing the
mechanism, in the checkpoint or handover of the task, so that the next worker (or the
same one after an interruption) does not re-derive them.

## 1. Establish the scope

State the requested outcome in one sentence, the explicit acceptance criteria, and the
criteria the repository's rules add implicitly. Classify the change (defect, feature,
refactor, performance, documentation, generated data, migration, provider integration,
CI, architecture), and answer: does public behaviour change, do persistence or state
semantics change, are generated projections affected, is a provider involved, does
compatibility matter. Declare the paths the work may touch (`majordomus start --scope`)
and do not widen them silently; if the request can be met without a new subsystem, meet
it without one. If the requested behaviour already exists, verify it instead of building
it again.

## 2. Load the applicable context

Read what applies to the paths in scope and no more: the root instructions, the rules in
force (`majordomus rules list`, noting which are machine-enforced), the scoped documents
from the root down (`majordomus context resolve <path>`), the architecture decisions
that touch the subsystem, the existing tests, the surrounding implementation, and the
public documentation that makes claims about the behaviour.

## 3. Inspect before mutating

Read the current implementation, its callers, its adjacent abstractions, its tests, its
public interfaces, the generated consumers of its output, its configuration and its
diagnostics. Read the history of the files where it changes a decision: why the
abstraction exists, whether the code is transitional, whether an earlier fix addressed
the same defect, whether a suspicious pattern is deliberate. Do not read history for
ceremony; read it where it changes what you would do.

## 4. Name the canonical truth

For every artifact the change touches, say which it is: canonical, a generated
projection, a cache, runtime state, persisted state, external state, documentation, or a
test fixture. Find the source of every derived artifact before editing it, and never
repair generated drift by editing the generated file. Two competing canonical sources
are an architectural defect; repair the direction of derivation rather than reconciling
the copies by hand.

## 5. State the invariant

Write down, in one falsifiable sentence, the invariant the change introduces, preserves
or repairs — "for every valid `<x>`, exactly one `<y>` exists and every projection is
derived from it", "a mutation is reported successful only after its record is durable",
not "make loading better". Where no invariant applies, state the observable behaviour
instead. Do not invent an invariant for a trivial change.

## 6. Define the observable behaviour

Before implementing, say what a user, a test, a command-line caller, an MCP client or a
later reader of the records will observe: the happy path, the failure path, edge
conditions, partial failure and restart where they apply, idempotency, invalid input, an
absent dependency or provider capability, stale projections, exit status, and the
machine-readable form. This list becomes the behavioural test. Implementation detail is
not a substitute for it.

## 7. Define the failure semantics

For every operation that can fail, answer: what fails, at which boundary, what is left
mutated, what is persisted, whether a retry is safe, whether the operation is idempotent,
whether a partial result can be mistaken for success, whether the failure is typed and
distinguishable, whether degraded behaviour is explicit and documented, and whether a
stale state is detectable. Never leave a swallowed error, a silent fallback to weaker
semantics, a pass that examined nothing, a success reported before durability, or a mock
that proves a boundary it bypasses.

## 8. Map the impact surface

List every surface the change may reach — canonical data and schema, implementation,
command line, MCP, generated projections, documents, the site, diagnostics,
configuration, CI, benchmarks, provider adapters, examples, decisions — and mark which
ones it actually touches. Modify those and only those; the list exists so that no
derived state is forgotten, not to manufacture work.

## 9. Decide the evidence before the mechanism

Choose how the behaviour will be proved: black-box behavioural first, then integration,
then contract, then focused unit tests. A defect is reproduced, or a regression test is
written that would have failed before the fix. Property-based tests belong where the
invariant ranges over many inputs (parsers, normalisation, deterministic generation,
idempotency, path safety, reconciliation), not as decoration. Where the change is a
performance claim, record the baseline, the workload and the metric first.

## 10. Implement the smallest correct change

Reuse the abstraction that already matches the invariant; extend a data-driven
mechanism rather than add a hard-coded branch; add no dependency without the exact
problem it solves, the alternatives already in the tree, its footprint and its
replacement cost written down; add no service, queue or store the request does not
require; add no extension point for a need not yet demonstrated; remove a dead
transitional path only when it is in scope and safe. Keep the tree provider-neutral: a
provider-specific difference belongs in an adapter, and a capability a provider does not
expose is represented as unavailable, never fabricated. Prior art may be consulted and
never depended on: understand the requirement, name the invariant, drop the source's
assumptions, design the interface this repository would have, implement independently.

## 11. Verify in layers

After each coherent layer: format the touched code, run the focused tests, run the
affected validation, read the failure messages, and continue only when the layer holds.
Do not accumulate unrelated changes and discover at the end which one broke the parse.

## 12. Reconcile derived state

After a canonical change, run the repository's generation and then its drift checks
(`majordomus generate`, `scripts/generate-site-data`, then their `--check` forms and
`majordomus doctor`), and commit the regenerated outputs with the change that caused
them. Generation is deterministic, owns its output, and must never be replaced by a
hand-synchronised copy.

## 13. Update the diagnostics

Where the change adds or alters operational behaviour, give `doctor`, `check` or `watch`
a finding for it that distinguishes configured, discovered, wired, reachable and
reconciled as they apply, reports what it examined, and never succeeds over nothing.

## 14. Document what is proved

Documentation follows evidence. Update the documents that make claims about the changed
behaviour, and only to what the tests now prove; keep the distinction between planned,
implemented, verified and guaranteed, and add a claim to the matrix with its source,
implementation and test when the change is a capability. Prefer invariants, commands,
observable behaviour, failure behaviour and examples over description. Keep published
examples runnable, and project them rather than copying them.

## 15. Run the broader verification

Run the suite that covers the scope, the generation and drift checks, the site build
where the site reads the change, and the CI-equivalent gates the repository names. A
result you did not observe is not evidence.

## 16. Review the diff

Read the whole diff as a reviewer would, with `repo-review`'s checklist: correctness,
the invariant, failure behaviour, canonical against generated state, duplicated truth,
manual registration where derivation exists, provider leakage, test quality,
diagnostics, documentation claims, drift, performance regressions, unnecessary
abstraction, and scope that crept. Fix what is substantive before continuing.

## 17. Commit coherent work

One commit per independently understandable, verified change, in the repository's
message convention; neither one commit for everything nor a commit per keystroke. Before
each commit the relevant tests pass, the formatter and linter pass, generated
projections are reconciled and no unexplained file is present.

## 18. Stop

Stop expanding when the requested behaviour is observable, the invariant holds, the
evidence passes, the projections reconcile and the documentation is accurate. A
possible enhancement, an inelegant neighbour or an unrelated roadmap item is recorded as
a question or an issue, not implemented "while here".

## 19. Report

Write the record in the shape under Output. Do not report success because files
changed.

# Output

The implementation record, kept concise, with these sections in this order:

| section | content |
|---|---|
| requested outcome | the change as asked, and the criteria it was held to |
| repository reality | what already existed in the scope, and what the review found |
| invariant | the sentence from phase 5 |
| observable behaviour | what is now demonstrably different, happy and failure paths |
| implementation | what changed, by file and mechanism, and what was deliberately not changed |
| evidence | the tests, checks and measurements actually run, with their results |
| reconciled state | the projections regenerated or checked, and the drift checks' results |
| documentation | the claims updated and the evidence that now supports each |
| commits | the commits created, one line each |
| remaining limitations | what is unsupported, incomplete or unverified, stated plainly |

Every statement in the record is an observed fact, an inference or a recommendation,
and is labelled when it is not a fact. Ask the requester only when a consequential
ambiguity cannot be resolved from the repository, its history and its tests, and a wrong
guess would build the wrong behaviour; prefer the reversible choice otherwise, and act.
