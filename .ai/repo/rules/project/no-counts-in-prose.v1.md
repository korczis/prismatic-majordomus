---
id: project.no-counts-in-prose
version: 1
kind: rule
title: No counts in prose
description: Prose never states a number the repository can compute; it states the command that computes it.
statement: Prose never states a number the repository can compute; it states the command that computes it.
status: active
class: blocking
depends_on: []
tags: [documentation]
---

# Rationale

A count written down is stale the moment the thing it counts changes, and README, .rules and .windsurfrules of the source environment disagreed with each other about the same counts for that reason.

# Required behaviour

Prose never states a number the repository can compute; it states the command that computes it.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. test/cases/28_no_hardcoded_values.sh proves that every list the tool knows about itself is derived, and the majordomus.context-budget doctrine fails on a hardcoded count in the always-loaded projection.
