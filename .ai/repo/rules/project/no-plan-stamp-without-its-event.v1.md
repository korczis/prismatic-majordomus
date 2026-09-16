---
id: project.no-plan-stamp-without-its-event
version: 1
kind: rule
title: No plan stamp without its event
description: A lifecycle stamp of an issue — started_at, verified_at, completed_at — is a fact only when the transition that appends its plan_* event wrote it, and the record proves that with the seal the transition writes beside the stamp; a stamp without its seal moves no status and fails plan validate.
statement: Move an issue only with `majordomus plan start|verify|done` or the plan.transition capability; never write a lifecycle stamp or its seal into an issue record by hand, by script or through any other surface, and never add an entry to a record's unsealed_stamps list.
status: active
class: blocking
depends_on: [project.development-semantics-are-canonical@1]
tags: [plan, lifecycle, events, integrity]

x-majordomus:
  tests: [test/cases/375_no_plan_stamp_without_its_event.sh, test/cases/376_the_done_guard_holds_in_both_engines.sh]
---

# Rationale

The plan derives an issue's status from its record: `started_at`, `verified_at`,
`completed_at` and the evidence entries. `plan.transition` and `majordomus plan
start|verify|done` are the guarded boundary — READY before start, started before done,
no open dependency, evidence complete — and each appends a `plan_start`, `plan_verify` or
`plan_done` event to the ledger. Until this rule, nothing tied the stamp to the event. A
worker who wrote `completed_at` and an evidence entry into the YAML got a DONE issue with no
transition, no guard and no event, `plan validate` exited 0, and every surface — the ready
set, the roadmap, the GitHub projection, the site — reported it as finished.

The ledger cannot be the proof a gate reads: `.ai/local/state/ledger.jsonl` is checkout-local
and never tracked, so a clone, a second worktree and CI all start without it. The proof has
to travel with the record. ADR 0072 records the choice: the transition writes a seal —
`sha256` of the event, the issue and the stamp — into the record in the same write as the
stamp, and both engines refuse a stamp whose seal is absent or does not match.

# Required behaviour

- A stamp is written only by the transition, which writes the stamp, its seal
  (`started_event`, `verified_event`, `completed_event`) and `updated_at`, then appends the
  event. The shell engine and the Rust engine write the same bytes.
- A stamp without a matching seal has no effect on the derived status and is reported as
  `stamp_without_event`, a FAIL naming the issue, the stamp and the missing event.
  `majordomus plan validate` exits 10 on it, and `scripts/ci/core-check`, which runs in
  every CI plan, runs `plan validate`.
- `done` is refused unless the issue is ACTIVE or VERIFY: a READY, BLOCKED, CANCELLED or
  DONE issue cannot be completed by the transition either.
- `unsealed_stamps` in an issue record names that record's stamps recorded before seals
  existed (`<field> <stamp>`). It may shrink and is never added to. An entry excuses only
  the exact stamp it names, and only one older than `SEALED_FROM`; a stamp written at or
  after that instant needs its seal whatever the list says.

# Failure behaviour

`plan validate` prints one FAIL per unproven stamp and exits 10; the derived status ignores
the stamp, so a forged DONE reads as whatever its proven stamps say. The remedy is the
transition: `majordomus plan start <id>` on an issue a forged `started_at` left READY
rewrites the stamp with its seal. A forged `completed_at` is removed, not sealed by hand.

# Limits

The seal is a digest of public values, not a signature. It turns "a stamp nobody
transitioned" from invisible into a named failure, and it makes forging one a deliberate
act — computing the digest — that is visible in the diff as a seal line no transition
wrote. It does not, and cannot without a key, prove that the tool computed it.

# Verification

```bash
bash test/run.sh 375_no_plan_stamp_without_its_event
bash test/run.sh 376_the_done_guard_holds_in_both_engines
majordomus plan validate
```
