+++
title = "executions.cancel"
description = "Set the execution's cancellation flag and say so on its stream. Cancellation is cooperative: a task looks at its flag and stops, and a capability whose policy says it is not cancellable runs to completion — which the answer says rather than pretending otherwise."
weight = 27
slug = "executions-cancel"
[extra]
id = "executions.cancel"
source = "apps/majordomus-cli/src/capability/builtin/executions.rs"
+++
