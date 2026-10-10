+++
title = "mesh.firewall"
description = "What inbound traffic the declaration implies this machine must admit — the multicast group's port, the broadcast port when it is a permitted fallback, every declared hub port whose address is this machine's, and the server's own port when it listens beyond loopback — from the private networks the declaration names; which firewall front this host runs (ufw, nftables, the macOS application firewall); the commands that admit the plan there; what the firewall itself says about the plan now (present, missing, inactive, or unobservable without root); and what the kernel logged it dropping toward those ports in the last five minutes. Fails when a rule is observed missing or a drop was logged. Reads the host; changes nothing."
weight = 99
slug = "mesh-firewall"
[extra]
id = "mesh.firewall"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
