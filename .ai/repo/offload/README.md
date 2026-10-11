---
schema: context/v1
id: ai.repo.offload
kind: context
title: Derive offload
description: The one declaration of the machines a branch's derive may be offloaded to and the limits each must meet; `majordomus offload` reads it, and no script carries a host address.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/derive-offload/derive-offload.v1.schema.json, share/kinds.yaml, docs/OFFLOAD.md]
---

# Derive offload

`derive.yaml` names the hosts that may derive a branch for a session — each one's ssh
address and identity file — the free space a host must have to be chosen, the FIFO queue
every waiter on a host joins, and where the checkout, the worktrees and the shared build
directory are on a host. `majordomus offload hosts` prints it with its defaults filled in,
`offload probe` measures every host against it, and `offload derive <worktree>` uses it.

Adding a machine is one entry under `hosts:`: it needs ssh access without a prompt, a
checkout of this repository at `remote.checkout` with `origin` reachable, and the
toolchain `scripts/derive` builds with. `offload probe` is the check that it answers.

The schema is `derive-offload/v1`
(`share/schemas/majordomus/derive-offload/derive-offload.v1.schema.json`), validated on
every index build; the typed reading is `apps/majordomus-cli/src/offload/declaration.rs`.
A repository declares one. `docs/OFFLOAD.md` is the reference.
