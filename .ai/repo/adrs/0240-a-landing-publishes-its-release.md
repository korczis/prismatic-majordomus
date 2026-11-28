---
schema: adr/v1
id: adr-0240
kind: adr
title: A green master commit that landed an issue publishes a dev pre-release, and one that completed a milestone publishes the minor, with no person tagging
status: proposed
date: 2026-11-28
tags:
  - release
  - versioning
  - automation
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0134-the-version-is-allocated-at-landing.md
  - file:.ai/repo/adrs/0051-the-minimum-release-version-is-measured-from-the-public-contract.md
  - file:.ai/repo/adrs/0029-the-changelog-is-a-projection-and-the-version-has-one-writer.md
  - file:.ai/repo/adrs/0085-the-version-is-authored-once-and-shipped-as-a-projection.md
  - file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
  - file:.ai/repo/ci/release-auto.yaml
  - file:.github/workflows/release-auto.yml
  - file:.github/workflows/release.yml
  - file:.github/workflows/version.yml
  - file:scripts/ci/release-autotag
  - file:scripts/release-stamp
  - file:docs/RELEASE.md
  - file:.ai/repo/project/issues/I4811.yaml
  - test:test/cases/4780_a_landing_publishes_its_release.sh
  - test:test/cases/4781_a_dev_tag_is_stamped_into_its_build.sh
---

# 240. A green master commit that landed an issue publishes a dev pre-release, and one that completed a milestone publishes the minor, with no person tagging

## Context

The owner's order of 2026-11-28, recorded with `majordomus decision add`: "again ages without a
version; why does automatic versioning not work; every issue must bump a dev version, every
milestone a minor, automagically; this is the third time."

Everything up to the tag was already automatic, and the tag was not. ADR 0051 measures the
version from the public contract, ADR 0029 and ADR 0085 give it one writer (`majordomus release
bump`), `version.yml` proposes a bump the contract requires, ADR 0134 allocates a version at
landing, and `release.yml` publishes whatever `v*` tag is pushed once its commit's `ci` is green.
ADR 0134's point 5 then stops: "Allocation is not publication ... Nothing in this decision tags or
publishes." `docs/RELEASE.md` called the tag "a decision" that automation must not take, and case
353 holds `version.yml` to never tagging. So `release.currency` reported landed work OVERDUE for
hours at a time, and 1.3.0 on 2026-11-28 went as every release had: a person tagged, a person
merged the record.

## Decision

This amends ADR 0134 point 5: allocation is still not publication, and publication is now
automatic. The decision whether a landing is released was the owner's to make, and the owner made
it once, for every landing; it is recorded in `.ai/repo/ci/release-auto.yaml` as `publication:
automatic`, read from the commit being judged. The switch lands `off` and is turned on by the
landing seat once the owner confirms that the workflow's GITHUB_TOKEN may push `v*` tags and a
first dry run over master is green.

1. **The trigger is a green master.** `.github/workflows/release-auto.yml` runs on `workflow_run`
   of `validate` when the run's event is `push`, its branch `master` and its conclusion
   `success`, and on an hourly schedule over master's tip as the backstop for a run that never
   came. `scripts/ci/release-autotag` reads the commit's `ci` check-run again through
   `scripts/ci/release-verdict` before it tags, and `release.yml` reads it a third time. A red
   commit is never tagged (decision `held`); an unreadable verdict refuses (exit 12).
2. **What landed is read from the records, by the one engine.** An issue *landed* at commit C
   when the plan engine (`bin/majordomus plan list`, `lib/project.awk`) derives it DONE over C's
   own `.ai/` tree and not over the tree of `newest`, the greatest release tag C contains. A
   milestone *completed* at C when `plan status` derives it DONE at C and not at `stable`, the
   greatest stable tag C contains. DONE carries everything the engine requires: the sealed
   completion stamp and the required evidence (and, for a milestone, its own evidence). No
   second derivation exists to disagree with it.
3. **One tag per commit.** A merge that lands several issues is one dev release whose
   annotation names every issue; per-issue tags would put several tags on one commit and
   publish the same bytes several times. A commit that already carries a `v*` tag is
   `already`; one that a tagged commit contains is `behind` and is never tagged after it.
