+++
title = "majordomus bench"
description = "Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations), report coverage, compare with the accepted baseline"
sort_by = "weight"
template = "docs-cli-group.html"
page_template = "docs-cli-command.html"
weight = 10
[extra]
route = "/docs/cli/bench/"
command = "majordomus bench"
source = "apps/majordomus-cli/src/cli.rs"
+++
