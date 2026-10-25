---
name: session-start
description: join the cluster and take up the work that is already owed, before anything else
---

# Join the cluster, then take up the work that is owed

You are one worker among several on this repository, on this machine and on others. A
session that starts by working alone repeats the day nineteen workers were mistaken for one
(ADR 0226). Join them first, then continue what is already owed. Do not ask what to do
first: the layer in `.ai/` says.

In this order:

1. **Orient.** `majordomus worktree` says which branch and worktree you are in and whether
   that is where the branch belongs. Read the session briefing already in your context (`majordomus context` asks for it again); a handover older than the
   policy's staleness is context, not an instruction.
2. **Say how you are attached.** If the `majordomus_*` tools are present, you are attached
   over MCP. If they are not (the server failed to connect), say so first with the client's
   reason, and use the CLI for every step below; absence of a surface is reported state, not
   a reason to stop.
3. **Read the board before you claim anything.** `majordomus_peers` (CLI: the shared
   server's `GET /api/v1/peers`) gathers every checkout's board; `complete: false` means a
   checkout could not be asked, and `boards` says which. `majordomus_mesh_peers` (CLI:
   `majordomus mesh peers`) shows the other machines. Read the open asks addressed to
   everyone and the reviews with no answer: they are work.
4. **Find what is owed.** The active task and its scope are the work; read its record and the
   one next action it names (`majordomus prompt show continue` is the discipline). With no
   active task, or a record that names no next action, say so and take one open ask from the
   board or none: do not invent scope.
5. **Check for collisions.** For anything new, `scripts/collision-check --new <paths>`, and
   compare the scope with the peers' claims. If another worker or machine holds an
   overlapping scope, do not start it: report who, where and what they announced.
6. **Announce.** `majordomus_announce` with your intent and the paths and identifiers you
   are about to take, and `majordomus_mesh_claim` when the work must not overlap across
   machines. Announce again whenever the connection is re-established.
7. **Work inside the task's scope**, evidence first, following `majordomus://skill/implement`.
   Checkpoint at the profile's interval. Finish with `majordomus check`, never with a claim.

Limits: everything you do now stays local until the person you work for says otherwise. Do
not commit, push, merge, open a pull request, delete what you did not create, or finish a
task as `completed` on your own. Compile nowhere the cluster does not route builds to, and
nowhere the disk cannot take it.

Report briefly, in the person's language: how you are attached, what the board looked like,
what you announced, what you are doing and what you will do next.
