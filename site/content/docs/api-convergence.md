+++
title = "HTTP API: convergence"
description = "Whether every unit of work in this repository is somewhere another worker could find it. A holding is anything that can hold work — a work tree with uncommitted files, a branch with commits, a stash — and each carries a disposition read from git: integrated (the trunk reaches it), published (a remote-tracking ref reaches it), local_only (committed on this disk and nowhere else) or uncommitted. The last two are at risk: they are invisible to every other worker, and a worker that stops is not a rollback. Measured offline, from this checkout alone; whether a pull request exists for a branch is a question for the forge and is not asked here."
weight = 1005
path = "docs/api/convergence"
template = "api-tag.html"
[extra]
api_tag = "convergence"
api_slug = "convergence"
source = "docs/generated/openapi.json"
+++
