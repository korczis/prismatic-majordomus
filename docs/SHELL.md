# Shell automation

This repository's automation is moving out of shell. ADR 0069 decides the direction: a
behaviour that belongs to the executable becomes a typed capability, and orchestration over
capabilities becomes a Rhai script — a scripted capability — rather than a shell script. A
migration of that shape converges only if new shell stops arriving while the old shell is
being moved, and that is what this page is about: the inventory that tracks the migration,
and the check that refuses shell nobody declared.

## The check

```sh
majordomus shell check                  # every finding with its remedy; exit 10 on any
majordomus shell check --format json    # the same answer as a document
majordomus shell check --canonical      # put the inventory in canonical order, then check
```

`shell.check` is a capability of the executable, so the same verdict is served over MCP
(`majordomus_shell_check`) and HTTP (`GET /api/v1/shell/check`). It reads git's index and
the first bytes of each file; it builds nothing and uses no network. In CI it is the
`shell-inventory` gate of the structure job, selected by any change under a governed
directory, the inventory or the module that judges it (`.ai/repo/ci/gates.yaml`).

A **shell unit** is a file under `bin/`, `lib/`, `scripts/`, `share/`, `.githooks/` or
`.claude/hooks/` whose first line names a shell (`#!/bin/sh`, `#!/usr/bin/env bash`,
`#!/usr/bin/env zsh`, `#!/usr/bin/env -S bash -eu`, ...) or whose name ends in `.sh`. The
files considered are the ones git would commit — tracked files, and untracked files no
ignore rule excludes — so a new script is refused before it is added. A symbolic link is
not a unit; what it points at is, where that lives.

`test/` is not governed. Every behavioural case is a shell script by the runner's contract,
so refusing them would refuse every new test until the harness itself moves; the harness is
one record of the inventory instead.

| code | refused when |
|---|---|
| `shell.undeclared` | a shell unit has no record, or its record carries no exemption |
| `shell.stale` | a record names a unit the tree no longer has |
| `shell.missing_field` | a record has no `disposition` or `reason`, or an exemption no `why` or `removal` |
| `shell.invalid_disposition` | a disposition is not one of A-F |
| `shell.exemption_not_shell` | an exemption is on a file that is not a governed shell unit, or on an anchor |
| `shell.invalid_unit` | a unit is absolute, climbs out of the tree, or is under `.ai/local/` |
| `shell.duplicate` | two records share a unit and an anchor |
| `shell.out_of_order` | the records are not in canonical order |
| `shell.malformed` | a line is not a record (an unknown field included) |

Exit `0` is clean, `10` is a finding, `12` is a tree that could not be measured — which is
not a pass.

## The inventory

`.ai/repo/automation/inventory.jsonl` holds one JSON object per line: every unit of the
automation the migration audit found — shell files, and also the awk, jq and JavaScript
they call, the shell embedded in workflows, recipes and hooks, and the places the
executable spawns a shell. A record is:

```json
{"unit":"scripts/ci/order-check","disposition":"A","reason":"…","exemption":{"why":"…","removal":"…"},"facts":{"purpose":"…","callers":["…"]}}
```

| field | holds |
|---|---|
| `unit` | the path, relative to the repository root |
| `anchor` | optional: a place inside the unit (`deploy :: publish`, `recipe build`) for embedded shell |
| `disposition` | `A` core capability → typed capability; `B` orchestration → scripted capability; `C` bootstrap glue; `D` dead; `E` generated; `F` platform adaptor |
| `reason` | why that disposition |
| `exemption` | required on a governed shell file, refused anywhere else: `why` shell is required today, and the `removal` condition |
| `facts` | the audit's descriptive facts; carried and never judged |

Nothing mechanical is recorded. A line count or an interpreter written into the file would
be stale after the next edit with nothing to say so; the check derives both from the tree.
The inventory and the exemption list are one file on purpose: a shell unit's disposition is
written once, and a second list of exemptions beside the inventory would be the repeated
definition ADR 0004 names as a defect.

## Adding shell

Don't, first. New repository automation is a typed capability of the executable
(`docs/CAPABILITIES.md`) when it decides something, or a scripted capability when it
orchestrates capabilities. When shell really is required — a hook a provider invokes before
the executable exists, an installer run as `curl | sh` — add a record with a disposition, a
reason and an exemption, and run `majordomus shell check --canonical` to put it in place.
The exemption is read in review; it is the whole argument for the file.

## Migrating a unit out

1. Move the behaviour: a typed capability for A, a scripted capability for B.
2. Point every caller the record's `facts.callers` names at the replacement, and check the
   tree for the ones the audit missed.
3. Delete the file **and its record** in the same change. Deleting only the file fails the
   check as `shell.stale`; that is the ratchet, and it is what keeps the list from
   outliving the units it describes.

A unit that stops being shell without being deleted — rewritten in place as something else
— keeps its record for the migration history but loses its exemption, which the check
otherwise refuses as `shell.exemption_not_shell`.
