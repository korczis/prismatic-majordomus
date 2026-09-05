---
id: trace-a-change-to-the-context-it-affects
kind: use-case
title: 'Trace a change to the context documents it affects'
summary: 'List the scoped context documents, resolve which apply to a path and why, and find out which documents a change set touches before it lands.'
category: knowledge
status: active
target: guaranteed
weight: 72
actors: [maintainer, reviewer]
difficulty: intermediate
commands: [context, doctor]
doctrines: [majordomus.context-integrity, majordomus.ai-layout-integrity]
claims: [context-documents, context-impact, ai-layer-manifest]
responsibilities: [layer]
applications: [repository-with-authored-governance, repository-opened-in-ai-clients]
scenario:
  setup: installed
  given:
    - 'a repository with Majordomus installed and projections generated, and one commit of work'
  steps:
    - id: list
      run: ['context', 'list']
      note: 'every scoped document: id, path, scope, composition, providers, status'
      expect:
        exit: 0
        stdout_contains: ['ai.layer']
    - id: resolve
      run: ['context', 'resolve', 'lib']
      note: 'the documents that apply to a path, root to target, in effective order with provenance'
      expect:
        exit: 0
        stdout_contains: ['^01  ai.layer']
    - id: explain
      run: ['context', 'explain', 'lib']
      note: 'the same, with why each applies and why each candidate was left out'
      expect:
        exit: 0
        stdout_contains: ['ai.layer']
    - id: affected
      run: ['context', 'affected']
      note: 'which documents the working tree’s changes touch; nothing is changed here, so nothing is affected'
      expect:
        exit: 0
        tree_unchanged: true
    - id: sync
      run: ['context', 'check-sync']
      note: 'the tree validates, every projection matches its stamp, and nothing is affected'
      expect:
        exit: 0
        stdout_contains: ['in sync']
  then:
    - 'resolution is deterministic: the same path gives the same ordered list'
    - 'a change under a scope names the document that governs it'
    - 'the projections are checked against the documents they were rendered from'
---

# Situation

Context is not one file. A directory carries its own document, the root carries another, and a change deep in the tree may fall under a document nobody remembers writing. When a document changes, which paths and which projections does it affect? Guessing is how a stale rule survives a review.

# What you run

- `context list`: every scoped document with its scope and composition
- `context resolve <path>` and `context explain <path>`: what applies to a path, in order, and why
- `context affected`: the documents a change set touches, from git
- `context check-sync`: validate the tree, then the projections against their stamps, then the affected set

# Outcome

Every path has one deterministic answer to “what governs me”, every change has one answer to “what did I touch”, and a projection rendered from a document that has since changed is reported, not trusted.
