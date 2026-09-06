---
schema: context/v1
id: ai.repo.project.milestones
kind: context
title: Milestones
description: One outcome each, written as the problem it ends and the state that proves it ended.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/allow/milestone.txt]
---

# Milestones

One YAML file per milestone, its `id` equal to nothing else in the section. A milestone
is an outcome, not a bucket of work: it states the `problem` that exists today, the
`outcome` that ends it, the `current_state` it was written against and the
`desired_state` that will prove it reached. Issues reference it; it references no issue,
so a milestone never has to be edited when the work under it is re-planned.

The keys are closed by `share/allow/milestone.txt`. Prose fields are complete sentences,
because they are read by a worker who was not in the conversation that produced them.
Progress is derived from the issues that name the milestone and is stored nowhere here.
