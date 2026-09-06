---
schema: area/v1
id: coordination
kind: area
title: Coordination and ownership
summary: 'Who holds what, who is doing what right now, and what happens where two workers meet.'
status: stable
weight: 20
tags: [coordination, ownership, parallelism]
---

# Coordination and ownership

Several workers, one repository. This area covers everything that goes wrong because
ownership is implicit: duplicated work, work undone by the next worker, tasks nobody holds,
and the collisions that isolation defers rather than prevents.

Two workers independently fixing one defect are not evidence of powerful parallelism. They
are evidence that ownership was never declared anywhere both could see.
