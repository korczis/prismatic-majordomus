+++
title = "mesh.firewall.apply"
description = "Run the commands mesh.firewall renders, on this host, as root: one allow per rule and source network on ufw or nftables, the executable admitted on the macOS application firewall; nothing else is touched, and every rule written carries the comment `majordomus mesh` so it can be told from an operator's own. Refuses, running nothing, without root or without a backend; records every command with its exit and output; and asks the firewall again afterwards, so the verdict is the firewall's. Offered on the command line only: it runs a privileged host tool, which nothing reachable over HTTP or MCP may do."
weight = 99
slug = "mesh-firewall-apply"
[extra]
id = "mesh.firewall.apply"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
