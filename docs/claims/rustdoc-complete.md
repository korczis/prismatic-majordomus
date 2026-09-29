# Every exported item has its rustdoc page at its derived route, and no page is an orphan

## What it means

The published reference is complete with respect to the crate: every item the crate exports has
the page rustdoc gives it, at the route derived from the item's path and kind, and no item page
exists for an item the crate does not export. A module added to the crate becomes a page the
check requires without anybody editing a list, and a module removed takes its pages with it.

## How it works

`majordomus quality rustdoc` reads the rendered tree against `quality::source::Inventory` — the
crate read as a syntax tree, walked from its root as the compiler walks it, which is the same
measurement `project.rust-public-api-quality` holds the crate to. From each exported item's path
and kind it derives the route rustdoc uses, and it compares the two sets both ways: an item without
its page is a finding, and so is an item page without its item. The same pass checks that the tree
was built from `HEAD`, that the library's index carries its identity, that the index's assets are
present, that internal links resolve, and that no page names a machine path or a secret.

It exits `0` clean, `10` with one finding per violation, and `12` when there is no tree to judge.
It never reports a pass over an empty tree.

## How to see it

```bash
scripts/rust-check --doc
majordomus quality rustdoc                   # the counts it joined, then any finding
majordomus quality rustdoc --tree site/public/rustdoc
bash test/run.sh 490_rustdoc_new_module
cargo test --manifest-path apps/majordomus-cli/Cargo.toml quality::rustdoc
```

## What it does not cover

Whether an item's documentation is worth reading is `project.rust-public-api-quality` and the
`rust-quality` gate; this claim is about the page existing where it should. Private items are not
documented and are not expected. The route convention belongs to the toolchain pinned in
`rust-toolchain.toml`; a bump that changes it is caught here as findings and corrected in the same
change.

## Why it exists

A reference is a large set, and the tempting way to check a large set is a list. A list of modules
kept for the reference goes stale in the direction that passes: the new module is not on it, so
its missing page is never a finding. Joining the tree with the crate's own inventory means the set
the check expects is derived from the code every time. `test/cases/490_rustdoc_new_module.sh` adds
a module to a fixture crate and watches it become a required page.
