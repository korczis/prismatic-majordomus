+++
title = "executions.start"
description = "Run any executable capability of this registry as an execution: the input is checked against that capability's own input schema, the work is queued, and this answers at once with the execution's id and the links to follow it. Nothing waits for the handler. With an idempotency_key, a repeated start is one execution: the same key with the same capability and input is answered the execution it started (deduplicated) and nothing new runs, and the same key with another input is refused as a conflict naming that execution. The capability runs through the same executor every other interface calls, so there is no second implementation of anything. On the command line (`majordomus run`) it runs in that process and is reported to the server serving the checkout, when one is, through executions.report; a run never starts a server."
weight = 70
slug = "executions-start"
[extra]
id = "executions.start"
source = "apps/majordomus-cli/src/capability/builtin/executions.rs"
+++
