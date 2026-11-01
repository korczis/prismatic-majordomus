+++
title = "A plan is attacked before it is built"
description = "In this repository, work on an intent does not start because someone has a plan. It starts once the plan has been attacked and every blocking finding has an answer a reader can check. What the critiques found, what was rejected and why, and what a critique cannot see."
template = "page.html"
[extra]
lang = "en"
related = ["a-pasted-brief-becomes-a-plan/", "proof-carrying-documentation/", "seats-not-names/", "written-once-derived-everywhere/", "a-doubt-is-settled-by-an-experiment/"]
[extra.translations]
cs = "plan-se-napadne-driv-nez-se-postavi/"
+++

A plan written by the same agent that will carry it out has a familiar weakness: it agrees with itself. The steps follow from the assumptions, the assumptions are never tested, and nothing in the plan says what would have to be true for it to be wrong. Code review arrives too late to help, because by then the plan has become code.

This repository puts an attack between the plan and the work. An intent is a typed record of what must become true, with criteria a test can settle ([ADR 0070](@/adrs/adr-0070.md)). The issues that realise it each name the criterion they serve ([ADR 0073](@/adrs/adr-0073.md)). Before any of them may start, the plan has to be criticised, and the criticism has to be answered.

## A critique is a record, not a conversation

A critique lives next to the intent it reviews, one file per intent under `.ai/repo/project/critiques/`. It records the commit the plan was reviewed at, who reviewed it, and a list of findings. Each finding is classed by the question it asks:

| class | the question |
|---|---|
| `missed_requirement` | what does the plan not do that its intent requires? |
| `unproven_assumption` | what does it take for granted that nobody has shown? |
| `insufficient_work` | where does a planned issue stop short of its criterion? |
| `unnecessary_work` | what does it build that nothing asks for? |
| `regression_risk` | what that works today could it break? |
| `surface_missing` | which other file, schema or page must change with it? |
| `delivery_verification` | what has to be seen running, not only written? |

Each finding is either blocking or not. Each one is resolved in one of three ways: `planned` into a named issue that serves the intent, `rejected` with a reason, or left `open`. The planning manual is plain about the reason: "A dismissal nobody can read is not a resolution."

## The gate reads it

The record would be an ornament if nothing read it. Before a task starts, `majordomus intent preflight` asks whether the work may proceed. It refuses with `intent_not_critiqued` when an intent the issue serves has no critique, and with `open_blocking_finding` when a blocking finding of that critique is still open. Where the repository's policy sets `intent.binding` to `required`, such an issue never becomes active. Work that started anyway is caught later: `intent validate` and the `intent-check` gate fail it as `executing_without_critique` or `executing_with_open_blocker`.

What the repository does not do is write the critique. The manual says it directly: "Majordomus judges these records; it does not write them." A person or an agent does the criticising. The repository refuses the result when it does not hold together.

## What the critiques found

One critique, of the plan that bound every task to an intent, shows what this produces. The plan's governance cited ADR 0111, an ADR that existed in no tree, and an unresolved reference fails `intent validate`. The finding was planned: the ADR became a deliverable of issue I2000, landing with the intent record. Another finding was more serious. `plan start` and the `plan.transition` capability could move an issue to active with no preflight at all, so the gate at `start` could simply be walked around. That was planned into an issue that gates the start transition in both plan engines.

Not every finding is accepted. The same requirement asked that a stale critique should block a start. The finding that the plan did not do this was rejected, and the reason is written into the record. A critique records the commit it was written at, not a revision of the plan, so there was nothing to judge staleness against. A later milestone gives the record that revision, and the intent names the limitation as a non-goal. The rejection is the useful part: a reader can disagree with it, because it says why.

And not every finding has to be closed before work starts. A non-blocking finding about the planning intent observes that a reader of majordomus.dev cannot see the planning half until the generated documentation is deployed and verified. It is still `open`, and that is allowed, because it blocks nothing and says so.

## Who does the attacking

The record says who wrote it, in its `reviewed_by` field, and the answers are not all the same. Some critiques were written by a fresh subagent. It was handed the plan records, the requirement and the code, and none of the author's reasoning. One of those records adds a sentence worth quoting in full: the reviewer was the "same model and operator session as the author, so independent of the authoring context and not of its blind spots".

Others are self-reviews, and say so. "SELF-REVIEW: the session that authored I2780 to I2783 also wrote this critique," one of them reads, and it goes on: "a critique carries more weight when another session or the owner writes it."

That is the honest limit of the method, and the records state it rather than hide it. A second reading by the same kind of mind catches what the first one was too busy agreeing with: a missing file, a gate with a side door, a figure no test proves. It does not catch what that kind of mind cannot see. Nothing stops another session, another model or the owner from writing the next critique, and the field says which one did.

## What a critique cannot do

The gate checks that a critique exists and that no blocking finding is open. It does not check that the critique was any good. A reviewer who finds nothing produces a record with no findings, and the gate lets the work start. A critique is also not compared with later versions of the plan, as the rejected finding above records, so a plan rewritten after its review is not refused for that. What the record guarantees is narrower and still worth having. Every objection somebody did raise is in the repository, next to the plan it objects to, with its answer, and the answer can be read and refused.

The method is described in [the planning manual](@/docs/planning.md). The decisions are [ADR 0070](@/adrs/adr-0070.md) on intents and [ADR 0073](@/adrs/adr-0073.md) on what an issue serves. The critiques themselves are plain YAML under `.ai/repo/project/critiques/` in the repository.
