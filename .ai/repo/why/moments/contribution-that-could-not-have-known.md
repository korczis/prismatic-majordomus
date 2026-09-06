---
schema: moment/v1
id: contribution-that-could-not-have-known
kind: moment
title: 'A fluent contribution that could not have known the rules'
short_title: 'Uninformed contribution'
hook: 'reviewed a well-written contribution that broke a convention it had no way to discover'
summary: 'Assistants let anybody produce plausible contributions at volume; the conventions that make one correct are still only in the maintainers.'
status: stable
severity: high
frequency: common
weight: 350
audiences: [open-source-maintainer, platform-team, agency, enterprise]
areas: [governance, context]
lifecycle: [review, onboarding]
tags: [contribution, review, conventions, governance]
signals:
  - id: plausible-and-wrong
    text: 'Contributions arrive that are fluent, plausible and wrong in a project-specific way.'
  - id: same-review-comment
    text: 'The same review comment is written repeatedly to different contributors.'
  - id: review-is-the-gate
    text: 'The only mechanism transmitting the project''s conventions is review.'
examples:
  - id: ai-assisted-pr
    audience: open-source-maintainer
    title: 'An assisted pull request'
    before: 'A contributor''s assistant produces a clean, idiomatic change that violates a decision the project made two years ago.'
    after: 'The conventions and the decisions are objects in the repository, discoverable from the paths they govern, before the change is written.'
  - id: review-does-not-scale
    audience: platform-team
    title: 'Review as the only channel'
    before: 'Contribution volume rises with assistants and review capacity does not; the queue becomes the bottleneck and the conventions the cost.'
    after: 'The rules are loadable by whatever is doing the writing, so review stops being the transmission mechanism.'
  - id: vendor-contribution
    audience: enterprise
    title: 'Work from outside the team'
    before: 'An external contribution meets the standards it could see and misses the ones held internally.'
    after: 'The effective rule set is readable — vendored baseline plus the project''s own — and says of each rule whether anything enforces it.'
commands: [context, rules, doctrine]
capabilities: [objects.list, objects.get]
claims: [context-documents, rule-resolution, vendored-rule-package, doctrine-class-decides, context-impact]
doctrines: [majordomus.context-integrity, majordomus.rule-package-integrity, project.context-locality, majordomus.doctrine-wiring-integrity]
use_cases: [read-the-rules-the-tool-applies, document-every-directory-of-the-layer, adopt-an-existing-repository]
related: [rule-in-a-readme-nobody-loaded, first-hour-in-an-unfamiliar-repository, re-arguing-a-settled-decision]
aliases: ['AI pull requests', 'contributor conventions', 'review does not scale']
---

## The moment

The contribution is well written. The tests pass. It uses the pattern the project abandoned
two years ago, for reasons that are excellent and recorded nowhere the contributor could
reach. The maintainer writes the explanation for the eleventh time.

## Why it happens

A project's real conventions are the residue of its arguments, and arguments are not
documents. They survive in the maintainers and in old review threads. Assistants changed the
economics on one side only: producing a plausible contribution is now nearly free, and
transmitting the conventions is exactly as expensive as it was.

## Why a better model does not fix it

The contributor's assistant did well with what it had — the code, the README, the tests.
None of them contains the reason the abandoned pattern was abandoned. A stronger model makes
a more convincing case for the wrong pattern.

## What it costs

The maintainer's scarcest resource, spent on repetition. And a queue that grows faster than
it drains, which eventually converts an open project into a closed one.

## What Majordomus does

The rules are objects, not folklore: the effective set is the vendored baseline plus the
project's own, resolved as a dependency graph, each saying whether the tool enforces it or
nobody does. The context documents attach to the directories and paths they govern and
compose for the path being changed, so a contributor's assistant can be told to resolve them
before writing. Decisions carry their reason and the alternative that was rejected, which is
the sentence review keeps having to supply.

## Before and after

```text
before   review comment #11: "we don't use that pattern here, because ..."

after    $ majordomus context resolve lib/auth
         $ majordomus rules list
         project.no-claim-without-test   blocking   enforced by: review
         majordomus.scope-integrity      blocking   enforced by: check, finish, watch
```

## What it does not do

It does not gate contributions, and it cannot make anyone read anything. It converts the
conventions from something a maintainer transmits into something a contributor can load.
