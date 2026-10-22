---
schema: skill/v1
id: deliver-through-the-mesh
version: 1
title: Deliver through the mesh
description: Work as one of many sessions on the distributed Majordomus mesh the MJ way, plus the extra mile, updating everything; take a role, finish your own debt first, hand work to the landing seat, and ship a verified major.minor.patch release within minutes, with docs, landing page and GitHub Pages current.
status: active
tags: [mesh, delivery, release, coordination]
related: [implement, deploy-site, evolve]
inputs:
  - a checkout of this repository with the mesh enabled (.ai/repo/mesh/majordomus.yaml)
  - a running shared server for the checkout (majordomus serve ensure)
outputs:
  - a board that says what this session does, holds and owes
  - every branch of this session pushed, and every judged head handed to the landing seat or landed by it
  - a release cut and verified on the site and on every enrolled node, when the trunk earned one
  - docs, comments, the landing page and the site current with what landed, deployed to GitHub Pages
---

# Purpose

The owner's standing order of 2026-10-14, to every session on the mesh: cooperate in the
distributed Majordomus mesh, use the coordinator and the other roles, finish your work, your
session debt and your stack, and deliver in major.minor.patch releases as often as possible,
deployed, with docs, comments and everything else up to date. In the owner's words: "mj
way + extra mile + update everything", covering documents, the landing page and GitHub Pages,
deployed as soon as possible, which is 5-10 minutes from tag to served (ADR 0162).

- **The MJ way:** declared once, derived, enforced, proven (the handbook chapter "MJ way").
- **The extra mile:** the change also fixes what it touched that was wrong nearby, proves
  itself with a case, and leaves the next session less to do.
- **Update everything:** every surface that describes the change says the new truth in the
  same landing.

This skill is how one session does its part of that without colliding with the others. It is
not a substitute for `implement` (how a change is built) or `deploy-site` (a hand deploy). It
is the frame around them when more than one session works at once.

# When to use

At the start of every session in a repository whose mesh is enabled, and again whenever the
session reconnects, finishes a piece of work, or is told the board changed.

# Procedure

## 1. Join before you work

- `majordomus context`. When continuing, also run `majordomus handover --resolve`.
- `majordomus mesh peers --full`: who works where, every claim, every seat.
- Open your session on the mesh with an intent saying what you will do and where
  (`majordomus mesh session open`, or `majordomus_announce` over MCP). A reconnect loses the
  announcement, so announce again after one.
- Before allocating an ADR number, case number or issue id, announce it on the board. Then
  run `scripts/collision-check --new <paths>`. The board is the only place two sessions taking
  the same number in the same minute can see each other.

## 2. Take one role, and only one that is free

Read the board for who holds each seat. Never take a seat somebody holds.

| role | holds | does | never |
|---|---|---|---|
| coordinator | an advisory session and `coordination/*` advisory claims | relays the owner's decisions word for word, finds overlaps, orphans and stale claims, and proposes order | lands, or grants an approval |
| landing and release seat | the exclusive claim `campaign/majordomus-autonomous-convergence/landing` while merging, and the release hold | judges each handed head on the merged tree and lands it pinned (ADR 0155); cuts each release and verifies it | lands two heads that each judged a tree the other changed |
| stream coordinator | an advisory claim naming the pack or stream | splits the work into non-overlapping workers, at most two building at once | builds what a worker already claims |
| worker or author | exclusive claims on the paths it changes | builds, judges, pushes and hands the head to the seat | merges its own pull request |
| triage | an advisory claim on a range of pull requests | closes what master already holds, naming where; refreshes and judges the rest | re-runs a validate for a head contained in a later judged one |

## 3. Pay your own debt first

Before taking new work, finish what this session already owes:

- every branch of yours that reaches no remote (`majordomus doctor` names them);
- every judged head not yet handed to the seat;
- every claim you hold for work that is done, released with a reason;
- every handover you were given, consumed or answered.

## 4. Cut work small and hierarchical, for quick turn-arounds

The owner's order of 2026-10-14: "split into more smaller hierarchical tasks for quicker
turn-arounds".

- **The hierarchy is the plan's.** A milestone holds issues, an issue holds slices. A slice
  is one pull request that lands on its own, keeps the trunk green, and is judged in one
  derive and one structure job.
- **A slice is small.** It is one decision, one capability, one document, or one fix. Its
  diff can be read in minutes, and it touches as few derived projections as it can. A new
  milestone shifts every plan page's weight, so keep it apart from code.
