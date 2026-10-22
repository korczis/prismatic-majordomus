+++
title = "The bet on a layer, not a model"
description = "Why Majordomus is not another AI agent: models will become interchangeable, their makers will take coordination for themselves, and what is left is impartial proof that the work is done."
template = "page.html"
[extra]
lang = "en"
translated_from = "teze/"
source_digest = "a49a543c1ad7315a0c264fbf330c543516ebc221058995d5abddd2509fd26cf6"
related = ["proof-carrying-documentation/", "a-pasted-brief-becomes-a-plan/", "everything-followed/", "seats-not-names/", "written-once-derived-everywhere/", "a-session-ends-the-work-does-not/"]
[extra.translations]
cs = "teze/"
+++

Models will be interchangeable. I do not know which one will be best a year from now, and building a product on the assumption that I do would be a lottery ticket. My bet is on something duller: that the value will sit in the layer that can direct the models' work and, above all, judge it.

## Engines came out of games

The usual comparison is with game engines. Every studio once wrote its own infrastructure; then a market for ready-made engines appeared, and today hardly anyone writes their own. That is true, but it leaves something out. The engines that won did not start as infrastructure looking for a customer. They were split off from a particular game that had tested them first.

My game is this repository. Majordomus was distilled from environments where agents worked at scale and failed for boring reasons. The numbers are in the README: instruction files for two tools with almost no content in common, checks documented as blocking that nothing invokes, dozens of worktrees full of duplicated patches of which few were ever merged. Since then it has supervised itself. Changes here go through the same `finish` I offer everyone else.

## Coordination cannot be defended

Coordinating agents is the most visible part of the whole thing, and the weakest. The model makers are adding it to their own tools: subagents, agent teams, runs in the cloud. Whoever builds only orchestration is building on land that belongs to someone else.

What a model maker cannot take is impartiality. I find it hard to believe that a lab will fairly judge the work of a competitor's model, and harder still that it will judge its own. A proof that the work is done is worth something precisely because it does not belong to whoever did the work.

## What proof means here

"Proof of correctness" for nondeterministic output is hand-waving until someone says exactly what they mean by it. Here it means a contract. When a worker declares its work done, `majordomus finish` walks every line of that contract: open blockers, the handover note, whether the changes stayed inside the declared scope, whether the recorded state matches HEAD, and whether the verification command passes. If a single line fails, it records nothing and says how to reproduce the failure.

A recording from [the challenge](@/challenge.md), anonymised and otherwise untouched. The scope is clean, but the repository's own test fails, so the worker's claim that it is done does not pass:

```
$ majordomus finish --outcome completed --verify-command 'sh test/parse.sh'
OK   blockers    t-<id> — none open
OK   note        t-<id> — <time>--main--<head>--<hash>.md
OK   scope       t-<id> — 1 touched file(s), all within scope
OK   state       t-<id> — exact (head <head>)
FAIL verification t-<id> — sh test/parse.sh — exit 1, <s>s  [reproduce: sh test/parse.sh]
finish: refused, 1 unmet
```

It does not say the code is right. It says which particular conditions of "done" are met and which are not, and it hands over a command anyone can use to check again. I do not dare claim more than that, and I do not need to.

The site you are reading follows the same pattern. When this page is built, {{ <cite_figure name="claims_guaranteed" /> }} of the site's {{ <cite_figure name="claims_total" /> }} claims stand as guaranteed, and each of them has a test that would fail. How that works is the subject of [Proof-carrying documentation](@/proof-carrying-documentation.md).

## Today's pain, not tomorrow's

Almost nobody runs hundreds of agents in one repository today. The questions teams are already asking are more down to earth: [who did this, and under what policy](@/why/who-did-this-and-under-what-policy.md), [what the money was spent on](@/why/spend-not-tied-to-outcomes.md), [whether it is done only because the model said so](@/why/done-because-the-model-said-so.md). Those are the ones I answer first. Scale will come or it will not; a tool that cannot answer for one agent will not carry it either way.

I am also betting that in regulated fields the question "who did this, and under what policy" will be asked more and more often. That is a bet, not a claim. Majordomus makes nobody compliant.

## A format, not a platform

`.ai/` is ordinary files in git. Majordomus runs locally and never calls a model. That is deliberate. A platform wants you to move into it; a format wants anyone to be able to read it: Claude, Codex, Gemini, and a tool that does not exist yet. If this layer is to outlive the next change of models, it must not become one more place you cannot leave.

## What I do not claim

I do not claim that Majordomus verifies that code is correct, that it replaces review, or that it can carry hundreds of agents. That last one I have not measured. What it does not do is listed under [limitations](../limitations/); what is guaranteed, what is only recommended, what is planned and what was rejected is in `docs/SITE_CLAIMS.md`. Where this page claims more than they do, they are the ones that hold.
