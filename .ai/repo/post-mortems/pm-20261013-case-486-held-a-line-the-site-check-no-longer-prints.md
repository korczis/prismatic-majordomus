---
schema: post-mortem/v1
id: pm-20261013-case-486-held-a-line-the-site-check-no-longer-prints
kind: post-mortem
title: Case 486 asserted a line of the site check that a landing had reworded
description: A case that greps a gate's output for a whole line breaks when the gate says more; hold the sentence and not the count it now carries.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: "38350251134"
    commit: 9e50ce2af3
    jobs: ["suite-shard (3)", "suite"]
    step: "behavioural tests"
  - workflow: validate
    id: "38364727993"
    commit: ba3fd916b3
    jobs: ["suite-shard (3)", "suite"]
    step: "behavioural tests"
cause:
  class: test
  summary: Pull request 1045 made the site check's navigation line end in a parenthetical count of snapshot roots; case 486 matched the old line to its end.
components:
  - test:test/cases/486_rustdoc_composition.sh
  - file:scripts/site-check
signals:
  - id: case
    text: "FAIL 486_rustdoc_composition"
  - id: line
    text: "every navigation link on the homepage resolves"
fixed_by: ["commit:ba49c586c2", "pr:1079"]
prevention: ["test:test/cases/486_rustdoc_composition.sh"]
tags: [suite, site]
related: ["issue:I2650"]
---

# Case 486 asserted a line of the site check that a landing had reworded

## What failed

`suite-shard (3)` failed in two `validate` runs with `FAIL 486_rustdoc_composition` after
about 870 seconds, while the site check inside the case reported `site-check: 0
failure(s)`: the check passed and the case's own expectation did not.

## Root cause

The case held `^OK   nav +every navigation link on the homepage resolves$`. Pull request
1045 (`b34259715e`) made the landing page link the published UI, and the site check's line
gained `(N of them the root of a snapshot the publication holds and this build does not
compose)`. The anchored pattern no longer matched. Whether shard 3 reported on the pull
request's own head before it was merged was not established for this record.

## Fix

`ba49c586c2` (pull request 1079) holds the sentence with the parenthetical and leaves the
count to the publication.

## Prevention

The case itself is the guard, and it takes fourteen minutes: a change to `scripts/site-check`
output runs case 486 before it is pushed.
