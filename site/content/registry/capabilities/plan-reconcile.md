+++
title = "plan.reconcile"
description = "For every issue (or the ones named) whose work plan.delivery proves merged, released or deployed while its declared status is READY, BLOCKED, ACTIVE or VERIFY: the proof (the newest merge naming it and how: title, branch, slug, subject or a `Delivers:` trailer), the plan.transition moves the plan admits now, in order, and the moves it does not with the guard's own reason (ADR 0238). A merge proves that execution began, so a READY issue is proposed `start`; `done` is proposed only where the done guard admits it, the issue declares evidence_required, and every covering evidence entry was recorded at or after that merge; `verify` is never proposed, because it is a worker's attestation and a merge can be one slice of an issue. Each proposal lists every required token as covered, stale or missing, the declared validation commands that would cover it, and every acceptance criterion as `unjudged`. Classes: recordable, acceptable, held (BLOCKED, or a done held by something other than evidence), unaccepted (ACTIVE or VERIFY with evidence owed), pending_pr (another open pull request still names it; nothing is proposed). `settled` is false when the forge was not read, because no open pull request can then be ruled out. It writes nothing: `majordomus plan reconcile --apply` performs the start moves, and `--done` the admitted done moves, one plan.transition each."
weight = 174
slug = "plan-reconcile"
[extra]
id = "plan.reconcile"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
