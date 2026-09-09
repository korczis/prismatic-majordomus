---
schema: context/v1
id: ai.repo.changes
kind: context
title: Release changes
description: One record per thing a release did that its contract diff cannot say on its own, and the only place a changelog entry is written.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 76
---

# Release changes

One file per change, `<id>.md`, contract `change/v1`
(`share/schemas/majordomus/change/change.v1.schema.json`). A record says what changed for
a reader, what it costs a caller, which contract entries it accounts for, and — for a
breaking change — the way through it.

## Why these exist at all

A release has two halves, and only one of them can be derived.

```text
what the machine knows                    what only a person knows
──────────────────────                    ────────────────────────
a capability appeared                     a bug was fixed
a route moved                             why a break was worth making
a target was withdrawn                    what a reader has to do about it
an input became required                  what the change was for

  the contract diff                          a change record
        └──────────────┬──────────────────────────┘
                       ▼
              the changelog, the release
              notes, the Cockpit, the API,
              MCP and the site
```

The contract diff is computed, at every commit, from the committed contract snapshot; see
`docs/RELEASE.md`. Nothing here restates it. What a record adds is the half a diff cannot
reach — and, for a breaking change, the obligation to say what to do about it.

## What reads them

```text
CHANGELOG.md                       generated; the reader's document
docs/generated/releases/<tag>.json one manifest per release
the GitHub release body            the same section, rendered by the same code
/api/v1/release/changelog          the model, over HTTP
majordomus_changelog               the same, over MCP
/cockpit/release                   the same, as a page
```

None of them is authored. A change written once appears in all of them; a change written
twice is a defect, and there is nowhere to write it twice.

## Writing one

Not every commit needs a record. A capability that appeared is already in the contract
diff, and a record repeating it adds nothing. Write one when:

- the change is **not visible in the contract** — a fix, a behaviour that moved inside an
  unchanged signature, a performance characteristic somebody depends on;
- the change **is** in the contract and is **breaking**, in which case a record is
  required: `majordomus release check` refuses a release whose breaking contract change no
  record names, and refuses a breaking record that names no migration document;
- the change is worth a reader's attention for its own sake.

```yaml
---
schema: change/v1
id: release-identity            # allocated once, never reused; the file name
kind: change
title: the release state is one model    # present tense, as the changelog prints it
type: added                     # added changed deprecated removed fixed security
impact: additive                # none patch additive breaking
scopes: [cli, api, mcp, cockpit]
contract: ["capability:release.status"]  # what this record accounts for
issues: []
pull_requests: []
adrs: [adr-0028]
# migration: docs/migrations/0-4-0.md    # required when impact is breaking
---

## Summary

What changed and why it mattered, for somebody reading the changelog months later.

## Migration

What a reader has to do. The short form of the document `migration` names.
```

## `released_in`

Absent means unreleased: the record belongs to whatever the next release turns out to be.
`majordomus release prepare` stamps it with the version being prepared, and nothing else
writes it — a record stamped by hand claims a release happened.

## Invariants

- one record per id, and the file name is the id;
- a record's `impact` may not understate the contract: the release check compares them;
- a breaking record names a `migration` document that exists;
- every breaking contract change is named by some record's `contract` list.

All four are decided by `majordomus release check`, which the release gate runs.
