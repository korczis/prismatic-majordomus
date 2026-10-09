---
schema: adr/v1
id: adr-0116
kind: adr
title: GitHub shows what an issue serves, and a milestone there follows the plan and not an intent's verdict
status: proposed
date: 2026-10-08
tags:
  - intent
  - github
  - projection
  - governance
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - rule:project.github-projection-gated
  - rule:project.derived-once
  - file:lib/plan.sh
  - file:scripts/github-sync
  - file:.ai/repo/project/intents/github-agrees-with-the-intent.yaml
  - test:test/cases/968_an_issue_body_names_what_it_serves.sh
---

# 116. GitHub shows what an issue serves, and a milestone there follows the plan and not an intent's verdict

## Context

GitHub is a projection of the plan (`project.github-projection-gated`): each milestone and
issue is generated from its canonical record, a hand edit of the generated region is
reported, and nothing is read back. It reads no intent. An issue there says what it is and
nothing of why it exists, and a milestone closes when its issues are done.

ADR 0070 and ADR 0075 each left "milestones reconciled with intents" as a later slice, and
the claim `intent-closes-github-milestones` is planned. The first plan for it had four
parts: the body names what an issue serves; a capability derives what GitHub should show; a
milestone is wanted closed only when every intent that names it has the verdict `satisfied`;
the gate refuses the disagreement. It was reviewed before any of it was built, and three of
the four parts did not survive.

### What the review found

- **A verdict of `unknown` never closes.** An intent with a criterion whose evidence is a
  command or a deployment can at best read `unknown`. `intent-lifecycle` has one. Its
  milestone could never close on GitHub.
- **The remote state would move with evidence.** A criterion is current only until something
  an issue serving it names changes. A milestone that was done and closed would be wanted
  open again after an unrelated commit, with no record edited — a `state` finding, which the
  gate refuses outright, on every branch, cleared only by an apply from the trunk; and where
  the staleness is a branch's own, by nothing.
- **The gate has no executable where it runs.** The projection gate is in the structure job,
  which builds none. An adapter that must ask the intent engine and may not fall back would
  report "cannot run" on every pull request.
- **The typed comparison had no caller.** The adapter and the gate would have kept their
  printed words; a function reachable only from tests is not a derivation anybody uses.

## Decision

**What an issue serves is projected.** The generated body of an issue carries a `Serves`
section listing what the record declares under `serves`, as authored and unjudged. It is
inside the generated region, so the states the projection already has judge it: a changed
mapping moves the body's hash and reads `behind`, a hand edit reads `edited`, and the gate
refuses both. An issue that serves nothing has no such section.

**Where a criterion stands is not projected.** It is the intent engine's answer and moves
with the evidence, so the body names the command that shows it and prints none of it. A body
moves when a record moves and never because a run went stale.

**A milestone's state on GitHub follows the plan.** It is closed when its derived status is
done or cancelled, as before. That an intent it realises is not satisfied is a fact about
this repository, reported here: `majordomus intent realization` names
`closed_work_not_satisfied` and the criteria holding it back, and the `intent-realization`
gate runs it. GitHub is not asked to show it and is never read to decide it.

**Who owns what.**

| | owned by | what the other side may do |
|---|---|---|
| an issue's state, milestone, labels, generated body | the record | GitHub's copy is overwritten on apply; a difference is drift |
| what an issue serves | the record, shown in the generated body | as above |
| a criterion's state, an intent's verdict or stage | the intent engine, here | never projected, never read from GitHub |
| an issue's title | the record when the issue is created | a later edit on GitHub is neither pushed over nor reported; an issue is matched by the marker in its body |
| comments, assignees, text outside the generated region | GitHub | never touched |
| a milestone's identity | the id that prefixes its title | renaming the prefix orphans it; no marker exists |

**The requirement's tests, and where each is held.**

| the requirement asks for | held by |
|---|---|
| the initial projection | case 45 (the payload is derived from the record) |
| an idempotent second sync | case 602 for a body; a milestone's description is rewritten on every apply, which is stated and not held |
| a title edited on GitHub | case 45: a renamed issue matched by its marker is in sync — by decision not a finding |
| a remote close | case 894 (reopened from the trunk) |
| a changed criterion mapping | case 968 |
| a moved milestone | case 894 (reassignment) |
| a deleted remote object | reads `missing`; no case of its own |
| a stale generated region | cases 45 and 97 |
| an apply from anywhere but the trunk | case 894 |
| a failed read or write | case 894 |

## Consequences

- From the moment the `Serves` section lands, every projected issue that declares `serves`
  reads `behind` on every branch until the projection is applied from the trunk. Four do
  today. The change is landed with that apply arranged, as a closure of issues is.
- The claim `intent-closes-github-milestones` stays planned. This record proposes that it
  not be built as written; whether to withdraw it, or to build it once a verdict can be
  judged on the trunk alone and the gate has an executable, is the owner's decision.
- The findings of the projection stay printed words that the gate counts. Typing them is
  worth doing when something consumes the types.
- A milestone that exists on GitHub and in no record, a duplicate milestone and a title
  changed on GitHub are not reported. Each is listed as a non-goal of the intent with its
  reason.

## Alternatives rejected

- **Close a milestone only when its intents are satisfied.** Found unsound in the four ways
  above before it was built.
- **Print each criterion's state in the body.** Every ledger change would make every serving
  issue read `behind`.
- **A marker in a milestone's description for a rename-safe identity.** Not forbidden, and
  not needed for what is decided here; the description is not read back by a check today.
- **A mapping file from record to GitHub number.** Rejected once already
  (`docs/PLANNING.md`): state to keep in step with two systems.
