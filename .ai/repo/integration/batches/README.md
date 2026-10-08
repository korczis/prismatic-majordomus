---
schema: context/v1
id: ai.repo.integration.batches
kind: context
title: Integration batches
description: One manifest per batch the integrator composed, naming its members in order with the head and the merge commit of each; written by prs compose, checked by batch-check.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/integration-batch/integration-batch.v1.schema.json, share/kinds.yaml, apps/majordomus-cli/src/integration/compose.rs]
---

# Integration batches

One file per batch, `<id>.yaml`, contract `integration-batch/v1`
(`share/schemas/majordomus/integration-batch/integration-batch.v1.schema.json`), kind
`integration-batch` (`share/kinds.yaml`). The decision is ADR 0114.

A manifest says which master the batch was composed on and, in composition order, each
member pull request: its number, the head that was merged, its title, and the merge commit
on the batch's first-parent line that carries it. It also records the version the composed
tree declared before and after the one `release bump` the composition takes.

## Who writes these

`majordomus prs compose --apply`, in the batch's one composition commit, and nobody else.
The id is derived — the first ten characters of the master, then the members' numbers — so
the same members on the same master are the same batch and the same file. A manifest lands
on the base with its batch and stays: it is how a regression found later is traced from a
merge commit on master's history to the member that brought it.

A fix is never written here or on the batch branch. It is written on the member's own
branch, and the batch is composed again.

## What reads them

```text
majordomus prs batch-check     the gate: a branch that merges two or more open pull requests
                               must carry exactly one manifest, and it must agree with git
majordomus prs explain <n>     a batch names its members; a member names the batch
/cockpit/integration           the open batches and their members
```

## Invariants

- the file name is the `id`, and the id is the `base_master` (ten characters) followed by
  the members' numbers in order;
- `members[i].merge_commit` is the i-th merge on the branch's first-parent line from
  `base_master`, and `members[i].head` is that commit's second parent;
- at least two members: one pull request is `prs repair`, not a batch.
