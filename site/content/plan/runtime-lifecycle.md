+++
title = "runtime-lifecycle — One usable server per checkout starts, attaches, stops and reports its state without races or hidden kills"
description = "Entering a checkout, opening the Cockpit, or starting any number of MCP clients converges on one server for that checkout, which is taken over only when it is dead, stops in order when asked, removes only its own lease, reports a typed state every surface reads, and stays up while any surface uses it."
weight = 28
template = "milestone.html"
[extra]
plan_id = "runtime-lifecycle"
source = ".ai/repo/project/milestones/runtime-lifecycle.yaml"
+++
