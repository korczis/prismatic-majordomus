---
schema: adr/v1
id: adr-0086
kind: adr
title: The crate's rustdoc is a published web surface at /rustdoc, produced by the invocation its gate proves and verified at the public URL
status: proposed
date: 2026-09-24
tags:
  - rust
  - documentation
  - web
  - publication
  - ci
related:
  - rule:project.the-crate-reference-is-published
  - rule:project.web-surface-declared-once
  - rule:project.land-and-publish
  - rule:project.every-link-and-control-is-tested
  - rule:project.rust-public-api-quality
  - rule:project.empty-is-not-failure
  - claim:rustdoc-discoverable
  - claim:rustdoc-composed-from-the-topology
  - claim:rustdoc-published
  - claim:rustdoc-complete
  - claim:rustdoc-verified-live
  - file:docs/RUSTDOC.md
  - file:docs/WEB.md
  - file:docs/GITHUB_PAGES_ARCHITECTURE.md
  - file:docs/GITHUB_PAGES_PERFORMANCE.md
  - file:scripts/rust-check
  - file:scripts/site-build
  - file:scripts/site-check
  - file:scripts/ci/link-check
  - file:lib/common.sh
  - file:scripts/pages
  - file:scripts/ci/pages-check
  - file:.github/workflows/pages.yml
  - file:apps/majordomus-cli/src/web/discover.rs
  - file:apps/majordomus-cli/src/quality/source.rs
  - file:.ai/repo/adrs/0013-every-web-surface-is-discovered-from-its-producer-resolved-o.md
  - file:.ai/repo/adrs/0028-the-crates-public-surface-is-measured-by-the-crate-and-absence.md
  - file:.ai/repo/adrs/0051-the-minimum-release-version-is-measured-from-the-public-contract.md
  - test:test/cases/77_rust_evidence.sh
provenance:
  origin: authored
---

# 86. The crate's rustdoc is a published web surface, produced by the invocation its gate proves and verified at the public URL

## Context

The owner's mission of 2026-09-24 asked for ExDoc as a first-class surface: the generated API
reference treated as part of documentation closure, produced, embedded, routed, linked,
tested, deployed and verified where the public reads it. ExDoc is Elixir's. This repository
has no Elixir: it is one Rust crate, `apps/majordomus-cli`, shell, jq and awk scripts, and a
Zola site. The equivalent is rustdoc — the crate's own `//!` and `///` documentation rendered
by the toolchain, one page per exported item, with the source each page was built from.

The quality of what would be published already had an owner. `#![warn(missing_docs)]` in
`lib.rs` with `-D warnings` in the gate makes an undocumented export a build failure, and
`project.rust-public-api-quality` (ADR 0028) measures examples and thin documentation from the
crate's syntax tree, ratcheted by `.ai/repo/rust-quality-baseline.txt`. What was missing was
not documentation. It was every stage after the renderer. Audited on 2026-09-24:

- **The output was thrown away.** `cargo doc` ran only as a gate — the `cargo doc -D warnings`
  step of `scripts/rust-check`, `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --quiet` — and
  nothing read what it wrote. The gate proved on every run that the reference rendered without
  a warning, and not one reader could open it.
- **No surface declared it.** `web::discover` knew the executable's routes, the site and the
  generated reports under `target/web/`; the rustdoc tree was none of them, so the router could
  not serve it, the home page could not link it and `docs/generated/web.json` did not name it.
- **No site step carried it.** `scripts/site-build` built Zola into `site/public` and nothing
  else went in.
- **The publication job had no Rust.** `.github/workflows/pages.yml` installs Node and Zola;
  it could not have produced the tree had it been asked.
- **Verification could not see it.** `scripts/pages verify` reads `/build.json` and nothing
  else, so a deployment that lost a whole subtree would have verified.
- **No identity joined an item to a page.** Nothing could say "this exported item has no
  page", because nothing mapped an item to the route rustdoc gives it.
- **Live, every candidate route answered 404** at `https://majordomus.dev`.

This is the failure ADR 0013 describes for a surface known in six places, one step further:
here no place knew it, and the one check that touched it proved a property of an artifact it
then discarded. Generation succeeding had been read, implicitly, as the reference existing.

## Decision

