---
id: project.the-crate-reference-is-published
version: 1
kind: rule
title: Generated reference documentation is part of documentation closure, from its source to its public URL
description: The crate's rustdoc is produced by the invocation its gate proves, declared as a web surface by discovery, composed into the publication from the topology, checked item by item against the crate's own inventory, deployed with the site and verified at the public URL against the commit it was built from; that generation succeeded proves none of the stages after it.
statement: Generated reference documentation is part of documentation closure — its source, generation, embedding, routing, references, tests, deployment and deployed verification are each valid and each decided by an executable check; its inventories and routes are derived from canonical metadata, and no list kept for one consumer exists unless it is generated and drift-checked; and a generation that succeeded is never taken as evidence that what was deployed is correct.
status: active
class: blocking
depends_on: [project.web-surface-declared-once@1, project.land-and-publish@1, project.every-link-and-control-is-tested@1, project.rust-public-api-quality@1, project.empty-is-not-failure@1]
tags: [rust, documentation, web, publication, testing]

x-majordomus:
  tests: [test/cases/486_rustdoc_composition.sh, test/cases/487_rustdoc_staleness.sh, test/cases/488_rustdoc_discoverable.sh, test/cases/489_rustdoc_deploy.sh, test/cases/490_rustdoc_new_module.sh, apps/majordomus-cli/tests/http_serve.rs, scripts/rust-check, scripts/site-build, scripts/site-check, scripts/ci/link-check, scripts/pages, scripts/ci/pages-check]
---

# Rationale

For as long as this repository had a Rust crate, its gate rendered the crate's reference
documentation on every run and threw the result away. `scripts/rust-check` ran
`RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --quiet`, proved that every exported item was
documented and that the pages rendered without a warning, and left the pages where cargo wrote
them. No web surface declared them, no build step carried them, the publication job had no
Rust toolchain, the public verification read one file that could not tell whether they were
there, and nothing mapped an item to the page rustdoc gives it. When the owner asked on
2026-09-24 where the reference was, every candidate route of the public site answered 404.

Nothing in that list is a defect of the documentation. Each is a stage after the renderer
that nothing owned, and the gate's green was read — by nobody in particular, which is how it
happens — as though the reference existed. A check at one stage says nothing about the next
seven. That is the general shape this rule refuses, and generated reference documentation is
where this repository met it: a closure is valid only when every stage is decided, and the
stage with the most evidence behind it is usually the first.

The second half is how each stage is decided. A reference is a large set — every item, every
page, every link — and the tempting way to check a large set is a list. A list of modules kept
for the reference's check goes stale the first time the crate gains one, and it goes stale in
the direction that passes: the new module is not on the list, so its missing page is not a
finding. The sets are therefore derived: the items from the crate's own measurement of itself,
the routes from the items by the toolchain's convention, the mount from the surface's
declaration, and what the publication carries from the resolved topology.

# Required behaviour

Every stage of the closure holds, and each is decided by something executable.

1. **Source.** Every exported item is documented, and carries what
   `project.rust-public-api-quality` asks of it. This rule adds no threshold: `missing_docs`
   with `-D warnings` and the `rust-quality` ratchet are the coverage policy.
2. **Generation.** One producer, `scripts/rust-check --doc`, runs the same `cargo doc`
   invocation the gate proves warning-free — there is exactly one such line — and writes
   `target/web/rustdoc/` with a `surface.json` that records the commit it was built from and a
   landing page. No second invocation exists for publication and the library's private items
   are not documented, because what is published must be what the gate proved and what ADR 0028
   decides is public; the executable is documented as cargo documents any binary target.
3. **Embedding.** The surface `rustdoc` is declared by discovery from the crate, in the
   topology whether or not the producer has run, mounted at `/rustdoc` and available both to a
   running process and to the publication. The committed topology projection carries no value
   that differs between checkouts.
4. **Routing.** `majordomus serve` answers `/rustdoc/` from the tree, and the publication
   carries the tree at the same mount. No path to it is written anywhere but its declaration.
5. **References.** Every exported item has its page at the route derived from its path and
   kind, no item page exists without its item, every internal link of the tree resolves, and a
   link from a site page into `/rustdoc/` resolves against the composed tree.
