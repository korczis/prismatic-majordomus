# An architecture decision is one file under the layer's adrs section, validated against the decision contract, with an identity nothing else claims, reciprocal supersession, and every reference it makes resolving

## What it means

A decision is data, not prose the tool happens to store. `.ai/repo/adrs/<NNNN>-<slug>.md`
carries front matter validated against `share/schemas/adr.schema.json` — the schema
identifier, the identity `adr-NNNN`, the kind, the title, one of four statuses, the date it
reached that status, and optionally tags, supersession and provenance — over a body with
non-empty `# Context`, `# Decision` and `# Consequences`. An unknown key is an error, an
unknown status is an error, and a schema version the tool does not know is refused rather
than guessed at.

Three invariants hold across the set rather than within one file. An identity is claimed by
exactly one record and fixes the file-name prefix, so a retitle moves the slug and never the
number. Supersession is reciprocal: `supersedes` on one record requires `superseded_by` on
the other, because a chain walkable from one end only is not a chain. And every reference a
record makes — a decision it supersedes, a file or test it was derived from — resolves.

## How it works

`share/kinds.yaml` declares the kind, `share/schemas/adr.schema.json` is its contract, and
the allow-list `share/allow/adr.txt` is generated from that schema. Discovery is the source
class `adr` in `.ai/repo/knowledge/sources.yaml`; nothing else registers a decision, so
adding the file is the whole registration.

`lib/adr.sh` derives one catalogue from that discovery and every surface reads it:
`adr list`, `adr show`, `adr check`, and `mj_validate_adr`, the validator of the doctrine
`majordomus.adr-integrity`, dispatched from `doctor` and `watch`, failing with exit 10 under
the category `adr`.

## How to see it

```bash
majordomus adr list                    # every decision: id, status, date, title
majordomus adr list --status proposed  # only the ones waiting for a person
majordomus adr check                   # every identity, status, reference and section
majordomus adr check --json            # the same, for a machine
```

## What it does not cover

The contract is structural. That a decision is *right*, or that its consequences were
thought through, is a person's judgement; what the tool guarantees is that the set is
readable, unambiguous and internally consistent, and that a broken reference is found by a
command rather than by a reader months later.

## Why it exists

This repository shipped two `0005`s and two `0007`s before anything checked. Duplicated
identities and one-sided supersession are exactly the failures a human reader does not
notice and a machine finds instantly, so the set is validated the way every other kind in
the layer is.
