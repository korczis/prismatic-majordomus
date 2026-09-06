---
schema: moment/v1
id: enforcement-nothing-invokes
kind: moment
title: 'A rule enforced by a hook that never runs'
short_title: 'Enforcement fiction'
hook: 'found a rule everyone believed was enforced and nothing ever invoked'
summary: 'Every artefact of enforcement exists — the rule, the script, the test — and no path connects them, so the control is fiction.'
status: stable
severity: high
frequency: common
weight: 180
audiences: [platform-team, enterprise, open-source-maintainer, engineering-lead]
areas: [governance, verification]
lifecycle: [maintenance, review]
tags: [enforcement, hooks, policy, wiring]
signals:
  - id: believed-enforced
    text: 'A rule is described as enforced and nobody has watched it stop anything.'
  - id: hook-exists-unwired
    text: 'A check script exists on disk and nothing is known to invoke it.'
  - id: exit-code-swallowed
    text: 'A gate runs and its exit code is discarded by whatever runs it.'
examples:
  - id: pre-commit-not-installed
    audience: platform-team
    title: 'The hook nobody installed'
    before: 'A pre-commit check is documented, committed and never installed in anyone''s clone, so it has stopped exactly nothing.'
    after: '`doctor` reconciles the declared enforcement against the hook that names it, and fails when the path is missing, not executable or not invoked.'
  - id: exit-swallowed
    audience: enterprise
    title: 'The gate whose result was discarded'
    before: 'CI runs the check inside a shell pipeline that swallows its exit code; the pipeline is green whatever the check says.'
    after: 'Reconciliation checks that the exit code is propagated, not merely that the command appears.'
  - id: contributor-check
    audience: open-source-maintainer
    title: 'The convention only review enforces'
    before: 'A convention is called a rule, and its enforcement is the maintainer noticing.'
    after: 'A rule declares whether the tool enforces it or nobody does, so an honestly advisory rule is labelled as one.'
commands: [doctor, doctrine, rules]
capabilities: [health.report]
responsibilities: [doctor, policy]
claims: [wiring-reconciliation, dispatcher-wiring, doctrine-registry, doctrine-class-decides, exit-code-contract]
doctrines: [majordomus.enforcement-wiring, majordomus.doctrine-wiring-integrity, project.rule-is-a-doctrine, majordomus.command-surface]
use_cases: [prove-a-rule-is-enforced, gate-ci-on-the-tool-itself, read-the-rules-the-tool-applies]
related: [two-rulebooks-one-repository, feature-without-a-test, site-claims-nothing-proves]
aliases: ['unwired hook', 'rule not enforced', 'declared but not wired']
---

## The moment

The repository says a rule is enforced. The rule is written down, the script that checks it
exists, there is even a test for the script. Nothing invokes it. It has never refused
anything, and no one noticed, because everything that would demonstrate the enforcement
exists except the enforcement.

## Why it happens

Enforcement is a chain — rule, validator, dispatch, exit code, test, CI — and every link
except one can be present. Each link is added by a different change at a different time,
and no single artefact represents the whole chain, so nothing can be inspected to find the
missing link. Seventeen such cases were found in the material this tool was distilled from.

## Why a better model does not fix it

There is no model in this failure at all. It is a structural gap between things people
wrote, and it is only visible to something that walks the chain from both ends.

## What it costs

Confidence that is unearned, which is worse than knowing you have no control: the team stops
watching for the violation because the check "handles it", and the violations accumulate
unobserved until something forces an audit.

## What Majordomus does

A rule the tool enforces declares its validator, the commands that dispatch it, the exit
code a violation produces, the claims it stands behind and the tests that prove it. Nothing
selects checks by hand: the dispatcher reads the declared rules, so a rule added there runs
from that moment, and a rule whose validator does not exist is a reported failure rather than
a silent skip. `doctor` walks the chain in both directions — a validator no rule declares
fails too — and reconciles the policy's enforcement list against the hooks that name it,
including whether the exit code survives.

## Before and after

```text
before   rule: "enforced by pre-commit"    hook: absent     result: green, always

after    $ majordomus doctor
         FAIL wiring  doctor-on-commit — declared in policy, no pre-commit hook invokes it
                      [reproduce: majordomus doctrine status]
```

## What it does not do

It reconciles what this repository declares against what this repository runs. It cannot
know that a rule ought to exist, and a rule that honestly says nobody enforces it is a
passing state, not a failure.
