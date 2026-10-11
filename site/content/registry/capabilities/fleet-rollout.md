+++
title = "fleet.rollout"
description = "On every machine named (every one by default), in parallel: reach it (this machine directly, the others by the first ssh destination that answers, never prompting); install the release with the published installer, which verifies the archive before touching anything, unless that version, or a newer one when none was named, is already there; for a hub, fast-forward its checkout to its remote's default branch (cloning it when absent, declining a dirty, diverged or off-branch one and leaving it as it is), write its service — a systemd user unit on Linux, a launchd agent on macOS — and restart it when anything changed or it does not answer at the version, then wait for it to answer at the version with its mesh active. Last, asks the converged hubs which nodes they see until every hub is seen by every other. Each machine's steps and verdict are reported; one machine's failure stops only that machine."
weight = 67
slug = "fleet-rollout"
[extra]
id = "fleet.rollout"
source = "apps/majordomus-cli/src/capability/builtin/fleet.rs"
+++
