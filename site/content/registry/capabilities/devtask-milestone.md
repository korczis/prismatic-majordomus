+++
title = "devtask.milestone"
description = "Every issue of the milestone as a node with its readiness, its wave and what waits on it; every dependency edge with at least one end inside, the crossing ones marked; the issues partitioned into ready, blocked, waiting, active, review, completion-blocked, complete and cancelled; the critical blockers ordered by how much unfinished work each holds back; the startable work partitioned into subsets that may genuinely run at the same time — same wave, each parallel-safe, no two sharing a scope path — with each serialisation naming the path that caused it; every dependency cycle as its strongly connected component; and every finding about the milestone or its issues. A pure function of the canonical records: no git, no clock, no network, every list in canonical order, so two runs on two machines produce the same bytes. A work surface reading this derives nothing itself."
weight = 21
slug = "devtask-milestone"
[extra]
id = "devtask.milestone"
source = "apps/majordomus-cli/src/capability/builtin/devtask.rs"
+++
