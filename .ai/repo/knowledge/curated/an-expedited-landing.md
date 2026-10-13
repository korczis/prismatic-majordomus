---
schema: knowledge/v1
id: an-expedited-landing
kind: knowledge
class: convention
title: How a pull request is landed past the required check, and what that never reaches
description: The lander's sequence for an expedited landing under the owner's standing order, the runs that are cancelled and the ones that never are, and why a tag is outside it.
status: verified
epistemics: decided
date: 2026-10-13
tags:
  - integration
  - deployment
  - ci
provenance:
  origin: authored
  derived_from:
    - file:.ai/repo/adrs/0155-an-expedited-landing-is-the-owners-word.md
    - file:.ai/repo/rules/project/an-expedited-landing-is-the-owners-word.v1.md
    - file:docs/INTEGRATION.md
relations:
  - type: documents
    target: file:.ai/repo/rules/project/an-expedited-landing-is-the-owners-word.v1.md
---

# An expedited landing

The owner ordered, on 2026-10-12 and again on 2026-10-13, that work be merged with `--admin`
and deployed as soon as it is judged locally, and that runs nobody will read be cancelled.
ADR 0155 records the words. This is the sequence that order is carried out by.

1. Read the board and the mesh. If another session holds the landing claim, wait for it.
2. Take the exclusive landing claim on the mesh.
3. Merge the current master into the branch and commit the merge. Then derive, and commit
   what the derive wrote. A derive taken before the merge commit is stale.
4. On that head run the whole structure job, the library tests, the rustdoc build if the
   crate changed, and the cases the change touches with skips refused. Write down the commit
   each ran on and what did not run.
5. Push. `gh pr merge <n> --admin --merge --match-head-commit <head>`.
6. Release the claim. Cancel the branch's own run. Tell the next lander the merged commit.
7. Confirm the commit production serves with `scripts/pages verify`.

A run is cancelled when its pull request is merged or closed, when its head is no longer its
branch's head, or when another run of the same workflow judges the same commit. The trunk's
only run of a commit, a deployment in flight, a release run and another session's run on a
current head are left alone.

A red on the trunk has one carrier, named by the integrator. Do not build a second fix.

A tag is outside all of this. It goes on a commit whose `ci` check concluded success, and
`scripts/ci/release-verdict` refuses anything else.
