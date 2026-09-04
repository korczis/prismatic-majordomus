# Ranked or semantic retrieval over durable records is deliberately not implemented

## What it means

There is no vector store, no embedding, no relevance score and no index. `search` is a literal grep across the record kinds in a fixed order. This is a decision, published so that a reader can see it was refused rather than forgotten.

## How it works

It does not. `lib/search.sh` shells out to `grep -nF -i` per record kind and prints what it finds.

## How to see it

```bash
majordomus search "callback"        # substring match, authority order, no score
majordomus search "call.ack"        # exit 12: literal, so a regex is not a pattern here
```

## What it does not cover

Everything an embedding would give you: synonyms, paraphrase, ranking by relevance, and finding the record that discusses a subject without naming it. If your records are numerous enough that this hurts, the honest answer today is that Majordomus is the wrong tool for that corpus.

## Why it exists as a refusal

Three reasons, in order of weight.

An index is a second source of truth about what exists, and it can disagree with the records. Everything else in Majordomus is derived on read precisely so that this cannot happen; a stale index that quietly omits a handover is the same class of bug as a stale projection, with none of the stamping that catches one.

An embedding adds a dependency, a model call and a similarity threshold. The tool makes no network call and invokes no model anywhere, and a retrieval feature is a poor reason to breach that.

The corpus does not justify it. A repository's continuity records are a handful of Markdown files and one JSONL; a scan over them is faster than the staleness problem an index introduces.

Revisited when a repository appears where the scan is measurably too slow — measured, not assumed.
