+++
title = "a-session-proceeds-through-the-mesh — A session proceeds through the mesh: it finds the work, claims it once, runs it watched and stops when the intent is met"
description = "A session asks /proceed what to do, receives one idempotent work order derived from intent_realization and the claim-aware plan, takes it under a mesh claim held by the run, starts it once, reports it and schedules its own next step until the intent is met; /watchdog wakes, hands over and reaps a stalled loop from deadlines; heavy work shares the machine through one slot."
weight = 104
template = "milestone.html"
[extra]
plan_id = "a-session-proceeds-through-the-mesh"
source = ".ai/repo/project/milestones/a-session-proceeds-through-the-mesh.yaml"
+++
