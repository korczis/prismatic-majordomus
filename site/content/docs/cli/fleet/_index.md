+++
title = "majordomus fleet"
description = "The machines that run this repository's mesh, as .ai/repo/fleet/ declares them: what a rollout would do, what every machine runs, and the rollout that installs this release everywhere, restarts each hub's service and verifies that the hubs see each other"
sort_by = "weight"
template = "docs-cli-group.html"
page_template = "docs-cli-command.html"
weight = 217
[extra]
route = "/docs/cli/fleet/"
command = "majordomus fleet"
source = "apps/majordomus-cli/src/cli.rs"
+++
