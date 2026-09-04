---
id: project.no-new-nouns
version: 1
kind: rule
title: No new nouns
description: The tool gains no agents, personas, roles, tiers or registries beyond the vocabulary it has.
statement: The tool gains no agents, personas, roles, tiers or registries beyond the vocabulary it has.
status: active
class: blocking
depends_on: []
tags: [design]
---

# Rationale

Every noun is a concept a worker has to learn and a surface that can drift; the design stays small on purpose, and docs/CONCEPTS.md is the whole vocabulary.

# Required behaviour

The tool gains no agents, personas, roles, tiers or registries beyond the vocabulary it has.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. test/cases/28_no_hardcoded_values.sh checks that every term in docs/CONCEPTS.md is one the tool uses.
