+++
title = "Conventional commits, committed and pushed incrementally"
description = "Conventional commits, committed and pushed incrementally"
weight = 70
[extra]
kind = "rule"
slug = "project-conventional-commits-1"
identity = "project.conventional-commits@1"
status = "active"
source = ".ai/repo/rules/project/conventional-commits.v1.md"
+++
{% raw %}

## Rationale

Small conventional commits are what make the history readable and revertible, and what let a second session on the same branch follow.

## Required behaviour

Commits use the conventional format type(scope): description with the co-author footer the session provides, and land incrementally rather than as one large change.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. 
{% endraw %}
