+++
title = "Verification integrity"
description = "Verification integrity"
weight = 55
[extra]
kind = "rule"
slug = "majordomus-verification-integrity-1"
identity = "majordomus.verification-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/verification-integrity.v1.md"
+++
{% raw %}

## Rationale

Completion requires a verification command that actually ran and exited 0; its exit code and duration are recorded.

An exit code on its own names no subject. A checkout with more than one worker in it — a
second agent, a person saving a file, a watcher regenerating an artefact — moves under a
long verification, and the tree that was proved is then not the tree that gets committed.
So the tree is read on both sides of the run and recorded with the result.

## Required behaviour

A completed outcome requires a verification command that ran and exited zero over a working tree that was the same before and after the run, with its exit code, duration and that tree's git id recorded. A run whose tree changed under it is a failure, not a pass: it describes neither the state it started on nor the state it left.

## Failure behaviour

A violation is a `FAIL` finding under the category `verification`, and the command that found it exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

## Verification

`mj_validate_verification` decides it, dispatched from `finish`. The behavioural cases `test/cases/19_end_to_end.sh` and `test/cases/572_verification_records_the_tree_it_proved.sh` prove it, and CI runs them.
{% endraw %}
