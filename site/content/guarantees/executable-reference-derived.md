+++
title = "The site's pages about the Rust executable (one per module, one per capability, the command line, the MCP surface, the benchmarks, the executable's own narrative) are derived from the registry manifest and the registry dataset the executable generates, nothing names a capability by hand, and a capability that joins or leaves the registry gains or loses its page, its index entries and its links from the generators alone"
description = "Nobody writes a page for a capability, a module, an MCP tool or a benchmark target. The executable generates a manifest of its registry (docs/generated/registry.json) and a dataset for the site (site/data/registry/registry.json); the site generator turns the manifest's ids into routes and links, and the templates lay the dataset out. Add a capability to the registry and run just derive: it has a page under /registry/capabilities/, a row on its module's page and on the capabilities index, an anchor on the API reference, a row on the MCP page and on the benchmarks page, and a link to the file it was composed in. Remove it: every one of those goes, and no page is left behind."
weight = 128
[extra]
claim_id = "executable-reference-derived"
status = "guaranteed"
source = "docs/claims/executable-reference-derived.md"
+++
{% raw %}

## What it means

Nobody writes a page for a capability, a module, an MCP tool or a benchmark target. The executable generates a manifest of its registry (`docs/generated/registry.json`) and a dataset for the site (`site/data/registry/registry.json`); the site generator turns the manifest's ids into routes and links, and the templates lay the dataset out. Add a capability to the registry and run `just derive`: it has a page under `/registry/capabilities/`, a row on its module's page and on the capabilities index, an anchor on the API reference, a row on the MCP page and on the benchmarks page, and a link to the file it was composed in. Remove it: every one of those goes, and no page is left behind.

## How it works

`scripts/generate-site-data` reads `docs/generated/registry.json` — a projection that depends on code alone, so the site's derivation never reads anything the index-dependent stage writes — and, through `scripts/lib/executable-site.jq`, writes `site/data/generated/executable.json`: one route per module and per capability (the slug is the id with `.` as `-`, the rule the API reference uses for its anchors), the module of each capability, its operation's anchor when it has an HTTP exposure, its tool name, the file it was composed in as a link into the repository, and the claims that speak about it, attached by the path of each claim's implementation: a claim implemented in the file a module's descriptors live in belongs to that module and its capabilities; one implemented under the MCP, HTTP, benchmark or command-line code to that surface; any other claim implemented in the crate to the executable as a whole. It then writes the content stubs under `site/content/registry/` (deleted and rewritten on every run, never committed) and the executable's own narrative, `apps/majordomus-cli/README.md`, projected as the documents are.

The templates (`site/templates/registry*.html` and the partials they share) read every fact — description, kind, stability, provenance, tags, benchmark and cache policy, every exposure, the input and output schemas, the command line, the tools and resources, the routes, the targets, the coverage, the policy, the baselines — from `site/data/registry/registry.json`, which `majordomus generate site` writes and `generate --check` refuses stale. The manifest is among the site generator's inputs, so its hash moves with the executable's code and a regenerated manifest without a regenerated site is stale to `generate-site-data --check`.

A manifest that names a module the registry does not hold, a source file the tree does not have, or another schema is refused with the reason, and nothing is published.

## How to see it

```bash
just derive                                                   # every derived file, in order
ls site/content/registry/capabilities/                        # one stub per capability of the manifest
jq '.capabilities[] | {id, route, module_route, api_anchor, claims: (.claims | map(.id))}' site/data/generated/executable.json
scripts/site-build && scripts/site-check | grep registry      # every entry a page, no page an orphan, nothing typed by hand
bash test/run.sh 95_executable_reference                      # a capability joins, leaves, breaks
```

## What it does not cover

The Rust side of the chain — that a descriptor changed in code moves the manifest and the dataset — is `site-registry-dataset` and `apps/majordomus-cli/tests/projections.rs`. The narrative pages (`docs/MCP.md`, `docs/CAPABILITIES.md`, the crate README) are canonical prose; the generators project them, they do not derive their sentences. The claims are attached to a surface by the path of their implementation, which is a rule of the site generator, not a field of the registry.

## Why it exists

The site had one page about the registry and a flat table on it; every other fact about the executable — what a command takes, what a tool returns, what the benchmarks measure — lived in prose that restated the code and rotted with it. The rule `project.interfaces-are-projections` says a capability is defined once and every interface is derived; the website is an interface. `test/cases/95_executable_reference.sh` edits the manifest in a fixture and follows one added and one removed capability through the routes, the links and the input hash, and sees three broken manifests refused.
{% endraw %}
