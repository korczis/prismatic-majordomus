---
id: project.tests-run-in-disposable-repos
version: 1
kind: rule
title: Tests run in disposable repositories
description: Behavioural cases run through bash test/run.sh in a temporary repository each, never against this checkout.
statement: Behavioural cases run through bash test/run.sh in a temporary repository each, never against this checkout.
status: active
class: blocking
depends_on: []
tags: [testing]
---

# Rationale

A case that runs against the checkout it lives in changes the state it is testing; test/run.sh creates a fresh repository per case for that reason.

# Required behaviour

Behavioural cases run through bash test/run.sh in a temporary repository each, never against this checkout.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. test/run.sh; a case that writes into ROOT is a bug the derived-artifacts case would report.
