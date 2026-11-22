---
schema: knowledge/v1
id: openttd-94-releases-v1-2-0
kind: knowledge
class: fact
title: "Owner decision 2026-11-22: the asking session (openttd-94, through its worker) takes the release seat for v1.2.0 only and releases it once validate on master is green"
description: "When a session asks who cuts the release of a version the landing seat will not tag, or which session held the release seat for v1.2.0"
status: verified
epistemics: decided
date: 2026-11-22
tags:
  - decision
  - release
  - owner-answer
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
    - file:docs/DISTRIBUTION.md
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md
---

# Owner decision 2026-11-22: the asking session (openttd-94, through its worker) takes the release seat for v1.2.0 only and releases it once validate on master is green

Pull request #3101 (I4665, typed coordination records in the mesh journal) landed on master
as d02c59d447c on 2026-11-22 at 04:23Z and declares version 1.2.0. The latest release was
then v1.1.0. The landing seat (session prismatic-majordomus-44) said that cutting a tag was
not covered by the owner's earlier answers, and would not run the release path itself. The
owner was asked who should release v1.2.0, and chose the asking session. That session,
openttd-94, carries it out through the worker that built I4665: it holds the exclusive mesh
claim `release/seat` for v1.2.0 alone, and follows the procedure of `docs/DISTRIBUTION.md`
("Releasing"). It tags only after `scripts/ci/release-verdict` sees master's `ci` succeed. It
tells the landing seat when the tag→record window opens and when it closes, and it lands
`release/record-v1.2.0`. The seat is not held for any later version by this answer.

# Evidence

- When: 2026-11-22, shortly after 04:30Z. The coordinator relayed the answer to the
  releasing worker at about 04:35Z.
- Asker: session 2845686e-0ccb-4c11-a008-3c46a644c866 (openttd-94), through AskUserQuestion.
- Question, verbatim: "prismatic-majordomus master is d02c59d447c (#3101, typed mesh records,
  declares 1.2.0); latest release is v1.1.0. The landing seat says cutting a tag is not covered
  by your earlier answers and won't run the release path itself; for 1.1.0 you told a session
  \"převezmi seaty a vydej\". Who should release v1.2.0 once validate on d02c59d is green?"
- Answer, verbatim: "This session releases it (Recommended)".
- Precedent named in the question: for v1.1.0 the owner told a session "převezmi seaty a
  vydej", that is, "take over the seats and release". v1.1.0's record landed as pull request
  #3091 (release/record-v1.1.0).
- The landing seat (prismatic-majordomus-44) replied at 04:33Z that nothing disagrees.
  `scripts/ci/release-path status` was clear, and no landing of theirs was in flight.
