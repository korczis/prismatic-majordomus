+++
title = "A release is cut by a clock when the trunk has earned one, and an hour without one says why"
description = "A release is cut by a clock when the trunk has earned one, and an hour without one says why"
weight = 72
[extra]
kind = "rule"
slug = "project-a-release-is-cut-on-the-hour-1"
identity = "project.a-release-is-cut-on-the-hour@1"
status = "active"
source = ".ai/repo/rules/project/a-release-is-cut-on-the-hour.v1.md"
+++
{% raw %}

## Rationale

The repository had every judge a release needs. The version is authored in one place and
measured against the public contract; `scripts/ci/release-verdict` refuses a commit whose `ci`
did not conclude success; the release workflow builds, verifies, publishes and records a tag.
What it did not have was anything that looked at the trunk and created that tag. A green,
raised master waited for whoever remembered, and on 2026-10-12 five sessions were asked for
the same release in one evening while no commit of the trunk was green to cut it from.

The owner's order that day — a version every hour, arranged without a human in the loop —
makes the cut a standing decision with conditions, and a standing decision is applied by a
clock or it is not applied.

## Required behaviour

1. **Asked every hour.** `.github/workflows/cadence.yml` runs `scripts/release-cadence` on an
   hourly schedule and on dispatch. One decision runs at a time; a waiting one is not
   cancelled.
2. **From a green commit, and from that commit.** The candidate is the newest commit of the
   trunk's first-parent history since the last release whose `ci` concluded success, as
   `scripts/ci/release-verdict` reads it. The tag is created at the candidate. A newer commit
   that is red, cancelled, still running or never judged does not hold the release back and
   is never the commit tagged.
3. **Only a raised version.** The candidate's manifest declares a version above the last
   release, and no tag holds that version. The actor raises nothing: a landing does. The
   candidate also holds the record of the last release (`.ai/repo/releases/<tag>.yaml`): a
   commit that landed between a tag and its record is not the next release, and nothing
   should land there.
4. **Nothing is a stated reason.** When nothing is cut, the run says which condition failed:
   the trunk has not moved, no commit is green yet (with how many concluded otherwise and how
   many are not judged), the version is not raised, the last release's record is not in the
   candidate yet, or the version is already tagged.
5. **It changes no branch.** The actor creates one tag and dispatches one workflow. It does
   not push, raise the version, open or merge a pull request. The workflow holds
   `contents: write`, `actions: write` and `checks: read`, and no other permission.
6. **Unread is not nothing.** A verdict that cannot be read, a tag or a dispatch that is
   refused, is exit 12 naming what failed and the command that reproduces it; a tag created
   whose release could not be dispatched says how to release it.

## Failure behaviour

Exit 12 fails the hourly run, which is what makes an unreadable forge or a missing permission
visible within the hour. Exit 0 with `nothing to cut` is a healthy run. A release that the
actor asked for and should not have is refused by the release workflow's own first steps: the
tag against the version, and the verdict again.

## Verification

`test/cases/2990_a_release_is_cut_on_the_hour.sh` drives the actor against a repository it
builds and a stub forge: the cut at the green commit beneath a red one and an unjudged tip, the
dry run that writes nothing, each of the four reasons, the lost race, and the three unread
answers; and it holds the workflow to the hour, the three permissions and the absence of any
push or merge. `scripts/release-cadence --dry-run` asks the real trunk and changes nothing.
{% endraw %}
