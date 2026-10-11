+++
title = "mesh-journal-converges — The cooperation journal is durable, converges under any delivery order, and never lets a stale claim win"
description = "An event the runtime accepted survives a crash or is reported lost; replicas with the same authenticated event set produce the same digest after any delivery order; a claim that expired never returns to beat a claim admitted while it was gone, and every claim conflict is announced; every page of the journal returns every event once."
weight = 29
template = "milestone.html"
[extra]
plan_id = "mesh-journal-converges"
source = ".ai/repo/project/milestones/mesh-journal-converges.yaml"
+++
