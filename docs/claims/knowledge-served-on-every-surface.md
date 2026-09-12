# The candidates awaiting review, one record by id with its references resolved, and the derivation status of the checkout answer with the same content over the command line, HTTP and MCP

## What it means

Three capabilities of the Rust executable serve the knowledge base: `knowledge_base.candidates` lists the records awaiting review with their branch, freshness and the policy cap; `knowledge_base.record` serves one record by id with every `derived_from` and `relations` reference resolved to an object of the index, an external fact of the ledger or git, or missing; `knowledge_base.status` reports the last derivation, the newest close, the freshness against the policy and the stopped-writer judgement. Each is declared once and answers identically over `bin/majordomus-cli knowledge candidates|record|status`, `GET /api/v1/knowledge/...`, the MCP tools `majordomus_knowledge_*` and the resources `majordomus://knowledge-candidates` and `majordomus://knowledge-status`. A fresh checkout with no candidate answers absence, not an error.

## How it works

The module `knowledge_base` under `apps/majordomus-cli/src/capability/builtin/` carries three `capability!` blocks with typed input and output; `majordomus generate` projects them to the HTTP routes, the OpenAPI document, the MCP surface, the command line, the cockpit's module catalogue and the benchmark inventory, and `generate --check` refuses a projection that has fallen behind (ADR 0004). The module id is `knowledge_base` and not `knowledge` because the registry refuses a builtin module named like a declarative kind. Candidates come from the index — tracked files of kind `knowledge` in the source class `candidates`, as every kind — the ledger from the session module's reader, and the thresholds from the continuity module's reading of the policy; nothing here restates a number or reads the shell tool's source. The Rust side makes the freshness half of the stopped-writer judgement; the wiring half belongs to the shell validator.

## How to see it

```bash
bin/majordomus-cli knowledge status --format json
bin/majordomus-cli knowledge candidates --format json
bin/majordomus-cli knowledge record <id> --format json
curl -s http://127.0.0.1:<port>/api/v1/knowledge/status          # the same value
curl -s "http://127.0.0.1:<port>/api/v1/knowledge/record?id=<id>"
bin/majordomus-cli capabilities list | grep knowledge_base
```

## What it does not cover

Nothing here writes. Derivation, promotion and rejection are the shell tool's, and the executable reports what the shell wrote. The index reads tracked files, so a candidate the hook just wrote and nobody has added is served by `majordomus knowledge candidates` (the shell listing over the directory) and not yet by the executable. There is no hand-written cockpit page; the catalogue entry and the generated page per capability are the projection.

## Why it exists

A record in the tracked tree that only the shell tool can list is served on one surface out of seven. The layer's rule is that one declaration reaches every surface at once, and `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` places the reader in one module beside `continuity.rs`, reading what the shell wrote and restating none of it.
