---
id: project.blocking-checks-cheap
version: 1
kind: rule
title: Blocking checks are deterministic and cheap
description: A check that can stop a command is deterministic and cheap; work in progress is reported, never blocked.
statement: A check that can stop a command is deterministic and cheap; work in progress is reported, never blocked.
status: active
class: blocking
depends_on: []
tags: [doctrine]
---

# Rationale

A slow or flaky gate is worked around, and a gate that blocks work in progress trains people to skip it. Only what is settled and decidable may refuse.

# Required behaviour

A check that can stop a command is deterministic and cheap; work in progress is reported, never blocked.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. Every doctrine's class is declared in the rule package; test/cases/17_doctrine_enforcement.sh proves the class decides whether a command stops.
