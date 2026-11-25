+++
title = "mesh.record.write"
description = "Write an observation, hypothesis, plan, decision, blocker, handoff or outcome about a task and a scope into this runtime's journal, replicated to every linked runtime; or revise a record of the writing session's own by naming it and the revision read. The author is the journal's `<stream>/<session>`, never a name in the input; over MCP a session writes only as itself. Refused, and nothing written: a malformed record or a handoff without objective, current_state, evidence, blockers and next_action (`invalid_input`); a path outside the session's live claims (`out_of_scope`); a revision of another session's record (`not_own` — write a new record with `supersedes` instead); a revision read that is no longer current (`stale_revision`, naming the current one); a retry key reused for other content (`idempotency_conflict`). The same retry key with the same content answers the first write. Contending live plans or decisions of other authors are written and reported."
weight = 137
slug = "mesh-record-write"
[extra]
id = "mesh.record.write"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
