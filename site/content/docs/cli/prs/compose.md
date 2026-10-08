+++
title = "majordomus prs compose"
description = "Compose a batch (ADR 0114): one pull request that carries several, each proved on its own head. A member is open, on the base, not a draft, unlabelled, in this repository, reviewed as the policy asks, with every required check passed on its head — which may be behind master — and every dependency landed or placed before it; everyone else is left out with the reason. A dry run by default, decided offline on the last recorded observation, like `status`. With `--apply` it takes the integration lease, observes the forge again, records the act on the trail first, and in a scratch worktree merges each member's head onto master in rank order, writes the manifest, derives, commits once, pushes a new branch `int/batch-<id>` and opens its pull request, which supersedes each member; it never merges into master. Between the merges and the derive it takes one `release bump` to what the public contract of the composed tree requires, and the manifest records the version before and after. Refused until the trail holds a verified merge, and while ADR 0114 is not accepted in the layer. Exit 10 on a refusal or an absent observation, 12 when it could not act"
weight = 91
[extra]
route = "/docs/cli/prs/compose/"
command = "majordomus prs compose"
source = "apps/majordomus-cli/src/cli.rs"
+++
