+++
title = "Context integrity"
description = "Context integrity"
weight = 11
[extra]
kind = "rule"
slug = "majordomus-context-integrity-1"
identity = "majordomus.context-integrity@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/context-integrity.v1.md"
+++
{% raw %}

## Rationale

Context that lives in one root file grows into the always-loaded monolith this tool was
distilled from; context scattered without a contract cannot be found, ordered or trusted.
A document that says where it applies, to whom, and how it composes with its ancestors
can be resolved by a tool and read by a person, and the tree stays honest only while every
such document is checked against that contract.

A directory with no document is the quieter half of the same failure: it contributes
nothing to the chain, it names no format for what it holds, and the worker who lands there
reads its ancestors and guesses the rest. Coverage turns the habit of documenting a
directory into an invariant, and the exemption for a subtree whose children are instances
of a kind is declared by the contract that governs it, so it moves with the tree it
describes.

## Required behaviour

Every directory of the layer carries a context document unless the contract governing it exempts its children, every document carries the contract with a unique identity, every reference resolves within its ancestor chain, no final document is superseded, no descendant weakens what an ancestor requires, and the effective context for any path is computed from the tree root down in one deterministic order.

## Failure behaviour

A violation is a `FAIL` finding under the category `context`, naming the file or the
directory and the class of the problem — `missing-contract` for a directory that owes a
document, `illegal-override` for a descendant that lowers `children.require_contract` from
an inherited `true` — and the command that found it exits 10. Under `watch` the same
violation is reported as drift and the command exits 11. `context resolve` refuses to
answer for a tree with any problem rather than answering partially.

## Verification

`mj_validate_context` decides it, dispatched from `doctor, watch`. The behavioural case
`test/cases/69_context_documents.sh` proves it, and CI runs that case.
{% endraw %}
