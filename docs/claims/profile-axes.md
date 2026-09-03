# Capability class, reasoning effort, output verbosity, context and verification are five independent axes

## What it means

A profile sets five things separately: which capability class of model the task deserves, how much reasoning effort, how verbose the output should be, which context to load, and what verification is required — plus a checkpoint interval and an output contract. None of them implies another. A debugging task can want high effort and concise output; a deep-work task detailed output and a decision record.

## How it works

Each axis is a distinct key in `share/skeleton/profiles/<name>.yaml`, validated against the allowlist. `majordomus update` renders the profile table into every generated instruction file, so the worker reads the axes for the profile its task names. `check --explain` prints the effective values for the active task.

## How to see it

```bash
majordomus check --explain | sed -n '/# profile/,/# policy/p'
```

## What it does not cover

**This claim is advisory.** The axes are validated as configuration and projected into the worker's instructions; whether a worker honours them is not observable from outside the worker, and Majordomus never selects or invokes a model. Nothing here measures what a session actually used.

## Why it exists

The routing document in the source environment treated "a model with reasoning" as a separate model, had no notion of verbosity at all, and routed by the worker's job title. Collapsing the axes into one "power" setting is how every task ends up on maximum.
