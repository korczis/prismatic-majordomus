# PHASE 01 — REPOSITORY FORENSICS AND CONTRACT

Execute after reading 00-MASTER-PROMPT.md. Do not assume a subsystem already exists.

1. Inspect actual git revision, workspace and instructions; report relevant root directories and current state. Audit all editor-specific files including ignored and user-modified state without exposing secrets.
2. Trace existing `.ai` policies, rules, generated instructions, typed schemas and validation; locate authoritative inventories for formats, command registry, tools, toolchain, generated paths, service discovery and web/API/MCP projection.
3. Read official Zed documents for each feature you intend to emit; inspect current supported version/capability. Build `feature -> official reference -> project-scope support -> exact configuration encoding -> fallback` matrix.
4. State which facts are observed, inferred or proposed. Record missing sources explicitly; no silent invented registries.
5. Produce design decision record (following real ADR conventions) covering ownership, precedence, extensibility, explainability, backcompat, failure semantics, security and performance.
6. Specify types and invariants with precise mapping into existing repository modules. Keep Zed-specific JSON types at adapter boundary.
7. Build failing integration test cases before production code for at least core inference, deterministic output and preservation of manual user settings.
8. Exit phase with a short implementation map and acceptance ledger. Then automatically proceed to phase 02. Do not stop for approval absent genuine external authorization boundaries.
