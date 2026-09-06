---
id: project.generated-artifacts-are-typed
version: 1
kind: rule
title: A generated artifact declares its document, its encoding, its contract and its source
description: Every file the generator writes declares the document it projects, the encoding it is written in, the JSON Schema its content satisfies and where it came from; it carries a provenance header in the form that encoding allows; a structured document is written in every encoding this repository commits it in from one value; and the manifest of the whole set is itself generated.
statement: A generated artifact declares the document it projects, its encoding, the schema its content satisfies and its source, carries a provenance header in the form its encoding allows, and appears in the generated manifest; a structured document is written in every encoding it is committed in, from one value.
status: active
class: blocking
depends_on: [project.derived-files-regenerated@1, project.interfaces-are-projections@1]
tags: [derived, generation, schema]
---

# Rationale

A generated tree that is only JSON is a tree only a program can read; a generated tree
whose files say nothing about themselves is a tree a person edits by accident. This
repository had both failures at once: some artifacts carried a do-not-edit banner and some
did not, `registry.json` named the schema it satisfied and `cli.json` did not, the
benchmark matrix existed for a reader and not for a program, no artifact was written in
YAML at all, and no file anywhere said what the set of generated files *was* — so the only
way to learn it was to read the generator.

Every one of those is the same defect: the generator knew things about its output that its
output did not carry. Typing the artifact is what closes it. The encoding is a property of
the file, the contract is a property of the document, and both belong in the declaration
rather than in the reader's head.

The second half of the rule is why the encodings cannot drift. A document is one value.
JSON, YAML and — where a reader is the audience — Markdown are renderings of it. Two
renderers over two values is how a matrix in Markdown comes to disagree with the same
matrix in JSON, and this repository refuses that shape everywhere else
([`project.interfaces-are-projections@1`](interfaces-are-projections.v1.md)).

# Required behaviour

Every artifact of `majordomus generate` declares:

- the **document** it projects, shared by every encoding of that document;
- the **encoding** it is written in — `json`, `yaml`, `markdown` or `text` — matching its
  own file suffix;
- the **schema** its content satisfies, by schema id, when the document has a contract;
- the **source** it was derived from, in one line.

Every artifact carries a provenance header in the form its encoding allows: members
(`schema`, `generated`, `generator`) in JSON, an `x-majordomus-` extension where the
document's own specification fixes its member names, a `#` comment banner in YAML and in
line-oriented text, an HTML comment in Markdown. The one exception is a provider bootstrap,
which carries the `majordomus update` stamp of the policy it was rendered from and is
checked by that command.

A structured document is written in **every encoding this repository commits it in**, from
one value: the JSON and the YAML of a document are two renderings of the same
`generate::Document`, never two computations. A document whose audience includes a reader
also has a Markdown rendering of the same value.

A document that names a schema id has a published contract under
`share/schemas/generated/<name>.schema.json`, pinned to the document by the `const` of its
`schema` member, and satisfies it.

The whole set is indexed by `docs/generated/artifacts.{json,yaml,md}`, which is itself
generated: it lists every artifact of the plan with its encoding, contract, source, size
and hash. Its own three encodings carry no hash — a document that hashed itself would have
no fixed point — and are compared byte for byte by `generate --check` instead.

# Failure behaviour

`majordomus generate` and `majordomus generate --check` verify the plan before a byte is
written or compared, and refuse with exit 10 naming every artifact and every reason. A
plan that breaks any clause above is never written: `generate` does not produce a
half-typed tree and then report it.

# Verification

`majordomus generate --check`, which CI and `scripts/derive-check` run on every push, and
which `capabilities validate` and `health.report` also reach.
`apps/majordomus-cli/tests/generated_documents.rs` proves each clause over a real plan and
proves the refusal of each clause broken, so none of them can pass vacuously;
`apps/majordomus-cli/tests/cockpit.rs` proves the reading half —
`artifacts.list`, `GET /api/v1/artifacts`, the MCP tool `majordomus_artifacts` and the
Cockpit page — reports the same set and marks an edited file stale;
`test/cases/52_generated_artifact_typing.sh` proves the committed tree of this repository
satisfies the rule and that the YAML encodings parse as YAML and equal their JSON siblings.
