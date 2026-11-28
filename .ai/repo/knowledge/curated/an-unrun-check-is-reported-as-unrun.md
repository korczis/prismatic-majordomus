---
schema: knowledge/v1
id: an-unrun-check-is-reported-as-unrun
kind: knowledge
class: constraint
title: "A check that did not run is reported as not run, with the reason and the command that would run it"
description: "When writing a report, a handover, a pull request body or a finish note after work in which some test, gate or verification was skipped, blocked or cut short"
status: verified
epistemics: observed
date: 2026-11-25
tags:
  - reporting
  - evidence
  - verification
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/skills/report-verification-state/SKILL.md
    - file:.ai/repo/rules/project/never-reported-is-not-green.v1.md
relations:
  - type: documents
    target: file:.ai/repo/skills/report-verification-state/SKILL.md
  - type: relates_to
    target: file:.ai/repo/rules/project/never-reported-is-not-green.v1.md
---

# A check that did not run is reported as not run, with the reason and the command that would run it

The skill `report-verification-state` gives every claim a state; the rule
`project.never-reported-is-not-green` says silence is not a pass. This note is the habit
between them. When a check did not run, the report says so in the same place a result would
stand, names why, and gives the exact command that would run it. It is never left out, and
never folded into a nearby pass.

Observed on 2026-11-25 on one shared workstation:

- The free disk fell below its floor and the 1-minute load stayed near 100. A coordinator
  ordered every session to start no new build or test until both recovered.
- Sessions then finished work whose tests had not run.
- A game-simulation fitness gate was nondeterministic. Its verdict was only meaningful over
  repeated runs against the trunk, so a single run was neither a pass nor a failure.
- The adopter's own contract states the same rule: never mark a validation step PASS that
  was not run.

How to report it:

| state | claim | evidence |
|---|---|---|
| NOT RUN | the suite passes | not started: disk 6 GiB under a 10 GiB floor; `bash test/run.sh <case>` |

`NOT RUN` sits with `BLOCKED` at the top of the report. A run cut short by a timeout, a usage
limit or a killed process is `NOT RUN` too, unless its output shows the deciding line.
