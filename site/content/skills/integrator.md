+++
title = "Take finished work to a verified master"
description = "Carry a finished change from an open pull request to master and beyond through the one integration engine and the one landing seat, answering at each stage why it is not there yet and stopping only at a landing verified on master, a release, or a named blocker with its next act."
weight = 5
[extra]
id = "integrator"
status = "active"
version = 1
source = ".ai/repo/skills/integrator/SKILL.md"
+++
{% raw %}

## Purpose

Implementation is not delivery. A pull request opened is a request; a green check is a
verdict on one head; only a commit on master whose required check passed is integrated, and
only a tag the release workflow published, and a site that serves it, is delivered. This
skill walks a change through those stages with the engine this repository already has — it
adds no queue, no lander and no second state.

The engine is `majordomus prs` (ADR 0101, `docs/INTEGRATION.md`): it observes the forge,
relates every head to master with git, classifies every pull request into one disposition
with its evidence, ranks them, and lands the next provably safe one by a non-forced
fast-forward pinned to the head CI judged. Every surface — the CLI, `integration.*` over
HTTP and MCP, the Cockpit, the briefing — renders that one assessment.

## When to use

- Implementation is done and pushed, and the question is "when is it on master?".
- `/proceed` finds finished work that has not landed, or landed work that is not released.
- You hold the landing seat and the queue has ready heads.
- A train failed, or a pull request has waited longer than its peers.

Not this skill: bringing a branch current (`rebaser`), deciding what to build next
(`majordomus plan next`), or recording lessons (`evolve`, which this skill feeds).

## Procedure

### 1. Where is it, and why

```
majordomus prs refresh
majordomus prs status             # every open pull request: disposition, lane, rank, waiting
majordomus prs explain <n>        # one of them: the evidence for each answer
majordomus prs plan               # what the executor would do next, changing nothing
```

Read the disposition table in `docs/INTEGRATION.md`. Every non-ready disposition names its
next act and whose it is. A pull request is never "probably fine": a required check that is
pending, missing, skipped or unreadable is not passed.

### 2. Who may move master

Exactly one session holds `seat/landing`, an exclusive mesh claim
(`project.the-cooperation-model-is-the-owners-word`, ADR 0167); `majordomus mesh peers --full`
shows it, and the integration lease (`prs brief`) shows which executor holds the base.
Branch protection requires the `ci` check; it does not require a branch to be up to date,
so the fast-forward the drain uses is what keeps master equal to a judged tree.

- **Not the seat holder:** make the head ready and hand it over. It contains master
  (`rebaser`), its required check passed on that head, it is not labelled `hold` unless it
  is a train member, and its issue and evidence are recorded. Announce the head on the board
  with `majordomus_announce` (or `majordomus mesh session`), naming the pull request and the
  sha. Do not merge it yourself, and do not re-run its checks to hurry it.
- **The seat holder:** `majordomus prs drain --dry-run` decides, then `majordomus prs drain`
  lands — one merge at a time, observe → decide → observe again → act → verify. A stale
  decision is recorded and the next step starts from the new master; never force it.

### 3. Choose the lane by what it costs

<div class="overflow-x-auto" tabindex="0">

| Situation | Lane |
|---|---|
| one independent pull request, ready | **drain**: it lands on its own required check |
| several related or derived-data-heavy members | **train**: one top combines them (`docs/INTEGRATION.md`, Trains); members carry `hold` and their own runs give their slots back (I4338) |
| a member of a red top | **split**: the top judges its members; drop the failing member to its own lane and land the rest, rather than holding everything behind one red |
| conflict on an authored path | **repair**: the owner's (`rebaser`, row `conflicting`) |
| waiting on a lower link, a review, a label | **wait**, with the blocker named |

</div>


Cost here is CI time: one required-check run took 226 minutes at the median in the week to
2026-11-01, and refresh throughput is one pull request per run. A strategy that adds a run
(a needless refresh, a re-run, a label that starts a run) must buy more than it costs.

### 4. Do not starve the queue

- A pull request passed over three times is `starving` (`prs status`): put it in the next
  train or give it the seat's next refresh, do not refresh it again on its own.
- A run that yielded (`ci:yielded`) is re-requested by `scripts/ci/release-path resume`;
  do not re-run it by hand while the release path holds — the re-run yields again.
- A label other than `ci:full` or `hold` starts no second verdict (I4347,
  `scripts/ci/carried-verdict`); `ci:full` deliberately does.

### 5. Verify the landing, then the release

Landed means all of these, each read, not assumed:

```
majordomus prs events             # the landing recorded, master before and after
git fetch origin && git merge-base --is-ancestor <head> origin/master   # exit 0
gh run list --workflow validate --branch master --limit 1               # master's own verdict on that commit
```

Then delivery: `majordomus run release.currency` says whether landed work waits past the
last tag (the release seat's act), and `majordomus served` / `majordomus deploy` say what
the site serves. A merge the forge accepted is not a green master, and a green master is not
a release.

### 6. Close the loop

Record the issue's evidence against the commit that landed it, and hand what the attempt
taught to `evolve`: a repeated stale decision, a train that failed for one member, a run
that yielded twice. Report the attempt's own numbers from `prs status` and `prs events`
(wait, passes over, retries), never an estimate.

## Output

<div class="overflow-x-auto" tabindex="0">

| pull request | stage reached | evidence (run, commit) | blocker, owner, next act |
|---|---|---|---|

</div>


with the stages open → ready → landed → released → deployed, each marked with the states of
`report-verification-state`. A pull request handed to the seat but not landed is PARTIAL;
nothing is done before master's own run on the landed commit has passed.
{% endraw %}
