+++
title = "executions.demonstrate"
description = "Walk a given number of steps, reporting each one, logging a line and advancing progress, then finish — or fail at a step you name. It exists so that an operator, a probe and an end-to-end test can prove the whole path works without waiting for real work: it reads nothing, writes nothing, and its only effect is the events it produces. It looks at its cancellation flag between steps and while it waits, so cancelling it stops it."
weight = 14
slug = "executions-demonstrate"
[extra]
id = "executions.demonstrate"
source = "apps/majordomus-cli/src/capability/builtin/executions.rs"
+++
