---
id: project.example-rule
version: 1
kind: rule
title: Example rule
description: Example portable repository rule.
statement: Replace this example with one precise normative statement.
status: active
class: blocking
depends_on: []
tags: [example]

x-majordomus:
  validator: example
  category: example
  enforced_by: [check]
  exit_code: 10
  claims: []
  tests: []
---

# Rationale

Explain why this invariant exists and which failure mode it prevents.

# Required behavior

State observable required behavior.

# Forbidden behavior

State prohibited behavior where useful.

# Verification

Describe executable evidence.
