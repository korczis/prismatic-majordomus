+++
title = "No counts in prose"
description = "No counts in prose"
weight = 101
[extra]
kind = "rule"
slug = "project-no-counts-in-prose-1"
identity = "project.no-counts-in-prose@1"
status = "active"
source = ".ai/repo/rules/project/no-counts-in-prose.v1.md"
+++
{% raw %}

## Rationale

A count written down is stale the moment the thing it counts changes, and README, .rules and .windsurfrules of the source environment disagreed with each other about the same counts for that reason.

## Required behaviour

Prose never states a number the repository can compute; it states the command that computes it.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. test/cases/28_no_hardcoded_values.sh proves that every list the tool knows about itself is derived, and the majordomus.context-budget doctrine fails on a hardcoded count in the always-loaded projection.
{% endraw %}
