+++
title = "executions.start"
description = "Run any executable capability of this registry as an execution: the input is checked against that capability's own input schema, the work is queued, and this answers at once with the execution's id and the links to follow it. Nothing waits for the handler. The capability runs through the same executor every other interface calls, so there is no second implementation of anything."
weight = 34
slug = "executions-start"
[extra]
id = "executions.start"
source = "apps/majordomus-cli/src/capability/builtin/executions.rs"
+++
