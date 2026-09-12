+++
title = "Tests run in disposable repositories"
description = "Tests run in disposable repositories"
weight = 124
[extra]
kind = "rule"
slug = "project-tests-run-in-disposable-repos-1"
identity = "project.tests-run-in-disposable-repos@1"
status = "active"
source = ".ai/repo/rules/project/tests-run-in-disposable-repos.v1.md"
+++
{% raw %}

## Rationale

A case that runs against the checkout it lives in changes the state it is testing; test/run.sh creates a fresh repository per case for that reason.

## Required behaviour

Behavioural cases run through bash test/run.sh in a temporary repository each, never against this checkout.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. test/run.sh; a case that writes into ROOT is a bug the derived-artifacts case would report.
{% endraw %}
