+++
title = "Asked before work on an issue, `intent preflight` names the intents the issue serves with the governance that applies, or refuses naming the cause"
description = "A worker about to take an issue can ask which intent the work serves and whether it may"
weight = 116
[extra]
claim_id = "intent-preflight-names-the-intent"
status = "guaranteed"
source = "docs/claims/intent-preflight-names-the-intent.md"
+++
{% raw %}

## What it means

A worker about to take an issue can ask which intent the work serves and whether it may
proceed. The answer follows the criteria the issue declares in `serves` (never its milestone
alone) and judges each link by the coverage `intent validate` reports from, so the two never
disagree about which link is broken. It is one of three verdicts:

- `serves` (exit 0): every link holds, and each intent served has a critique with no blocking
  finding open. The answer returns each intent served with its served criteria, invariants,
  non-goals and governance.
- `maintenance` (exit 0): the issue serves nothing, under a milestone no live intent names.
- `refused` (exit 10), naming the cause: `unknown_issue`, `no_issue_covers_paths`,
  `issue_serves_nothing`, `serves_another_intent`, `serves_unknown_criterion`,
  `intent_not_critiqued` or `open_blocking_finding`.

## How it works

`IntentPreflight` in `apps/majordomus-cli/src/intent.rs` walks the plan, the intents and their
critiques; `--path` asks the same question for every open issue whose scope covers a path and
answers the worst verdict among them. The command line and the `majordomus_intent_preflight` MCP
tool return the same answer, which `apps/majordomus-cli/tests/intent.rs` asserts.

## How to see it

```bash
majordomus intent preflight --issue I1900
majordomus intent preflight --issue I9999   # exit 10, unknown_issue: the issue is not in the plan
```

## What it does not cover

The preflight itself is a question and takes no step. The commands that ask it are
`majordomus start` and `plan start`, through `intents.binding` (ADR 0111): what a task is
briefed with is `intent-in-session-context`, and what a start refuses is
`intent-refused-at-plan-start`.

## Why it exists

Work that cannot say which intent it serves is the commonest way an implementation ends up with
no reason behind it. Asking before the first edit costs one command.
{% endraw %}
