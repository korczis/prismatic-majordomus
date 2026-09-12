+++
title = "development-runtime — The runtime has a development surface"
description = "A development transition is decided in exactly one place — a `capability!` of kind `command` in the Rust registry — and every surface that offers it is a projection: the CLI, the MCP tool, the HTTP route, the OpenAPI operation and the Cockpit's generated runner form, none of them written by hand. The lifecycle stops being a second program with no surfaces, and the execution history a development surface would show exists somewhere durable."
weight = 20
template = "milestone.html"
[extra]
plan_id = "development-runtime"
source = ".ai/repo/project/milestones/development-runtime.yaml"
+++
