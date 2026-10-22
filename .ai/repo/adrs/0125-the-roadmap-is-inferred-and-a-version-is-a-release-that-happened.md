---
schema: adr/v1
id: adr-0125
kind: adr
title: The roadmap is inferred from the plan and the releases, and a version is a release that happened
status: proposed
date: 2026-10-09
tags:
  - plan
  - roadmap
  - release
  - projections
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0029-the-changelog-is-a-projection-and-the-version-has-one-writer.md
  - file:.ai/repo/adrs/0085-the-version-is-authored-once-and-shipped-as-a-projection.md
  - file:.ai/repo/adrs/0070-intent-is-a-typed-record-and-satisfaction-is-derived-from-evidence.md
  - rule:project.derived-once
  - rule:project.the-version-is-measured
  - file:apps/majordomus-cli/src/roadmap.rs
  - file:apps/majordomus-cli/src/capability/builtin/plan.rs
  - file:site/templates/roadmap.html
  - file:docs/ROADMAP.md
  - test:apps/majordomus-cli/tests/roadmap.rs
---

# 125. The roadmap is inferred from the plan and the releases, and a version is a release that happened

## Context

The published roadmap listed eight milestones under versions `0.1` to `1.0`. The versions were a
`version:` field typed into each milestone record. `0.5` was typed twice. The tool had shipped
`0.18.0` by then, and none of those numbers had anything to do with it. The page showed only the
milestones that carried a version, so eighteen of twenty-six milestones, including every one
where work was happening, were missing from it. `docs/ROADMAP.md` said "the version is a field"
and "there is no list of versions anywhere". Both were true of the generator. The records were
the list.

ADR 0085 makes the crate manifest the one place a version is authored. A milestone version was
a second place, and a worse one, because it named a release that did not yet exist.

Three other defects followed from the same page:

- Its statuses were derived, but nothing compared them with what had shipped. `0.18.0` released
  commits naming I2000 and I2004, while both records still read `BLOCKED`.
- "Now" was the first unblocked milestone by the record's `order` field. It was a `PLANNED`
  milestone with no started issue, while seven milestones had active work.
- The graph's text rendering read its edges backwards: "foundation requires runtime-adapters".

## Decision

1. **A version is a published release record.** The roadmap gives no milestone a version before
   it ships. A milestone's `version` field stays readable, so an adopter's records do not start
   failing validation, but nothing reads it any more. This repository's records no longer carry
   one.

2. **The roadmap is the plan in four horizons.** Each horizon is a function of the derived
   status alone: now (`ACTIVE`, `VERIFY`), next (`PLANNED`), later (`BLOCKED`), done (`DONE`).
   Inside a horizon the order is rank, then priority, then how many milestones each one
   unblocks. `order` breaks only what is left.

3. **What shipped is attributed from committed facts.** An issue was completed in a release
   when its `completed_at` falls after the previous release and no later than that one. It was
   named by a release when a commit in that release's range names it, as the changelog already
   resolves. Neither moves when an unrelated commit lands.

4. **What is inferred is said as inference.**
   - A pace is the number of a milestone's issues completed over the last five published
     releases.
   - A forecast divides what remains by that pace. It is absent when nothing was completed,
     because nothing was measured.
   - A signal is two committed facts that disagree: a released commit names an issue whose
     record reads not started (`shipped_unrecorded`); every issue is done and the milestone's
     own evidence is missing (`awaits_evidence`); no issue names the milestone (`no_issues`);
     work started and nothing was completed or named in the window (`idle`).
   - A signal never changes a status. A record is transitioned by its worker, and a milestone
     is accepted by a person.

5. **One function, every surface.** `roadmap::infer` decides.
   - `plan.roadmap` answers with it as `inferred` over HTTP and MCP, including the work since
     the last release.
   - `majordomus generate` writes `site/data/registry/roadmap.json` from the published releases
     only, because the unreleased window would go stale at the next commit.
   - The site is rebuilt with every change to the trunk, so the page moves when the plan or the
     releases do.

6. **The graph's edges are named from their source.** "foundation unblocks runtime-adapters".
   An arrow still runs from the milestone that must be done to the one waiting on it.

## Consequences

- The page shows every live milestone, and each one says why it stands where it does.
- A record that lags the code becomes visible as a signal instead of being invisible on the
  page. The I2000 and I2004 records still have to be transitioned, by their worker.
- The forecast is coarse on purpose. A milestone whose issues are completed in bursts gets a
  forecast that moves in bursts. The page states the window and the counts it used, so a
  reader can judge it.
- `doctor`'s roadmap doctrine still holds a README version table to the milestone versions.
  Once no adopter declares a version, that check holds trivially. Retiring it is a separate
  change, because an adopter may still have such a table.

## Alternatives considered

- **Project a target version for each milestone from the pace.** Rejected. The project
  releases patches and minors on the measured contract (ADR 0051), so "the release after next"
  is not a version number anyone can name in advance. A count of releases at a stated pace is
  the honest form.
- **Read the open pull requests for "in flight".** Rejected for the committed copy, because the
  forge's state is not a committed fact. The served capability could add it later, the way it
  adds the unreleased window.
