+++
title = "majordomus prs service"
description = "The executor as a user service (launchd on macOS, systemd --user on Linux) outside every agent session: restarted after a crash, an outage or a lease another executor held, never after a halt, an unverified merge or the rollout gate (I3913)"
sort_by = "weight"
template = "docs-cli-group.html"
page_template = "docs-cli-command.html"
weight = 101
[extra]
route = "/docs/cli/prs/service/"
command = "majordomus prs service"
source = "apps/majordomus-cli/src/cli.rs"
+++
