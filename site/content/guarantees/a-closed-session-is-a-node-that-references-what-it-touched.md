+++
title = "A closed session is a knowledge node of operational scope whose edges reference the tasks, issues, checkpoints, handovers, decisions and questions its record names, each with the record and field it was observed in, so the graph answers which sessions touched an issue without any session body reaching a node, an edge or a shared projection"
description = "Every closed session record under .ai/repo/sessions/ is one node of the knowledge graph. Its edges are typed references to the tasks, issues, checkpoints, handovers, decisions and questions the record names. Each edge carries the session record and the front-matter field it was observed in. majordomus knowledge related <issue> therefore answers which sessions touched an issue, and every answer names the record it came from. No text from the session's body reaches a node or an edge."
weight = 241
[extra]
claim_id = "a-closed-session-is-a-node-that-references-what-it-touched"
status = "guaranteed"
source = "docs/claims/a-closed-session-is-a-node-that-references-what-it-touched.md"
+++
{% raw %}

## What it means

Every closed session record under `.ai/repo/sessions/` is one node of the knowledge graph. Its edges are typed `references` to the tasks, issues, checkpoints, handovers, decisions and questions the record names. Each edge carries the session record and the front-matter field it was observed in. `majordomus knowledge related <issue>` therefore answers which sessions touched an issue, and every answer names the record it came from. No text from the session's body reaches a node or an edge.

## How it works

`edges_session()` in `lib/knowledge.awk` emits one `references` edge per item of the record's `tasks`, `issues`, `checkpoints`, `handovers`, `decisions` and `questions` lists. A checkpoint or handover is named by its path's basename, the id its own extraction would give it. Both are references from the session rather than nodes of their own, so a high-volume record kind does not dominate the graph. The source class `closed-session` is declared in `share/knowledge-sources.yaml` with `discovery: vcs`. That makes its scope operational even though the file is tracked (ADR 0014), so a session node never reaches a shared projection.

## How to see it

```bash
majordomus knowledge sources --scope operational      # closed-session is listed here, not under shared
majordomus knowledge nodes --scope operational --kind session
majordomus knowledge related <issue-id> --scope all   # the sessions that name the issue, each edge with its record and field
majordomus knowledge nodes --scope shared --kind session   # nothing: a session is never shared
```

## What it does not cover

It infers nothing about why a session touched something, and it copies no summary into a node. A session that names nothing yields a node with no edges. The session record itself is still published on the site's /sessions/ pages from the tracked file. That is the record's own publication, not a projection of the graph.

## Why it exists

A worker with no conversation history wants temporal and causal navigation: which earlier sessions worked on this issue, which handover continued that task. The record already states those links, so the graph can carry them without reading prose. Keeping the node free of body text means the graph cannot become a second copy of what a session said (I0813, `test/cases/4407_a_closed_session_is_a_node_that_references_what_it_touched.sh`).
{% endraw %}
