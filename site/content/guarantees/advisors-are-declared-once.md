+++
title = "Advisors are declared once, in share/advisors.yaml, by reference to the provider table and the model catalogue; a new advisor reaches the command line, the policy, HTTP, OpenAPI, MCP and the Cockpit with no consumer edited"
description = "There is one advisor inventory. Every surface that lists advisors — the command line,"
weight = 216
[extra]
claim_id = "advisors-are-declared-once"
status = "guaranteed"
source = "docs/claims/advisors-are-declared-once.md"
+++
{% raw %}

## What it means

There is one advisor inventory. Every surface that lists advisors — the command line,
HTTP, MCP, the Cockpit, the environment banner, doctor — derives it from the catalogue,
and no reasoning code, adapter driver or page holds a list of its own.

## How it works

`share/advisors.yaml` declares each advisor's transport, adapter, executable and
capabilities, and references providers, vendors and models by id; `reasoning check`
refuses a reference that resolves to nothing, and a source file of the provider-independent
code that names any of them. All surfaces are projections of the `reasoning` module's
capabilities (ADR 0027).

## How to see it

```
majordomus reasoning advisors
curl $SERVER/api/v1/reasoning/advisors
```

`test/cases/735_a_new_advisor_needs_no_consumer_edit.sh` adds `example-reviewer` and finds
it on every surface without editing any of them.

## What it does not cover

A new *transport* needs one adapter module; a new advisor on an existing transport needs
none. The catalogue is the distribution's; a repository adding its own advisors is a later
extension of the share resolution, as for the model catalogue.

## Why it exists

Inventories maintained per consumer drift. Discover once, model once, derive everywhere
is this repository's answer to that, applied to advisors (ADR 0098).
{% endraw %}
