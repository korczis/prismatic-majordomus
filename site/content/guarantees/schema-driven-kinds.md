+++
title = "How each declarative kind is read and which keys it may carry is data read at run time, and a repository adds a kind with its JSON Schema without a code change"
description = "The Rust executable compiles nothing about kinds or keys. On every start it locates the tool distribution's share directory and reads kinds.yaml (for each kind: the format, the front-matter rule, the identity, title and description fields, the version field and its supported values, the schema it must satisfy) and schemas/<name>.schema.json, one JSON Schema per contract, validated with a JSON Schema validator. A repository adds kinds under .ai/repo/knowledge/kinds.yaml and schemas under .ai/repo/knowledge/schemas/, declares a class for them in sources.yaml, and the objects are served; a kind or schema the distribution already declares cannot be redefined."
weight = 101
[extra]
claim_id = "schema-driven-kinds"
status = "guaranteed"
source = "docs/claims/schema-driven-kinds.md"
+++
{% raw %}

## What it means

The Rust executable compiles nothing about kinds or keys. On every start it locates the tool distribution's share directory and reads `kinds.yaml` (for each kind: the format, the front-matter rule, the identity, title and description fields, the version field and its supported values, the schema it must satisfy) and `schemas/<name>.schema.json`, one JSON Schema per contract, validated with a JSON Schema validator. A repository adds kinds under `.ai/repo/knowledge/kinds.yaml` and schemas under `.ai/repo/knowledge/schemas/`, declares a class for them in `sources.yaml`, and the objects are served; a kind or schema the distribution already declares cannot be redefined.

## How it works

`share.rs` locates the directory (`--share`, `MAJORDOMUS_SHARE`, the repository's `share/`, the executable's installation); `metadata/mod.rs` parses the kinds files in order, refuses a kind declared twice naming both files, compiles every schema, and `index.rs` validates each file's metadata against its kind's schema before it becomes an object: a key the schema does not allow is `unknown_key`, any other failed constraint is `schema_violation`. The shell tool's allow-lists under `share/allow/` are generated from the same schemas.

## How to see it

```bash
mkdir -p .ai/repo/knowledge/schemas .ai/repo/notes
printf 'schema: majordomus-kinds/v1\nkinds:\n  note:\n    format: markdown\n    front_matter: required\n    schema: note\n    identity: [id]\n    title: title\n' > .ai/repo/knowledge/kinds.yaml
printf '{ "type": "object", "additionalProperties": false, "required": ["id", "title"], "properties": { "id": { "type": "string" }, "title": { "type": "string" } } }\n' > .ai/repo/knowledge/schemas/note.schema.json
# add to sources.yaml:  - id: note / kind: note / discovery: vcs / pathspec: ':(glob).ai/repo/notes/*.md' / required: false
printf -- '---\nid: first\ntitle: The first note\n---\n' > .ai/repo/notes/first.md
git add -A
apps/majordomus-cli/target/debug/majordomus capabilities describe note.first
```

## What it does not cover

A kind needing a format the reader does not have (something other than Markdown with front matter, YAML, a YAML collection, or plain text) or an identity rule other than named fields or the path is a change in `index.rs`; data describes objects, it does not define how a format is read. The shell tool reads its allow-lists, not the schemas, so a repository-defined kind is visible to the Rust executable only.

## Why it exists

The operator's requirement that extension be data-driven at run time, with a typed and standard contract rather than regex lists. `apps/majordomus-cli/tests/external_extension.rs` adds a kind with its schema to a fixture repository, reads the object back, sees the schema enforced on a bad one, and sees a redefinition refused; `test/cases/76_capabilities_projections.sh` proves the distribution's kinds serve the layer `init` writes with state `ok`.
{% endraw %}
