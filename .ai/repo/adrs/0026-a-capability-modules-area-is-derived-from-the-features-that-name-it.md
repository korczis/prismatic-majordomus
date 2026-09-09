---
schema: adr/v1
id: adr-0026
kind: adr
title: A capability module's area is derived from the features that name it, not declared beside it
status: accepted
date: 2026-09-09
tags:
  - architecture
  - taxonomy
  - projections
  - product
related:
  - rule:project.canonical-order
  - rule:project.no-new-nouns
  - rule:project.interfaces-are-projections
  - file:apps/majordomus-cli/src/product.rs
  - file:apps/majordomus-cli/src/cockpit/nav.rs
  - file:.ai/repo/why/README.md
  - file:.ai/repo/features/README.md
provenance:
  origin: authored
---

# 26. A capability module's area is derived from the features that name it, not declared beside it

## Context

The sidebar's sixteen capability modules were a flat alphabetical list, and the obvious next
step was to group them. Every way of doing that begins by answering one question: where does
a module's group come from?

The tempting answer is a new one — a namespace model, an id with segments, a file that
declares the tree. It is also the wrong answer here, because this repository already has the
pieces and adding a seventeenth taxonomy would be the sediment `project.no-new-nouns`
exists to refuse. What it has:

- **`ModuleId` is already the namespace.** It is regex-validated, it is machine-enforced —
  the registry refuses a capability composed outside its module's namespace
  (`RegistryError::ModuleMismatch`) — and it is already unified across both kinds of module:
  a builtin Rust module declares it, and a declarative object's module *is* its kind. It is
  also already the OpenAPI tag. A typed namespace model would be a second one.
- **The areas already exist, ranked.** `.ai/repo/why/areas/` holds nine operational areas,
  each with a title, a summary and a `weight` that says how much the operator cares.
- **Features already join the two.** A feature declares the `modules:` it is built from and
  the `areas:` it serves (ADR 23). The edge from module to area was already in the data;
  nothing read it.

So the missing thing was never a namespace. It was one level of parent, and one function.

## Decision

A capability module's area is the areas of the features that name it, resolved by the
catalogue's own `weight`: lowest first, ties broken by id. No module declares an area, no
file lists the pairs, and the Cockpit's sidebar asks the product model rather than holding a
grouping of its own.

Where two features name one module and their area sets do not overlap at all, they disagree
about what the module is for. The resolution stays deterministic — the ranking decides — and
the disagreement is reported as `contested_area` against the module, so that it is settled
in the feature file where the semantics live rather than by a tiebreak in a function. On
this repository that is exactly two modules, `health` and `repository`, and the finding
names both claimants and the areas each serves.

A module that no feature with an area names is shown under no heading. The canonical order
(ADR 25) already puts an absent group last, so nothing special-cases it.

The resolution reads a ranking, not a catalogue: `resolve_module_areas` takes the areas as
weights and returns the assignment and the disagreements. That is the whole of what the rule
needs, and it is what the tests exercise.

## Alternatives rejected

**A `namespace:` or `area:` field on the module declaration.** One line per module in Rust,
and the taxonomy then lives in two places — the feature that says what the module is for,
and the module that says where it goes — with nothing keeping them honest.

**A namespaces file.** `namespaces.yaml` with ids, parents and titles is the list a person
maintains, which is the failure mode this repository has removed everywhere else: providers,
kinds, capabilities, features, moments and areas are all discovered, and the one thing
nobody has to remember to update is a list that does not exist.

**A new hierarchical identity (`governance.rules`, `ai.providers`).** It would be a second
namespace beside `ModuleId`, and it would need its own validation, its own resolution, its
own aliases for the ids that are already published in MCP resource URIs, HTTP routes and
OpenAPI tags. The parent is one derived attribute; a new identity would be a migration.

**Picking the majority area, or the first feature's.** Neither is a fact about the domain.
The weight is: the areas were ranked by the operator when they were written, and reusing
that ranking is reusing an answer rather than inventing one.

**Silently picking a winner when features disagree.** The tiebreak has to exist so that the
sidebar is deterministic, but a disagreement between two feature files is a data defect and
the tool says so. Two of sixteen modules are contested today, which is a small enough number
to be worth a person's attention and too small to justify a mechanism.

## Consequences

- The Cockpit's sidebar groups its modules under five area headings, all sixteen placed, in
  the canonical order at both levels.
- The derivation found a real gap on its first run: the `install` feature declared no
  `areas:`, so `distribution` had no parent. That is now declared — `verification` and
  `documentation`, which is what the feature's own summary describes — and the gap would
  have been reported rather than hidden if it had not been.
- `product validate` gained the `contested_area` warning; the repository has two.
- Nothing else changes to add a module to the hierarchy: compose it, name it from a feature,
  and it appears under that feature's area on every surface that groups.
- The same derivation is available to any other surface that wants to group modules. It has
  one consumer today, and was written for that consumer rather than in advance of one.
