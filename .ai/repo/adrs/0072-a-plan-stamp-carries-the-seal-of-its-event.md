---
schema: adr/v1
id: adr-0072
kind: adr
title: A plan stamp carries the seal of its event, because the ledger never leaves the checkout
status: accepted
date: 2026-09-15
tags:
  - plan
  - lifecycle
  - events
  - integrity
provenance:
  origin: authored
related:
  - rule:project.no-plan-stamp-without-its-event
  - rule:project.development-semantics-are-canonical
  - test:test/cases/375_no_plan_stamp_without_its_event.sh
  - test:test/cases/376_the_done_guard_holds_in_both_engines.sh
  - test:test/cases/133_plan_transition.sh
  - test:test/cases/99_plan_capabilities.sh
---

# 72. A plan stamp carries the seal of its event

## Context

An issue's status is derived from its record: `started_at`, `verified_at`, `completed_at`
and its evidence entries. The transition — `plan::transition` behind the `plan.transition`
capability (MCP `majordomus_plan_transition`, `POST /api/v1/plan/transition`) and
`mj_plan_transition` behind `majordomus plan start|verify|done` — is the guarded boundary,
and it appends `plan_start`, `plan_verify` or `plan_done` to the ledger after writing the
stamp.

On 2026-09-15 the boundary was measured to be optional. In a fixture, `completed_at`,
`started_at`, `verified_at` and one evidence entry were appended to an issue's YAML by hand
and committed. `majordomus plan show` derived DONE, `majordomus plan validate` printed
`0 failure(s)` and exited 0, and the ledger held no `plan_*` event. No gate compared the
stamps with the events. Separately, `done` had no status precondition in either engine, so
a READY issue could be completed without ever being started, and the shell engine wrote
`completed_at` onto a CANCELLED issue.

The obvious check — every stamp has its event in the ledger — cannot be decided where it
matters. `.ai/local/state/ledger.jsonl` is gitignored and belongs to one checkout: a clone,
another worktree and every CI job start without it. The repository's own plan holds 142
stamps and the primary checkout's ledger holds 57 `plan_*` events, because transitions ran
in many worktrees. A check against the ledger would be unknown in CI and false locally.

## Decision

**The transition writes a seal beside every stamp, in the same write, and the plan refuses
a stamp without it.** The seal is

```
sha256:<hex of "majordomus.plan-event/v1\n<event>\n<issue>\n<stamp>\n">
```

in `started_event`, `verified_event` or `completed_event`. It is the tracked half of the
event: the event name, the issue and the instant, which is exactly what the ledger line
records beyond the envelope of the checkout it was written in. There is still one truth —
the transition — and two records of what it did, one local and complete, one tracked and
minimal, written by the same call.

Both derivations (`lib/project.awk`, `plan.rs`) treat a stamp as a fact only when its seal
matches. An unproven stamp moves no status and is a `stamp_without_event` FAIL naming the
issue, the stamp and the event. `plan validate` exits 10 on it; `scripts/ci/core-check` runs
`plan validate` in every CI plan, so the check is decided from the tree alone and is never
unknown. The shell engine hashes every stamp of the plan with one `sha256sum`/`shasum`
process, so the check costs one fork per load, not one per stamp.

`done` now requires ACTIVE or VERIFY in both engines, after the dependency check and before
the evidence check.

**The stamps that predate seals are named, not grandfathered by date.**
`unsealed_stamps` in each issue record lists its own as `<field> <stamp>` — in the record,
because the index does not discover `project.yaml` and both engines must read the same
list. An entry excuses exactly that value, and only when it is older than `SEALED_FROM`
(`2026-09-16T00:00:00Z`, a constant in both engines). A stamp written at or after that
instant needs its seal whatever the list says, so the list can shrink and cannot be used to
excuse new work.

## Alternatives rejected

- **Compare with the ledger.** Unknown in CI and wrong in any checkout that did not perform
  every transition. Rejected by measurement, above.
- **Track the ledger, or a plan-event log beside the records.** A second tracked history of
  the same events, with merge conflicts on every parallel transition and a new writer to
  keep in step. The seal lives in the record the transition already rewrites.
- **Exempt every stamp older than a date.** A backdated stamp would pass. The named list
  closes that: a backdated stamp must also be added to a list the rule forbids adding to,
  in the same diff.
- **Delegate the shell transition to the Rust capability in this change.** The capability
  resolves issues through the index, which enumerates tracked files only, costs about two
  seconds a call, and reports every refusal as exit 10; nine behavioural cases drive the
  shell transition over untracked fixtures and assert exits 12 and 15. That is its own
  change. Until it lands, both engines carry the same guards and write the same bytes, which
  `test/cases/133_plan_transition.sh` and `376_the_done_guard_holds_in_both_engines.sh` hold.

## Consequences

- A hand-written stamp is visible: it fails `plan validate` by name and shows no progress on
  any surface.
- The seal is a digest of public values, not a signature. Forging one is possible and is
  now a deliberate act whose output is a diff line no transition wrote; making it impossible
  would need a key the repository does not have.
- Evidence entries are still free text appended by `plan evidence`; a forged evidence entry
  can satisfy the `done` guard. Sealing evidence is the next slice of the same law.
- A branch that stamps an issue with an older executable after `SEALED_FROM` fails
  validation until the issue is moved again with a current one.
