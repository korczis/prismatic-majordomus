---
schema: knowledge/v1
id: fabric-starts-with-p00-s00-s01
kind: knowledge
class: fact
title: "Owner decisions 2026-11-24: Majordomus Fabric starts with P00, S00 and S01; ADR 0199 is amended into its decision; #3108 is held; an unattended follower requires provenance; one OS user locally, encrypted between hosts"
description: "When a session asks whether the Majordomus Fabric program (user-scoped host agent, local IPC, host-to-host mesh multiplexing, user services, managed update) has been ordered, where it starts, which ADR carries it, what happens to PR #3108 or #3110, or what Fabric's trust boundary is"
status: verified
epistemics: decided
date: 2026-11-24
tags:
  - decision
  - fabric
  - owner-answer
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
---

# Owner decision 2026-11-24: the Majordomus Fabric program starts with P00, S00 and S01

The owner handed a session a set of prompt packs for **Majordomus Fabric**. Their decided
architecture has four parts:

- a hybrid of one opt-in user-scoped host agent, logical project/worktree contexts and isolated
  workers where they are needed;
- local IPC over a Unix domain socket or a supported named pipe, with HTTP, MCP and REST as derived
  projections;
- authenticated host-to-host mesh multiplexing with logical routing;
- explicit user-scoped service installation with a standalone fallback.

The pack is a base program (`P00`-`P13`) and a hardening addendum (`S00`-`S08` with a corrected
execution DAG and the gates `G0`-`G9`), plus `S09`, the closing convergence of every surface. Its
base anchor is master `93739746f8d9f488bbd3e999fc9ed27818ac81a9`, which was still `origin/master`
when the owner answered.

The owner chose to start the program with the reconnaissance and gate phases the addendum
prescribes, and nothing else yet:

- `P00`: the baseline and ownership audit;
- `S00`: reconciling the ADRs, the ownership graph and the corrected phase DAG (gate `G0`);
- `S01`: measuring the legacy per-checkout runtime under controlled workloads (gate `G1`).

No host-agent code comes before `G0`, and no efficiency claim comes before `G1`. The first useful
milestone is the addendum's `G5` (local pilot): one opt-in host service for 2 repositories,
10 worktrees and 20 MCP sessions, with standalone fallback.

The pack is a brief, not authority. Its file paths and ADR numbers are historical claims that the
phases verify against the current tree. Accepted ADRs change only through this repository's own
procedure.

# Evidence

- When: 2026-11-24.
- Asker: session 9ae82e7a-b0e4-44c0-a305-9c69cd1acf6a (openttd-7d), through AskUserQuestion, after
  the owner sent the pack files without an instruction.
- Question, verbatim: "The files you sent are a Claude Code implementation program for the
  prismatic-majordomus repository (not OpenTTD). It is 'Majordomus Fabric': one user-scoped host
  agent with UDS/named-pipe IPC, logical contexts, a host-to-host multiplexed mesh, user service
  installers, managed updates, then retirement of per-checkout servers. It has 14 base phases
  (P00-P13), 9 hardening phases (S00-S08) and S09, a closing convergence of every surface. The
  duplicates are byte-identical, and the integrated pack = base + hardening. Nobody has started
  yet: prismatic-majordomus origin/master is exactly the pack's anchor 93739746f8 and there is no
  Fabric branch or PR. What should I do with it?"
- Answer, verbatim: "Start with P00+S00+S01 (Recommended)".
- The pack files the owner sent: `majordomus-fabric-integrated-pack-20261123.zip` (its
  `START_HERE.md` points at `hardening/MASTER_ADDENDUM.md` and `hardening/EXECUTION_DAG.md`;
  both `SHA256SUMS` verify), `majordomus-fabric-hardening-addon-20261123.zip`,
  `majordomus-fabric-prompt-pack-20261123.zip`, and the loose copies `MAJORDOMUS_FABRIC_MASTER_PROMPT.md`,
  `MAJORDOMUS_FABRIC_ARCHITECTURE.md`, `MASTER_ADDENDUM.md`, `EXECUTION_DAG.md`, `PROOF_MATRIX.md`
  and `MAJORDOMUS_FABRIC_S09_DERIVED_SURFACE_CONVERGENCE.md`. The pack is not committed here.

# Further answers of 2026-11-24 (after the P00 + S00 reconnaissance)

The reconnaissance (read-only, at master `93739746f8`) found four questions that only the owner
can answer. They were asked in one AskUserQuestion, from the same session, and each answer is
recorded verbatim.

**1. The ADR that carries Fabric.** Question: "Fabric (one host agent per user) contradicts
accepted ADRs 0001 ("no port, no daemon"), 0003 ("a lease, not a daemon") and 0044 (one
server per checkout, no registry, no daemon). The closest existing decision is ADR 0199 ("a hub
is a declared host service": one user-scoped unit, the same executable, user-only IPC, never a
second registry or update system). It is proposed and you have not accepted it. How should
Fabric be decided?" Answer: "Amend ADR 0199 (Recommended)". The option read: "I amend ADR 0199
into Fabric's decision (opt-in host agent, UDS, contexts, workers), and it explicitly narrows
0001/0003/0044 for the opt-in host mode while the standalone mode stays. You accept or reject it
afterwards in the PR; no host-agent code until then."

**2. PR #3108.** Question: "PR #3108 (mesh out of the box, ADR 0234) makes every server of an
enrolled machine bind 0.0.0.0, every linked worktree included. That means one cleartext LAN
listener per checkout, the opposite of Fabric's one host link pool. What should happen to
#3108?" Answer: "Hold, rework into Fabric (Recommended)". The option read: "Don't merge #3108 as
is. Reuse its enrolment logic (resolve_node_http_host) as host-transport admission in Fabric
P03/S06, so only the host binds the LAN, not every checkout."

**3. Provenance of an unattended update.** Question: "PR #3110 (self update and a follower in
every enrolled server) runs install.sh with the inherited environment. Unless
MAJORDOMUS_REQUIRE_PROVENANCE=1 is set, install.sh installs on the checksum alone when gh is
missing or signed out (install.sh:513-552). An unattended `follow: stable` would then install a
release without verified provenance. What should be required?" Answer: "Required for unattended
(Recommended)". The option read: "An unattended follower (follow: stable) always runs with
provenance required, and refuses the update when provenance can't be verified. A manual `self
update` keeps today's behaviour with a warning. #2977 closes as a duplicate of #3110."

**4. The trust boundary.** Question: "What is Fabric's trust boundary on one machine and between
machines? Today a loopback caller gets every capability of every checkout, and the hub links on
the private network and tailnet are cleartext HTTP (docs/MESH.md)." Answer: "One OS user +
encrypted (Recommended)". The option read: "Locally only the same OS user, through a UDS with
0600 permissions and checked by peer credentials (another OS user is out). Host-to-host only
over an authenticated, encrypted channel, tailnet included. Cleartext hub links stay only for
the legacy mode."
