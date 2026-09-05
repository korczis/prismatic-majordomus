---
id: extend-what-the-executable-serves
kind: use-case
title: 'Extend what the executable serves by adding a file, never by rebuilding it'
summary: 'Add a rule, a prompt or a knowledge node to .ai/ and have it appear as an MCP resource, an HTTP route and a reference entry, with the same binary.'
category: extension
status: active
target: guaranteed
weight: 9
actors: [maintainer]
difficulty: advanced
commands: [init, update, doctor, knowledge]
mcp_tools: [majordomus_capabilities, majordomus_capability]
doctrines: [majordomus.ai-layout-integrity, majordomus.projection-integrity, majordomus.policy-integrity]
claims: [mcp-data-driven, capability-registry, interfaces-are-projections, capability-modules, mcp-degraded-not-silent]
responsibilities: [layer, projection, doctor]
applications: [repository-with-authored-governance, repository-opened-in-ai-clients]
scenario:
  setup: installed-wired
  given:
    - 'the layer installed; the distribution declares the kinds it reads'
  steps:
    - id: what-is-declared
      run: ['knowledge', 'sources']
      note: 'every source class the repository declares, with what it discovered'
      expect:
        exit: 0
        stdout_contains: ['^policy +shared +policy', '^knowledge sources: [0-9]+ file']
    - id: nothing-to-add
      run: ['init', '--extend']
      note: 'a new kind is a declaration under .ai/repo/knowledge, not a change to the tool'
      expect:
        exit: 0
        stdout_contains: ['nothing to add']
    - id: still-healthy
      run: ['doctor']
      note: 'the layer is real after the extension'
      expect:
        exit: 0
        stdout_contains: ['doctor: 0 failure']
  then:
    - 'a kind added with its schema under .ai/repo/knowledge is served by the executable without a code change'
---

# Situation

A team wants its own kind of object served to its AI clients. With most tools that means a fork and a build; the knowledge would then live in the binary, and the binary would own it.

# What you run

- `init`: writes the layer; kinds and their JSON Schemas are read at run time from the share directory, and a repository may add its own
- `update`: regenerates the projections (AGENTS.md, CLAUDE.md) from the policy, so the bootstrap every worker reads names the new object
- `doctor`: validates every declarative object against the schema of its kind and reports a duplicate identity or an unknown key as a diagnostic, never as a crash

# Outcome

The executable discovers the file through the version-control index, validates it against the schema of its kind, and serves it as a resource; MCP, HTTP, OpenAPI, Swagger UI and the generated reference all show it because they are projections of one registry. A broken file is excluded and named; nothing was compiled and nothing was rebuilt.
