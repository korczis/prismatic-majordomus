---
schema: context/v1
id: ai.repo.project.intents
kind: context
title: Intents
description: What must become true above the milestones that realise it, with the evidence that settles it.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/allow/intent.txt, share/schemas/majordomus/intent/intent.v1.schema.json]
---

# Intents

One YAML file per intent, its `id` equal to its file name. An intent is a statement about
reality: the `statement` that must become true, the `invariants` that must stay true, the
`milestones` that realise it, and the `satisfaction` criteria that settle it, each naming
its evidence — a `test`, a `claim`, a `command` or a `deployment` — by `ref`.

No status is stored and nothing transitions an intent. Its stage is derived on every read
from the status the plan derives for its milestones, and each criterion's state from the
evidence ledger; a `command` or `deployment` criterion resolves but is never counted as met
until it can be derived. The plan stays the only lifecycle of work (ADR 0070).

The keys are closed by `share/allow/intent.txt`.

```bash
majordomus-cli intent list                    # every intent, with its derived stage
majordomus-cli intent show <id>               # one intent, milestones and evidence
majordomus-cli intent validate                # exit 10 on a failure
majordomus-cli intent preflight --issue I0001 # which intent the work serves, or why none
```
