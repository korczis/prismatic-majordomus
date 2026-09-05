# Every capability of the Rust executable is defined once, and the registry refuses a duplicate id or a colliding projection name, naming both parties

## What it means

The Rust executable has one place where a capability exists: the registry composed in `apps/majordomus-cli/src/app.rs` from the executables listed in `capability/builtin.rs` and from every object of the repository's layer. Two definitions claiming one canonical id, two capabilities claiming one MCP tool name or resource URI, one HTTP method and path, or one CLI path, a route outside `/api/v1/`, a tool name outside `[a-z0-9_]+`, or an executable exposure on a planned capability: the registry does not build, every violation is listed with the provenance of both parties, and no interface is served.

## How it works

`capability/registry.rs` sorts the pending descriptors by id and provenance, validates each against what it already holds, collects every error, and either returns the registry or the list. `majordomus capabilities validate` runs the same build and prints one `OK` line per invariant or exits 10 with the list; `mcp`, `serve` and `generate` refuse to start on the same errors.

## How to see it

```bash
majordomus init && git add -A && git commit -qm install
apps/majordomus-cli/target/debug/majordomus capabilities validate
```

prints the registry, MCP, HTTP and OpenAPI lines, each `OK`. A duplicate is shown by the crate's own suite (`cargo test --test registry`), which builds registries with the same id from two Rust sources and from Rust and the layer and reads both provenances out of the error.

## What it does not cover

Uniqueness is per registry, so per repository and per process: two repositories may each declare `rule.project.alpha@1`. The registry checks names and shapes, not the truth of a description.

## Why it exists

Four interfaces over one behaviour, each with its own table, drift on the first edit; a registry that refuses to build on a collision makes the drift impossible rather than merely detectable. `test/cases/76_capabilities_projections.sh` validates the registry over a repository the shell tool's `init` wrote; `apps/majordomus-cli/tests/registry.rs` covers every collision and refusal.
