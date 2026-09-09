<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `artifacts` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# Module `artifacts` — Generated artifacts

What `majordomus generate` writes: every document with the encodings it is committed in — JSON for a program, YAML beside it, Markdown for a reader — each with its schema, its source and its hash, reconciled with the working tree. The declaration is the generator's own manifest; nothing here keeps a list.

Stability: behaviorally_verified. Capabilities: 1.

## `artifacts.list` — List the generated artifacts

The manifest `majordomus generate` commits as docs/generated/artifacts.json, reconciled with the working tree: every document with the encodings it is written in, and every file with its format, schema, source, size, hash and whether the file on disk still matches. Optionally narrowed to one document or one encoding. Reads only; `majordomus generate` writes and `majordomus generate --check` is the byte-for-byte verdict.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_artifacts` |
| MCP resource | `majordomus://artifacts` |
| HTTP | `GET /api/v1/artifacts` |
| cache | process, 8 entries, 5s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::artifacts |
| tags | artifacts, generation, introspection |

| input | type | required | description |
|---|---|---|---|
| `document` | string or null | no | Only the artifacts of this document (`registry`, `cli`, `openapi`, ...). |
| `format` | object | no | Only the artifacts written in this encoding. |

Output: `ArtifactReport`.

