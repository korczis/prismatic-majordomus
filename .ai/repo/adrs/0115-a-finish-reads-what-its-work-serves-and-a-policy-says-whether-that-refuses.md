---
schema: adr/v1
id: adr-0115
kind: adr
title: A finish reads what its work serves, reports it in every mode, and refuses only where the policy has chosen to
status: proposed
date: 2026-10-08
tags:
  - intent
  - completion
  - evidence
  - governance
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0107-completion-discharges-obligations-and-never-decides-whether-an-intent-is-satisfied.md
  - file:.ai/repo/adrs/0111-a-task-names-what-it-serves-and-the-binding-is-derived.md
  - file:.ai/repo/adrs/0113-satisfaction-is-explained-and-held-to-its-guards.md
  - rule:project.completion-never-decides-satisfaction
  - rule:project.derived-once
  - file:share/completion.yaml
  - file:apps/majordomus-cli/src/gates/done.rs
  - file:.ai/repo/project/intents/finish-reads-what-its-work-serves.yaml
  - test:test/cases/966_a_finish_reads_what_its_work_serves.sh
---

# 115. A finish reads what its work serves, reports it in every mode, and refuses only where the policy has chosen to

## Context

A task starts bound to the issue and the intent it serves (ADR 0111). The intent engine says
which criteria have current evidence and which guards are violated, and says why (ADR 0113).
The completion policy says what `completed` means and refuses the word while a question is
owed. The two met in no line of code: a worker could finish `completed` while the criterion
the work exists for had no evidence, or while a guard of its intent was failing.

ADR 0111 and ADR 0113 both end by naming this as the next decision. ADR 0107 bounds it:

> Completion policy decides whether work is done. It may read an intent's derived standing
> as an input to a question … but it never declares a question whose answer is that an
> intent is satisfied, never writes an intent record, and never stores a stage or a verdict.

ADR 0107 also says, among its consequences, that a rule requiring a criterion's evidence
before an issue is accepted "belongs to the plan, not to completion". That sentence is about
accepting an issue — `plan done` — and stays true: nothing here touches when an issue is
done. What is decided here is whether a *task* may be called `completed`, which is a
statement about a session's work and not about the plan or the intent.

### The measurement

The plain reading of the requirement — refuse `completed` while a served criterion has no
current evidence — was measured before anything was built. On 2026-10-08, `majordomus intent
binding --issue <id>` was asked of every issue of this repository that declares what it
serves:

| issues that declare `serves` | 22 |
|---|---|
| with current evidence for every required criterion they serve | 0 |
| whose served criteria were never run | 15 |
| the rest | stale, unresolved, or of a kind the ledger cannot derive |

A criterion is current only for a passing run recorded in the tracked ledger, at a commit in
the history, over a clean tree. The ledger held fourteen executions. This repository says
`verification.completed_means_complete: true`. So the plain reading would have refused
`completed` to every issue-bound task from the day it landed, and `partial` would have
become what every finished task is called — which says nothing.

## Decision

**Two questions join the completion policy**, in its `validation` stage, each answered from
`intents.binding` for what the active task named and from nothing else:

- `criteria-served` — does every required criterion this task's issue serves have current
  evidence?
- `guards-hold` — is a guard of an intent this task serves violated?

Neither asks whether an intent is satisfied, and neither's text says the word. A finish
writes no intent record and stores no intent stage, verdict, criterion state or evidence
state. ADR 0107 stands.

**The engine's word is translated once.** Under a policy that holds the answer:

| a served required criterion's evidence is | the question |
|---|---|
| `failing`, or `unresolved` (the record names evidence that does not exist) | fails |
| `stale` | is stale |
| `not_run` | is owed |
| `not_derivable` (`command`, `deployment`) | is unknown |
| `current`, all of them | passes |

The worst decides. A guard is violated only by a failing run (ADR 0113): one violated guard
fails `guards-hold`; guards that hold or are not judged pass it.

**Who is asked.** A task that names an issue is held to the required criteria that issue
serves — not to the others of the intent, and not to optional ones — and to the guards of
the intents it serves. A task that names only an intent is held to that intent's guards and
to no criterion: no criterion is one session's to prove. A task that names nothing, one
started under an exemption, and maintenance are exempt; the `issue` question already says
what a task that names nothing owes. The task's scope is never sent to the engine as paths:
a finish is held to what the task said it serves, not to what its paths touch.