4. **Which version.** `declared` is the crate manifest's version at C.
   - A completed milestone, with `declared` at least `stable`'s next minor: `v<declared>`,
     stable. That is `vX.(Y+1).0` when the tree declares the next minor, as the trains do.
   - A landed issue, with `declared` above `stable`: `v<declared>-dev.<N>`, a prerelease of the
     version the work leads to, published `--prerelease --latest=false` by the existing
     publish step.
   - `declared` not above `stable` (the tree declares what is already released), or below the
     minor a completed milestone owes: no tag can carry the work. `release-autotag` dispatches
     `version.yml` with `to=<stable's next minor>` unless `release/bump-<it>` is already open,
     and `version.yml` proposes it through `release bump --exact`. The work is released by the
     first green commit after that proposal lands, because the next decision diffs against the
     same last release.
5. **The dev number is derived from the tags.** `N` is one past the highest `v<declared>-dev.*`
   tag the remote holds (fetched first), so a retry never reuses a number and two landings
   never share one. The concurrency group `release-auto` runs one decision at a time; should
   two runs race anyway, the second push of the same tag name is refused by git, and that run
   exits 10 for the next one to recompute.
6. **A dev tag's version is stamped into its build, never committed.** `release.yml` required
   the tag to equal the declared version, and no master commit declares `X.Y.Z-dev.N`: every
   landing would owe a bump pull request of its own. `scripts/release-stamp --check` accepts a
   dev tag only as `v<declared>-dev.<N>` of a final declared version and is `release-version
   --check --tag` for every other tag. The build job then stamps its own checkout (the manifest's
   `[package] version`, the lock's own entry, `share/version.txt`, exactly the lines `release
   bump` writes) so the executable and the shell tool print the tag's version, and
   `release-verify` holds the archive to it. The authored version keeps one writer: the stamp
   writes a build that is thrown away with the job.
7. **The tag starts its release exactly once.** The tag is pushed with the job's token, and
   GitHub starts no workflow for a push that token makes, so `release-autotag` dispatches
   `release.yml --ref <tag> -f tag=<tag>` itself. A rerun over a tagged commit dispatches only
   when GitHub lists no release run of that tag and holds no release of it; a listing that
   cannot be read refuses rather than dispatching twice on a guess.
8. **A dev release is a pre-release on every surface.** `-dev.N` joins `-alpha.N`, `-beta.N` and
   `-rc.N` in the one pattern `scripts/site-deploy`, `scripts/ci/install-check`,
   `scripts/ci/release-check`, `scripts/release-probe` and `release.yml`'s smoke share, so
   `latest.json` never names one, and `release-check` orders alpha < beta < dev < rc below the
   final release.

## Alternatives rejected

- **A bump commit per dev release.** Every landing would owe a pull request and a full
  validate before its tag; the queue this order exists to shorten would double.
- **Tagging an unstamped dev tag and teaching `release.yml` to accept a mismatch.** The archive
  would print `1.4.0` for `v1.4.0-dev.3`, and `release-verify` exists to refuse exactly that.
- **One tag per issue.** Several tags on one commit publish identical bytes under several names.
- **A second status derivation in the script** (reading `completed_at` by hand). DONE gates on
  evidence and seals; a reader that skipped them would release work the plan calls VERIFY.
- **A Rust capability first.** The decision belongs beside `release.currency` eventually
  (the inventory records the migration); the owner asked for it now, and the script is the
  scripted capability ADR 0069 allows, with its exemption written down.

## Consequences

- `release.currency` reads current after every green landing that carried an issue, because the
  newest tag the trunk contains is the dev tag.
- Each dev release runs the whole pipeline: the build matrix, the record and its pull request
  (which lands through I4342's lane), the site metadata and the smoke. While a release run is
  in flight, `scripts/ci/release-path` makes pull-request validates other than a train's yield,
  so a landing now costs a release run of runner time; trains are exempt and are what lands.
- The release seat no longer cuts tags. It watches the decisions in the `release-auto` runs'
  summaries, lands the record pull requests and bump proposals until they land themselves, and
  sets `publication: off` in `.ai/repo/ci/release-auto.yaml` to stop it.
- `version.yml` still never tags (case 353); it gains a `to` input so the tagger can ask for
  the version a released tree must declare next.
- Case 4780 drives the decision end to end on a fixture with the real plan engine, a bare
  origin and a stub `gh`; case 4781 the stamp, the wiring and the pre-release pattern.

## Verification

- `bash test/run.sh 4780_a_landing_publishes_its_release 4781_a_dev_tag_is_stamped_into_its_build`
- `bash test/run.sh 353_the_version_bump_has_an_actor 87_release_pipeline 3021_a_tag_does_not_freeze_the_queue 3029_a_tag_in_flight_is_pending`
- `bin/majordomus shell check`
