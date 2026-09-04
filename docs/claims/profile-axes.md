# Capability class, reasoning effort, output verbosity, context and verification are five independent axes

## What it means

A profile sets five things separately: which capability class of model the task deserves, how much reasoning effort, how verbose the output should be, which context to load, and what verification is required — plus a checkpoint interval and an output contract. None of them implies another. A debugging task can want high effort and concise output; a deep-work task detailed output and a decision record.

## How it works

Each axis is a distinct key in `share/skeleton/profiles/<name>.yaml`, seeded into `.ai/repo/profiles/` by `init` and validated against the allowlist. The generated bootstraps name the default profile and point at the layer; the axes themselves are read from the profile file by `check --explain`, which prints the effective values for the active task, and by `context`, which lets the profile's context block decide what the briefing offers.

## How to see it

```bash
majordomus check --explain | sed -n '/# profile/,/# policy/p'
```

## What it does not cover

**This claim is advisory.** The axes are validated as configuration and projected into the worker's instructions; whether a worker honours them is not observable from outside the worker, and Majordomus never selects or invokes a model. Nothing here measures what a session actually used.

## Why it exists

The routing document in the source environment treated "a model with reasoning" as a separate model, had no notion of verbosity at all, and routed by the worker's job title. Collapsing the axes into one "power" setting is how every task ends up on maximum.
