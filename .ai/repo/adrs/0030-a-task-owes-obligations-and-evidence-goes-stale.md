---
schema: adr/v1
id: adr-0030
kind: adr
title: A task owes obligations, and the evidence that discharges them goes stale
status: proposed
date: 2026-09-09
tags:
  - completion
  - evidence
  - tasks
provenance:
  origin: extracted
  derived_from:
    - file:lib/finish.sh
    - file:lib/plan.sh
    - file:scripts/pages
    - file:scripts/generate-site-data
---

# 30. A task owes obligations, and the evidence that discharges them goes stale

## Context

`finish` already refuses. It dispatches the doctrine registry and a repository selects which
lines apply through `verification.finish_requires`, so the contract is data rather than a
list in shell. What the selected lines cover is the working tree: the touched files sit
inside the task's scope, a verification command ran and exited zero, the branch and head
still match the record, no question is open, a note exists with the sections its outcome
requires.

Every obligation past the working tree is absent. Nothing in the contract knows whether the
work was committed, whether the commit reached a remote, whether the trunk contains it,
whether the generated projections were current at the moment the task was declared finished,
whether the site was published, whether anything was deployed, or whether the deployed thing
was ever looked at. `share/events.yaml` has no event for a publish or a deploy. A worker can
satisfy every line of the contract and leave the work on a laptop.

The pieces that would answer those questions already exist and are not joined.

`plan` is the obligation model, one level up: an issue declares `evidence_required`, evidence
is recorded with a command or an artifact because narrative is not evidence, and `plan done`
refuses while a token is unproved. `usecase run` is an execution-evidence engine whose output
is normalised so that two runs of one scenario are byte-identical. `usecase impact` already
computes which scenarios and cases a change obliges a worker to re-run, and
`project.use-case-evidence` already states in writing that this must happen *before* `finish`
— a sentence that declares no validator and therefore holds nobody. `scripts/pages verify`
polls the published site until it serves the expected commit; nothing calls it, and it writes
no record.

The deeper gap is staleness. `usecase` evidence carries no head and no input hash. `plan`
evidence stamps a commit that nothing recomputes. So evidence, once recorded, is true
forever, which is the same as being unchecked: a test result that cannot go stale is a claim
about the past presented as a claim about the present.

The repository has already solved exactly this problem once, for the website. The site
generator hashes an explicit list of the files it reads, writes the value into the derived
data as `source_hash`, and `pages current` recomputes and compares, failing with both hashes
and the repair command. That mechanism is wired to a git hook and `doctor` proves the hook
does not swallow its exit code. It is the shape the rest of the evidence in this repository
lacks.

## Decision

**An obligation is a field on a task, and evidence is a ledger event carrying the hash of
what it was taken over.**

- **`requires: [...]` sits beside `scope: [...]`** in the task record. Scope is a containment
  promise — where a worker may write. `requires` is a delivery promise — what the worker owes
  before the outcome `completed` is available. Token names are data in `share/obligations.yaml`,
  beside `share/events.yaml`, not strings in shell.
- **Evidence is `task.evidence` in the ledger**, not a new store. The ledger is already
  append-only, ordered, integrity-checked, and its envelope already carries head, branch and
  session. The event adds what it must: which obligation it covers, how it was taken, and the
  hash of the inputs it was taken over.
- **Staleness is recomputation, not a timestamp.** The validator re-hashes the obligation's
  declared inputs and compares, and reports a difference with the same two-hash message
  `pages current` prints. The recorded head is labelled against the current one with
  `mj_git_label`, whose four values — exact, advanced, diverged, different_context — are the
  repository's only staleness vocabulary. A fifth vocabulary is not invented.
- **Discharge paths are commands that already exist.** Git answers commit, push and trunk
  containment. `generate --check` and `pages current` answer generated currency.
  `pages verify --commit` answers publication, and its use here is the first caller that
  mechanism has ever had. `usecase impact` and the cases it names answer tests, which
  discharges the sentence `project.use-case-evidence` has been unable to enforce.
- **One rule, dispatched where the others are.** `majordomus.obligation-closure`, blocking,
  validator `obligations`, enforced by `check` and `finish`, selected through a policy key
  like every other contract line. It is skipped for outcomes other than `completed`, exactly
  as the verification and profile lines already are: a blocked task is not a dishonest one.
- **The wiring proof gains a direction.** `doctor` already proves that every declared doctrine
  resolves to a validator and that no validator runs under no rule. It gains the same proof for
  obligations: every token names a reachable check, and every check is reachable from a token.

## Alternatives rejected

**A session manifest.** The mandate that prompted this asked for one. A session in this
repository gates nothing by design — it claims no paths, holds no acceptance, and is optional
— and its closed record is *derived from the ledger* precisely so that no second mutable
account of the same events exists. A manifest would be that second account, and making a
session refuse acceptance would invert the reason the noun was separated from `task` in the
first place. The obligation belongs to the thing that already promises, already has a typed
outcome, and already refuses.

**A checklist file per session.** `session_checklist.yml` with `update_cli: true` is the
inventory nobody updates; this repository has spent several decisions removing exactly that
shape, and the audit that produced this ADR found a neighbouring codebase where such
inventories had drifted by a factor of three while claiming to be generated.

**Extending `session close` to the task outcomes.** It accepts `closed` and `interrupted`.
Teaching it `completed` and `blocked` would give the repository two outcome vocabularies for
one idea, and the session record validator already refuses a session that claims more than it
can know.

**A new evidence store under `.ai/local/`.** One exists for use cases and is not a section of
the manifest. Adding a second unregistered store to hold closure evidence would repeat that
mistake rather than fix it.

## Consequences

A task that declares no `requires` behaves exactly as today, so this is opt-in per task and
per repository through the policy key. A task that declares obligations cannot reach
`completed` on assertion alone. Evidence taken before a change no longer counts after it,
which will be inconvenient in exactly the cases where it should be.

The outer obligations — publication, deployment, verification — are the ones a laptop cannot
prove alone. Their evidence names a remote fact and a time, and the honest report for a task
that has not reached the trunk is `blocked by`, not `pass`.
