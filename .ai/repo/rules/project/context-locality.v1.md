---
id: project.context-locality
version: 1
kind: rule
title: Context locality
description: Context lives in the narrowest directory scope where it stays correct; root files bootstrap and link, provider files are projections.
statement: Context lives in the narrowest directory scope where it stays correct; root files bootstrap and link; provider files are projections.
status: active
class: blocking
depends_on: []
tags: [context, documentation]
---

# Rationale

A fact written at the root is read by every worker on every task, whether or not it applies, and it goes stale the moment the thing it describes moves. A fact written next to what it describes is read by the worker who is there, stays with it when the directory moves, and costs the others nothing. The always-loaded files paid for the opposite habit in the source environment: an operating contract that swung between nothing and a thousand lines because everything was written at the top.

# Required behaviour

Information about a directory lives in that directory's context document, or the nearest ancestor where it is still true for the whole subtree; it is not repeated at the root. The root documents (`README.md`, `.ai/README.md`, `.ai/repo/README.md`) bootstrap and link: they say where things are and how the chain is read, not what every section contains. Provider files (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`) are projections of the policy and carry no context of their own. A document that describes code names it in `tracks`, so a change to that code names the document for review.

# Failure behaviour

No command decides where a sentence belongs; a reviewer does, and a change that puts section-local context at the root, or repository context into a provider file, is not merged. What the tool does decide is the contract around it: `majordomus context validate` fails the tree on a missing or malformed contract, a duplicate id, a broken or cyclic `supersedes`, an override of a `final` document, or a provider the policy does not know, and `doctor` fails on a provider file that carries a rule corpus of its own.

# Verification

Review. test/cases/69_context_documents.sh proves the resolver, the validation and the impact analysis by mutation; `majordomus context validate` is the command a reviewer runs, and `majordomus context affected` names the documents a change set touches.
