# `majordomus adr propose` writes a decision with status proposed and refuses to write any other status, allocates its identity under a lock so concurrent workers never collide, and records what the decision was derived from

## What it means

`propose` writes a record with `status: proposed` and has no flag that says otherwise.
Acceptance is the person's act: a tool that can write `accepted` can turn its own inference
into repository truth by writing it down, and a reader months later cannot tell which of the
accepted decisions a person actually accepted. Moving a record to `accepted` is editing the
field.

A record the tool derived says so — `provenance.origin: extracted` — and names its evidence
in `provenance.derived_from`, whose references are typed (`decision:`, `session:`,
`commit:`, `issue:`, `file:`, `test:`) and whose file and test references must resolve. An
extracted record with no evidence is an assertion and is refused as one. It may become
`superseded`, because that is something a later record did to it; it may not claim
`accepted`.

The identity is allocated under a lock over the section directory, so two worktrees
proposing at the same moment get two numbers rather than one number twice.

## How it works

`lib/adr.sh`: `mj_adr_propose` takes the title, optional `--from <type>:<value>` references,
`--tag` and `--supersedes`, allocates the next free number under the lock, writes the file
and its skeleton body, and appends the event to the ledger. `--supersedes` writes both
halves of the relation, so the pair is reciprocal from the moment it exists. The refusal to
write any other status is not a flag check but the absence of any path that writes one.

## How to see it

```bash
majordomus adr propose "The registry is the one canonical declaration" \
  --from decision:t-2026-09-05-a --from file:docs/CAPABILITIES.md --tag architecture
majordomus adr list --status proposed
majordomus adr check
```

## What it does not cover

`propose` does not decide anything. It records that a decision was made and where the record
came from; whether the decision is sound, and whether it is accepted, is the person's.

## Why it exists

The decisions a repository lives by are made in conversations that are gone by the next
morning. Extraction is worth having only if the extracted record cannot pass itself off as
one a person approved — so the boundary between "the tool proposed this" and "a person
accepted this" is enforced rather than trusted, and the concurrent case is proved with real
processes rather than asserted.