6. **Tests.** Each stage has a case that breaks it and watches the refusal. A case that cannot
   reach its subject says so rather than passing.
7. **Deployment.** The publication is composed from the committed topology: every published
   static surface is copied to its mount, with no line naming this one. A published surface
   whose artifact is absent refuses the build and names its producer, and a composed surface
   built from another commit than the build is refused before anything is pushed. The tree is
   produced in CI and never committed.
8. **Deployed verification.** Once the site serves the deployed commit, the public reference is
   verified against it: its declaration and the crate's own `COMMIT` page name that commit, and
   the crate's index answers with its identity. A deployment that fails this is a failed
   deployment.

Inventories and routes come from canonical metadata. The item inventory is
`quality::source::Inventory`, the crate's syntax-tree measurement; the routes are derived from
it; the mount is the surface's declaration; the composition is `docs/generated/web.json`. A
list of modules, items, pages or routes written for one consumer is a defect regardless of
whether it currently agrees, unless it is generated and a check refuses its drift.

A generation that succeeded is not evidence of a deployment. The `cargo doc` step going green
says the pages render; it says nothing about whether they are served, composed, complete,
current or public, and no report may present it as though it did.

# Failure behaviour

- `majordomus quality rustdoc` exits `10` with one finding per violation — an item without its
  page, a page without its item, a tree built from another commit or declaring none, a crate
  index without its identity, an index asset missing, an internal link that does not resolve, a
  machine path or a secret in a page — each naming the item or page and the remedy. It exits
  `12` when there is no tree to judge, which is never reported as a pass over nothing
  (`project.empty-is-not-failure`).
- `scripts/site-build` exits `12` when a surface the topology publishes has no artifact, naming
  the producer that writes it, and `10` when the site itself already fills the surface's mount.
- `scripts/site-check` fails (section 13m) when a composed surface is missing from the build,
  carries no declaration, declares another id or mount than the topology, or was built from a
  commit other than the build's, and when `/build.json` records it differently.
- `scripts/ci/link-check` exits `10` for a link from a site page into `/rustdoc/` that does not
  resolve in the composed tree.
- `scripts/pages verify-rustdoc` fails when the public reference is absent, broken or built
  from a commit other than the one deployed. In `pages.yml` it is a hard step, so the
  deployment run is red; in `scripts/ci/pages-check` it is a section of the `pages-live` gate,
  which asks again on the next full validation and at `finish`.
- `majordomus web validate`, the `web-topology` gate, refuses a topology in which the surface
  collides with another or nests inside one.

The gates are `rust-check` (whose rustdoc step denies warnings and produces the tree),
`rustdoc` (the integrity check over the tree that run produced, which it requires; both in the
`rust` job), `site-build` (composition and the static checks over the composed tree),
`web-topology`, and `pages-live`.

# Verification

`test/cases/486_rustdoc_composition.sh` through `test/cases/490_rustdoc_new_module.sh`, one
stage each:

- 486: composition — the site build carries the reference at its mount from the topology, and
  refuses, naming the producer, when the artifact is absent.
- 487: staleness — a tree built from another commit is refused by the integrity check and by
  the public verification.
- 488: discoverable and reachable — the surface is in the topology and its projections whether
  or not the producer ran, and a served process answers `/rustdoc/`.
- 489: deployment — the composed tree is deployed to a bare remote and the published branch
  carries the reference built from the commit the deployment names.
- 490: completeness — a module added to a fixture crate becomes a required page without a list
  being edited, and its absence is a finding.

The crate's own suites hold the two Rust halves:
`cargo test --manifest-path apps/majordomus-cli/Cargo.toml quality::rustdoc` for the integrity
check — a clean tree has no finding and each way of failing produces its own — and
`cargo test --manifest-path apps/majordomus-cli/Cargo.toml web::discover` for the declaration —
present before the producer runs, checkout-independent in the committed projection — and
`apps/majordomus-cli/tests/http_serve.rs` for the tree served whole over a real socket.

`scripts/rust-check`, `scripts/site-build`, `scripts/site-check`, `scripts/ci/link-check`,
`scripts/pages` and `scripts/ci/pages-check` are the executable stages the cases break. The
decision is ADR 0086; the operating document is `docs/RUSTDOC.md`.
