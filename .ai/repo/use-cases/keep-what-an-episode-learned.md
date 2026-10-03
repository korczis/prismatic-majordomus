---
id: keep-what-an-episode-learned
kind: use-case
title: 'Keep what an episode learned, without a model and without a conversation'
summary: 'Record a decision inside an episode, derive it into a candidate knowledge record from the ledger, read the evidence that the derivation ran, and watch a second derivation change nothing.'
category: knowledge
status: active
target: guaranteed
actors: [maintainer, agent]
difficulty: basic
commands: [start, decision, knowledge, history]
doctrines: [majordomus.knowledge-observed, majordomus.knowledge-integrity]
claims: [knowledge-derived-at-the-boundary, knowledge-derivation-is-idempotent]
responsibilities: [layer]
applications: [repository-with-authored-governance]
---

# Situation

A worker decided something an hour ago, in a conversation that will be compacted by lunch and gone by tomorrow. The decision is in the ledger of one machine, under a retention cap, and nowhere a second checkout or a public surface can reach it. Nobody will remember to write it down, and a paragraph somebody did write would be a summary of a conversation, which the layer refuses to store.

# Scenario

```yaml
setup: session-open
given:
  - 'Majordomus installed, with one execution episode open in this worktree'
  - 'the knowledge section carries the curated and candidates source classes and no candidate yet'
steps:
  - id: open-the-work
    run: ['start', 'keep what this episode learned', '--scope', '.ai/repo/knowledge']
    note: 'a task is optional for the derivation; it is here so the decision has one to name'
    expect:
      exit: 0
  - id: decide-something
    run: ['decision', 'add', 'A candidate is reviewed by a person before it is curated', '--why', 'a record only one machine can read is a record nobody reads']
    note: 'one decision.recorded line in the ledger, stamped with the open episode'
    expect:
      exit: 0
  - id: derive-it
    run: ['knowledge', 'derive']
    note: 'from the ledger and git, deterministically: one candidate record, status candidate, origin extracted, derived_from naming the episode and the decision'
    expect:
      exit: 0
      stdout_contains: ['^written \.ai/repo/knowledge/candidates/', 'knowledge derive: 1 written']
  - id: the-evidence-it-ran
    run: ['history', '--event', 'knowledge.derived']
    note: 'the ledger says which episode was derived and what was written; the line exists even when nothing was'
    expect:
      exit: 0
      stdout_contains: ['knowledge.derived']
  - id: derive-it-again
    run: ['knowledge', 'derive']
    note: 'the same evidence yields the same bytes: the record id is the episode plus a digest of the evidence, so the file is rewritten identically and reported unchanged'
    expect:
      exit: 0
      stdout_contains: ['^unchanged \.ai/repo/knowledge/candidates/', 'knowledge derive: 0 written. 1 unchanged']
then:
  - 'the decision is a tracked record with resolvable provenance, readable from every checkout and every surface'
  - 'verified is still nobody''s: the deriver wrote a candidate, and promotion is an act a person performs with evidence'
```

# Outcome

`knowledge derive` turns the episode's ledger lines into candidate records under `.ai/repo/knowledge/candidates/`, the ledger records that it ran, and a second run over the same evidence writes nothing. The provider's end and compaction events do the same without anybody typing the command; `majordomus knowledge promote` is where a person makes a candidate verified.
