---
schema: adr/v1
id: adr-0134
kind: adr
title: The next version is allocated at landing, by the integration executor, from the trunk's version and the change's measured impact under a typed policy floor
status: proposed
date: 2026-10-11
tags:
  - release
  - versioning
  - integration
provenance:
  origin: authored
related:
  - rule:project.the-version-is-measured
  - rule:project.release-is-a-projection
  - file:apps/majordomus-cli/src/release/compat.rs
  - file:apps/majordomus-cli/src/release/landing.rs
  - file:apps/majordomus-cli/src/integration/drain.rs
  - file:.ai/repo/adrs/0029-the-changelog-is-a-projection-and-the-version-has-one-writer.md
  - file:.ai/repo/adrs/0051-the-minimum-release-version-is-measured-from-the-public-contract.md
  - file:.ai/repo/adrs/0085-the-version-is-authored-once-and-shipped-as-a-projection.md
  - test:test/cases/1060_the_version_is_allocated_at_landing.sh
---

# 134. The next version is allocated at landing, by the integration executor, from the trunk's version and the change's measured impact under a typed policy floor

## Context

The version has one writer (ADR 0029, ADR 0085) and its minimum is measured from the public
contract against the last release (ADR 0051). Who writes it, and when, was left to whoever
noticed first: `version-surface` refuses the first pull request after a release that moves
the contract until that pull request raises the version itself.

On 2026-10-09 and 2026-10-10 that cost, measured in this repository's own work:

- the next number was negotiated between sessions by message (0.19.1 for one pull request,
  0.19.2 for another, 0.20.0 for "the first feature");
- every feature branch then conflicted with the trunk on the version line of
  `apps/majordomus-cli/Cargo.toml`, an authored path the integration executor's refresh
  refuses, so each was repaired by hand (#807, #821, #827, #829);
- a release in between (v0.20.0) made every carried bump stale, and each branch had to be
  bumped again to 0.21.0.

A pull request chose its version before it knew which master it would land on. Nothing
serialised the choice except people.

## Decision

1. **The version is allocated at landing.** When a repository's policy says
   `release.allocation: landing`, a pull request does not raise the version. The
   integration executor (ADR 0101) raises it in the refresh it already performs before a
   pull request's last CI run, after master is merged in and before the derive, so the
   head the required check judges and the head that lands carry the same version.
2. **The number is computed from the trunk, never from the branch.** The next version is
   `trunk_version.raised_to(selected)`, where `trunk_version` is what master declares at the
   moment of the refresh. A retry, a second refresh after master moved, or a recovery after
   a crash recomputes from the new trunk, so no integration can raise the version twice and
   two pull requests cannot hold the same number: the executor refreshes one pull request
   at a time under the integration lease, and a landing that moves master makes every other
   head `behind` again.
3. **What is selected is typed.** `selected = max(required_by_compatibility, floor)`:
   - `required_by_compatibility` is ADR 0051's measurement of this pull request's own
     movement of the public surface, master against the candidate, translated by the
     version's mode (below 1.0.0 a removal costs a minor, from 1.0.0 a major);
   - `floor` is `release.floor` in the policy: `none`, `patch`, `minor` or `major`, read
     through the version's mode as compatibility is, so below 1.0.0 a `major` floor asks a
     minor; a change that touches no path is no landing and owes no floor;
   - the changed paths are listed without rename detection, so a file moved into an exempt
     tree is also judged at the path it left;
   - a change that writes `.ai/repo/releases/v<trunk>.yaml`, the record of the release the
     trunk already declares, owes no floor: the record follows its tag and may change more
     than the record class covers (the v0.20.0 record also changed `.gitattributes` and
     `docs/INSTALL.md`), and raising it would carry it past the tag it records;
   - an unknown movement stays what ADR 0051 makes it, never a safe patch.
4. **An exemption lowers the floor and nothing else.** `release.exempt` declares classes,
   each with an id, a description and the paths it covers. A pull request is exempt when
   every path it changes is covered by one class: then the floor does not apply, and the
   compatibility requirement still does. A release record, the executor's own refresh and
   a change to plan records are the classes this repository declares. There is no
   per-pull-request switch.
5. **Allocation is not publication.** A raised version is an integrated fact; a release is
   published by a tag and verified by its record and smoke. Nothing in this decision tags
   or publishes.
6. **A repository that says nothing is unchanged.** `release.allocation` defaults to `off`
   and `release.floor` to `none`: the gate and the bump work exactly as before.

## Consequences

- `majordomus release next` (capability `release.next`) answers, read-only, what the
  executor would allocate for a head against a base, and why: the trunk version, the
  measured movement, the floor, the exemption if any, and the next version.
- This repository sets `allocation: landing` and `floor: minor`: every eligible
  integration advances at least a minor (the owner's request of 2026-10-10).
- The executor's refresh must resolve a conflict on the version line to the trunk's before
  it allocates, because the branch's own number is no longer an author's statement.
- Consistency model, stated plainly: GitHub does not merge and publish atomically. What is
  guaranteed is that each landed integration carries exactly one allocation computed from
  the master it landed on, because the head that lands is the head the executor refreshed
  and CI judged (`--match-head-commit`), and the executor is the only writer of the
  allocation.

## Delivery

- Slice A (this record): the typed policy block, the decision engine, `release.next`.
- Slice B: allocation inside the executor's refresh, version-line conflict resolution, the
  allocation trailer, and the tests of retries, crashes and three concurrent candidates.
- Slice C: `version-surface` on a pull request under `allocation: landing` refuses a hand
  bump instead of demanding one; `version.yml` stops opening bump pull requests.
- Slice D: the release lifecycle as one typed state machine over the existing workflow.
- Slice E: the manifest path from the policy, so an installed repository uses the same
  engine.
