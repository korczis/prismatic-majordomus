+++
title = "plan.transition"
description = "Record that execution of an issue began (`start`), that implementation is complete with evidence outstanding (`verify`), or that it is finished (`done`). One field of the issue's own record is stamped and one event is appended to the ledger. The move is refused when the model says it is illegal — an issue that is not READY cannot start, one that was never ACTIVE cannot be verified, and one whose dependencies are unfinished or whose required evidence is absent cannot be done — and the refusal names what is in the way. The status that comes back is derived from the record afterwards, not announced by the move: a `done` whose evidence is missing leaves the issue in VERIFY and says so."
weight = 65
slug = "plan-transition"
[extra]
id = "plan.transition"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
