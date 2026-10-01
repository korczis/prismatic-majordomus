+++
title = "Every advisor transport is held to one contract — typed failures, deadlines, cancellation, normalised answers, reported usage, redacted secrets — proven against fakes with no network, model or credential"
description = "Whatever an advisor is — an API, a client tool, a local runtime, a mesh peer — talking to"
weight = 212
[extra]
claim_id = "advisor-transports-share-one-contract"
status = "guaranteed"
source = "docs/claims/advisor-transports-share-one-contract.md"
+++
{% raw %}

## What it means

Whatever an advisor is — an API, a client tool, a local runtime, a mesh peer — talking to
it ends in one of the same results: a normalised answer, or a typed failure (`timeout`,
`rate_limited`, `auth_failed`, `malformed`, `empty`, `unavailable`, `cancelled`, `error`)
with a diagnostic that carries no secret. Provider quirks stop at the adapter.

## How it works

`scripts/lib/advisors/contract.mjs` defines the failure type, the classification of HTTP
statuses and process exits, the prompt (evidence, the executor's hypothesis and prior
advice marked as opinions), the normalisation of an answer, and `consultOne`, which runs
an adapter under a deadline and a cancellation signal and turns any failure into a result.
`contract.test.mjs` runs every adapter against a fake fetch or a fake process.

## How to see it

```
node --test scripts/lib/advisors/contract.test.mjs
```

`test/cases/731_advisor_transport_contract.sh` runs the suite with no credential in the
environment.

## What it does not cover

It does not prove a live vendor behaves as the fakes do; that is learned at the first
consultation and recorded. It does not test a live mesh of two runtimes; the peer adapter
is tested against a fake server.

## Why it exists

An adapter that throws a vendor's own error into the workflow makes every caller learn
that vendor. One contract, tested without the vendor, is what keeps CI free of live models
(ADR 0098).
{% endraw %}
