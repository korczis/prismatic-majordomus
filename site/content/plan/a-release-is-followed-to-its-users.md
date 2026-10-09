+++
title = "a-release-is-followed-to-its-users — Every surface can tell which stage a release is at, from the bump to the installer that serves it, and a release in flight is not reported as a lost one"
description = "One read capability says, for the declared version and the newest tag, which stage each release is at: declared, verdict, tagged, run, published, record proposed, record on master, served, smoke. Each stage carries pass, pending, fail or unknown, a reason and the next command. release-check tells a record in flight from a lost one. An agent releases by following a workflow that reads that capability."
weight = 49
template = "milestone.html"
[extra]
plan_id = "a-release-is-followed-to-its-users"
source = ".ai/repo/project/milestones/a-release-is-followed-to-its-users.yaml"
+++