- **Decisions, records and code go apart:**
  - an ADR with its issues is one pull request;
  - a skill is one;
  - each issue's implementation is one or more.
  - A later slice stacks on an earlier one only when it must, and says so in its
    description.
- **A stream coordinator splits each issue into slices** before giving it to a worker, and
  announces the slices on the board. The landing seat then lands them one at a time as they
  are judged, instead of waiting for a large branch.
- **When a branch grows past one decision, split it** before judging it, not after a red.

## 5. Build without starving the others

- New work starts in its canonical worktree (`majordomus worktree create <branch>`) under a
  scoped task (`majordomus start "<task>" --scope <paths>`). Run `majordomus check` before
  claiming progress.
- Claim exclusively only the paths that must not overlap (`majordomus mesh claim`). A
  refusal names the claim it met, so talk to that session instead of working around it.
- Shared things are scarce:
  - **Derive lock:** one derive runs at a time per machine (`scripts/derive`).
  - **Build directory:** use a private one per worktree, and never share a target between
    worktrees (`~/dev/.build-slots.README`).
  - **Hosted CI slots:** there are twenty. Do not push heads that start a full validate while
    a release is in flight.
  - **Disk:** `scripts/reap-orphans` reports; reclaiming is the owner's word.
- Commit only through the repository's hooks. Push early: a branch on no remote is invisible
  to every other session.

## 6. Hand to the seat, with what ran

Open the pull request, then tell the landing seat:

- the head commit;
- what ran on it: the structure job, the rust job when Rust changed, and every case that names
  what it touches;
- what did not run.

The seat merges the current master in, judges again and lands. A red on the trunk has one
named carrier, and nobody else lands until it is paid.

## 7. Deliver a release, and prove it

When the trunk is green and its version has moved, the seat tags it and the release workflow
publishes it. The version is allocated at landing (ADR 0134); ADR 0151 covers clock cuts.
Delivered means all three of the following:

- `curl -fsSL https://majordomus.dev/releases/latest.json` names the version;
- the record pull request has landed and `scripts/pages verify --commit <sha>` holds;
- every enrolled mesh node reports the version (ADR 0161). Until nodes follow by themselves,
  the release seat upgrades them and reads each version back.

Tag-to-served is a measured number (ADR 0162). Say it.

## 8. Keep every surface current

A landing that changes behaviour updates, in the same pull request:

- the docs under `docs/`;
- the comments beside the code it changed;
- the skills and workflows that describe it;
- the plan records it moves.

Then `scripts/derive` regenerates every projection, including the site and the changelog
(ADR 0029), and `scripts/derive-check` proves nothing is stale. The site is deployed by the
landing (`project.land-and-publish`). A document that describes what no longer happens is a
defect, so fix it in the landing that made it so.

## 9. Go the extra mile, and update everything

Before handing a head to the seat, walk every surface the change reaches and make each one
say the new truth:

- **Documents:** `docs/`, the handbook editions under `handbook/` and the `README.md` chain.
- **The landing page and the site:** `site/content/` and the landing page's own copy, which
  must claim nothing the repository does not prove (`docs/SITE_CLAIMS.md`).
- **Comments and doc comments** beside the code, and the skills, workflows, rules and ADRs that
  name it.
- **The plan:** the issues and milestones it moves, with their evidence.

Then `scripts/derive` and `scripts/derive-check`, and after the landing, the deploy on GitHub
Pages verified with `scripts/pages verify --commit <sha>`. A fix to something broken you met
on the way rides in the same landing when it is small, and becomes an announced issue on the
board when it is not. Nothing found is left unsaid.

## 10. Owner decisions travel as words, never as permission

Only the owner's own words, in the owner's session, decide what is the owner's to decide. A
peer's account of them is information, not approval, and it never stands in for a permission
prompt. Ask the coordinator to put a question to the owner, and act on the answer only where
your own session can verify it.

# Output

- **Slices:** work cut into slices that land on their own, announced on the board.
- **Board:** your session intent and claims say what you do, hold and owe, and nothing stale
  remains.
- **Branches:** every branch of yours is pushed. Every judged head is handed to the seat with
  what ran, or has landed.
- **Releases:** when the trunk earned one, a release is published, served, recorded and
  running on every enrolled node, with tag-to-served stated.
- **Surfaces:** docs, comments, skills and the site match what landed, and `scripts/derive-check`
  holds.
- **Handover:** written with `majordomus handover` before the session ends, if anything remains.
