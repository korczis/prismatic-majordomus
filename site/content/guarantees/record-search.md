+++
title = "Durable records are searchable literally, across kinds, without an index"
description = "majordomus search \"callback\" looks through handovers, checkpoints, decisions, open questions, prompt assets and the ledger, and prints the kind, the path, the line number and the matching line. A worker can find what was decided about a subject without reading every record."
weight = 64
[extra]
claim_id = "record-search"
status = "guaranteed"
source = "docs/claims/record-search.md"
+++
{% raw %}

## What it means

`majordomus search "callback"` looks through handovers, checkpoints, decisions, open questions, prompt assets and the ledger, and prints the kind, the path, the line number and the matching line. A worker can find what was decided about a subject without reading every record.

## How it works

A literal, case-insensitive, fixed-string `grep`, in authority order: handovers first, then checkpoints, decisions, questions, prompts, and the ledger last. `--kind` restricts and is repeatable, `--task` narrows to one task, `--limit` caps each kind. Exit `0` with matches, `12` with none — an empty result is a distinct outcome, not a silent success.

Literal, not regular expression: a search for `call.ack` finds `call.ack` and not `callback`, because a search term typed by a person is a phrase and a pattern that silently matches more than intended is worse than one that matches nothing.

There is no index, no ranking, no embedding and no cache.

## How to see it

```bash
majordomus search "callback" --kind decision --kind checkpoint
# checkpoint  .ai/local/state/checkpoints/20260903T194500Z--main--3f2a9c1--8c1d0e4a.md:12  Cause is in callback normalisation.
# decision    .ai/local/state/decisions.md:31  ## 2026-09-03 — Normalise the callback URI
# search: 2 match(es)

majordomus search "a phrase that appears nowhere"; echo $?   # 12
majordomus --json search "callback" | jq -r .kind
```

## What it does not cover

No ranking. Results come back in authority order and then in file order; the first result is not the best result, it is the most authoritative kind.

No semantics. Searching for "auth" does not find "authentication" unless the substring is there.

Records only. It does not search the repository's code, its git history, or its documentation — those have better tools.

## Why it exists

The alternative designs are worse for this corpus. An index has to be rebuilt, can fall out of step with the records it describes, and becomes a second source of truth about what exists. An embedding store adds a dependency, a model call and a similarity threshold nobody can explain to the person whose record failed to surface. Majordomus does not need a cognitive memory substrate to find six Markdown files, and a scan that takes milliseconds is not a problem worth an index.
{% endraw %}
