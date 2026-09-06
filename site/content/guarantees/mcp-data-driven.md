+++
title = "What the MCP server serves is decided by the manifest, the declared source classes and each file's front matter, so an object added to the layer is served with no change to the executable"
description = "The executable names no repository file beyond the two conventions the layer documents, .ai/manifest.yaml and sources.yaml under the knowledge section. The manifest says which sections exist, sources.yaml says which files carry which kind, the same share/allow/<kind>.txt the shell tool uses says which keys a kind may carry, and the executable's embedded schema/kinds.yaml says how a kind is read. Write a rule under .ai/repo/rules/project/, track it, restart the server, and it is majordomus://rule/<id>@<version> with its own provenance. Declare a new class of a known kind in sources.yaml and it is served too."
weight = 97
[extra]
claim_id = "mcp-data-driven"
status = "guaranteed"
source = "docs/claims/mcp-data-driven.md"
+++
{% raw %}

## What it means

The executable names no repository file beyond the two conventions the layer documents, `.ai/manifest.yaml` and `sources.yaml` under the `knowledge` section. The manifest says which sections exist, `sources.yaml` says which files carry which kind, the same `share/allow/<kind>.txt` the shell tool uses says which keys a kind may carry, and the executable's embedded `schema/kinds.yaml` says how a kind is read. Write a rule under `.ai/repo/rules/project/`, track it, restart the server, and it is `majordomus://rule/<id>@<version>` with its own provenance. Declare a new class of a known kind in `sources.yaml` and it is served too.

## How it works

`src/discovery/mod.rs` reads the classes and enumerates each pathspec through `git ls-files`, sorted; `src/index.rs` reads each file by its kind's format, validates its keys against the allow-list, takes identity and title from the fields the schema names, and records path, directory, class and section. Nothing in the code branches on a path, a file name or a provider name.

## How to see it

```bash
majordomus init && git add -A && git commit -qm install
apps/majordomus-cli/target/debug/majordomus mcp --inspect | grep -c '^resource    majordomus://rule/'
cat > .ai/repo/rules/project/example.v1.md <<'R'
---
id: project.example
version: 1
kind: rule
title: Example
description: An example.
statement: Do the example thing.
status: active
class: advisory
depends_on: []
tags: []
---
R
git add -A
apps/majordomus-cli/target/debug/majordomus mcp --inspect | grep '^resource    majordomus://rule/project.example@1'
```

The count grows by one and the new URI is listed; delete the file and it is gone.

## What it does not cover

A kind the executable does not read (a `kind:` in `sources.yaml` with no entry in `schema/kinds.yaml`) is reported on every file of that class and served for none: data describes objects, it does not define how a new kind is read. Discovery follows the layer's contract and goes through the git index, so an untracked file is not served unless `--discovery filesystem` is asked for. Hierarchical context files are served as documents with their directory recorded and are not merged, because the repository defines no merge semantics.

## Why it exists

A table of resources written into the code would drift from the layer on the first new rule, which is the second source of truth `28_no_hardcoded_values.sh` exists to prevent. `apps/majordomus-cli/tests/external_extension.rs` runs the add, remove and break sequence through the built binary with no Rust change; `test/cases/72_rust_mcp.sh` proves a rule added to the layer `init` writes is served.
{% endraw %}
