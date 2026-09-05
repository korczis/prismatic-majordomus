---
id: classify-what-belongs-in-the-context
kind: use-case
title: 'Decide what belongs in the AI context, as data'
summary: 'Read the repository’s declared scope policy, see which paths are in and out of the index, and let doctor and the knowledge index apply it rather than a hardcoded list.'
category: knowledge
status: active
target: guaranteed
weight: 78
actors: [maintainer]
difficulty: intermediate
commands: [knowledge, doctor]
mcp_tools: [majordomus_scope, majordomus_scope_classify]
doctrines: [majordomus.ai-layout-integrity, majordomus.policy-completeness]
claims: [scope-declared, ai-layer-manifest, local-state-ignored]
responsibilities: [layer, doctor]
applications: [repository-opened-in-ai-clients, repository-with-authored-governance]
scenario:
  setup: installed-wired
  given:
    - 'installed, and the two enforcements the policy declares are actually in place as hooks'
  steps:
    - id: declared
      run: ['doctor']
      note: 'the scope file is present and well-formed; init seeded it'
      expect:
        exit: 0
        stdout_contains: ['^OK   layout      .ai/repo/scope.yaml', 'doctor: 0 failure']
    - id: sources
      run: ['knowledge', 'sources']
      note: 'the source classes and the files each discovers, after the scope policy is applied'
      expect:
        exit: 0
        stdout_contains: ['^policy +shared', '^scope +shared']
    - id: nodes
      run: ['knowledge', 'nodes', '--scope', 'shared']
      note: 'the shared objects only: nothing under .ai/local/ is a node'
      expect:
        exit: 0
        stdout_contains: ['shared']
        stdout_not_contains: ['operational']
  then:
    - 'what is in and out of the context is one file in the layer, with a schema'
    - 'build outputs, local state, binaries and secrets are out by declaration, not by luck'
    - 'the MCP server classifies a path with the same data'
---

# Situation

An AI client indexes what it finds: build outputs, the local state directory, a vendored binary, a data dump. The context fills with noise and, sometimes, with a secret. Every tool has its own ignore list, and none of them is the repository’s own statement of what matters.

# What you run

- `doctor`: the scope policy exists and parses
- `knowledge sources`, `knowledge nodes --scope shared`: what the index discovers once the policy is applied
- over MCP: `majordomus_scope` reads the policy, `majordomus_scope_classify` answers for one path

# Outcome

What belongs in the AI context is declared once in the layer, checked, and applied by the shell tool and the server alike. A client that honours it indexes the repository, not the machine.
