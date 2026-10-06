+++
title = "A session will load the intents its task serves when it starts"
description = "**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered."
weight = 117
[extra]
claim_id = "intent-in-session-context"
status = "planned"
source = "docs/claims/intent-in-session-context.md"
+++
{% raw %}

## What it means

**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered.

The intended shape is that `majordomus context` puts the intent a task serves — its statement, invariants and the criteria still unmet — in front of the worker when the session starts, beside the rules, handovers and decisions it already compiles.

## How it works today

`majordomus context` compiles rules, handovers, decisions and the plan, and no intent record is among them. A worker reaches an intent only by asking for it: `majordomus intent preflight --issue <id>` names it, and `intent show` prints it.

## How to see it

```bash
majordomus context                          # rules, handovers, decisions, the plan: no intent record
majordomus intent preflight --issue <id>    # the intent, only when asked for
```

## What it does not cover

Loading the intent would inform the worker. It would not refuse anything, and it would not decide satisfaction.

## Why it exists

The model argues that a worker should start from the desired state rather than from an edit. Until the session carries the intent, that depends on the worker remembering to ask.
{% endraw %}
