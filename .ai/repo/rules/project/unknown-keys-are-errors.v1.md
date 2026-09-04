---
id: project.unknown-keys-are-errors
version: 1
kind: rule
title: Unknown configuration keys are errors
description: Every configuration file rejects a key nothing reads, and every state field is both written and read.
statement: Every configuration file rejects a key nothing reads, and every state field is both written and read.
status: active
class: blocking
depends_on: []
tags: [schema]
---

# Rationale

A key nobody reads is a typo that silently does nothing, which is worse than a refusal; the allowlists under share/allow/ are the schema, and a field written and never read is a second source of truth.

# Required behaviour

Every configuration file rejects a key nothing reads, and every state field is both written and read.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. test/cases/00_yaml_flatten.sh and the policy, profile, manifest and rule allowlists under share/allow/.
