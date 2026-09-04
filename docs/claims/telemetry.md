# Execution telemetry will be recorded once a provider exposes it honestly

## What it means

**Planned for v0.3.** Per task: input, output and cached tokens, the model and effort actually used, elapsed time and tool-call count — recorded into the ledger next to what it already holds. Only from providers that expose these numbers; nothing estimated is ever labelled as measured.

## How it works

Nothing is implemented. The ledger already records per task the session count, handovers, workers, verification command with exit code and duration, and outcome; the provider-side numbers are the missing half. Any `estimated_` field, when it appears, is excluded from enforcement and from comparisons.

## How to see it

```bash
jq -c 'select(.event=="task.finished") | {task_id, outcome, verify}' .majordomus/state/ledger.jsonl
```

## What it does not cover

Everything about tokens and cost, today. The economics document states what can be measured now from the ledger alone and what needs this.

## Why it exists

The team the tool was distilled from built a competent cost profiler for its product's own model calls while having zero telemetry on its coding workers; its routing documents carried "35 % better" figures with no benchmark behind them. This page is the commitment not to repeat that.
