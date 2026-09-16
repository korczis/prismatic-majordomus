+++
title = "plan.transition"
description = "Record that execution of an issue began (`start`), that implementation is complete with evidence outstanding (`verify`), or that it is finished (`done`). One field of the issue's own record is stamped, the seal of its event is written beside it, and the event is appended to the ledger; a stamp without that seal moves no status. The move is refused when the model says it is illegal — an issue that is not READY cannot start, one that was never ACTIVE cannot be verified, and one that was never started, whose dependencies are unfinished or whose required evidence is absent cannot be done — and the refusal names what is in the way. The status that comes back is derived from the record afterwards, not announced by the move: a `done` whose evidence is missing leaves the issue in VERIFY and says so."
weight = 78
slug = "plan-transition"
[extra]
id = "plan.transition"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
