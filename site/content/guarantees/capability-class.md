+++
title = "A profile names a capability class rather than a vendor model"
description = "Profiles say fast, standard, strong or strongest, never a model identifier. The generated instructions tell the worker to pick the closest model its environment offers. The canonical layer stays provider-neutral; mapping a class to a concrete model is left to whoever runs the worker, or to a future adapter."
weight = 51
[extra]
claim_id = "capability-class"
status = "advisory"
source = "docs/claims/capability-class.md"
+++
{% raw %}

## What it means

Profiles say `fast`, `standard`, `strong` or `strongest`, never a model identifier. The generated instructions tell the worker to pick the closest model its environment offers. The canonical layer stays provider-neutral; mapping a class to a concrete model is left to whoever runs the worker, or to a future adapter.

## How it works

`capability` is a validated field in the profile schema; the projection body carries the sentence "Capability classes are not vendor model names; pick the closest your environment offers". No file under `share/skeleton/` contains a model identifier.

## How to see it

```bash
grep -h '^capability:' .ai/repo/profiles/*.yaml
grep -rnE 'claude-|gpt-|gemini-' .ai/repo/   # nothing
```

## What it does not cover

**Advisory.** Majordomus does not choose or invoke a model. The class is guidance the worker reads.

## Why it exists

Dated model identifiers pinned in the source environment's configuration caused their own end-of-life migration; the routing doctrine that replaced them said "aliases only, never a dated ID". A class is one step more neutral than an alias.
{% endraw %}
