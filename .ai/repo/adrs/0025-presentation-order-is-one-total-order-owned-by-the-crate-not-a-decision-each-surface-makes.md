---
schema: adr/v1
id: adr-0025
kind: adr
title: Presentation order is one total order owned by the crate, not a decision each surface makes
status: accepted
date: 2026-09-09
tags:
  - architecture
  - presentation
  - determinism
  - projections
related:
  - rule:project.canonical-order
  - rule:project.interfaces-are-projections
  - rule:project.no-new-nouns
  - file:apps/majordomus-cli/src/order.rs
  - file:scripts/ci/order-check
  - file:.ai/repo/order-baseline.txt
provenance:
  origin: authored
---

# 25. Presentation order is one total order owned by the crate, not a decision each surface makes

## Context

Every fact this repository shows had been reduced to one declaration and many projections:
a capability is declared once and reaches MCP, HTTP, OpenAPI, the command line, the Cockpit
and the generated reference without any of them holding a definition (ADR 2, ADR 4, ADR 5);
an object of the layer is a file, and every surface that shows it is derived (ADR 7, ADR 8,
ADR 18); a web surface is discovered from its producer and resolved once (ADR 13). One
property of a collection had never been through that reduction, and it is the one a person
sees first: the sequence.

The crate carried eighty comparators, written beside the renderers that needed them. Most
were harmless duplicates of "sort by id". Some were not:

- The same class of finding was ordered worst-first in `why` and `product` and best-first in
  `web::validate`, because the two `Severity` enums were declared in opposite directions.
  Identical intent had to be written as opposite code, so a reader reconciling the two
  comparators would have inverted one report without touching a test.
- The same benchmark document was ranked by `p50` on the website and by `p95`, without a
  tiebreak, on the command line.
- The Cockpit's route table was sorted by the rendered HTML of its own rows, making every
  CSS class name part of the ordering.
- The HTTP route listing sorted on `Value::to_string()` — the JSON encoding of the path,
  quotes included — and had no method tiebreak.
- Exactly one comparator in the crate folded case, in the Cockpit's navigation; every other
  surface compared raw bytes beside it. Nothing anywhere compared digit runs by value, so
  the first `.v10` rule or four-digit ADR would have sorted wrong in the index, the graph,
  the Cockpit, the site and every generated document at once.

None of this was nondeterminism in the usual sense, and it is worth recording that the
usual suspects were absent: discovery is single-threaded, its enumerators sort before
returning, the index is sorted by URI and its sequence is part of the published fingerprint,
the registry is a `BTreeMap`, and the crate holds exactly one `HashMap` — the executor's
cache, which nothing projects. Two runs on one machine agreed. The defect was that six
surfaces agreed about the facts and disagreed about the sequence, which reads to a person
as the repository disagreeing with itself.

## Decision

Presentation order is one total order, declared once in `apps/majordomus-cli/src/order.rs`,
and every surface renders the sequence it is handed.

A type implements `Ordered` and answers an `OrderKey` of four parts, most significant first:

1. **group** — the semantic bucket, compared naturally; no group sorts after every group, so
   an ungrouped tail is legible and an ungrouped head cannot hide the groups.
2. **rank** — an explicit position inside the group, for the collections whose domain has
   one. `weight` on a moment, an audience, an area or a feature is such a rank. It defaults
   to unranked, so a collection with no such semantics never mentions it.
3. **label** — what a person reads, compared naturally: digit runs by value, ASCII case
   folded so `Alpha` and `alpha` stay adjacent.
4. **identity** — the canonical id, and the reason the order is total rather than merely
   tidy.

The fourth part is the decision, not a detail. Without it two items with the same label
exchange places whenever an unrelated item is added, a different iterator is used, or
another machine enumerates differently — and a comparator that leaves ties unbroken is
exactly what makes an order look like a race when there is none.

The order is enforced by `project.canonical-order` and the gate `scripts/ci/order-check`:
three absolute checks (no case-folded comparator outside `order.rs`, no sort key that
renders markup, no `.localeCompare(` in a generated or served surface) and one ratchet over
the debt that predates the rule, in `.ai/repo/order-baseline.txt`, which may fall and may
not rise.

No new noun. `project.no-new-nouns` is blocking and `docs/CONCEPTS.md` is the vocabulary;
an order is a property of a projection, not a thing the tool now has. `order.rs` names no
concept a worker must learn to use the tool.

## Alternatives rejected

**A comparator per collection, reviewed rather than owned.** What the tree had. It is the
cheapest thing to write and the most expensive thing to reconcile: the two inverted
`Severity` enums are the proof, because each was locally correct and the pair was a trap
for whoever noticed.

**Sorting in the consumers — the Cockpit's JavaScript, the site's templates, the generated
indexes.** It puts the sequence furthest from the data and duplicates it once per surface,
which is the shape `project.interfaces-are-projections` already refuses for names, routes
and schemas. A consumer may still offer a sort a person explicitly asked for; that is a
different thing and stays.

**A declared order per collection, in data — an `order:` field on everything.** Rejected as
the default. It makes every addition an edit in two places and encodes today's visual
sequence as a fact about the domain. An explicit rank is kept only where the domain already
had one and had already declared it: `weight`, and the `order` of a context document.

**A dependency for natural ordering.** The comparator is thirty lines, the crate's
dependency list is argued for line by line, and a general Unicode collation would answer a
question this repository does not ask.

**Fixing only the surfaces that looked wrong.** The sidebar that started this could have
been sorted in place, and was, earlier. That is how the repository arrived at eighty
opinions: each one was a small local fix to a visible symptom.

## Consequences

- Ordering is now a projection with an owner, and a new collection joins it by implementing
  one trait rather than by being remembered in each consumer.
- The two `Severity` enums are declared in the same direction, so "worst first" is `b.cmp(a)`
  wherever it appears and the reconciliation trap is gone. The schema of the topology
  finding's severity changes variant order; nothing reads it positionally.
- The Cockpit's route table and the HTTP route listing change visible order, from
  method-then-path to path-then-method: a route table reads by path, and neither order was
  ever asserted.
- `.ai/repo/order-baseline.txt` records the debt as two numbers rather than a promise. The
  crate still holds sort sites that could be the canonical order and are not yet, and the
  shell still holds `sort` invocations that collate by the locale. Both may only fall.
- The first four-digit ADR and the first `.v10` rule will sort correctly, in every surface
  at once, because there is one place that decides.
