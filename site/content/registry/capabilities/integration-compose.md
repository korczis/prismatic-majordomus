+++
title = "integration.compose"
description = "What `majordomus prs compose` would compose, decided offline from the recorded observation (ADR 0114): the base and the master commit every member was decided against, the members in composition order — each an open pull request on the base that is not a draft, carries no holding label, has its head in this repository, satisfies the review policy and has every required check passed on its own head, which may be behind master, with every declared dependency landed or placed before it, taken in rank order up to the policy's `integration.batch.max_members`, which `max` may lower and never raise — each with its head and title, and every other open pull request with the one typed reason it is left out; then what would be done (`would_compose`, or the refusal: fewer than two eligible is not a batch), the queue's diagnostics, and whether the rollout record or the status of ADR 0114 in the layer would refuse `--apply`. With no `max` and no policy key there is no plan, and the reason names the key. A read: it merges, pushes and records nothing — `majordomus prs compose --apply` is the act, under the integration lease. `observed: false` with the reason when this checkout has recorded no observation."
weight = 75
slug = "integration-compose"
[extra]
id = "integration.compose"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
