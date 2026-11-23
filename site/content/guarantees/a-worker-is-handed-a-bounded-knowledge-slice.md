+++
title = "When the profile sets context.knowledge, majordomus context carries pointers to the task's issue, its dependencies, its milestone, the claims that milestone declares and the nodes its scope names, and nothing selected by similarity; the item limit cuts it with a count, a tight budget drops it before git, the task, the profile and open questions with the drop named, and a stale graph is reported rather than the section vanishing"
description = "When the profile sets context.knowledge, majordomus context adds a section of pointers: id, kind, source and title. It points at the task's issue, the issues that issue depends on, its milestone, the claims that milestone declares and the nodes the task's scope names. Nothing is selected because its words resemble the issue's. The section is cut at context.knowledge_max_items, and the cut is counted. Under a tight line budget the section is dropped before git, the task, the profile and the open questions, and the drop is named. When the issue is not in the graph, the section says the graph is stale rather than vanishing."
weight = 242
[extra]
claim_id = "a-worker-is-handed-a-bounded-knowledge-slice"
status = "guaranteed"
source = "docs/claims/a-worker-is-handed-a-bounded-knowledge-slice.md"
+++
{% raw %}

## What it means

When the profile sets `context.knowledge`, `majordomus context` adds a section of pointers: id, kind, source and title. It points at the task's issue, the issues that issue depends on, its milestone, the claims that milestone declares and the nodes the task's scope names. Nothing is selected because its words resemble the issue's. The section is cut at `context.knowledge_max_items`, and the cut is counted. Under a tight line budget the section is dropped before git, the task, the profile and the open questions, and the drop is named. When the issue is not in the graph, the section says the graph is stale rather than vanishing.

## How it works

The knowledge pointers block in `lib/context.sh` reads the toggle and the item limit from the profile, both declared in `share/allow/profile.txt`. It selects nodes by explicit reference only, through the graph's own keep list. It sits low in the drop order, below every immutable fact. A profile that does not set the toggle gets the section named under EXCLUDED with the reason.

## How to see it

```bash
majordomus context                                  # EXCLUDED names "knowledge pointers" when the profile does not ask
majordomus context --budget-lines 40               # under a profile that sets context.knowledge: the pointers, the cut counted, the drop named
majordomus knowledge show <id>                      # follow a pointer instead of reading the base
```

## What it does not cover

It does not rank by similarity, rebuild the graph from the context, or raise the context's line budget. It points; reading what a pointer names is the worker's next step.

## Why it exists

A worker that loads the whole knowledge base spends its context on what the task does not need, and the facts it does need are crowded out first. A slice chosen from what the task references is small, explainable and reproducible (I0814, `test/cases/3073_context_knowledge_slice.sh`).
{% endraw %}