The crate's rustdoc is a web surface with the id `rustdoc`, mounted at `/rustdoc`, available
in both worlds — served by `majordomus serve` and published at
`https://majordomus.dev/rustdoc/` — and every stage between the source and the public URL is
decided by an executable check. `project.the-crate-reference-is-published` is the rule;
`docs/RUSTDOC.md` is the operating document.

| stage | what holds | decided by |
|---|---|---|
| source | every exported item documented; examples per the quality ratchet | `cargo doc -D warnings`, the `rust-quality` gate |
| generation | one producer, the gate's own invocation | `scripts/rust-check --doc`; case 77 holds the single invocation |
| embedding | the surface declared by discovery, checkout-independent | `majordomus web validate`, `generate --check` |
| routing | `/rustdoc/` served from the tree, published at the same mount | `web::files`, the router, case 488 |
| references | every exported item has its page, no orphan, internal links resolve | `majordomus quality rustdoc`, `scripts/ci/link-check` |
| deployment | the tree composed from the topology, never committed, built from the build's commit | `scripts/site-build`, `scripts/site-check` section 13m, `scripts/site-deploy`, cases 486 and 489 |
| deployed verification | the public reference names the deployed commit and is whole | `scripts/pages verify-rustdoc`, the `pages-live` gate, case 487 |

### Its own top-level mount

`/rustdoc`, not a path under an existing namespace. `/docs` belongs to the served `docs`
surface, and `web::validate` refuses a nested mount the outer surface did not declare; `/api`
would sit on top of `/api/v1`, the capability routes; "reference" already names several things
here (`scripts/ci/reference-check`, `/docs/api/`, `/docs/cli/` and `/registry/`). The surface
is named after its producer, as `swagger` is.

### Declared by discovery from the crate

The surface is declared by discovery from the crate itself, the way `docs` is declared from
`site/config.toml`: it is in the topology whenever the crate is, whether or not the producer
has run in this checkout. So the topology, the home page and the committed
`docs/generated/web.json` never depend on a build having happened. The walk over
`target/web/` skips the `rustdoc` directory as it skips `docs`, `built_from` is read from the
producer's `surface.json`, and the committed projection omits it exactly as it omits the
documentation build's, because a value that differs per checkout would make every clone report
drift.

### The producer is the gate's own invocation

