+++
title = "A doubt is settled by an experiment"
description = "A material engineering uncertainty decided silently cannot be audited, and an advisor's answer pasted into a chat is gone by the next session. How Majordomus records the doubt with its evidence first, computes how much independent review it is worth, treats an absent advisor as state, settles a disagreement by an experiment rather than a count, and hands the conclusion to the next session."
template = "page.html"
[extra]
lang = "en"
related = ["a-plan-is-attacked-before-it-is-built/", "a-session-ends-the-work-does-not/", "many-machines-one-journal/"]
[extra.translations]
cs = "pochybnost-rozhodne-experiment/"
+++

A session that is unsure of an engineering decision has two easy ways out: decide alone and say nothing, or paste the question into a chat with another model and follow the answer. [`docs/REASONING.md`](@/docs/reasoning.md) names what each one costs. Uncertainty decided silently is uncertainty nobody can audit, and an advisor's opinion pasted into a chat is gone by the next session.

[ADR 0098](@/adrs/adr-0098.md) (proposed), dated 1 October 2026, starts from the owner's working protocol for such a doubt: investigate locally, ask one vendor's model and then another's, synthesise, continue. It lived in one worker's private notes and a helper script in a session scratchpad, so nothing recorded what an advisor said, and nothing could tell a review that happened from one that was only claimed. It was also a sequence of vendor names, a shape that breaks when a CLI is not installed or a key is missing. The decision makes the uncertainty, the advice and the decision typed records with one writer, and keeps the vendors out of the logic. The working session owns every decision, and advisors decide nothing.

## Evidence comes first

The chain starts with an assessment: the subject in one line, how material the uncertainty is on a scale from `trivial` to `critical`, how confident the session is, its hypothesis, and the evidence gathered so far. Evidence is a reference, never a copy: a file and line, a test, an ADR, a rule, a commit, a command and what it showed, a runtime observation.

The writer enforces the order. An assessment of `material`, `high` or `critical` uncertainty that cites no evidence is refused, with the reason that a material assessment carries the evidence gathered before anyone is asked.

One such doubt was weighed on 21 October 2026. A planned change would have dated a landing by the forge's record of its merge, read where the executor observes the forge before every decision, and a test holds that this read never asks for merged pull requests. The session measured before it decided: this clone's reflog had seen each of the last four landings within 94 seconds of the forge's record, against a threshold of six hours. Its conclusion kept the reflog and set the forge read aside, and [`docs/RELEASE.md`](@/docs/release.md) states the measurement that decided it.

## The policy computes the budget

A pure policy then decides whether independent review is worth having, starting with a budget: how many advisors a plan may select. Under `standard`, the mode that applies when nothing sets another, a trivial or low doubt gets no advisor, a material one gets one, and a high or critical one gets two. `strict` allows three for a critical doubt, `fast` at most one, and `ci`, the mode of a CI runner, none. High confidence in the local hypothesis lowers the budget by one.

The plan is computed, never supplied. A caller names only the assessment, and the writer computes the plan from the mode and every advisor's availability at that moment, then stores that snapshot with it. Each advisor left out is listed with its reason. A consultation is admitted only for an advisor its plan selected, so nobody can record a review that no plan asked for.

## An absent advisor is a status

The policy names capabilities, such as `independent_reasoning` or `code_review`, and never an advisor. Which advisors offer one is the answer of the catalogue, `share/advisors.yaml`, whose order holds a repository's preference; whether they can offer it now is the answer of availability. A linked mesh runtime that offers reviews is an advisor too.

Availability is derived from presence, the mode and recorded outcomes, and nothing on the way into a repository, in `doctor` or in CI contacts a model. Presence is an executable on `PATH` or a credential variable set, whose value the crate never reads; whether an advisor answers is learned by asking. An advisor that is missing or switched off is `unavailable`, `not_configured` or `disabled`, and recorded failures make it `temporarily_failed` or `rate_limited` for a cooldown. The bootstrap the repository generates for its workers says it plainly: no advisor is needed, and an absent one is reported state, never a reason to stop or to ask a person what to do.

When the plan selects nobody for a material doubt, it carries the structured local review instead. The session states the hypothesis and its strongest alternative, lists each assumption with the evidence behind it, names what would falsify the hypothesis, runs the smallest experiment that discriminates between them, and records the conclusion.

The rule [project.advisors-are-optional](@/rules/project-advisors-are-optional-1.md) records why. The first time the protocol ran through code, in the session that built ADR 0098, one advisor's credential was a placeholder and another refused to run in an untrusted directory. A workflow written as "ask this one, then that one" would have stopped there. Recorded as `auth_failed` and `error`, the two failures cost one bounded wait each, and the work went on.

## No count decides

When advisors disagree with each other or with the session, the disagreement is a record of its own: the positions, the assumptions they divide on, and the question whose answer decides. A conclusion on that assessment is refused until a resolution settles the disagreement, and a resolution is refused without evidence. It names the experiment, what it showed and the position it favours, which may be neither.

The number of advisors on each side never enters the decision. The rule [project.review-is-recorded-not-claimed](@/rules/project-review-is-recorded-not-claimed-1.md) gives the reason: models share training data and failure modes, so a count of agreeing answers measures correlation, not correctness. Case 732 holds this with two scripted advisors, one supporting the session's hypothesis and one proposing another. With two of the three positions agreeing, the writer refuses the conclusion and a "resolution" without evidence, and it admits the minority's position once an experiment settles the question.

## A conclusion the next session can read

The conclusion records the decision, its rationale and evidence, the consultations it weighed, the alternatives it rejected, the risks that remain and a validation plan. It is refused while it leaves out an answer its assessment received, and on material uncertainty it is refused without evidence or a validation plan. Who reviewed it, `reviewed_by`, and how many, `independent_review_count`, are computed by the writer from the consultations it cites. The input has no field for either, because a review count a caller can type is a review a caller can invent.

The records are checkout state, one JSON file each under `.ai/local/state/reasoning/<task>/`, never tracked and never published. `majordomus reasoning status` derives the task's state from them, and the same report appears in `majordomus context` and in every handover that `majordomus handover --derive` writes. A new session reads the decision and who reviewed it instead of rediscovering it. A plan for a later assessment of the same subject reuses the standing conclusion and consults nobody, unless that assessment declares new evidence; a new conclusion then names the old one in `supersedes`.

## What it does not do

It keeps no transcript, only the fields an adapter normalised from each answer. The executable calls no model, because network code stays out of the crate. It does not check that the local review was really performed, or judge whether an experiment was a good one; a reviewer judges those from the record. Subjects match by normalised text, not by meaning. And the review count counts advisors, not sources, so two mesh peers behind one adapter count as two reviews; issue I3419 plans to change that.

ADR 0098 is still proposed, and the rules that carry it are active and blocking. They are held by cases 730 to 736 and by `reasoning check`, which refuses provider-independent code that names an advisor, vendor or model of the catalogue, and CI that names a model credential. The same bootstrap sums up the practice: record the assessment with its evidence, let `scripts/advisor-consult` ask whichever optional advisors the plan selects, settle a disagreement with an experiment rather than a count, record the conclusion, and continue. How a plan is challenged is in [A plan is attacked before it is built](@/a-plan-is-attacked-before-it-is-built.md), how work outlives a session in [A session ends, the work does not](@/a-session-ends-the-work-does-not.md), how a review crosses machines in [Many machines, one journal](@/many-machines-one-journal.md), and the design in [`docs/REASONING.md`](@/docs/reasoning.md).
