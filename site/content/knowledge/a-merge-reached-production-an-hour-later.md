+++
title = "A merge reached production an hour later, and the workflow that published it was green"
description = "A merge reached production an hour later, and the workflow that published it was green"
weight = 2
[extra]
kind = "knowledge"
slug = "a-merge-reached-production-an-hour-later"
identity = "a-merge-reached-production-an-hour-later"
status = "verified"
source = ".ai/repo/knowledge/curated/a-merge-reached-production-an-hour-later.md"
+++
{% raw %}

## A merge reached production an hour later

### What was observed

2026-10-13. Two pull requests merged at 20:21Z and 20:41Z. At 20:54Z production still
served the commit before both, and the page one of them added answered 404. GitHub's status
page said every system was operational. The owner asked what was slow.

### Where the time went

Read from the jobs and steps of two finished runs of the pages workflow (45 and 61 minutes
from start to end) and from GitHub's own Pages workflow:

<div class="overflow-x-auto" tabindex="0">

| where | measured | cause |
|---|---|---|
| GitHub's own "pages build and deployment" | its deploy job, 19 s of work, waited 47 minutes for a runner | Pages is published from the `gh-pages` branch, so after our push GitHub starts a workflow of three jobs in the same runner pool |
| the step that waits for the site to serve the commit | 602 s in both runs, its whole limit | the publication above did not arrive in ten minutes; the step may fail without failing the job |
| jobs waiting for a runner | 78 to 876 s; 20.5 minutes in a third run | several validation runs of branches, about nineteen jobs each, in one pool |
| reading the UI for its published copy | 749 and 974 s, the Cockpit 127 and 164 s | every screen is read again on every deployment |
| the executable and the checkout | 106 to 126 s and 85 to 103 s; after a version bump the toolchain step was still running at eleven minutes | not yet explained |
| two landings in a row | the second run waits for the whole first | runs on one ref are serial |

</div>


The largest part was not a build. A site that was built, checked and pushed waited
three quarters of an hour for nineteen seconds of somebody else's job.

### Why nothing was red

The step that waits for production is allowed to fail, on purpose: the wait is over a
queue this repository does not control. So the job that deploys ended green in both runs
while production served the previous commit, and the gate that reads the live site runs
in the next validation, which stood in the same queue. A lander who read the workflow's
colour was told the opposite of what a visitor saw.

### What removes each cause

- I2660: the job that built the site publishes it as an artifact, so no second workflow
  is queued, and one publisher writes everything the site serves.
- I2661: a publication that did not arrive is a finding in the run's summary, in its
  timing row and in the deployment record, not a green step.
- I2662: validation declares how much of the pool it takes and cancels its own superseded
  runs; a deployment and a release record never wait behind a branch's matrix.
- I2663: the UI's published copy is reused when what it reads did not change, and the
  executable it needs is not rebuilt because a version line moved.

### What would have caught it earlier

- Measuring the thing the owner means by "deployed": the time from a merge to the site
  naming that commit, not the workflow's duration. The second number was recorded all
  along; the first ended in a step nobody read.
- Reading "may fail" in a workflow as a place where a report is owed. A check that is not
  allowed to stop anything still has to say what it found where the next person looks.
- Counting queues. A deployment that crosses two workflows crosses two queues, and the
  second one was not ours to see.
{% endraw %}
