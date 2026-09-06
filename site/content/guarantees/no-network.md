+++
title = "Nothing performs a network call, evaluates generated text, or deletes recursively"
description = "Majordomus is local. No command opens a network connection, no text that came from a worker, a model, a handover body or a policy file is ever passed to eval or a shell, and no command deletes recursively outside a temporary directory it created itself. Writes are confined to .ai/ and the projection targets the policy names."
weight = 38
[extra]
claim_id = "no-network"
status = "guaranteed"
source = "docs/claims/no-network.md"
+++
{% raw %}

## What it means

Majordomus is local. No command opens a network connection, no text that came from a worker, a model, a handover body or a policy file is ever passed to `eval` or a shell, and no command deletes recursively outside a temporary directory it created itself. Writes are confined to `.ai/` and the projection targets the policy names.

## How it works

There is no behavioural way to prove the absence of a network call, so this claim is backed by a source scan: `test/cases/08_no_forbidden_constructs.sh` reads `bin/majordomus`, every module in `lib/` and the provider templates and fails on `eval`, network clients (`curl`, `wget`, `nc`, `ssh`, `scp`), `/dev/tcp`, recursive deletion of the repository, of the `.ai/` layer or of the distribution, and any redirect into a path variable outside the allowed set. The scan is deliberately crude and deliberately in the test suite, so a change that introduces one of these constructs fails CI.

## How to see it

```bash
bash test/run.sh 08_no_forbidden_constructs
grep -rnE 'curl|wget|eval ' bin lib   # nothing
```

## What it does not cover

A source scan is not a sandbox. It proves the shipped code contains none of these constructs; it does not constrain what a hook you write yourself does.

## Why it exists

The security commitments — local only, no telemetry, no eval, no credential handling, confined writes — are only worth stating if something fails when they are broken. The scan is that something.
{% endraw %}
