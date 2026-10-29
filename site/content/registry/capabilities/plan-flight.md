+++
title = "plan.flight"
description = "The milestones with work in flight, the one the graph allows next, and the latest release as the releases themselves say it, read live on every call (ADR 0166, decision 4). Now is every open milestone holding an open issue with an open pull request that names it (`plan.delivery`'s `pr_open`), a live mesh claim made for it, or a declared ACTIVE status, each with those issues, why they are in flight, their declared status beside their inferred stage, and the objects that prove it (branch, pull request, merge commit, release tag, published `latest.json`, claim), linked on the forge where the forge has them. Next is the first open milestone in the plan's order that is not now and whose unfinished prerequisites are all now. The latest release is the highest stable version among the release records, the `v*` tags of this clone and the `latest.json` the last successful Pages deployment published, so a release whose record merged after its tag is not shown older than it is; `executable_tagged` says whether the answering executable's own version is tagged. `milestone` adds that milestone with every one of its issues. A source that was not read (the forge, the publication, the mesh outside a server) is named with its reason, never treated as empty."
weight = 165
slug = "plan-flight"
[extra]
id = "plan.flight"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
