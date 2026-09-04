---
id: project.conventional-commits
version: 1
kind: rule
title: Conventional commits, committed and pushed incrementally
description: Commits use the conventional format type(scope): description with the co-author footer the session provides, and land incrementally rather than as one large change.
statement: Commits use the conventional format type(scope): description with the co-author footer the session provides, and land incrementally rather than as one large change.
status: active
class: advisory
depends_on: []
tags: [git]
---

# Rationale

Small conventional commits are what make the history readable and revertible, and what let a second session on the same branch follow.

# Required behaviour

Commits use the conventional format type(scope): description with the co-author footer the session provides, and land incrementally rather than as one large change.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. 
