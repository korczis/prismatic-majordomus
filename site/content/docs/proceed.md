+++
title = "Proceeding"
description = "proceeding: what a session told only to proceed does — `proceed.plan`'s claim-aware decision, the take, and `proceed.step`'s work order (`do`, `fan_out`, `report`, `next_due_at`), why the same state answers the same order and key, when a step is done, and the generated `/proceed` skill"
weight = 21
[extra]
source = "docs/PROCEED.md"
+++

{% raw %}

How a session that is told only to proceed finds its work, takes it once, and knows when to
stop. Behaviour as implemented and tested; where this page and the executable disagree, the
page is wrong and changes in the same commit. The decision is
[ADR 0177](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0177-a-session-proceeds-through-one-entrypoint-that-composes-the-capabilities.md);
the module is [`proceed.rs`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/apps/majordomus-cli/src/capability/builtin/proceed.rs) and the
step is [`proceed/step.rs`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/apps/majordomus-cli/src/capability/builtin/proceed/step.rs).

## Two capabilities and a take, one order

<div class="overflow-x-auto" tabindex="0">

| what | command line | what it answers |
|---|---|---|
| `proceed.plan` | `majordomus proceed plan` | the decision: the READY issue nothing refuses, every READY issue passed over with the claim that refused it, and the sources read |
| the take | `majordomus proceed take --session <s>` | the command line's own composition of `proceed.plan`, `mesh.claim`, `worktree create` and `start`: claim, worktree and task for the selection, created once |
| `proceed.step` | `majordomus proceed step [--intent <id>]` | the step: one work order a loop acts on |

</div>


None of them ranks anything of its own. The order is the plan's (`plan.next`: the asked
milestone first, then wave, priority, what an issue unblocks, and id); a claim on the board or
the mesh removes an issue from it without reordering what remains.

## The work order

`proceed.step` asks `proceed.plan` for the decision, `intent_realization.work` for the intent
it serves and `models.route` for a worker's model, and answers:

<div class="overflow-x-auto" tabindex="0">

| field | holds |
|---|---|
| `do` | the capability to run next and its input: `devtask.implement` with the issue and the idempotency key `implement-<issue>`, which is the key `devtask.implement` itself defaults to |
| `fan_out` | the other free READY issues that may run beside it: declared `parallel_safe` (and the ordered issue must be too), sharing no path with anything already ordered, at most four |
| `report` | what to announce on the board, as `peers.announce` takes it: the claim `proceed`, one line, the ordered paths |
| `next_due_at` | when to step again: the next five-minute boundary after `at`; absent when the order is `done` |
| `outcome`, `done`, `reason` | `implement`, `wait` or `done`, and the one sentence that says why |
| `intent` | for an intent: its stage, criteria met, the unmet ones and the issues serving them |
| `key` | the order's identity: the digest of every field but `at`, `next_due_at` and `sources` |
| `at`, `sources` | the moment it was asked at (the clock, or `--at`), and what the decision was read from |

</div>


Each item of `do` and `fan_out` names the issue's execution profile, the effort that profile
declares and the model `models.route` selects for a worker that calls tools.

## The same state answers the same order

The order is a function of what it reads and of the moment it is asked at, and the moment
moves only `next_due_at`. Two steps inside one five-minute interval answer the same order,
byte for byte; a step in a later interval answers the same `key` with a later `next_due_at`;
any change of a fact the order rests on — a claim, a finished issue, new evidence — changes the
key. Nothing is stored: there is no queue of orders and no second claim ledger.

Running the order is idempotent because `devtask.implement` is: an order repeated over the
same state names the same key, and a repeated key answers the execution it started
(`deduplicated`) and creates nothing.

## When it stops

With an intent, only the issues serving its unmet criteria are ordered. The order is `done` —
no `do`, no `fan_out`, no `next_due_at` — when every criterion has current evidence, or the
intent is satisfied, cancelled or superseded. Without an intent the whole plan is stepped, and
the order is `done` when no unfinished issue is left. A step that may take nothing (every
serving issue held or waiting, or a provisional decision because the board or a declared mesh
could not be read) is `wait`: nothing to run, step again at `next_due_at`.

<div class="overflow-x-auto" tabindex="0">

| exit | `proceed step` |
|---|---|
| 0 | work is ordered, or the order is done |
| 10 | the step waits |

</div>


## `/proceed`

`/proceed` is generated from the declaration of `proceed.step` — `.skill("proceed",
"intent")` and its `.follow(...)` — by `majordomus generate editors`, for every product that
reads skills: `.claude/skills/proceed/SKILL.md` for Claude Code and
`.agents/skills/proceed/SKILL.md` for Antigravity and Zed. No file holds a procedure of its
own; a hand edit of either fails `majordomus generate --check`. The text typed after
`/proceed` is the intent, and with none the whole plan is stepped.

```text
/proceed a-session-proceeds-through-the-mesh
  -> proceed.step {intent}            the order
  -> peers.announce report            on the board
  -> devtask.implement do.input       once; a repeat is the same execution
  -> a worker per fan_out item        on the model and effort it names
  -> step again at next_due_at        (/loop), until done
```

The evidence is `test/cases/4005_proceed_step_is_one_idempotent_work_order.sh`.
{% endraw %}
