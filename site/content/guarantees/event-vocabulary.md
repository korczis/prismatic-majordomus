+++
title = "The ledger's event vocabulary is closed, on the way in and on the way out"
description = "share/events.yaml declares every event name the ledger accepts, the command that writes"
weight = 79
[extra]
claim_id = "event-vocabulary"
status = "guaranteed"
source = "docs/claims/event-vocabulary.md"
+++
{% raw %}

## What it means

`share/events.yaml` declares every event name the ledger accepts, the command that writes
it, and the payload keys that name must carry. Writing an unregistered name is refused.
Filtering on one is refused. A stored line carrying one is reported.

This is the repository's own rule — an unknown configuration key is an error — applied to
its own durable records, which had been exempt from it.

## How it works

`mj_ledger_append` loads the registry and refuses two things as internal errors: a name the
registry does not declare, and a registered name whose payload is missing a key that entry
requires. They are internal errors rather than findings because a command writing an event
the vocabulary does not define is a bug in Majordomus, not a fact about the repository being
supervised.

`history --event <name>` refuses an unregistered name instead of answering "no matching
events", which is the answer a registered name that has not occurred would also give; a
filter that cannot distinguish those two is not a filter. `history --validate` reports any
stored line whose name no reader recognises.

`test/cases/33_event_registry.sh` holds four things to each other: the registry, the
`mj_ledger_append` call sites, the rendering in `lib/history.sh`, and the table in
`docs/SCHEMAS.md`. A name in any one of them that is absent from the others fails.

The names are registered exactly as they were already written, including the inconsistent
ones. Renaming would break every ledger that exists, including this repository's own, and a
reader that silently omits everything before a rename is worse than an inconsistent name.

## How to see it

```bash
majordomus history --event task.startd     # exit 2, and lists the names that exist
majordomus history --validate              # exit 10 on a line no reader recognises
sed -n 's/^  - id: //p' share/events.yaml
```

## What it does not cover

Payload validation checks that a required key is present, not that its value is meaningful.
Events already written before the registry existed are not migrated — they do not need to
be, because the registry declares the names that were already in use.

A refused `finish` is still not recorded. An accepted completion is written and a refusal
leaves no trace, so a task refused four times is indistinguishable in the ledger from one
that passed first time. That is an absent event rather than an unvalidated one, and
`test/cases/32_refusal_lifecycle.sh` carries the assertion that will hold when it exists.

## Why it exists

`mj_ledger_append` took any string. A mistyped name produced a durable, append-only record
that every reader silently skipped — the worst kind of failure for a record whose purpose is
to let someone reconstruct what happened. `docs/SCHEMAS.md` had documented a `bootstrap`
event with a `reason` field since before the ledger had readers, and nothing had ever
written it.
{% endraw %}
