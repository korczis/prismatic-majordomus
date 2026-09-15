---
id: project.a-brief-is-kept-as-received
version: 1
kind: rule
title: A campaign brief this repository keeps is the one it was handed
description: The prompt packs under campaigns/ are provenance — what an operator asked for, on what date, in what order — and nothing reads them at run time. That is why they rot silently: a pack edited to agree with what the repository later did still reads like the brief that drove the work, and a half-copied pack still reads like a pack. Every file under campaigns/ is recorded in campaigns/MANIFEST.sha256 with its bytes, every pack has a row in the index, and every row names a pack the tree holds.
statement: Keep a campaign brief as it was received, record its bytes in the manifest and its pack in the index, and let the repository rather than the brief say what came of it.
status: active
class: blocking
depends_on: [project.no-claim-without-test@1]
tags: [provenance, campaigns, evidence, records]

x-majordomus:
  tests: [scripts/ci/campaigns-check, test/cases/340_a_brief_is_kept_as_received.sh]
---

# Rationale

A brief is the only record of what was asked for. The repository records what it decided —
in its rules, its decisions and its tests — and for a long time it recorded nothing of what
it was told to do. Every pack that drove a campaign here lived in one person's `~/Downloads`
or under the repository's own ignored `tmp/`, which is to say it survived on one disk, in a
directory git was instructed not to see.

That is a gap of a particular kind. The repository can prove what it does; it could not show
why any of it was started. A reader who wants to know whether a subsystem answers what was
actually asked has nothing to compare it against, and the person best placed to notice a
half-answered brief is the one who can no longer find the brief.

Keeping the packs fixes that only if they are kept honestly, and there are exactly two ways
a provenance directory stops being provenance:

- **A brief is edited.** Not maliciously: a pack is tidied to match what the repository later
  did, and the record of a wrong turn becomes a record of the right one. What made the history
  worth keeping was that it disagreed with the outcome.
- **A pack is incomplete.** A copy interrupted, a file added afterwards, a phase quietly
  dropped. It still looks like a pack, and nothing about reading it says which half is missing.

`campaigns/MANIFEST.sha256` is what makes either checkable, and a manifest nothing verifies is
a file. `scripts/ci/campaigns-check` decides the set in both directions — every declared file
present with its bytes, and every present file declared — because a checker that walks only the
manifest passes a tree somebody added a file to, which is the one-sided shape this repository
has rediscovered more than once. `test/cases/340_a_brief_is_kept_as_received.sh` plants all of
it: an edited brief, an undeclared file, a lost file, an unlisted pack, an index that promises a
pack the tree does not hold, and a manifest that is missing altogether.

# What this rule does not say

It says nothing about what a pack contains, whether its phases were completed, or whether its
plan was any good. A pack is not evidence that what it asked for exists — the gates are. Where
a brief and the tree disagree, the tree is right, and the brief is the reason somebody once
thought otherwise.
