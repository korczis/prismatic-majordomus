+++
title = "Checkout-local state lives under .ai/local/, is ignored by git, and is absent on a fresh clone or worktree"
description = "The task record, the ledger, decisions, open questions, checkpoints, handovers and closed session envelopes belong to the checkout that wrote them. They live under .ai/local/state/, init writes the one .ai/local/ line into .gitignore, and nothing Majordomus does stages or commits them. A fresh clone or a new worktree starts with no local state at all, and that absence is reported as absence rather than mistaken for a record."
weight = 69
[extra]
claim_id = "local-state-ignored"
status = "guaranteed"
source = "docs/claims/local-state-ignored.md"
+++
{% raw %}

## What it means

The task record, the ledger, decisions, open questions, checkpoints, handovers and closed session envelopes belong to the checkout that wrote them. They live under `.ai/local/state/`, `init` writes the one `.ai/local/` line into `.gitignore`, and nothing Majordomus does stages or commits them. A fresh clone or a new worktree starts with no local state at all, and that absence is reported as absence rather than mistaken for a record.

## How it works

`lib/common.sh` resolves every state path under the layer's `local` section. `init` appends the ignore line once, however often it runs, and `doctor` fails when anything under `.ai/local/` is tracked. Every record still carries computed identity — `repository_id`, `worktree`, `branch`, `head` — so a record that reaches another checkout by other means (a copied working directory, a synced folder) is recognised as foreign: `check`, `finish --check` and `watch` report it and enforce nothing from it, and the other checkout may start its own task beside it.

## How to see it

```bash
majordomus init && git add -A && git commit -qm install
git ls-files .ai/local             # nothing
git worktree add ../elsewhere -b elsewhere
ls ../elsewhere/.ai/local          # No such file or directory
majordomus start "here" --scope docs
cp .ai/local/state/current.yaml ../elsewhere/.ai/local/state/   # after mkdir -p
( cd ../elsewhere && majordomus check )                          # OK task — foreign record (worktree …), not enforced here
```

## What it does not cover

Durability of local state is the checkout plus whatever backup the operator keeps; git does not carry it, by design. A handover meant for another machine is carried there deliberately, as the `handover` command's output says. Migration from the layout that tracked state makes a verified backup first, which `test/cases/66_migrate_legacy.sh` proves.

## Why it exists

A tracked task record travels with the branch, and on 2026-09-04 one stale committed record held every worktree of this repository to a scope none of them had claimed and blocked every push until somebody closed it by hand. State that belongs to one checkout must not be able to do that to another. `test/cases/01_init.sh` proves the ignore and the absence; `test/cases/27_foreign_task.sh` proves a record that arrives anyway is honoured nowhere but where it was written.
{% endraw %}
