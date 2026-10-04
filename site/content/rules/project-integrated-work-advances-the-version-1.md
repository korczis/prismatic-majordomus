+++
title = "A merge into the trunk that carries work carries its version advance"
description = "A merge into the trunk that carries work carries its version advance"
weight = 106
[extra]
kind = "rule"
slug = "project-integrated-work-advances-the-version-1"
identity = "project.integrated-work-advances-the-version@1"
status = "active"
source = ".ai/repo/rules/project/integrated-work-advances-the-version.v1.md"
+++
{% raw %}

## Rationale

`project.the-version-is-measured` decides what a *release* owes the public contract, and it
says plainly that an unchanged surface owes no bump at all. That is right for compatibility
and it left most of the trunk's history without a version: fixes, tests, documents and
refactors behind the boundary landed by the dozen on one number, and nothing could tell from
the version which of them a running copy carried.

The owner's requirement is that completed work cannot accumulate behind a stale release
number. ADR 0106 answers it with a cadence the policy declares — a minor — owed by every
change set that carries work, over the version the trunk declares rather than the last
release. The contract's own requirement still applies, and the larger of the two is owed: a
breaking change on 1.x is still a major.

Measuring against the trunk is what makes concurrent work safe. Two branches that both
started from master at 1.30.0 each advance to 1.31.0, and the two edits are identical, so git
merges them without a conflict. Only a check against the first parent of the second merge —
which already declares 1.31.0 — can see that the second one advanced nothing.

## Required behaviour

**Measured, not counted.** The obligation is a predicate over the tree and the trunk; a
merge either declares the minimum or it does not. Nothing increments a counter, so a rerun
workflow, a retried finish or a second hook advances nothing.

**Classified by structure.** A path the trunk's `.gitattributes` marks `merge=derived` is a
projection, a file under `.ai/repo/releases/` is publication evidence, and the manifest and
lock differing only by what the version writer writes are the advance itself. Nothing is
classified by a commit message. A change set with no other path owes no cadence, which is
what lets the release pipeline land its record without raising the version it records.

**Written once.** `release advance` chooses the version and `release bump`'s write applies
it; nothing else writes the manifest's version line.

**Enforced without the automation.** The gate `version-obligation` is `always` planned and
runs `release obligation --base HEAD^1` in the structure job, on every pull request's merge
ref and on every push to master.

## Failure behaviour

The gate exits 10 when the obligation is owed or the tree is behind its trunk, naming the
declared version, the minimum and why each input requires what it does, with the remedy:
merge the trunk, run `majordomus release advance`, derive. It exits 12 when the trunk or a
version cannot be read, which is never a pass.

## Verification

`test/cases/802_a_merge_carries_its_version_advance.sh` runs the gate exactly as the CI model
declares it over merges built the way CI sees them: advanced work passes, a bypass and a
reused version are refused, a release record and a projection refresh pass.
`test/cases/801_completed_work_advances_the_version.sh` proves the finish half;
`apps/majordomus-cli/tests/release_obligation.rs` and the unit tests of
`release::obligation` prove the decision, its classification and its idempotency.
{% endraw %}
