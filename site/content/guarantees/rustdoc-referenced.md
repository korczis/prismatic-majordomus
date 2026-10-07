+++
title = "Every builtin module's page on the site links the page the crate's reference documents that module on, from the one module-to-page mapping the rustdoc check holds the published tree to, and only in a build that composed the reference"
description = "Each builtin module has a page on the site, under /registry/modules/, and each module the crate"
weight = 185
[extra]
claim_id = "rustdoc-referenced"
status = "guaranteed"
source = "docs/claims/rustdoc-referenced.md"
+++
{% raw %}

## What it means

Each builtin module has a page on the site, under `/registry/modules/`, and each module the crate
exports has a page in the crate's reference, under `/rustdoc/`. The first links the second: the
module page's provenance names the module's crate path as "crate reference" and links the page
rustdoc documents it on. A module rustdoc gives no page — one declared `pub(crate)` or private —
links nothing, and a build that did not compose the reference links nothing either, so a link on a
module page is never a link to a page that is not there.

## How it works

The mapping from a module to its rustdoc page is decided once, where the reference is checked.
`quality::rustdoc::module_routes` reads the crate's own syntax-tree inventory
(`quality::source::Inventory`), keeps the modules the crate exports, and derives each one's page by
rustdoc's route rule — the same derivation `majordomus quality rustdoc` holds the published tree to,
where a module without its page is a `missing-page` finding. `majordomus generate` carries that
answer into the site's registry dataset (`site/data/registry/registry.json`) as `rustdoc` on each
builtin module: the surface the page is published under, the module's crate path and the page
within the surface. It is read from the crate's source, so the dataset is the same on every machine
whether or not the tree has been built.

The template, `site/templates/registry-module.html`, joins that with the surfaces the build
composed: `scripts/site-build` records each one, with its mount, in `data/build.json`, from the
selection `mj_web_composed` reads out of `docs/generated/web.json`. The link is the composed
surface's mount and the page, through `get_url`, so a build for another base path keeps working.
The template derives no route, and names no surface of its own.

## How to see it

```bash
scripts/rust-check --doc
majordomus quality rustdoc --format json | jq '.modules[] | select(.path | test("builtin::quality$"))'
jq '.registry.modules[] | select(.id == "quality") | .rustdoc' site/data/registry/registry.json
scripts/site-build && grep -o 'crate reference.*quality/index.html' site/public/registry/modules/quality/index.html
bash test/run.sh 492_rustdoc_module_links
cargo test --manifest-path apps/majordomus-cli/Cargo.toml site::tests quality::rustdoc
```

## What it does not cover

The Cockpit's module pages are another surface with their own renderer; this claim is about the
published site. Whether the linked page is complete is [`rustdoc-complete`](@/guarantees/rustdoc-complete.md),
and whether it is this commit's is [`rustdoc-verified-live`](@/guarantees/rustdoc-verified-live.md). A module
page links the module's own page, not the pages of the items in it: rustdoc's page lists them.

## Why it exists

The crate's reference was published with no way in from the pages that describe the same code. The
tempting fix is a line in the template that turns `src/capability/builtin/<x>.rs` into
`majordomus_cli/capability/builtin/<x>/index.html`, and that line is a second statement of the route
rule the rustdoc check already owns. It is also wrong for every module the crate does not export:
rustdoc writes no page for a `pub(crate)` module, so that rule would link a 404 from every module
page whose module the crate keeps to itself. `test/cases/492_rustdoc_module_links.sh` reads every module page of a real build
against the check's own answer, and builds again without the reference to see every link go.
{% endraw %}
