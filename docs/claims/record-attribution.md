# A durable record names the session that wrote it

## What it means

Every durable record carries the session that ran the command: a decision, a question opened or resolved, a checkpoint, `check --checkpoint`, a handover, and `knowledge observe`. The session is stamped on the record's ledger line as `session`. The record is filed under the active task, as `task_id`, unless a different episode that is still open opened that task. In that one case it is still written, but with no task. A worker's decision cannot land in somebody else's task, and nothing is refused for want of an identity (ADR 0164).

## How it works

`mj_record_attribution` in `lib/common.sh` prepares the attribution once, before a record is published, and every writer passes it to the ledger. The session comes from `mj_open_session_id`. It is the caller's own episode, found by the identity the caller supplies: the provider session a hook names, `MAJORDOMUS_PROVIDER_SESSION`, or the session variable the provider's lifecycle adapter declares (`CLAUDE_CODE_SESSION_ID` for Claude Code). With no supplied identity, the one open episode of the checkout is used. With several open, no session is recorded rather than the last one opened. An identity that names no open episode records none. The task relation reads the task's one `task.started` line and asks whether the episode it names is a different one that is still open.

## How to see it

```bash
majordomus session start --provider-session worker-a
MAJORDOMUS_PROVIDER_SESSION=worker-a majordomus start "A's task" --scope lib
majordomus session start --provider-session worker-b
MAJORDOMUS_PROVIDER_SESSION=worker-b majordomus decision add "B's decision" --why "B decided it"
jq -c 'select(.event == "decision.recorded")' .ai/local/state/ledger.jsonl | tail -n 1
# names B's session and carries no task_id: the task is A's, and A is still open
```

`test/cases/3049_a_second_workers_record_lands_on_the_first_workers_task.sh` runs this and the other shapes: no episode at all, the caller's own task, an unknown identity, and an anonymous call with several episodes open.

## What it does not cover

**The identity is supplied, never verified.** Majordomus records the session it was told about. It does not authenticate the worker behind it, and nothing here proves who a worker was. A process that sets another session's variable is attributed to that session. Read a record's `session` as "the session Majordomus was told wrote this", not as proof of authorship.

With no episode open at all, two workers cannot be told apart, and the active task is treated as the caller's. A provider whose adapter declares no session variable gets records with no session whenever several episodes are open. That is the cost the decision accepts instead of inventing a worker identity.

## Why it exists

Two workers in one checkout used to file each other's records. Worker B's decision, checkpoint and question landed under worker A's task and named nobody, because every writer took the task from the checkout and the session from whichever episode was opened last. A record that names the wrong worker is worse than one that names none: the next reader acts on it.
