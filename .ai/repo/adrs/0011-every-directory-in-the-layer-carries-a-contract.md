---
schema: adr/v1
id: adr-0011
kind: adr
title: Every directory in the layer carries a contract
status: proposed
date: 2026-09-05
tags:
  - context
  - governance
provenance:
  origin: extracted
  derived_from:
    - file:lib/context_docs.sh
    - file:docs/CONTEXT.md
---

# 11. Every directory in the layer carries a contract

## Context

The layer already resolves scoped context: a `README.md` under `.ai/` that carries
`schema: context/v1` is a directory contract, the effective context for a path is the
chain of contracts from the root down, and `majordomus context validate` refuses a tree
whose contracts are malformed, duplicated, cyclic or illegally superseded. What it never
asked is whether a directory has a contract at all. A `README.md` present without the
contract is an error; a directory with no `README.md` is silence, and silence is what a
tree drifts into. When this decision was taken, nine directories under `.ai/` carried a
contract and eighteen did not, including sections a worker is sent to daily
(`rules/project`, `project/issues`, `knowledge/curated`, `benchmarks/`).

The gap is not cosmetic. A rule of this repository says context lives in the narrowest
scope where it stays correct (`project.context-locality`), and the resolver is built to
compose exactly that. A directory without a contract cannot participate: it contributes
nothing to the chain, it names no format for the files it holds, and a worker who lands
there reads its ancestors and guesses the rest. The obligation to document a directory
was a habit, and a habit is not an invariant.

The same question arrived from the outside as a proposal for a general governance engine:
rule severities (`enforced`/`advisory`/`optional`), evaluation ordering (`before`,
`after`, `priority`), trigger and applicability vocabularies, per-directory declarations
of the artifact kinds a directory may hold, and field-level merge algebra over inherited
front matter. That proposal is answered here too, because most of it already exists under
other names and the rest buys nothing this repository can consume.

## Decision

Coverage becomes a validation class. Every directory of the governed tree carries a
context document, and `majordomus context validate` reports `missing-contract` for one
that does not, naming the directory and the contract that put it under the obligation.
The governed tree is the one the resolver already reads: the manifest's directory, minus
`local/`, minus the vendored rule package, whose integrity is the manifest's job and not
a reader's.

Which directories are exempt is declared by the contracts themselves, not by a list at
the root. A contract may carry `children.require_contract`, and the value that applies to
a directory is the one from the nearest ancestor contract that declares it; where nothing
declares it, the default is `true`. The field composes by narrowing only: a descendant
may raise `false` to `true` for its own subtree, and may not lower an explicitly declared
`true` to `false`. That refusal is a validation class of its own, `illegal-override`,
which the tree already uses for a descendant that supersedes a `final` document. One
field, one documented algebra, one consumer — instead of a generic deep merge whose
result nobody can predict.

Rejected, with reasons.

*Severity as a third axis.* A rule here declares `class: blocking` or `class: advisory`,
and whether the tool can decide it is already visible in the presence of an
`x-majordomus` block: a rule with a validator is enforced by a command, a rule without
one is enforced by a reviewer. `optional` has no consumer — nothing in this tool activates
a rule per task or per profile — and a field nothing reads is a typo that silently does
nothing (`project.unknown-keys-are-errors`).

*Ordering as `before`, `after` and `priority`.* Rules here are resolved as a dependency
graph and applied as a set; nothing evaluates them in sequence, so an evaluation order is
a number with no meaning to defend. `depends_on` already yields a deterministic order for
rendering, and a tie is broken by identity, not by a hand-tuned integer.

*`triggers` and `applies_to` vocabularies.* Applicability here is a path question, and
paths are what the tool can check. A rule that claims to apply to "planning" and
"refactoring" cannot be refuted by anything the tool observes, and an unrefutable
declaration is decoration (`project.no-claim-without-test`).

*`content.kinds` per directory.* The manifest already maps a section to its directory and
the kind that lives there; repeating the mapping in each contract is a second registry
that can disagree with the first (`project.derived-once`).

## Alternatives rejected

A list of governed directories in the manifest or the policy. Central, and wrong for the
same reason the root file is wrong for context: it puts a fact about a directory somewhere
the directory's own reader never looks, and it goes stale on the first rename. The
contract that governs a subtree is the honest place for the exemption, and it moves with
the tree.

Requiring a contract for every directory with no exemption at all. It reads well until a
kind's instances are directories rather than files: a skill is `SKILL.md` plus its
examples, and a contract in `skills/<name>/examples/` would document the format of the
kind, once per instance, which is the duplication the section contract exists to prevent.
The exemption is not a loophole; it names where instances of a kind begin.

Detecting exemptions by heuristic — a directory that holds no Markdown, a directory whose
name matches a pattern. Silent, unexplainable and different on every tree. The tool
refuses to guess elsewhere and does not start here.

## Consequences

A directory added under `.ai/` now fails validation until it says what it is for, which
is the obligation this decision exists to create; `context validate` runs in the
pre-commit hook through `doctor`, so the failure arrives before the branch does. Eighteen
directories are given contracts as part of this change, and each one is a place a worker
lands with less guessing.

The contract schema gains one key, `children.require_contract`, with a composition rule
that is narrowing-only. That is the first field-level algebra in the layer; every other
key is scalar and local. The cost is documenting it (`docs/CONTEXT.md`, `docs/SCHEMAS.md`)
and proving it by mutation in the behavioural case, both of which this change carries.

What stays outside the tool remains outside it: whether a contract's prose is any good is
a reviewer's judgement, and the tool checks only that the contract exists, parses, and is
not weakened by a descendant. The layered claim is the honest one — the presence of a
contract is mechanical, its usefulness is not.
