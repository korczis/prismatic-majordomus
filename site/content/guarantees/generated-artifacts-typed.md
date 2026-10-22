+++
title = "Every generated artifact declares the document it projects, the encoding it is written in, the schema its content satisfies and its source, and a structured document is written in every encoding it is committed in from one value"
description = "majordomus generate writes a *typed* tree. Every file in it declares four things: the **document** it projects, the **encoding** it is written in (json, yaml, markdown or text, matching its own suffix), the **schema** its content satisfies where the document has a contract, and the **source** it was derived from. Every file carries a provenance header in the form its encoding allows, so a person who opens one learns it is a cache before they edit it."
weight = 112
[extra]
claim_id = "generated-artifacts-typed"
status = "guaranteed"
source = "docs/claims/generated-artifacts-typed.md"
+++
{% raw %}

## What it means

`majordomus generate` writes a *typed* tree. Every file in it declares four things: the **document** it projects, the **encoding** it is written in (`json`, `yaml`, `markdown` or `text`, matching its own suffix), the **schema** its content satisfies where the document has a contract, and the **source** it was derived from. Every file carries a provenance header in the form its encoding allows, so a person who opens one learns it is a cache before they edit it.

A *document* is one value. `docs/generated/registry.json` and `docs/generated/registry.yaml` are two renderings of it; `docs/generated/benchmarks.md`, `.json` and `.yaml` are three. They cannot disagree, because there is one computation and several encoders — the same shape this repository requires of every other interface.

The index of the whole set is `docs/generated/artifacts.{json,yaml,md}`, and it is generated. Nothing anywhere keeps a list of what is generated: the plan is the list.

## How it works

`generate::Artifact` carries the declaration and `generate::Document` carries the value; `Document::artifacts` renders the value into every encoding the document is committed in. The contracts live under `share/schemas/generated/`, one JSON Schema per document, matched to it by the `const` of its `schema` member — kind-schema discovery does not recurse, so nothing there can be read as an object kind's schema.

`generate::verify` checks the whole plan before a byte is written or compared: the declared encoding against the suffix, the header against the encoding, the document against its published contract, and the manifest against the plan. `generate` and `generate --check` both run it, so a half-typed tree is never produced and never reported in sync.

The manifest's own three encodings carry no hash — a document that hashed itself would have no fixed point — and `--check` compares them byte for byte instead, which is the stronger statement.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus generate --check   # names each file with its schema, then: in sync
sed -n '1,6p' docs/generated/registry.yaml                     # the banner, then schema/generated/generator
printf '\n' >> docs/generated/registry.yaml
apps/majordomus-cli/target/debug/majordomus generate --check   # exit 10: docs/generated/registry.yaml (differs)
apps/majordomus-cli/target/debug/majordomus generate registry  # both encodings restored, from one value
curl -s localhost:8741/api/v1/artifacts | jq '.tallies'        # the same manifest, reconciled with the tree
```

`docs/generated/artifacts.md` is the index for a reader; `/cockpit/artifacts` and the site's `/registry/artifacts/` are the same index as a page.

## What it does not cover

The provider bootstraps (`AGENTS.md`, `CLAUDE.md`) carry the `majordomus update` stamp of the policy they were rendered from rather than this banner: a region projection owns part of a file it did not write, and a header at the top of it would be a claim over text the generator does not own. The site's copy of the manifest carries the declarations without the sizes and hashes, because two of the manifest's subjects are written by a later stage of `scripts/derive` than the copy is; the hashes are answered live instead. And a schema here fixes the top level and the shape of each entry rather than restating the Rust types, so adding a field to a descriptor is not a schema change while removing one is.

## Why it exists

The generator knew things about its output that its output did not carry: some files said they were generated and some did not, one named the schema it satisfied and the next did not, the benchmark matrix existed for a reader and not for a program, nothing was written in YAML at all, and no file anywhere said what the generated set *was*. Every one of those is the same defect. `project.generated-artifacts-are-typed@1` is the rule; `apps/majordomus-cli/tests/generated_documents.rs` proves each clause over a real plan and proves each clause, broken, refused; `test/cases/52_generated_artifact_typing.sh` proves it of the committed tree and reads every `.yaml` with a YAML parser this repository does not own.
{% endraw %}
