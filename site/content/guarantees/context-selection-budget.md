+++
title = "The assembled context obeys a line budget, and every section it drops is named with its reason"
description = "context.builder_budget_lines caps what majordomus context prints. When the briefing would exceed it, sections are dropped — but never silently, and never the parts a worker cannot work without."
weight = 56
[extra]
claim_id = "context-selection-budget"
status = "guaranteed"
source = "docs/claims/context-selection-budget.md"
+++
{% raw %}

## What it means

`context.builder_budget_lines` caps what `majordomus context` prints. When the briefing would exceed it, sections are dropped — but never silently, and never the parts a worker cannot work without.

## How it works

Sections are dropped in a fixed order: recent history, then touched files, then decisions, then the bodies of the checkpoint and the handover. Git, the task, the profile and open blockers are never dropped, because a worker missing those is not under-informed but misinformed.

The two record bodies degrade rather than disappear: the section header stays and the body becomes `body omitted for budget — read <path>`, so the worker knows the record exists and where to find it.

Every drop is appended to the exclusion list with `context budget <n> lines` as its reason.

The count is of the document actually printed, header and trailer included — `mj_ctx_render` writes the whole page to a file and measures that file, re-rendering after each drop. An earlier version counted only the sections, which let a fifty-line page report "40 of 40 lines". A budget that reports a number smaller than the thing it governs is worse than no budget.

If what cannot be dropped is still over budget, the context is printed anyway and the exit code is `10`: the worker gets the facts, and the caller learns the budget was not met.

## How to see it

```bash
majordomus context --budget-lines 50 | tail -8
# ## EXCLUDED
# - history — context budget 50 lines
# - decisions — context budget 50 lines
#
# ## BUDGET
# 48 of 50 lines (dropped: history decisions)

# the reported count is the printed count
[ "$(majordomus context --budget-lines 50 | wc -l)" = 48 ]

majordomus context --budget-lines 5 >/dev/null; echo $?   # 10
```

## What it does not cover

Lines, not tokens. Majordomus does not measure token spend anywhere and does not estimate it here; a line count is what it can compute deterministically on any machine. See [`ECONOMICS.md`](../ECONOMICS.md) for why no context-saving claim is made.

The budget governs the briefing, not what a worker loads afterwards. A worker is free to read the whole repository; nothing stops it and nothing measures it.

## Why it exists

Context is the resource that runs out, and it runs out silently. A budget that is only advice is a wish; a budget with no visible exclusions produces a briefing that is mysteriously thin. Naming both — the cap and everything it cost you — is what makes the number actionable rather than decorative.
{% endraw %}
