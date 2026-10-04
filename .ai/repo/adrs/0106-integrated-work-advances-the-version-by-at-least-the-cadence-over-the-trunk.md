---
schema: adr/v1
id: adr-0106
kind: adr
title: Integrated work advances the version by at least the cadence over the trunk, and the contract's requirement still wins when it is larger
status: proposed
date: 2026-10-04
tags:
  - release
  - versioning
  - lifecycle
  - integration
provenance:
  origin: authored
related:
  - rule:project.integrated-work-advances-the-version
  - rule:majordomus.version-obligation
  - rule:project.the-version-is-measured
  - rule:project.release-is-a-projection
  - rule:project.land-and-publish
  - rule:project.integration-follows-the-current-master
  - file:apps/majordomus-cli/src/release/obligation.rs
  - file:apps/majordomus-cli/src/release/version.rs
  - file:apps/majordomus-cli/src/release/reconcile.rs
  - file:apps/majordomus-cli/src/commands/release.rs
  - file:apps/majordomus-cli/src/integration/drain.rs
  - file:apps/majordomus-cli/src/integration/relation.rs
  - file:apps/majordomus-cli/src/served.rs
  - file:lib/finish.sh
  - file:share/events.yaml
  - file:.ai/repo/policy.yaml
  - file:.ai/repo/ci/gates.yaml
  - test:test/cases/801_completed_work_advances_the_version.sh
  - test:test/cases/802_a_merge_carries_its_version_advance.sh
  - test:test/cases/803_the_version_has_one_writer.sh
  - test:test/cases/872_an_episode_end_is_not_a_completion.sh
  - test:test/cases/873_concurrent_branches_never_share_a_version.sh
  - test:test/cases/874_the_release_pipeline_does_not_advance_itself.sh
  - test:test/cases/875_a_failed_deploy_is_retried_on_the_same_version.sh
  - test:test/cases/876_unblock_reconciles_the_version.sh
  - test:test/cases/877_a_refused_advance_leaves_the_version_as_it_was.sh
  - test:apps/majordomus-cli/tests/release_obligation.rs
---

# 106. Integrated work advances the version by at least the cadence over the trunk, and the contract's requirement still wins when it is larger

> **Status: proposed.** Acceptance is the maintainer's act. This record amends ADR 0051 in
> one respect — what a change set owes when the public contract owes nothing — and leaves its
> analysis, its policy modes and its writer exactly as they are. It amends ADR 0029's writer
> only by adding a second way of choosing the version the one writer writes.

## Context

The repository already measures what compatibility requires. ADR 0051 compares the public
capability surface of the tree with the one the last release published, and the smallest
version a release may carry follows from that — a capability gone is breaking whatever the
commit that removed it called itself. ADR 0085 authors the version once, in the crate
manifest, and projects it everywhere else. `release bump` is the one writer.

That machinery answers *what a release owes*. It is deliberately silent about everything
behind the public boundary: `Policy::required_of` maps an unchanged surface to no bump at
all, and `the-version-is-measured` states it as a rule. On this repository most work is
behind the boundary. Between two releases the trunk took dozens of merges — fixes, tests,
documents, the site, refactors — on one version, and the version a running copy reported
said nothing about which of them it carried.

Three facts made the gap structural rather than a matter of discipline:

- **No step owned the advance.** `finish` judged scope, verification, obligations, gates and
  the publication; nothing in it asked about the version. A provider's end hook writes a
  handover, never a completion. The `version.yml` actor proposes a bump only when the
  contract requires one.
- **Nothing compared a branch with the trunk.** `version-surface` compares the declared
  version with the last *release*; `release-check` only says it is not behind the newest
  record. Two pull requests that both bumped `0.12.0 → 0.13.0` each passed alone, and since
  the edits are identical git merged the second without a conflict: master said `0.13.0`
  after both, and nothing noticed that the second advanced nothing.
- **Machine commits were recognised by name.** A release record lands on a branch called
  `release/record-vX` with a subject the publish job chose; nothing reads either back, so
  any rule keyed on "every merge advances" would have raised the version on the commit that
  records it, forever.

## Decision

**1. A completion cadence, declared in the policy.** `release.cadence` (`none` by default,
`minor` here) is the least a change set that carries work advances the trunk's version by.
`release.trunk` names the trunk (`origin/master`).

**2. One obligation, the larger of two requirements.**

```text
  contract floor  = last release raised by what the contract requires   (ADR 0051, unchanged)
  cadence floor   = trunk version raised by the cadence, when the change set carries work
  minimum         = max(trunk version, contract floor, cadence floor)
  effective       = the bump from the trunk's version to the minimum
```

A breaking change on `1.x` still costs a major; an internal fix costs the cadence's minor.
Below `1.0.0` ADR 0051's policy already floors a breaking change at a minor, and the
obligation adds no second rule about `0.x`.

**3. The trunk, not the last release, is the cadence's baseline.** The work is integrated
into the trunk, and only a comparison with the version the trunk declares *now* can tell the
second of two concurrent advances from the first.

