---
schema: adr/v1
id: adr-0235
kind: adr
title: A node follows the release only by consent, and hands its server over in order
status: proposed
date: 2026-11-22
tags:
  - distribution
  - release
  - mesh
  - runtime
related:
  - file:.ai/repo/adrs/0161-a-mesh-node-follows-the-published-release.md
  - file:.ai/repo/project/milestones/safe-self-update.yaml
  - file:docs/DISTRIBUTION.md
  - file:docs/MESH.md
  - file:apps/majordomus-cli/src/release/follow.rs
  - file:apps/majordomus-cli/src/capability/builtin/self_update.rs
  - test:test/cases/2720_self_update_wraps_the_installer.sh
  - test:test/cases/2721_a_node_follows_the_stable_release.sh
provenance:
  origin: authored
---

# 235. A node follows the release only by consent, and hands its server over in order

## Context

ADR 0161 decided that an enrolled node follows the stable release by itself, through the one
installer, forward only. It left three questions to the implementation, and each of them
decides whether following is safe or merely automatic:

- **When may a machine upgrade without a person?** A node of the owner's mesh is often also a
  machine somebody is working on. The installed tool is what a checkout's hooks, derivations
  and gates run in a repository that adopts Majordomus. An upgrade in the middle of a task
  makes that task's tree be judged by another generation's rules: a projection derived by
  the old tool is refused as stale by the new one, and the worker sees a failure that
  nothing in their change caused.
- **What happens to the running server?** The installer puts the new release in a directory
  of its own and swaps the launcher. The running server keeps serving the old code. Killing
  it would lose its attached sessions; leaving it would mean the board runs yesterday's code
  until it idles out.
- **How often, and configured where?** ADR 0161 said 15 minutes. The owner asked on
  2026-11-22 for a check every five minutes by default, configurable in the committed
  declaration.

The mechanisms the answers need already exist. The installer verifies an archive's digest,
size and provenance attestation before it unpacks anything (case 85, I2165). The lease
replaces an older server when a newer executable asks for one, and the old server hands its
sessions on and ends once it is alone (I2127, I2163). The cooperation journal persists every
session, claim, handover and review per checkout, and a new process of the same checkout is
the same runtime.

## Decision

**1. The follower is a thread of the server of an enrolled node.** It starts once per
process, when the repository's mesh declaration is enabled and this node's key admits it to
the mesh. A repository without a mesh, or a machine its declaration does not list, starts no
check and fetches nothing.

**2. It checks every five minutes by default.** The interval is the declaration's
`self_update.check_interval_seconds` (300 when absent, at least 60), with up to a tenth
added at random so the nodes of a mesh do not ask the site in the same second. The first
check waits up to 30 seconds after the server starts, so a start is never also a download.
A check is `self.update` without `--apply`: one read of `releases/latest.json`, never in the
path of a request.

**3. Consent is checked in a fixed order, and any failure holds the upgrade back until the
next interval.**

1. The stable pointer names a newer, stable, listed release (`decide`, ADR 0161).
2. The running executable is an installation that `install.sh` laid out. A build in a
   checkout is updated by building it, and is never touched by the follower. This is why no
   development tree of this repository is ever upgraded underneath its worker.
3. The mode is `follow: stable`. `follow: pinned` checks and reports, and only a person's
   `majordomus self update --apply` installs. `follow: off` starts no check.
4. The version has not already failed to install on this installation. A failed version is
   recorded and not retried until a newer one is published.
5. The served checkout has no uncommitted changes. A dirty checkout, or one whose state git
   cannot report, defers the upgrade. Two consents lift this, and only these two: the
   declaration's `self_update.dirty: apply` (every node of the repository) and
   `MAJORDOMUS_SELF_UPDATE=follow-dirty` (one machine, in its service unit or profile).

A person who runs `majordomus self update --apply` has consented by running it. The command
does not ask about the checkout.

**4. The machine may narrow and never widen, except to consent to a dirty tree.**
`MAJORDOMUS_SELF_UPDATE` is `off`, `check`, `follow` or `follow-dirty`. A machine can turn
following off or down to reporting. It cannot turn on what the declaration turned off.

**5. The section is the trunk's word.** Whether nodes upgrade themselves decides what the
next tool on every machine is. The `self_update` section a server obeys is therefore the
trunk's committed copy, as the trust root is (ADR 0050 rule 6). A branch that changes it is
narrowed back, and `mesh doctor` names the line.

**6. Only what the installer accepts is installed.** The apply step is `self update --apply`,
which runs the published `install.sh`. The installer refuses an archive whose digest, size or
attestation does not match the metadata, and swaps nothing. The follower reads the version
back from the new launcher, and anything other than the target counts as a failure under
consent 4.

**7. One applier per installation.** An exclusive lock file in the installation's prefix
holds the applying process's pid. A live holder means another server is applying, and this
round holds. A lock whose process is gone is taken over.

**8. The hand-over reuses the existing one.** After a successful install, a server that a
client owns (an MCP session's stdio, a pipe, or `--idle`) runs the new launcher's
`serve ensure` in its checkout. That server finds the running one older and starts its own,
which takes the lease (I2163). The old server retires from the mesh, hands its sessions on and
ends when the last has left (I2127). The journal belongs to the checkout, not to the process,
so every session, claim and handover is still there. A server that runs until it is stopped,
such as a hub under a service manager, is left to that manager or its operator. I2722 owns
its activation, health check and rollback.

**9. Everything is recorded.** Each round writes `<prefix>/self-update.json`: when it ran,
the running and the stable version, the step, the reason, what was installed, how the server
was handed over, and the versions that failed. `majordomus self update --check` prints the
follower's mode and that record. `majordomus mesh peers` prints the version each runtime runs
(I2723).

**10. The repository's own declaration names the section only once the fleet can read it.**
The mesh declaration refuses a key it does not know, so committing `self_update:` while any
enrolled node still runs an executable without it would close that node's mesh. Until then,
this repository's nodes run the defaults (stable, every 300 seconds, deferring over a dirty
checkout), and the declaration documents the section in a comment.

## Alternatives rejected

**Upgrade whenever a release appears, whatever the checkout holds.** That is the failure the
context describes. It costs a worker a refusal they cannot explain, on the machine they are
working on.

**Refuse to upgrade any machine on which a person works.** Then the owner's laptops, which
are most of the mesh, never follow, and ADR 0161's problem comes back.

**Kill the running server after installing.** It loses the sessions attached to it. The
lease's ordered take-over already exists, and its correctness is already tested.

**Configure it per machine only.** Then the repository has no reviewed statement of what its
nodes do. The declaration says what the fleet does. A machine's environment may narrow that,
or consent to a dirty tree, and nothing more.

**Commit the section to this repository's declaration now.** Every node on 1.2.0 would refuse
the declaration and drop out of the mesh. The default is what the declaration would say
anyway.

## Consequences

- I2720 (`self update`) is complete. I2721 is implemented for checkout servers: the check,
  the interval, the consent and the hand-over. Its "mesh announcement triggers a check"
  criterion is open. I2723 is partly done: versions are printed; follow settings and
  `mesh doctor`'s "more than one release behind" are not.
- I2722 (a hub's activation under its service manager, its health check and rollback), I2724
  (an archive taken from a peer), I3177 (two adjacent versions keep linking) and I3178
  (a killed update leaves a usable installation) remain as planned.
- A release now reaches every following, clean, installed node within about one interval of
  `releases/latest.json` naming it. That makes the milestone's "a wire, lease or journal
  schema change needs a version bump" a precondition, as ADR 0161 already said.
- This decision is `proposed`. Accepting it is the owner's act.
