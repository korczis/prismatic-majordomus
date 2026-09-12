+++
title = "A collection has one order, owned by one place, and it does not depend on who ran the command"
description = "A collection has one order, owned by one place, and it does not depend on who ran the command"
weight = 64
[extra]
kind = "rule"
slug = "project-canonical-order-1"
identity = "project.canonical-order@1"
status = "active"
source = ".ai/repo/rules/project/canonical-order.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

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

## Failure behaviour

`scripts/ci/order-check` is the gate. Three of its checks are absolute, because the tree
satisfies them and a new violation is always a mistake: a case-folded comparator outside
`order.rs`, a sort key that calls `render()`, and a `.localeCompare(` under `scripts/`,
`share/` or `site/`. The fourth is a ratchet over the debt that predates the rule — the
number of sort sites in the crate outside `order.rs`, and the number of shell `sort`
invocations not pinned with `LC_ALL=C` — held in `.ai/repo/order-baseline.txt`, which may
fall and may not rise. The crate count is over code that ships: a sort inside anything
`#[cfg(test)]` introduces — a module, a function, or a whole file a `#[cfg(test)] mod`
declaration gates — or inside a doc comment orders a fixture or an example, is compiled out
of the binary, and is not a second opinion any reader can see. Counting those asked for
canonical order in a test whose whole purpose is to assert an order, and a gate that cries
wolf is a gate somebody switches off. `scripts/ci/order-check --sites` lists exactly what
each ratchet counts, so a reader of the number is never left to reconstruct which sites it
is made of. The shell count is over `sort(1)` and nothing else: a `.jq` file holds a jq
program rather than shell, and a `sort` inside a single-quoted region is an argument the
shell never builds a pipeline from — in both, the word is jq's own filter, which orders JSON
values by an order the language defines and no environment variable reaches, so there is no
locale to pin and `LC_ALL=C` in front of it would be nonsense. A commit that adopts the
canonical order lowers the baseline with `scripts/ci/order-check --update`, and the gate
refuses a baseline that no longer matches the tree in either direction, so the debt cannot be
quietly rewritten either way.

The gate runs in the `structure` job for changes to the crate, the scripts, the shell tool
and the site.

## Verification

`test/cases/99_canonical_order.sh` drives the gate against fixture trees rather than against
this checkout — a case that asserted the repository's own counts would be a copy of the
baseline — and writes each violation to see it refused: a folded comparator, a sort key that
renders, a locale comparison, a count that rose, a count that fell, and a tree with no
baseline at all. It also asserts that the word in prose is not the call in code, because the
gate first failed on the documentation of this rule.

`scripts/ci/order-check` on this tree. The order itself is proved where it is declared:
`apps/majordomus-cli/src/order.rs` orders every one of the 5040 permutations of a seven-item
fixture identically, and checks the comparator for antisymmetry and transitivity over every
pair and triple — a comparator that fails either can make `sort` answer differently for a
different input order, which is the failure this rule exists to prevent. The doc examples
run under `cargo test --doc`.
{% endraw %}
