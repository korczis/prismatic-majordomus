# The always-loaded instruction file has a line budget with a failing check

## What it means

Exactly one projection is marked `always_loaded: true` — the file every session of the worker reads before doing anything. The policy sets `context.always_loaded_budget_lines`; `update` refuses to write a projection over the budget, and `doctor` fails if the file on disk is over it. A budget without a failing check is a wish.

## How it works

`lib/update.sh` counts the rendered lines of the always-loaded target after generation; over budget, it writes nothing and exits 10. `lib/doctor.sh` re-checks the file on disk on every run, so an edit that pushes it over is caught even if someone forced it. The same file is also checked for unresolved references and hardcoded counts.

## How to see it

```bash
sed -i.bak 's/always_loaded_budget_lines: 150/always_loaded_budget_lines: 10/' .majordomus/policy.yaml
majordomus update
# FAIL budget      CLAUDE.md — would be 78 lines, budget 10; nothing written
```

## What it does not cover

The budget is in lines because that is what can be counted honestly without a tokenizer. It says nothing about token count and is never presented as one.

## Why it exists

One always-loaded operating contract in the source environment oscillated between empty and about eleven hundred lines across roughly two hundred hand edits before a hard budget with a failing check pinned it under fifty. The uncapped sibling file regrew several times over within weeks.
