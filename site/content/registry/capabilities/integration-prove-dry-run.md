+++
title = "integration.prove_dry_run"
description = "Runs the executor's non-mutating cycle — refresh, plan, drain --dry-run and cleanup without --apply — between two snapshots of everything it could move if it were wrong: every ref origin serves, every open pull request's number, head, state and labels, the integration audit trail, the executor's lease, and every local ref outside the two namespaces the refresh mirrors. `ok` is true exactly when the snapshots are equal and refs/remotes/origin/<base> and every refs/majordomus/prs/<n> equal what origin serves. A read that reaches the network: the refresh asks the forge through the GitHub CLI and fetches the base and the pull-request heads, and like every read it rewrites the observation, relation and summary caches. It merges, closes and pushes nothing, and takes no input that could make it."
weight = 67
slug = "integration-prove-dry-run"
[extra]
id = "integration.prove_dry_run"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
