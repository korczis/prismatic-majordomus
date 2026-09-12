---
schema: context/v1
id: ai.repo.knowledge
kind: context
title: Knowledge
description: How repository knowledge is declared and discovered, never collected.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [lib/knowledge.sh, lib/capture.sh, lib/session.sh, share/knowledge-sources.yaml]
---

# Knowledge

Knowledge is declared, not collected. `sources.yaml` names the classes of repository
files that carry knowledge and the version-control pathspec each class is discovered
through. Discovery reads the Git index, never the filesystem: an untracked file, build
output or a vendored tree is not a source, and two machines discover the same list in the
same order.

`curated/` holds notes this repository chose to write about itself. It is not a copy of
the documentation, the source or the tests; those are referenced by the sources file and
read where they live. Anything compiled from the sources — an index, a graph — is a
rebuildable local product and belongs under `../../local/cache/`.

`candidates/` holds what the tool derived about this repository and nobody has reviewed
yet: one record per decision recorded, question resolved or task finished in an episode,
written from the ledger and git at the episode's end and at every compaction, never from a
conversation and never by a model. A candidate is the same kind as a curated note with
`status: candidate`; it becomes a curated note through `majordomus knowledge promote`, an
act a person performs with evidence, and it is set aside through `majordomus knowledge
reject` with a reason. The directory is tracked so that every checkout can review the queue,
and `doctor` measures the queue against the policy's cap and age so that it is reviewed
rather than accumulated. `docs/KNOWLEDGE.md` describes the derivation and every command.
