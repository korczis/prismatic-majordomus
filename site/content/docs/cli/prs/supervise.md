+++
title = "majordomus prs supervise"
description = "Judge every in-flight CI run against the bounds `.ai/repo/ci/supervise.yaml` declares, and name each one to cancel with its reason: a pull request's waiting run of a commit that already has a run in flight or judged (unless the pull request is labelled ci:full), a pull request's run of a head that moved while a newer run of it waits behind it in the same concurrency group, a run queued past its workflow's bound with no job started (never an integration/* top), or one running past its bound from its first job's start. A run of the default branch, a tag, the schedule, a dispatch or a release, and every pages or release run, is never cancelled for being superseded; the supervising workflow's own run never at all. It also reports supervise_stale when the supervisor's newest success is older than 30 minutes. A dry run by default, which reaches the forge to read and changes nothing. Exit 0 when nothing needs doing, 10 for decisions a dry run found, 12 when the forge could not be read or an act could not be taken"
weight = 96
[extra]
route = "/docs/cli/prs/supervise/"
command = "majordomus prs supervise"
source = "apps/majordomus-cli/src/cli.rs"
+++