**4. What carries work is decided by structure.** Every path the tree changes against its
merge base with the trunk is classified: a path the trunk's `.gitattributes` marks
`merge=derived` is a projection; a `.yaml` under `.ai/repo/releases/` that the change set adds
is publication evidence, and what the record generator writes from it is its projection (see
the 2026-10-04 amendments);
the manifest and the lock are a version advance when they differ from the base only by what
the writer writes (`version::rewrite_manifest`, `version::rewrite_lock`). Anything else is
work. Only work owes the cadence, so the pipeline's own follow-ups — a projection refresh, a
release record with the metadata it publishes, an advance alone — terminate.

**5. A predicate, not an event.** The obligation's verdict is `satisfied`, `not-owed`,
`owed`, `behind` or `unverified`, decided from the tree and the trunk. Its identity is the
subject and the trunk version it advances from (`feature/x@0.12.0`): stable across retries,
new when the trunk moves. Nothing increments anything, so a retried `finish`, a rerun
workflow or a provider hook that fires twice finds it satisfied and writes nothing.

**6. One writer, two choosers.** `release advance` computes the obligation and writes its
minimum with the same write `release bump` makes (`write_and_verify`: the manifest's version
line, the lock's record, both read back). `release bump`'s overrides are floored at the
obligation's minimum as they already were at the contract's. Neither is on a machine
surface: both are repository mutations.

**7. `finish` applies it; a gate enforces it.**

- `finish --outcome completed`, when the policy selects `version_advanced`, judges the whole
  contract first: the doctrine `majordomus.version-obligation` accepts an obligation that is
  satisfied, not owed, or owed and payable, and refuses `behind` and `unverified`. Only when
  every line holds does it run `release advance`, project `share/version.txt` with `generate
  distribution`, read the obligation again — it must hold — and record `release.advanced`
  before `task.finished`. A refused completion advances nothing and records no advance.
  Other outcomes owe nothing; `check` reports and refuses nothing.
- The gate `version-obligation` is `always` planned in the structure job and runs
  `release obligation --base HEAD^1` over the pull request's merge ref and over every merge
  pushed to master. It holds whatever path the merge took.

**8a. Where it is enforced, and where it is only detected.** master's protection does not
require a pull request to be up to date with master (`strict: false`, an owner setting this
record does not change), so two pull requests that each satisfy the obligation against the
same trunk can both turn green. The enforcement points are therefore two: the gate on the
pull request's merge ref, and the integration executor's refresh before it merges (ADR 0101
already requires a pull request to contain the current master, and its refresh re-runs
`release advance`). The run on the push to master is detection: if a merge slipped past both,
master is red and the refusal names the remedy. Requiring `strict` would make the merge-ref
gate sufficient on its own, and is recommended to the owner rather than assumed here.

**8b. The version line merges by itself.** Every branch that carries work writes the version
line, so two branches advanced from different trunks would conflict on it at every refresh.
A version-neutral merge driver (`merge=version` on the manifest and the lock, `release
merge-version`) rewrites the base and both sides to the larger of the two versions with the
writer's own pure rewrite, merges the rest of the file normally, and leaves the advance to be
recomputed against the trunk — so the version line never conflicts and a dependency edit
still does. It is a precondition of landing this decision.

**8. Read-only everywhere else.** `release.obligation` is a capability:
`GET /api/v1/release/obligation`, the MCP tool `majordomus_release_obligation`, and
`majordomus release obligation` renders it.

**9. The migration boundary is this record's merge.** The gate judges a merge against its
own first parent, so history before it is never judged and nothing is rewritten or recorded
retroactively. The first governed merge is the one that lands this decision, and it carries
its own advance.

## What does not change

- **A release is still evidence of a publication.** Advancing the declared version writes
  no release record, creates no tag and publishes no binary. Tags are still a person's act
  (`docs/DISTRIBUTION.md`), records are still written by the publish job after publication,
  and the changelog still composes one section per record — so many minor advances between
  two releases appear in the unreleased section, then in the release that ships them.
- **Deployment is still the merge's.** The site deploys from master on every push that owes
  a publication (`project.land-and-publish`), its `build.json` names the commit and the
  declared version it was built from, and `finish` still runs the at-finish publication gate.
- **ADR 0051's analysis, policy and gate** — `version-surface` still refuses a version below
  the contract's floor against the last release.

## Alternatives rejected

**Bump on every finish, unconditionally.** One piece of work is seen at several boundaries —
a finish, a retry, a merge, a hook — and a counter at any of them advances once per boundary.
A predicate over the tree and the trunk cannot double-count.

**Bump in a hook or a workflow.** A provider hook observes an episode boundary, not accepted
work; a workflow bump after the merge leaves master carrying completed work on the old
version until a second pull request lands, and that pull request is itself a merge.

**Exclude machine commits by subject or branch name.** A label is a claim; the structure of
the change set is a fact the gate can measure.

**A global counter or a lock file committed to git.** The trunk's own manifest is the value
both sides compare against; the gate's first-parent comparison is the compare-and-swap.

**Replace the contract analysis with the cadence.** A minor never hides a required major.

## Consequences

- A merge that carries work and does not advance is refused on its pull request and, if it
  lands anyway, on the push to master.
- Two branches racing from one trunk: the second is refused until it merges the trunk and
  runs `release advance`, which advances from the version that won.
- `release obligation` explains any version change: the subject, the trunk and its version,
  the contract's requirement since the last release, the cadence's, and the paths that made
  the change set work.
- The integration path that refreshes a branch with master (`scripts/unblock`, the
  integration drain) re-runs `release advance` after the merge; `release obligation` treats a
  merge of the trunk still in progress as containing it, so the refresh is one commit.
- **The cost, stated.** Below 1.0 a minor per merge spends minors quickly — a queue of five
  integrations between two release cuts moves the declared version five minors past the
  released one, and the site's released pill and declared footer differ by that much. The
  owner's mandate sets the minor cadence; `release.cadence: patch` is the policy's other
  setting, and the obligation is computed the same way for either.

## 2026-10-04 amendments

An audit of the first implementation found four defects. Each is fixed in place, with a test
seen failing before its fix; the decision above is otherwise unchanged and stays proposed.

1. **The release record no longer owes its own advance.** §4 read derived paths only from the
   trunk's `.gitattributes`, but a release record lands with the public metadata the record
   generator writes from it (`site/static/releases/v<V>.json`, the `latest.json` pointer) *and*
   the `merge=derived` line declaring the new file, in one change set — so the trunk could not
   mark the JSON, it was work, and the record PR owed a minor (master's #748, e08aec8651). The
   classification now takes the record's projections from what the generator declares
   (`distribution::release::{PUBLIC_DIR, LATEST}`, `obligation::record_projections`): for every
   record the change set *adds*, those files are projections, and so is a `.gitattributes`
   whose only attribute change is adding `<one of them> merge=derived` lines (comments are not
   attributes). Release evidence is restricted to adding a record: editing or deleting a
   published record is work, so a hand edit cannot ride the exemption. Trunk attributes git
   cannot read (`derived_paths` failing) leave the change set unclassified and the verdict
   `unverified`, never an empty set that would make every projection work.
2. **The writer is atomic.** `version::write` refuses a file it may not write before anything
   moves, stages both texts beside their files and renames them into place; when a later
   rename fails, every file already replaced is put back to its previous bytes. A manifest
   raised over a lock still stating the old version is no longer left behind by an error.
   `write_and_verify` still reads both back.
3. **A refused finish leaves nothing advanced.** Before it advances, `finish` copies aside
   every file the advance can change — the manifest, the lock, and each file
   `generate distribution` writes, which it names by generating into a scratch directory
   first — and when `release advance`, `generate distribution` or the re-read of the
   obligation fails, puts each back byte for byte (or removes it, if it did not exist) and
   refuses with `task.refused`. A projection that cannot be generated is a refusal, not a
   note: the contract requires the version the shell tool reads to be the one declared. The
   contract recorded with the refusal is edited with `jq`, not `sed`.
4. **The event names what it paid; the obligation stays a predicate.** §5 is kept: retries are
   idempotent because nothing counts. What a ledger reader could not answer was *which line
   paid an advance*, so `release.advanced` now carries `event_id`
   `release.advanced/<subject>@<trunk commit>/<task id>` — the subject, the full trunk commit
   the obligation was computed against, and the task that paid it — declared required in
   `share/events.yaml`. Giving the obligation itself a counter or a sequence was rejected: it
   would reintroduce exactly the per-boundary double count §5 exists to rule out, and the
   event's identity is derivable from facts the predicate already reads.

## Verification

`release::obligation`'s unit tests decide the matrix (contract × cadence → effective), the
arithmetic across `0.x`, `1.x` and multi-digit versions, the concurrent and the major-first
orders, the termination of the pipeline's follow-ups, and the classification of a version
advance. `apps/majordomus-cli/tests/release_obligation.rs` runs the command line against git
repositories with a trunk. `test/cases/801_completed_work_advances_the_version.sh` proves the
finish half, and `test/cases/802_a_merge_carries_its_version_advance.sh` runs the gate exactly
as the CI model declares it. `test/cases/803_the_version_has_one_writer.sh` holds the writer to
one caller, refuses a hook, script or workflow that rewrites the version itself, and holds the
merge driver to the writer's pure rewrites. `test/cases/874_the_release_pipeline_does_not_advance_itself.sh`
replays #748's record change set and an edited record, and
`test/cases/877_a_refused_advance_leaves_the_version_as_it_was.sh` proves that a finish refused
after its advance restores every file it changed. The
changelog orders releases that share a moment by version, as numbers, so many small releases
compose deterministically (`release::changelog`'s
`many_releases_on_one_day_are_ordered_by_their_version_as_numbers`).
