---
id: project.rule-is-a-doctrine
version: 1
kind: rule
title: A new enforced rule is a doctrine, not an inline check
description: A rule the tool enforces is declared as a rule object with an x-majordomus block and a validator named mj_validate_<name>; no command selects checks by hand.
statement: A rule the tool enforces is declared as a rule object with an x-majordomus block and a validator named mj_validate_<name>; no command selects checks by hand.
status: active
class: blocking
depends_on: []
tags: [doctrine]
---

# Rationale

A check that a command calls by name is enforcement nobody declared; the dispatcher reads the declared rules, so a rule added there runs from that moment and one whose validator is missing fails doctor.

# Required behaviour

A rule the tool enforces is declared as a rule object with an x-majordomus block and a validator named mj_validate_<name>; no command selects checks by hand.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. The doctrine_wiring_integrity rule proves the chain both ways; test/cases/18_doctrine_wiring.sh mutation-tests it.
