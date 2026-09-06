+++
title = "A closed execution episode is a shared object of the layer, written by the tool from git and the ledger, valid against a contract that admits no conversation and no absolute path, and discovered rather than registered"
description = "A stretch of work ends, and what it did becomes a file the whole repository can read: when"
weight = 134
[extra]
claim_id = "session-records"
status = "guaranteed"
source = "docs/claims/session-records.md"
+++
{% raw %}

## What it means

A stretch of work ends, and what it did becomes a file the whole repository can read: when
it opened and closed, the branch, the commit it started from and the one it ended at, the
commits between them, the paths the working tree held changed, the tasks, issues,
checkpoints, handovers, decisions, questions and evidence it produced, and how it ended.

The record is not a story about a session. Every field is derived — the times from the
clock, the heads and the commits from git, the reference lists from the ledger's own events
for that episode — and the only authored part is a summary of the work, given on standard
input when the episode closes.

## How it works

`majordomus session close` writes it into the section the manifest names, `.ai/repo/sessions/`,
and nothing else writes or edits one. The front matter satisfies
`share/schemas/session-record.schema.json`, from which the allow-list
`share/allow/session-record.txt` is generated; `schema: session/v1` identifies the format and
a version the executable does not read is refused rather than guessed at.

Discovery is one line: the source class `session` in `.ai/repo/knowledge/sources.yaml`. From
it follow the index, the MCP resource `majordomus://session/<id>`, the object routes and
their OpenAPI description, the knowledge graph's `session` nodes and the site's registry
pages. No list of sessions exists anywhere, and closing an episode edits nothing.

Two refusals are mechanical rather than remembered. The contract has no field for a
conversation, so a record cannot carry one and an unknown key is an error — which is
`project.never-store-transcripts` enforced by a schema instead of by a habit. And no value
may be an absolute path: the repository is named by its remote and the working copy by
`worktree_id`, a hash the resolver uses to tell this checkout's records from another's
without disclosing where either lives.

## How to see it

```bash
majordomus session close                     # writes the record, prints its path
majordomus session list                      # closed episodes, newest first
majordomus session show <id>                 # one record, whole
majordomus knowledge nodes --kind session    # the same records as graph nodes
majordomus doctor                            # the contract, over every record
```

`test/cases/63_session_records.sh` proves it by mutation: a record with an unknown key, with
an absolute path, with a duplicate identity and with an unknown schema, each refused by
name, and one closed episode reaching the index, the graph and the listing with nothing
registered.

## What it does not cover

Whether a summary is any good. The tool checks that the record exists, parses, carries every
field the contract requires and claims an identity nobody else claims; whether the sentence
a worker wrote about the work is accurate is a reviewer's judgement, as it is for a
checkpoint or a decision.

Nor does it make the ledger public. The detailed account of events stays in the
checkout-local half, along with the absolute paths and the open session's own state; the
shared record names what happened, and the local one remains for whoever holds the checkout.

## Why it exists

The record already existed and almost nobody could read it, because it was written where no
projection reaches. A repository that can list every rule, claim, decision and use case over
four surfaces could not say what happened last Tuesday — and the memory of a repository
should not depend on who was in the room. ADR 0014 records the decision, including the
disclosure it makes in a public repository and why the closed field set is what makes that
safe.
{% endraw %}
