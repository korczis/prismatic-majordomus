---
schema: adr/v1
id: adr-0022
kind: adr
title: Repository knowledge is a typed, evidence-backed model held against a committed baseline, and canonicality is its root doctrine
status: accepted
date: 2026-09-07
tags:
  - knowledge
  - documentation
  - canonicality
  - evidence
  - architecture
related:
  - rule:project.canonicality
  - rule:project.knowledge-integrity
  - rule:project.interfaces-are-projections
  - rule:project.generated-artifacts-are-typed
  - rule:project.derived-files-regenerated
  - file:apps/majordomus-cli/src/knowledge/mod.rs
  - file:apps/majordomus-cli/src/knowledge/model.rs
  - file:apps/majordomus-cli/src/knowledge/canonicality.rs
  - file:apps/majordomus-cli/src/capability/builtin/knowledge.rs
  - file:docs/KNOWLEDGE.md
  - file:docs/CANONICALITY.md
provenance:
  origin: authored
---

# 22. Repository knowledge is a typed, evidence-backed model held against a committed baseline, and canonicality is its root doctrine

## Context

This repository already held most of what it knows about itself in typed, discovered
form: the capability registry, the layer's objects with their schemas and typed
references, the generation manifest, the scope, the claims matrix, the curated
knowledge records. What it did not have was a way to say whether any of it still held.
A curated record verified against a file in one commit read exactly as verified after
the file changed; a guide that linked to a directory that was moved kept linking; two
documents that listed the same capability identifiers by hand disagreed the week one of
them was edited. Each of those was found by a person, late, and none by the tool.

Three approaches were on the table. A documentation linter would have caught the broken
links and nothing else. A vector index over the prose would have found related passages
and could not have said which of two was right. A model over the tree — typed nodes with
claims, each claim resting on evidence with a fingerprint — could say what was known,
how, from what, and whether the evidence had moved since. The third is the one a
supervisory tool can stand behind, because every answer it gives is reproducible from
the tree.

The same week, the principle behind every earlier decision about sources and projections
(ADRs 4, 5, 18, 19, 20) was written out as one rule, because a system that audits
canonicality needs the doctrine it audits to exist above its instances.

## Decision

**One model, read off the tree.** The repository knowledge system reads the checkout
through deterministic extractors — git, the layer's index, the Cargo manifests, the
documentation, the capability registry with the generation manifest, the CI workflows
and deployment manifests, and the cached derivations of a semantic provider — into one
typed model: nodes with claims, evidence with fingerprints, typed relations. Every node
carries its provenance (`observed`, `declared`, `curated`, `derived`), its ownership
(external, Majordomus, hybrid), its visibility and its confidence, with the basis of the
confidence stated in words rather than as a number.

**Freshness from fingerprints.** A claim is current while the evidence it was verified
against carries the fingerprint recorded for it, stale when the fingerprint moved,
possibly stale when a node it propagates from changed since the baseline, unverified
when nobody verified it or a reference it makes resolves to nothing, conflicted while it
is party to an open conflict. Dates decide nothing.

**Conflicts stay open.** Two values for a functional predicate about one subject are a
conflict with both sides and their evidence. It is accepted by a person, by name, with a
reason, or corrected; never resolved by the tool.

**A committed baseline that ratchets.** A brownfield repository adopts the system with
one command that records the present state — node fingerprints, verified claims,
tolerated debt, accepted conflicts, tolerated canonicality violations — as
`.ai/repo/knowledge/baseline.yaml`. The check refuses new debt in `protect` mode and
reports tolerated debt that is gone; the baseline is recorded again deliberately, with
the diff as the review.

**Reuse, not a parallel registry.** The model is served by the capability registry as
one module (`rks`), so MCP, HTTP, OpenAPI, the command line, the Cockpit and the
generated documentation are projections of the same descriptors as everything else. The
extractors reuse the index, the graph's relation table, the scope, the generation
manifest and the worktree service rather than reading the tree a second way. The
baseline and the exceptions are kinds of the layer with schemas and allow-lists like
every other file. The layer's curated record gained two fields — `visibility` and
`asserts` — and nothing else changed shape.

**Canonicality above it.** `project.canonicality` is the root doctrine every
canonical-source rule now depends on. The knowledge system audits it: every capability's
canonical source and derived surfaces, every generated artifact's typed `derived_from`,
every orphan projection, undeclared generated file, suspected hand-kept mirror and
expired exception; the manual maintenance surface as the metric; typed, time-limited
exceptions; `majordomus change inspect` as the pull-request gate that lists what a change
adds with the surfaces derived for it.

**Offline core, optional semantics.** Nothing in a scan, a check or a projection calls a
provider. A semantic provider runs under one command, only when the policy enables it,
only off the machine when the policy allows it, only over public evidence, through a
redactor, writing a checkout-local cache that the next scan reads as derived knowledge.
The executable ships one provider, a deterministic offline stub, so that the path is
tested end to end without a network.

## Consequences

- Documentation knows when it is wrong: a curated claim whose evidence moved is named
  stale with the reason and the remedy, and the check refuses to let it pass unnoticed.
- The repository is adopted in one command with its debt recorded, and the debt can only
  shrink. This repository's own baseline is committed; the two hand-kept tables it
  tolerates are the first items of the tightening program.
- Every projection of a capability is checked to be derived from one declaration. A
  change that adds a capability is reviewed against a checklist the tool prints.
- The cost is a scan on every check and on the first Cockpit request of a process,
  memoised for a few seconds: well under a second here, measured, and bounded by the
  index and the file limit rather than by the size of the prose.
- What is not decided here: a real remote provider, which the trait admits and no
  configuration enables; the tightening of the tolerated mirrors, which is planned as
  issues rather than done in this decision; and a shared baseline across repositories,
  which the shared-policy milestone owns.
