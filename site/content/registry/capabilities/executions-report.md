+++
title = "executions.report"
description = "What `majordomus run` posts while it runs a capability in its own process and a server already serves the checkout: the execution's description and its events after the last sequence this server took. The server holds it as an execution started by the command line, with the pid, in the same store, listing, Cockpit pages and live channel as its own; applies each event once and in order, answering where the next report starts; tells the reporter when somebody asked it to stop; and fails it with `unavailable` once the reporter has been silent for 30 s. Nothing runs here. Changes this process's memory only."
weight = 75
slug = "executions-report"
[extra]
id = "executions.report"
source = "apps/majordomus-cli/src/capability/builtin/executions.rs"
+++