`scripts/rust-check --doc` runs the `cargo doc -D warnings` step — the same line the full and
CI modes run, not a second variant — and hands its output off: the pages are copied from the
directory cargo reports as its target into `target/web/rustdoc/`, beside a `surface.json`
(`web-surface/v1`: id, mount, title, producer, availability `both`, category
`documentation`, visibility `public`, `built_from` the checkout's `HEAD`) and a landing
`index.html`, because rustdoc writes an index per crate and none at the root. The library
crate's page is `/rustdoc/majordomus_cli/index.html`.

What is documented is the library's public items and the executable, with rustdoc's own
source pages kept. A reference built by a different invocation from the one the gate checks is
a second artifact nobody proved, so there is one `cargo doc` line and case 77 fails a second.
`--document-private-items` is not passed: ADR 0028 decides what the library's public surface
is, and publishing the private one would state the opposite. The executable is documented as
cargo documents any binary target — with its own items, since a binary exports nothing — which
today is its `main` and nothing else. The source pages are the source at exactly the commit
the tree was built from, which is the strongest source link a reference can carry.

### Not committed; generated in CI and proven fresh

The producer's tree measured 81 MB in 3083 files on 2026-09-24 at `f7e05f4c5` — the pages
rustdoc wrote, the landing page and `surface.json`, summed by byte size — and the pages embed
the values of the crate's `COMMIT` and `GENERATION` constants: rustdoc renders
`pub const COMMIT: &str = "<forty hex digits>";` on the constant's page. Committed, the tree
would be stale on every commit that touched the crate, and on every commit at all through that
one page, and would put its whole size into every clone. It is generated in CI instead: by a
`rustdoc` job of its own in `pages.yml`, not by steps of the deploy job, which hands the tree to
the deploy job as an artifact of the same run; and the validation workflow's jobs that build the
site obtain it through the same composite action, `.github/actions/rustdoc`, so the two
workflows cannot build it differently. The deploy job needs the `rustdoc` job, so the two run
one after the other: the site's build composes the tree and the site's checks judge the
composed output, so neither can start before the tree exists.

Freshness has two witnesses that do not share a writer. `built_from` in `surface.json` is the
producer's statement of the commit; the crate's own page for its `COMMIT` constant is what the
compiler embedded when the pages were built. Both must name the commit being checked, and the
public verification reads both.

The second witness has a precondition. `COMMIT` is what `build.rs` read when it last ran, and
`build.rs` declares its reruns: the crate's sources, its manifest, its lock file and
`MAJORDOMUS_BUILD_COMMIT` — not `HEAD`. After a commit that touches none of those, a plain
`cargo doc` is fresh, rustdoc does not run, and the constant page still names the earlier
commit while a producer that reads `HEAD` records the new one. That was measured on
2026-09-24 with rustdoc 1.98.1, on a fixture crate carrying the same rerun declarations. So the
producer hands the build the commit it records — `MAJORDOMUS_BUILD_COMMIT`, which `build.rs`
already honours — and both witnesses are written from one value by one build. Two witnesses
that disagree then mean one thing: the tree was assembled from two builds.

### Composition is derived from the topology

`scripts/site-build` copies every published static surface other than the site itself into its
output at the surface's mount, reading the list from `docs/generated/web.json`. There is no
line naming `rustdoc` in it: a surface added tomorrow is composed tomorrow. A published surface
whose artifact is absent refuses the build with exit 12 and names the producer that writes it,
so a site cannot be published missing a surface the topology says it carries. A mount the site
itself already fills is refused with exit 10 rather than overwritten: one path has one owner.
The build records every composed surface, with the commit its producer declared, under
`surfaces` in `/build.json`, and `scripts/site-check` (section 13m) refuses a composed surface
that is missing, carries no declaration, declares another id or mount than the topology, or was
built from another commit than the build.

The selection is one helper, `mj_web_composed` in `lib/common.sh`, over the committed
`web.json`: every surface of kind static directory that the publication carries, other than the
site. The build that composes and every check that must tell the site's pages from a composed
surface's ask it, so no script names a mount.

### Each surface's pages are judged by that surface's check

The site's page contract — landmarks, metadata, no inline style, the design tokens — judges the
pages Zola renders. Pages under another surface's mount, derived from `web.json`, are pruned
from that contract, because rustdoc's markup is the toolchain's and no template here can change
it; they are judged by `majordomus quality rustdoc` instead. A link from a site page into
`/rustdoc/` is decided by `scripts/ci/link-check` against the composed tree, as an internal
file like any other.

### The integrity check joins the canonical inventory

`majordomus quality rustdoc [--tree DIR]`, the capability `quality.rustdoc`, is Rust and reads
the rendered tree against `quality::source::Inventory` — the measurement of the crate's
exported items that `project.rust-public-api-quality` already uses. It requires that every
exported item has its page at the route derived from its path and kind, that no item page
exists without an item, that the tree declares itself and that its `built_from` is `HEAD`,
that the crate index carries its identity (`<title>majordomus_cli - Rust</title>`), that the
index's assets are present, that internal links resolve, and that no page names a machine path
or a secret. It exits 0 clean, 10 with findings and 12 when there is no tree to judge — never
an empty pass. The inventory is not a list kept for this consumer: it is the crate measuring
itself, and the routes are derived from it by the toolchain's own convention. The executable's
pages are judged by the same walk over the binary target's own inventory.

### Public verification is enforced

`scripts/pages verify-rustdoc --commit SHA [--url URL] [--timeout N]` is a function of its own
beside `verify`, not a widening of it. It runs as a hard step of `pages.yml` once the site
serves the deployed commit, and as a section of `scripts/ci/pages-check`, the `pages-live`
gate. A deployment whose public reference is absent, broken or built from another commit fails.

That is a deliberate exception to the principle that nothing measured after the push to
`gh-pages` becomes the run's verdict (`docs/GITHUB_PAGES_PERFORMANCE.md`), and it is consistent
with the reason for that principle. The principle exists because a latency breach on a
deployment that happened is not a failed deployment. A reference that is broken at its public
URL is one — the same category as GitHub's own build of `gh-pages` erroring, which that
workflow already makes red.

### What does not change

The coverage policy. No threshold is invented: `missing_docs` with `-D warnings` already
requires every exported item to be documented, and the `rust-quality` ratchet holds examples
and thin documentation. `quality rustdoc` reports the counts it joins and asks for nothing the
existing policy does not.

The contract. Publishing rustdoc does not make the crate's Rust API a compatibility promise.
The crate is `publish = false`, and ADR 0051 measures the public contract from the capability
registry. The landing page says so in words.

## Alternatives rejected

*Commit the tree.* Its size, and the constants it embeds, make it a file that is stale the
moment the crate moves and a weight on every clone that never reads it. A committed tree is
also one more derived artifact to prove current, when the deployed commit and the constant page
prove it more directly.

*Mount it at `/docs/rust`.* `/docs` is the served documentation's, and a nested mount the outer
surface did not declare is refused by `web::validate` and by router construction. Declaring the
nesting would give the site's build a hole its own source knows nothing about, and would put
the reference inside the one namespace this repository has already fought over once.

*`majordomus web compose` on the deploy path.* It composes from the resolved topology and is
the right tool where the executable is at hand, but the publication job carries no Rust
toolchain and should not build the executable to copy directories. `site-build` reads the
committed `web.json`, which is the same resolution already projected and drift-checked by
`generate --check`, so nothing is decided twice.

*rustdoc's JSON output as the inventory.* `--output-format json` is unstable and needs a
nightly toolchain; this repository pins a stable one in `rust-toolchain.toml`. It would also be
a second invocation of rustdoc, and the check would then be judging a different run from the
one it publishes. The crate's own syntax-tree inventory is stable and is already the measure of
the public surface.

*`--document-private-items`.* It publishes what ADR 0028 decided is not the surface, and it
would make the rendered item set disagree with the inventory the check joins it against.

*`cargo doc` inside the deploy job.* It puts a Rust toolchain install, the crate's build cache
and a documentation build inside the job whose phases `.ai/repo/ci/pages.yaml` budgets, in
series with every one of them. A job of its own is not faster — the deploy job needs it, so the
two still run in series — but it keeps the Rust toolchain and its build cache out of the deploy
job, keys that cache by its own job so it restores a documentation build's cache rather than the
validation workflow's, makes the reference's cost a row of its own in the measured publication,
and runs the same action the validation workflow runs.

*A separate verifier script.* A new shell unit for one function that belongs beside `verify`,
reads the same model and shares its polling. `verify-rustdoc` is a subcommand of
`scripts/pages`, and the gate reads it through `pages-check` rather than through a second
entry point.

## Consequences

- **gh-pages grows by the tree once, then by its differences.** The first deployment adds the
  whole tree to the branch — about 81 MB at the measurement above, before git compresses it;
  later ones add what changed — the pages of items that changed, the constant pages, the
  search index, the landing page. Unchanged pages are the same blobs, so git stores them once.
  `scripts/site-deploy` fetches the one branch it pushes, and that fetch grows with it.
- **Publication waits for the reference.** The deploy job needs the `rustdoc` job, so the time
  from push to public grows by that job's whole duration, the fetch of its tree, and GitHub's
  queue between the two jobs. The reference's share is a row of its own among the phases the
  deploy job measures — the job's wall time and the fetch — and the queue between the jobs is
  reported as queue, because it is GitHub's. It is measured by the run that pays it, not
  estimated here; `.ai/repo/ci/pages.yaml` budgets the controlled phases, and a row it does
  not name cannot be judged.
- **A toolchain bump can move routes.** rustdoc's layout, its static file names and its route
  conventions belong to the toolchain. Raising the pin in `rust-toolchain.toml` can change
  them, and `quality rustdoc` fails before the public sees a broken page; the route derivation
  is corrected in the same change as the bump.
- **`link-check`'s subject changes.** It now decides links from the site's pages into
  `/rustdoc/` against the composed tree, and it does not walk the rustdoc pages themselves,
  whose internal links `quality rustdoc` decides. Its denominator is the site's pages, and it
  says so.
- **A site build needs every published surface's artifact.** Locally, `scripts/site-build`
  refuses until `scripts/rust-check --doc` (or `just rustdoc`) has run, and names that command.
  In CI, every job that builds the site obtains the tree first.
- **The crate's sources can change the published bytes.** A change to the crate changes the
  reference, so the paths that start a publication must include what produces it: the crate's
  sources, manifest, lock file and build script, the toolchain pin, the producer and its
  action. They are derived, as every trigger path is, by `scripts/pages paths` from the gate
  model and the publication model, never written into the workflow, and case 97 holds the
  workflow's trigger to that derivation. A trigger that misses them leaves the public
  reference at the last deployment while master moves on, and nothing reports it as owed,
  because the `pages-live` gate decides what is owed from the same paths.
