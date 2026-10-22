+++
title = "Written once, derived everywhere"
description = "A capability of this repository is declared in one place and reaches the command line, MCP, HTTP, OpenAPI and this site as projections of that declaration. The projections are committed, regenerated rather than edited, and a commit that leaves them behind is refused. Why, how, and what it costs."
template = "page.html"
[extra]
lang = "en"
related = ["proof-carrying-documentation/", "a-plan-is-attacked-before-it-is-built/", "the-bet-on-a-layer/", "claimed-before-it-is-built/", "one-branch-one-place/", "automation-is-a-capability/", "knowledge-is-derived-a-person-promotes-it/", "a-use-case-is-run-not-described/", "one-source-every-language/"]
[extra.translations]
cs = "napsano-jednou-odvozeno-vsude/"
+++

A tool that offers the same operation on several surfaces tends to describe it several times. The command line has its help text, the HTTP API its route, the OpenAPI document its schema, an MCP server its tool list, and the documentation its own page. Each description is written by hand, and each drifts from the others at its own pace. The first person to notice is usually a user who followed the documentation.

This repository decided that a fact is written once and everything else is derived from it ([ADR 0004](@/adrs/adr-0004.md), accepted). A capability is declared in one place in the Rust executable. Its command, MCP tool, HTTP route, OpenAPI operation and its page on this site are all projections of that declaration. The bootstrap every agent reads says it plainly. An operation is never added or changed by editing a transport registry, an OpenAPI definition or a documentation table: the canonical declaration changes, and the projections are generated again. A semantic definition repeated across projections is called a design defect, not a convenience.

## The projections are committed

Generated files are often kept out of a repository and produced at build time. Here they are committed, and that is deliberate. A reviewer, a session on another machine or this site can read a projection without building anything, and a change shows in its diff exactly what it does to every surface. A pull request that adds one field to an answer also shows the field appearing in the OpenAPI document, the registry and the site's data.

Committing generated files has one well-known danger: they go stale. The rule [project.derived-files-regenerated](@/rules/project-derived-files-regenerated-1.md) answers it directly: "A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together; the fingerprint gate refuses the commit that does not." The commit hook enforces it through `scripts/pages current`. That script compares the input hash recorded with the derived data against the hash of the tree being committed, which takes a fraction of a second. It also runs the executable's own `generate --check`, so the registry's projections are compared by the code that writes them. A commit whose derived data is behind its inputs is refused before it exists.

## One derivation, in order

The projections are produced by one script, `scripts/derive`, which spells out the whole graph in stages:

1. `majordomus generate` writes what depends on code alone: the OpenAPI and registry documents, the command reference, the capability and module pages, the allow-lists and the provider bootstraps.
2. `scripts/generate-site-data` writes the site's data and content and the derived documents, from the canonical files and from the first stage's output.
3. `majordomus generate` runs again over the index, because the documents the second stage wrote are objects the index reads: the site's registry dataset, the benchmark matrix and the manifest of every artifact.
4. `scripts/gitattributes` writes the merge policy from the generators.

No stage reads its own output, and running the whole thing twice changes nothing. That second run is exactly what `scripts/derive-check` compares against, and continuous integration refuses a tree the check refuses.

## One at a time, and nothing behind its back

A derivation is not free. It builds or reuses the executable, reads the whole repository and writes many files, and it takes minutes rather than seconds. Several sessions working on one machine would otherwise derive at the same time and starve each other. The rule [project.one-derive-at-a-time](@/rules/project-one-derive-at-a-time-1.md) makes it a queue. `scripts/derive` holds the machine's lock for its whole run, waits for a live owner, yields to a priority marker that names another branch, and takes its whole process tree down when it is stopped.

A second rule closes a quieter gap. A derivation projects only what git tracks, so a new file nobody has added would be invisible to it, and the commit that adds the file would then be refused for derived data that does not match. `scripts/derive` therefore refuses an untracked, unignored file before it waits for the lock, and names it.

## Merging derived files

Two branches that both regenerate the same derived file conflict in places no person should resolve by hand. The repository declares a merge driver for derived paths. It resolves such a conflict to one side, says the result is now stale, and leaves the real resolution to the next derivation. The merge commit is then derived once, over the merged sources, rather than stitched together line by line from two stale outputs.

## What it costs

The price is real. Every change to a declaration costs a derivation, a derivation waits its turn on a busy machine, and the generated files make diffs longer. A session that forgets to stage a new file is refused, and one that edits a generated file by hand is refused again. The repository's answer is that each of these refusals is cheaper than the alternative, which is a published surface that says something the code does not.

The architecture is [ADR 0004](@/adrs/adr-0004.md). The capability model is described in [the capabilities manual](@/docs/capabilities.md), and the path from the sources to this site in [the architecture of the published site](@/docs/github-pages-architecture.md).
