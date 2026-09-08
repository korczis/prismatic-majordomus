+++
title = "Declare once, derive every interface"
description = "One typed declaration per capability, one file per object of the layer; every interface, generated document and page is derived from the registry those declarations build, and a stale projection fails the build."
weight = 10
[extra]
id = "declare-once"
status = "stable"
source = ".ai/repo/features/declare-once.md"
+++
{% raw %}

## What it does

An executable capability is one `capability!` block in its module's file, with a typed
input, a typed output and a handler. A declarative object — a rule, a skill, a decision, a
use case, an operational moment, a feature like this one — is one file under `.ai/` whose
kind and schema the tool distribution declares. The registry is built from both on every
start, validated and fingerprinted, and everything a person or a program reaches is a
reading of it: the MCP tools and resources, the routes under `/api/v1/`, the OpenAPI
document and the Swagger UI over it, the words after `majordomus`, the benchmark targets,
the Cockpit's pages, the generated reference under `docs/generated/`, and the datasets this
website is rendered from.

`majordomus generate` writes every committed projection from one plan; `majordomus generate
--check` refuses a tree in which any of them differs from what the sources produce, and CI
refuses to merge or deploy such a tree. The feature you are reading is itself one file under
the layer, and the surfaces listed beside it were derived from that file rather than typed
onto this page.

## What it does not do

It does not discover a capability nobody declared: a handler without a `capability!` block
is not exposed anywhere, by design. It does not infer a relation from a name or a
substring; every reference a file makes is typed and resolved against the thing it names,
and one that resolves to nothing fails validation rather than becoming a link to nothing.
{% endraw %}
