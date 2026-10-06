+++
title = "Asked before work on an issue, `intent preflight` names the intent the issue serves with the governance that applies, or refuses naming the missing link"
description = "A worker about to take an issue can ask which intent the work serves. The answer follows the issue to its milestone and the milestone to the intents that name it, and returns each intent with the governance entries it declares. An issue the plan does not hold, or a milestone no intent names, is refused with exit 10 and the link that is missing."
weight = 115
[extra]
claim_id = "intent-preflight-names-the-intent"
status = "guaranteed"
source = "docs/claims/intent-preflight-names-the-intent.md"
+++
{% raw %}

## What it means

A worker about to take an issue can ask which intent the work serves. The answer follows the issue to its milestone and the milestone to the intents that name it, and returns each intent with the governance entries it declares. An issue the plan does not hold, or a milestone no intent names, is refused with exit 10 and the link that is missing.

## How it works

`IntentPreflight` in `apps/majordomus-cli/src/intent.rs` walks the plan and the intents; `--path` asks the same question for the paths the work will touch. The command line and the `majordomus_intent_preflight` MCP tool return the same answer, which `apps/majordomus-cli/tests/intent.rs` asserts.

## How to see it

```bash
majordomus intent preflight --issue I1900
majordomus intent preflight --issue I9999   # exit 10: the issue is not in the plan
```

## What it does not cover

It is a question, not a step: `plan start` does not run it, and a session does not load the intent it names (`intent-in-session-context`).

## Why it exists

Work that cannot say which intent it serves is the commonest way an implementation ends up with no reason behind it. Asking before the first edit costs one command.
{% endraw %}
