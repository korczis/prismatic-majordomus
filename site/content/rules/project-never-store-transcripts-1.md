+++
title = "Never store or summarise transcripts"
description = "Never store or summarise transcripts"
weight = 98
[extra]
kind = "rule"
slug = "project-never-store-transcripts-1"
identity = "project.never-store-transcripts@1"
status = "active"
source = ".ai/repo/rules/project/never-store-transcripts.v1.md"
+++
{% raw %}

## Rationale

A transcript is one worker's past; the next worker needs the present. Every record with computed front matter refuses identity fields precisely so that prose cannot pose as state.

## Required behaviour

No record stores or summarises a conversation; records carry what is true now and the next action.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. test/cases/05_handover.sh and test/cases/20_checkpoint.sh refuse identity fields in an authored body.
{% endraw %}
