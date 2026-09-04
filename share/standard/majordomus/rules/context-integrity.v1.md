---
id: majordomus.context-integrity
version: 1
kind: rule
title: Context integrity
description: The scoped context documents under the AI layer carry the context contract, compose into one deterministic chain for every path, and a tree that does not validate resolves nothing.
statement: Every context document under the layer carries the context contract with a unique identity, every reference in it resolves within its ancestor chain, no final document is superseded, and the effective context for any path is computed from the tree root down in one deterministic order.
status: active
class: blocking
depends_on: [majordomus.ai-layout-integrity@1, majordomus.minimum-sufficient-context@1]
tags: [context, layout, ai]

x-majordomus:
  validator: context
  category: context
  enforced_by: [doctor, watch]
  exit_code: 10
  claims: [context-documents, context-impact]
  tests: [test/cases/69_context_documents.sh]
---

# Rationale

Context that lives in one root file grows into the always-loaded monolith this tool was
distilled from; context scattered without a contract cannot be found, ordered or trusted.
A document that says where it applies, to whom, and how it composes with its ancestors
can be resolved by a tool and read by a person, and the tree stays honest only while every
such document is checked against that contract.

# Required behaviour

Every context document under the layer carries the context contract with a unique identity, every reference in it resolves within its ancestor chain, no final document is superseded, and the effective context for any path is computed from the tree root down in one deterministic order.

# Failure behaviour

A violation is a `FAIL` finding under the category `context`, naming the file and the
class of the problem, and the command that found it exits 10. Under `watch` the same
violation is reported as drift and the command exits 11. `context resolve` refuses to
answer for a tree with any problem rather than answering partially.

# Verification

`mj_validate_context` decides it, dispatched from `doctor, watch`. The behavioural case
`test/cases/69_context_documents.sh` proves it, and CI runs that case.
