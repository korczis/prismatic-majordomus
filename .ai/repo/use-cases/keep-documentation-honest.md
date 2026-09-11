---
id: keep-documentation-honest
kind: use-case
title: 'Know when the documentation is wrong before a reader does'
summary: 'Adopt the repository knowledge system in one command, and from then on a curated claim whose evidence moved, a link to nowhere, a contradiction between a note and a manifest, and a hand-kept copy of the registry are refused by name instead of found by a reader.'
category: knowledge
status: active
target: guaranteed
weight: 84
actors: [reviewer, agent]
difficulty: intermediate
commands: [knowledge, doctor]
mcp_tools: [majordomus_knowledge, majordomus_knowledge_explain, majordomus_knowledge_context, majordomus_canonicality, majordomus_change_inspect]
doctrines: [majordomus.ai-layout-integrity]
claims: [knowledge-freshness-from-evidence, knowledge-baseline-ratchet, knowledge-conflicts-never-silent, canonicality-audit, knowledge-public-projection-leaks-nothing]
responsibilities: [layer, doctor]
applications: [ci-gated-project, repository-opened-in-ai-clients]
---

# Situation

A repository has years of documentation, a handful of curated notes a person once verified, and a
capability registry that three documents describe by hand. Some of it is wrong and nobody knows
which part. Turning on a checker that fails on all of it at once is a checker nobody turns on.

# What you run

- `majordomus knowledge bootstrap` (the Rust executable): scan the checkout, verify every curated
  claim against its present evidence, tolerate every present debt by name, write the baseline
- `majordomus knowledge check`: exit 10 with every new debt item named; what CI and the pre-commit
  hook run
- `majordomus knowledge stale`, `conflicts`, `gaps`: what a person should look at, with the remedy
- `majordomus explain <subject>`: why the model holds a thing, and what it rests on
- `majordomus canonicality check` and `majordomus change inspect`: every capability's one source
  and derived surfaces, and what a change adds
- over MCP: `majordomus_knowledge_context` before touching a path, `majordomus_knowledge_explain`
  for the why
- `knowledge sources` (the shell tool): the declared source classes the scan reads from

# Scenario

```yaml
setup: installed-wired
given:
  - 'the layer installed; the knowledge section declares its source classes'
steps:
  - id: sources-declared
    run: ['knowledge', 'sources']
    note: 'the curated records, the baseline and the exceptions are declared classes, not special cases'
    expect:
      exit: 0
      stdout_contains: ['^curated +knowledge', '^knowledge_baseline +knowledge-baseline', '^knowledge_exceptions +knowledge-exceptions']
  - id: layer-healthy
    run: ['doctor']
    note: 'the two new kinds validate like every other'
    expect:
      exit: 0
      stdout_contains: ['doctor: 0 failure']
then:
  - 'a curated claim whose evidence moved is reported stale with the reason, and the check refuses it until a person verifies it again'
  - 'a note that contradicts a manifest is an open conflict with both sides, accepted only by name'
  - 'a document that lists capability identifiers by hand is a suspected mirror the audit names'
  - 'nothing outside the public visibility appears in the public projection or reaches a provider'
```

# Outcome

The repository is adopted with its debt written down rather than hidden, and from that commit on
the documentation knows when it is wrong: every stale claim, every contradiction, every orphan
projection is refused by name before it is merged. The full flow through the built executable is
`test/cases/99_knowledge.sh` and `apps/majordomus-cli/tests/knowledge.rs`; the model is
`docs/KNOWLEDGE.md`, the doctrine `docs/CANONICALITY.md`.
