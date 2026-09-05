+++
title = "Extend what the executable serves by adding a file, never by rebuilding it"
description = "Add a rule, a prompt or a knowledge node to .ai/ and have it appear as an MCP resource, an HTTP route and a reference entry, with the same binary."
weight = 9
[extra]
id = "extend-what-the-executable-serves"
source = ".ai/repo/use-cases/extend-what-the-executable-serves.md"
category = "extension"
maturity = "guaranteed"
+++

## Situation

A team wants its own kind of object served to its AI clients. With most tools that means a fork and a build; the knowledge would then live in the binary, and the binary would own it.

## What you run

- `init`: writes the layer; kinds and their JSON Schemas are read at run time from the share directory, and a repository may add its own
- `update`: regenerates the projections (AGENTS.md, CLAUDE.md) from the policy, so the bootstrap every worker reads names the new object
- `doctor`: validates every declarative object against the schema of its kind and reports a duplicate identity or an unknown key as a diagnostic, never as a crash

## Outcome

The executable discovers the file through the version-control index, validates it against the schema of its kind, and serves it as a resource; MCP, HTTP, OpenAPI, Swagger UI and the generated reference all show it because they are projections of one registry. A broken file is excluded and named; nothing was compiled and nothing was rebuilt.
