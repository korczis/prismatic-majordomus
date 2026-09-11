---
id: project.knowledge-integrity
version: 1
kind: rule
title: What the repository knows about itself is evidence-backed, freshness-aware, and never drifts silently
description: Every piece of repository knowledge carries its provenance, its evidence and its freshness; a claim whose evidence moved is reported stale, two sources that disagree are reported as a conflict and never reconciled silently, existing debt is recorded in a committed baseline that may only shrink, and new debt is refused.
statement: Repository knowledge is a typed node with claims, each claim carrying its provenance, its evidence with fingerprints and its freshness; freshness is decided from evidence, never from a date; a contradiction between sources is a conflict the model reports until a person accepts it by name; the debt a repository had when it adopted the system is recorded in a committed baseline; new debt is refused by the check in protect mode, and the recorded debt may only shrink.
status: active
class: blocking
depends_on: [project.canonicality@1, project.derived-files-regenerated@1]
tags: [knowledge, freshness, documentation, evidence]
---

# Rationale

Documentation drifts because nothing tells it that the thing it describes has moved. A
guide names a version, the manifest changes, the guide is now wrong, and the first person
to notice is the one it misled. A curated note records what a person verified, the file
it was verified against changes, and the note reads exactly as confidently as before.

The repository knowledge system (`docs/KNOWLEDGE.md`) exists so that documentation knows
when it is wrong. It reads the tree — manifests, documents, decisions, rules, the
capability registry, the generation manifest, the curated records — into typed nodes
with claims, and every claim carries the fingerprint of the evidence it was made against.
When the evidence moves, the claim is stale, and the system says so with the reason.

This rule fixes what that system may never do: it may not decide freshness by a date,
because a date says when a thing was written and nothing about whether it still holds;
it may not resolve a contradiction by picking a side, because the tree and the person
who wrote the note are both witnesses and choosing between them is a decision a person
makes; and it may not let a brownfield repository's debt hide, because a check that fails
on adoption is a check nobody turns on.

# Required behaviour

**Provenance and evidence.** Every node says how it is known — `observed` off the tree,
`declared` in the layer, `curated` by a person, `derived` by a provider — and every claim
names the evidence it rests on, each piece with a content fingerprint.

**Freshness from evidence.** A claim is `current` while the evidence it was verified
against carries the fingerprint recorded for it; `stale` when that fingerprint moved;
`possibly_stale` when something it propagates from changed since the baseline;
`unverified` when nobody verified it or when a reference it makes resolves to nothing;
`conflicted` when it is party to an open conflict. No freshness is decided by a date.

**Conflicts are reported.** Two values for one functional predicate about one subject
are a conflict with both sides, their provenance and their evidence. The model reports it
until a person accepts it by name with a reason (`majordomus knowledge accept`) or
corrects one side. Nothing accepts a conflict on its own.

**The baseline ratchets.** `majordomus knowledge bootstrap` records the present state —
every node's fingerprint, every curated claim verified against its present evidence,
every present debt tolerated by name, every open conflict accepted with the reason that
it was present. From then on `majordomus knowledge check` in `protect` mode refuses debt
the baseline does not tolerate and reports tolerated debt that is gone, so the baseline
can be recorded again with less in it and never silently with more.

**Nothing writes but a person's command.** A scan, a check and every projection are
read-only. Recording the baseline, accepting a reconciliation or a conflict, and running
a semantic provider are commands a person runs, each writing one file with the diff in
the commit.

**The semantic layer is off, and offline first.** No provider runs unless the policy
enables it; no provider that sends text off the machine runs unless the policy allows it;
what a provider derives is marked `derived`, carries the provider, the model, the
operation and the prompt version, and is never current until a person verifies it.
Evidence outside the public visibility never leaves the machine.

# Failure behaviour

`majordomus knowledge check` exits 10 in `protect` mode naming every new debt item: a
stale or unverified node the baseline does not tolerate, an open conflict nobody
accepted, a canonicality violation. It runs in the `rust-integration` gate and in the
pre-commit hook. `observe` and `warn` report without failing, for adoption; `strict`
refuses any debt, for a repository that has paid it down.

# Verification

- `apps/majordomus-cli/tests/knowledge.rs` — a curated claim is current after bootstrap,
  stale after its evidence changes, current again after `reconcile --accept`; a
  contradiction is an open conflict until accepted; the check fails and passes at each
  step with the documented exit codes; the public projection carries nothing restricted.
- `test/cases/99_knowledge.sh` — the same flow through the built executable.
- The crate's unit tests under `apps/majordomus-cli/src/knowledge/` — freshness,
  conflicts, the baseline round trip and migration, the redactor, the provider policy.
