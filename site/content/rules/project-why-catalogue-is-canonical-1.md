+++
title = "An operational failure mode is a catalogue object, never a page"
description = "An operational failure mode is a catalogue object, never a page"
weight = 134
[extra]
kind = "rule"
slug = "project-why-catalogue-is-canonical-1"
identity = "project.why-catalogue-is-canonical@1"
status = "active"
source = ".ai/repo/rules/project/why-catalogue-is-canonical.v1.md"
+++
{% raw %}

## Rationale

The section this rule governs began as ten hand-written pages with a bash parser behind
them, and it worked exactly as long as ten was the number. The failure mode of that shape
is not that it breaks; it is that the eleventh entry costs a page, a data file, a template
list, a navigation entry and a backlink somewhere else, so the eleventh entry is not
written. A catalogue nobody extends is a catalogue that stops describing the product.

The same argument that makes a capability one declaration with many projections
(`project.interfaces-are-projections`) applies here, for the same reason: the moments are
read by a person on a page, by a worker over MCP, by a script over HTTP and by the site
generator, and four readings of one file are four chances to disagree.

## Required behaviour

An operational moment, an audience and an operational area are each one Markdown file
under the layer's why section, with front matter declaring a supported `schema:` version.
The front matter is the machine contract: discovery, validation, membership, relations,
filtering, ordering, search, the command line, the HTTP routes, the OpenAPI document, MCP,
the derived graph, the site's pages and the diagnosis are all read from it, and no
projection derives structure from the prose.

Adding a valid file is the whole act of adding an entry. Nothing may require a second
edit: not a Rust list, a JavaScript constant, a navigation file, a template list, a
transport registry, a schema document or a generated index.

Membership and every reverse relation are derived. An audience does not list its moments,
a claim does not list the moments that answer it, and a moment does not carry its own
route, its API path, a count, or the moments that name it. A derived value written into a
source file is a violation of this rule even when it is correct on the day it is written.

Every reference a record makes is typed and resolves against the thing it names — the
catalogue itself, the command registry, the capability registry, the claims, the effective
rule set, the use cases — and an unresolved name is an error with the nearest candidate
offered, never a link to a page that does not exist.

## Failure behaviour

`majordomus why validate` exits `10` and names every finding with its file, its front-matter
key and, for an unresolved reference, the nearest candidate. The site generator refuses to
render a catalogue that does not validate, and `scripts/site-check` refuses a projection
that a moment did not produce or that does not link back. No command decides whether a new
projection has begun registering entries by hand; a reviewer does, and the behavioural case
below proves the invariant that makes the answer checkable.

## Verification

`majordomus why validate`, and `bash test/run.sh 98_why_catalogue`, which adds one file,
finds it in the domain index, the command line, the HTTP API, MCP and the site projection
with nothing else changed, then removes it and finds it gone from all of them.
{% endraw %}
