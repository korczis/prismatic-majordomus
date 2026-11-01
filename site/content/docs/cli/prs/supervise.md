+++
title = "majordomus prs supervise"
description = "Judge every in-flight CI run against the bounds `.ai/repo/ci/supervise.yaml` declares, and name each one to cancel with its reason: a pull request's first-attempt run waiting with no job created while a run of its branch and commit created within 120 seconds of it is in flight and is not itself to be ended within 30 minutes (never a re-run, an admitted run, nor when the pull request is labelled ci:full), a pull request's run of a head that moved while a newer run of it waits behind it in the same concurrency group, a pull request's run whose attempt queued past its workflow's bound with no job of it started, or one with a job in progress past its bound from that job's start (an integration/* top is never ended for a bound, nor a queued run of the default branch, a tag, the schedule, a dispatch or a release). A run of the default branch, a tag, the schedule, a dispatch or a release, and every pages or release run, is never cancelled for being superseded; the supervising workflow's own run never at all. It also reports supervise_stale when the supervisor's newest success (with `--supervised-by` and a window of 30 minutes or more, that lane's) is older than 30 minutes. A dry run by default, which reaches the forge to read and changes nothing. Exit 0 when nothing needs doing, 10 for decisions a dry run found, 12 when the forge could not be read or an act could not be taken"
weight = 96
[extra]
route = "/docs/cli/prs/supervise/"
command = "majordomus prs supervise"
source = "apps/majordomus-cli/src/cli.rs"
+++