**Could not ask is never exempt.** No active task, a binding that could not be executed or
read, and a binding refused before it reached an intent — an issue the plan does not hold —
are `unknown`, in every mode. A binding refused late (a stale critique, an open blocking
finding) still has its criteria read: whether the plan was reviewed is `start`'s question.

**A policy key says whether an owed answer is held: `intent.completion`.**

| | the two questions |
|---|---|
| `off`, or absent | are not asked, and say so |
| `advisory` | are answered; a verdict that would be owed is reported, and withheld |
| `required` | are answered and held: where `completed_means_complete` is true, an owed answer refuses `completed` |

Under `advisory` a withheld answer has the status `exempt`, so `complete` and `finish` behave
as before, and it carries the status it would have had as data — `withheld` on the question
— not only in prose. An answer that was reached and not held is not the same thing as a
question that does not apply, and the difference is countable: `check` prints each withheld
answer, and the number of them is the measure of how far this repository is from `required`.

This repository is `advisory`, for the reason `intent.binding` is: it was measured. The
skeleton is `off`.

**The remediation names what makes evidence current**, not merely the test: commit the work,
run the criterion's test on the clean tree, stamp the tree the run left, record the run with
that stamp, commit the ledger. And it always says that a partial finish is available. A
record without a stamp carries an unknown tree and is never current, so the stamp is named.

**Several issues serving one criterion** are each held to it: the criterion is not met for
any of them until it is met, and the last to finish is not singled out.

**A violated guard holds every task under its intent**, including the one that did not break
it. That is deliberate and it is the same decision ADR 0113 took for the verdict; `start` is
not refused (the task may be the repair), and under `required` the repair's own finish is
`completed` only once the guard holds.

## Consequences

- Finish records nothing about an intent, and records no evidence. The run that produced
  evidence records it (`majordomus evidence record`); a second writer for the ledger was
  rejected, and a verify command run over a dirty tree would be evidence of nothing.
- Recording the run has an order and a cost, both found by walking the path in case 966.
  The ledger is committed before the gates are run and the obligations discharged, or the
  evidence taken for those no longer describes the tree. And the ledger is an input of the
  derived data, so committing it makes the change owe `generated`, which is discharged the
  way any generated obligation is. Neither is hidden: the report names both.
- A repository with no CI model is asked none of this: the completion gates are not judged
  there at all. An intent in such a repository is held at `start` and by `intent validate`,
  and not at `finish`.
- A criterion of a kind the ledger cannot derive keeps `criteria-served` unknown for the
  issues serving it, so `required` puts `completed` out of their reach until that evidence
  can be derived. This repository has one such criterion. It is a reason not to choose
  `required` yet, and it is stated rather than papered over with a pass.
- A squash merge leaves the commit a run was recorded at outside the history, and its
  evidence then reads stale. Recording after the merge, or merging without squashing, keeps
  it.
- The completion answer now derives the intents and the evidence on every `check` for a
  task that names work. A task that names nothing does not ask the engine.
- Moving from `advisory` to `required` is the owner's switch. What a release can be asked to
  lower first is a repository figure, not a task's: `issues_without_current_evidence`, the
  number of issues that declare what they serve and whose served required criteria are not
  all current, with their ids — 22 of 22 on the day this was written. It is to be exposed
  read-only from the binding, reported in a release and not yet refused; the release-debt
  declaration gains a `measured` form naming a capability and a field, and this figure
  becomes one declared entry there, counted, recorded and payable. That work is not in this
  change.

## Alternatives rejected

- **Refuse outright, with no key.** Measured: every issue-bound task refused on the first
  day, and the word `completed` retired in practice.
- **Count never-run evidence as passing, or as not applicable.** That is a pass invented
  from an absence, which the completion policy exists to refuse.
- **Record the evidence at finish from the verify command.** The command is arbitrary and
  usually runs over uncommitted work; a ledger with two writers is two ledgers.
- **A new status for "withheld".** Every surface and the stage fold would have to learn it.
  The status stays one of the existing ones and the withheld verdict is data beside it.
- **Hold a task that names only an intent to every criterion of it.** The requirement says a
  session is not responsible for a whole intent, and the engine would bind it to every open
  issue's criteria.
- **Ask at finish whether the plan's review is current.** `start` asks it; asking twice
  gives two places to disagree.
