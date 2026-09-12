+++
title = "Never author identity fields"
description = "Never author identity fields"
weight = 96
[extra]
kind = "rule"
slug = "project-never-author-identity-1"
identity = "project.never-author-identity@1"
status = "active"
source = ".ai/repo/rules/project/never-author-identity.v1.md"
+++
{% raw %}

## Rationale

A record that names a commit looks checkable; one whose commit was typed is not, and cannot be told apart later.

## Required behaviour

repository_id, branch, head, working_tree and changed_files on any record are computed from git and never written by hand.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. test/cases/05_handover.sh, test/cases/20_checkpoint.sh and test/cases/61_session_envelope.sh refuse an authored identity field.
{% endraw %}
