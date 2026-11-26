---
schema: knowledge/v1
id: a-push-may-start-at-35-gib-free
kind: knowledge
class: fact
title: "Owner decision 2026-11-19 (~04:00 local, ~03:00Z): a Majordomus push may start at 35 GiB free, the build floor, instead of the 45 GiB pre-push floor sessions kept; the owner also asked the session to free disk"
description: "When a session decides whether the machine has disk enough to push, or quotes a push floor"
status: verified
epistemics: decided
date: 2026-11-19
tags:
  - decision
  - disk
  - owner-answer
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
    - file:.ai/repo/adrs/0055-disk-is-the-fifth-quantity-reclaimed-by-predicate-and-bounded-before-a-build.md
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
---

# Owner decision 2026-11-19 (~04:00 local, ~03:00Z): a Majordomus push may start at 35 GiB free, the build floor, instead of the 45 GiB pre-push floor sessions kept; the owner also asked the session to free disk

A push of this repository, whose pre-push hook runs tests, may start when the machine has
35 GiB free. Until this answer sessions waited for 45 GiB: 35 GiB to start a build (clause 7
of rule `project.the-cooperation-model-is-the-owners-word`) plus about 10 GiB that a test or
coverage build adds. The 45 GiB figure was never a repository setting: no hook, policy key or
script holds it (searched at origin/master 8df53bc26e9 for `45 GiB`, `min_free_gib`,
`heavy_work` and `MIN_FREE` in `.githooks/`, `lib/`, `scripts/`, `.ai/repo/policy.yaml` and
`.ai/repo/rules/`). It lived only in sessions' practice, so this record is where the change is
written. The 35 GiB build floor of clause 7 is unchanged, and a push still runs every hook and
check it ran before.

# Evidence

- When: 2026-11-19, about 04:00 local time (CET), about 03:00Z.
- Asker: session cd8d20a8-9d79-4332-b63e-b2592beb2cbd, through AskUserQuestion, multi-select.
- Question, verbatim: "Nothing can land right now without a rule change. Majordomus pushes need
  45 GiB free under your own pre-push floor, and the disk is at 29 GiB. Prismatic MRs need a
  fully green MR pipeline, and !204's last one failed, with lundra running about one pipeline
  every 2 hours. What should I do to get 3-7 merges?"
- Chosen options, verbatim, label then description:
  - "Free disk for me": "You clear space (Chrome cache 3.2 GB, idle sessions, old backups) to
    45 GiB; then I push I4661 and the landing seat lands #3087, #3088, #3089 and #3076."
  - "Lower push floor to 35": "Allow Majordomus pushes at 35 GiB free (the derive floor)
    instead of 45, recorded as your decision; lands #3087-#3089, #3076 and I4661 now."
  - "Commit real-world slice": "Let openttd-07 commit and push its reviewed real-world scenario
    slice to OpenTTD master (it was told 'no commit'); one open MEDIUM (site-box flag) stays
    recorded." This option concerns the OpenTTD repository, not this one, and is recorded here
    only as context of the same answer.
- Recorded by the asking session's agent on branch decision/push-floor-35-gib-20261119, and in
  that worktree's decision ledger (`majordomus decision show "Owner decision 2026-11-19"`), which
  is local state and not durable; this note is the durable record.
- Disk at the time of recording: 33 GiB free on the data volume (`df -g`, 2026-11-19 ~03:27Z).
