---
id: project.never-store-transcripts
version: 1
kind: rule
title: Never store or summarise transcripts
description: No record stores or summarises a conversation; records carry what is true now and the next action.
statement: No record stores or summarises a conversation; records carry what is true now and the next action.
status: active
class: blocking
depends_on: []
tags: [records, continuity]
---

# Rationale

A transcript is one worker's past; the next worker needs the present. Every record with computed front matter refuses identity fields precisely so that prose cannot pose as state.

# Required behaviour

No record stores or summarises a conversation; records carry what is true now and the next action.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. test/cases/05_handover.sh and test/cases/20_checkpoint.sh refuse identity fields in an authored body.
