<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `editors` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.30.1 -->
# Module `editors` — Editor and agent projections

The files Zed and Google Antigravity read from this repository, derived from the skills and the tracked justfile: what each product supports and on what evidence, the state and content address of every projected file, and what to do about each finding.

Stability: behaviorally_verified. Capabilities: 1.

## `editors.status` — Editor and agent projections

Every adapter (Zed, Antigravity) with each capability it supports, does not, or cannot be verified for, and the evidence; every projected file with the inputs it is derived from, its content address and its state here (current, stale, absent, or a conflict with a file this generator did not write); orphaned projections; and every finding with its remedy. Writes nothing: `majordomus generate editors` does.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_editors` |
| MCP resource | `majordomus://editors` |
| HTTP | `GET /api/v1/editors` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::editors |
| tags | editors, projections, introspection |

Input: none.

Output: `EditorProjection`.

