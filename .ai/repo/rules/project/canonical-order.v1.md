---
id: project.canonical-order
version: 1
kind: rule
title: A collection has one order, owned by one place, and it does not depend on who ran the command
description: Every collection this repository shows a person or a stable machine consumer is ordered by apps/majordomus-cli/src/order.rs — a total order over group, explicit rank, natural label and canonical identity — and no consumer holds an ordering opinion of its own; a comparator that folds case by hand, a sort key that renders markup, and a comparison that collates by the reader's locale are all refused.
statement: Order a collection once, through crate::order, on a key that ends in a unique identity; never beside a renderer, never on rendered output, never by the machine's locale, and never in a second consumer that sorts the same collection again.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.no-new-nouns@1]
tags: [architecture, presentation, determinism, rust]
---

# Rationale

Ordering was the last thing in this repository that every surface decided for itself. The
crate carried eighty comparators, and the ones that mattered disagreed: the same findings
were sorted worst-first in `why` and `product` and best-first in `web::validate`, because
the two `Severity` enums were declared in opposite directions and so identical intent had to
be written as opposite code — a reader reconciling them would have inverted one report. The
same benchmark document was ranked by `p50` on the website and by `p95`, with no tiebreak,
on the command line. The Cockpit's route table was sorted by the HTML of its own rows, so a
CSS class rename reordered it. One case-folded comparator existed, in the Cockpit's
navigation, and every other surface compared raw bytes beside it.

None of that was nondeterminism in the usual sense. Discovery here is single-threaded, the
index is sorted by URI, the registry is a `BTreeMap`, and the crate holds exactly one
`HashMap` — the executor's cache, which nothing projects. The defect was worse than a race:
six surfaces that agreed about the facts and disagreed about the sequence, so a person
comparing them concluded that the repository disagreed with itself.

An order is a projection, and `project.interfaces-are-projections` already says a projection
carries no definition of its own. This rule is that sentence applied to sequence.

# Required behaviour

1. **One owner.** `apps/majordomus-cli/src/order.rs` defines the order: a type implements
   `Ordered`, answers an `OrderKey` of group, rank, label and identity, and callers call
   `canonical()`. A collection ordered any other way is ordered by an opinion.
2. **The key ends in an identity.** The last part of every key is unique across the
   collection, so the order is total: two items with the same label may not exchange places
   because an unrelated item was added, because a different iterator was used, or because
   another machine enumerated differently.
3. **Order values, not renderings.** A sort key is data. Sorting rendered markup makes a
   class name, an escape and a whitespace decision part of the sequence.
4. **Compare code units, never a locale.** `LC_ALL=C sort` in shell, `<` and `>` in
   JavaScript, `Ord` in Rust. A comparison that collates by the reader's locale orders the
   published output by whose machine produced it.
5. **A severity is greater when it is worse.** Both severity enums are declared least
   first, so a comparator that puts the worst first reads `b.cmp(a)` everywhere it appears.
6. **A consumer does not re-sort.** The Cockpit, the site templates, the generated indexes
   and the JavaScript that runs in a published page render the sequence the registry
   handed them. Explicit sorting a person asked for is a different thing and stays.

# Failure behaviour

`scripts/ci/order-check` is the gate. Three of its checks are absolute, because the tree
satisfies them and a new violation is always a mistake: a case-folded comparator outside
`order.rs`, a sort key that calls `render()`, and a `.localeCompare(` under `scripts/`,
`share/` or `site/`. The fourth is a ratchet over the debt that predates the rule — the
number of sort sites in the crate outside `order.rs`, and the number of shell `sort`
invocations not pinned with `LC_ALL=C` — held in `.ai/repo/order-baseline.txt`, which may
fall and may not rise. A commit that adopts the canonical order lowers the baseline with
`scripts/ci/order-check --update`, and the gate refuses a baseline that no longer matches
the tree in either direction, so the debt cannot be quietly rewritten either way.

The gate runs in the `structure` job for changes to the crate, the scripts, the shell tool
and the site.

# Verification

`scripts/ci/order-check` on this tree. The order itself is proved where it is declared:
`apps/majordomus-cli/src/order.rs` orders every one of the 5040 permutations of a seven-item
fixture identically, and checks the comparator for antisymmetry and transitivity over every
pair and triple — a comparator that fails either can make `sort` answer differently for a
different input order, which is the failure this rule exists to prevent. The doc examples
run under `cargo test --doc`.
