+++
title = "Observing local runtimes reaches only declared or named endpoints on the loopback interface unless another host is admitted outright, scans nothing, and keeps the observation in checkout state that is never committed or published"
description = "The observer reaches the endpoints the catalogue declares, the value of each runtime's declared"
weight = 234
[extra]
claim_id = "models-observation-stays-local"
status = "guaranteed"
source = "docs/claims/models-observation-stays-local.md"
+++
{% raw %}

## What it means

The observer reaches the endpoints the catalogue declares, the value of each runtime's declared
variable, and endpoints named with `--endpoint`, and nothing else: no subnet, no port range. An
endpoint off the loopback interface is refused unless `--allow-remote` admits it, and the crate
refuses a report carrying one the observer did not admit. An endpoint carrying a credential is
refused by both. The observation is `.ai/local/state/inference/observation.json`, under the
ignored `.ai/local/`, so no machine's endpoints or models reach a commit or the site.

## How it works

`scripts/lib/inference/endpoint.mjs` plans the endpoints and reports every refusal with its
reason; `observed::locate` and `observed::admit` (`apps/majordomus-cli/src/models/observed.rs`)
re-check locality and credentials and mark each runtime `loopback` or `remote`, which
`--local-only` routing reads. Secret shapes in diagnostics are redacted before writing.

## How to see it

```
scripts/inference-observe --endpoint ollama=http://192.0.2.1:11434 --dry-run
majordomus models observed --format json
bash test/run.sh 3211_a_local_runtime_is_observed
```

## What it does not cover

Admitting a remote endpoint is the operator's act and trusts that endpoint's answers as far as
their evidence goes; mesh trust for another machine's runtimes is a later issue. It does not
authenticate to a runtime that requires a key.

## Why it exists

A port that answers is not a trusted runtime, and a discovery feature that scans or publishes
what it finds turns a convenience into an exposure. Declared reach and local storage keep the
feature useful on a laptop without making the laptop visible.
{% endraw %}
