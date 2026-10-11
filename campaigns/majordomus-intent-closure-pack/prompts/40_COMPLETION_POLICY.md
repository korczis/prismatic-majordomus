# Milestone 40: Make Session and Work Completion Intent-Aware

## Mission

Integrate the satisfaction engine into the repository's existing finish/completion policy. A session may end, but it must not claim the intent is complete without evidence.

Distinguish cleanly between:

- session finished;
- work item/issue finished;
- milestone projected state;
- intent satisfied/completed.

## 1. Audit current completion path

Find canonical commands/services for:

- session finish;
- issue completion/closure;
- checkpoints/handover;
- tests/docs/commit/push/deploy gates;
- completion reports;
- version bump/release ceremony.

Do not replace them.

## 2. Completion decision model

Extend completion evaluation so it considers intent obligations from the session envelope and satisfaction engine.

A completion result must identify:

- criteria this session/work was responsible for;
- evidence produced;
- criteria newly satisfied;
- remaining criteria;
- blockers;
- whether session completion is allowed;
- whether issue completion is allowed;
- whether intent completion is derived;
- whether a handover is required.

## 3. Partial sessions

Do not force every session to finish an entire intent. Support deliberate partial progress while preventing misleading success claims.

A partial session should:

- record completed work/evidence;
- preserve unresolved obligations;
- create/update canonical handover/checkpoint state;
- not mark intent complete;
- remain easy to resume.

## 4. Premature completion guards

Reject or downgrade completion when:

- required evidence is missing;
- evidence failed or is stale;
- work violates intent invariant;
- criterion claimed by issue remains unsatisfied and issue policy requires satisfaction;
- intent is declared complete while required criteria remain open;
- opposition became stale during implementation.

## 5. Replanning on failure

When completion evaluation discovers an unmet criterion not represented by remaining work, integrate with existing gap/coverage mechanisms to produce an actionable gap rather than silently ending.

Do not automatically invent large new plans here; the full plan derivation milestone comes later. But the system must detect "intent still incomplete and no work covers this criterion."

## 6. User/agent output

Completion output should answer concisely:

- what was accomplished;
- what evidence proves it;
- what intent criteria changed state;
- what remains;
- why completion is or is not permitted.

Use structured output for machine consumers.

## 7. Tests

Cover:

- session completes while intent remains legitimately in progress;
- issue refuses completion when its mandatory criterion remains unsatisfied;
- intent completes automatically when final required evidence lands;
- intent cannot be completed by closing issues only;
- missing evidence forces handover/block;
- stale opposition/evidence prevents false completion;
- finish report is stable across CLI/structured representation;
- restart/resume carries remaining obligations.

## 8. Governance

Update completion doctrine/rules and gates to encode the distinctions. Remove any legacy path that can directly set intent complete in contradiction to the satisfaction engine.

## Acceptance criteria

- session/work completion and intent satisfaction are distinct but integrated;
- premature intent success is impossible through supported paths;
- partial work produces a useful handover;
- final evidence can cause intent completion without manually editing status;
- completion output explains the decision.
