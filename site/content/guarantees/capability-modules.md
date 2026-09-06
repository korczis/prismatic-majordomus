+++
title = "A capability of the Rust executable is declared once and composed into its module, the root composes modules, and the registry refuses a capability outside its module's namespace"
description = "Adding an executable capability is one capability! block in the file of the module it belongs to, with its typed input and output and the input's benchmark cases. The module's module() lists its capabilities; compose_modules! at the root lists the modules and nothing else. MCP, HTTP, OpenAPI, Swagger UI, the command line, the benchmark targets, the cache behaviour, the generated reference and the registry manifest follow from the block; no other file changes. A capability whose id namespace is not its module, a module composed twice, an invalid module id, a cache policy that keeps nothing, a cached command or a benchmark policy that contradicts the kind stops the registry from building, with the id and the provenance named."
weight = 111
[extra]
claim_id = "capability-modules"
status = "guaranteed"
source = "docs/claims/capability-modules.md"
+++
{% raw %}

## What it means

Adding an executable capability is one `capability!` block in the file of the module it belongs to, with its typed input and output and the input's benchmark cases. The module's `module()` lists its capabilities; `compose_modules!` at the root lists the modules and nothing else. MCP, HTTP, OpenAPI, Swagger UI, the command line, the benchmark targets, the cache behaviour, the generated reference and the registry manifest follow from the block; no other file changes. A capability whose id namespace is not its module, a module composed twice, an invalid module id, a cache policy that keeps nothing, a cached command or a benchmark policy that contradicts the kind stops the registry from building, with the id and the provenance named.

## How it works

`apps/majordomus-cli/src/capability/handler.rs` holds `capability!`, which builds the descriptor (id, kind, title, description, schemas from the types, provenance, exposure, stability, tags, benchmark and cache policy) with its handler and the input type's case provider; `capability/module.rs` holds `module!` and `compose_modules!`, plain `macro_rules!` that build values; `capability/builtin/mod.rs` is the one root composition; `capability/registry.rs` keeps the modules (composed, derived from a namespace, or one per declarative kind) and enforces the invariants at build time, before anything serves.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus capabilities validate      # OK   modules … every executable composed in the module its namespace names
apps/majordomus-cli/target/debug/majordomus capabilities describe objects.search --format json | jq '{module, kind, cache, benchmark}'
sed -n '/compose_modules!/p' apps/majordomus-cli/src/capability/builtin/mod.rs
```

## What it does not cover

The shell tool's commands are not capabilities of the Rust executable and keep their own registry (`share/commands.yaml`). Declarative objects are composed by the repository through `sources.yaml`, not by a Rust module; their module is their kind. A new module still needs one name in `compose_modules!`: that is the one manual composition, and it is a module, never a capability.

## Why it exists

The operator's invariant: one canonical declaration, modules compose capabilities, the root composes modules, projections derive everything else. ADR 0004 records it; `project.rust-canonical-declaration` is the rule. `apps/majordomus-cli/tests/registry.rs` proves the module invariants, `tests/projections.rs` that a change to one descriptor reaches every projection, `tests/bench.rs` that it reaches the benchmark targets, and `test/cases/91_canonical_architecture.sh` reads the registry, the generated files and the live executable back against each other in a repository `init` wrote.
{% endraw %}
