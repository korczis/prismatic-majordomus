+++
title = "Why"
description = "the operational failure modes this tool answers, as objects of the layer: the three kinds, the flow from one Markdown file to every projection, what is authored and what is derived, and what enforces each"
weight = 38
[extra]
source = "docs/WHY.md"
+++

{% raw %}

The `/why/` section of the website states the operational failure modes this tool is a
response to. Every entry in it is a declarative object of the layer, and every listing,
route, filter, count, backlink, API answer, MCP resource and diagnosis is derived from
those objects. Behaviour as implemented and tested; where this document and the executable
disagree, the document is wrong and changes in the same commit. The decision is
[ADR 18](../.ai/repo/adrs/0018-operational-moments-are-objects-of-the-layer-not-pages-of-th.md);
the rule is `project.why-catalogue-is-canonical`; the directory's own contract is
[`.ai/repo/why/README.md`](../.ai/repo/why/README.md).

## The three kinds

```text
.ai/repo/why/moments/<id>.md      one operational failure mode   schema: moment/v1
.ai/repo/why/audiences/<id>.md    who recognises it              schema: audience/v1
.ai/repo/why/areas/<id>.md        the operational area           schema: area/v1
```

The file name is the `id` is the slug is the route. `moments/two-agents-one-bug.md` is
`majordomus://moment/two-agents-one-bug` is `/why/two-agents-one-bug/`, and no table
anywhere maps one to another. `audiences`, `areas`, `graph` and `index` are reserved: the
section's own routes use them, and a moment claiming one is refused.

## The flow

Nothing in this chain is specific to the catalogue except the domain model in the middle:
discovery, front-matter parsing, schema validation and the MCP resource are the mechanism
every kind of the layer already goes through.

<pre class="mermaid">
flowchart TD
  MD["Markdown under .ai/repo/why/&lt;br/&gt;front matter + body"]
  SRC["sources.yaml&lt;br/&gt;three pathspecs"]
  KIND["kinds.yaml&lt;br/&gt;three kinds"]
  SCH["share/schemas/*.schema.json&lt;br/&gt;the contract"]
  IDX["Index&lt;br/&gt;typed objects, validated"]
  CAT["why::Catalogue&lt;br/&gt;records, references resolved,&lt;br/&gt;relations derived, indexes built"]
  CAP["why.list · why.moment · why.audiences&lt;br/&gt;why.areas · why.diagnose · why.validate"]
  CLI["majordomus why"]
  HTTP["GET /api/v1/why*"]
  OAS["openapi.json + /docs/api/"]
  MCP["majordomus_why* tools&lt;br/&gt;majordomus://moment/&amp;lt;id&amp;gt;"]
  GEN["generate site&lt;br/&gt;site/data/registry/why.json&lt;br/&gt;+ why-graph.json"]
  SITE["/why/ · /why/&amp;lt;id&amp;gt;/&lt;br/&gt;/why/audiences/&amp;lt;id&amp;gt;/ · /why/areas/&amp;lt;id&amp;gt;/"]

  MD --&gt; IDX
  SRC --&gt; IDX
  KIND --&gt; IDX
  SCH --&gt; IDX
  IDX --&gt; CAT
  CAT --&gt; CAP
  CAP --&gt; CLI
  CAP --&gt; HTTP
  CAP --&gt; MCP
  CAP --&gt; GEN
  HTTP --&gt; OAS
  GEN --&gt; SITE
</pre>


The catalogue is built once, when a capability context is composed, and shared by every
projection; a request never rebuilds it (`project.rust-hot-path`). Its fingerprint is a
hash of its own sources and of nothing else, so the two `generate` passes of
`scripts/derive` — which run either side of the site generator — produce the same bytes.

## Authored and derived

The distinction is the whole design, and the schemas enforce it: a derived key is a key the
contract does not have.

<div class="overflow-x-auto" tabindex="0">

| authored in the file | derived, and refused in a source file |
|---|---|
| title, short title, hook, summary, body | the route, and the API path |
| audiences, areas, lifecycle, tags | the moments of an audience or an area, and their counts |
| severity, frequency, weight, featured | the responsibilities a moment touches (through its claims) |
| signals, examples | the backlinks: what names this moment, this claim, this capability |
| commands, capabilities, claims, doctrines, use cases | the neighbours: explicit, then reverse, then nearest by shared metadata |
| explicit `related` | every facet, every count, the search text, the diagnosis |

</div>


## References

Every reference is typed and resolves against the thing it names. An unresolved name is an
error with the nearest candidate offered, never a link to a page that does not exist.

<div class="overflow-x-auto" tabindex="0">

| key | resolves against |
|---|---|
| `audiences`, `areas`, `related` | the catalogue itself |
| `commands` | the command registry (`share/commands.yaml`, an object of the index) |
| `capabilities` | the capability registry of the executable |
| `claims` | `docs/CLAIMS.yaml` |
| `doctrines` | the effective rule set (`majordomus rules list`) |
| `use_cases` | `.ai/repo/use-cases/` |

</div>


## The diagnosis

`majordomus why diagnose --signal <id>` and the questionnaire on `/why/` answer the same
question with the same arithmetic, and the arithmetic is counting:

```text
a selection            signal ids, or moment ids
  resolves to          the moments that own them
  an area scores       how many of those moments name it
  a recommendation is  a capability, command, claim, rule or use case those moments name,
                       carrying the moments that produced it
```

There is no weighting and no percentage, because there is no model behind one. Every row is
explainable: `matched_because` names the moments, and a reader can check it.

## Reading it

```bash
majordomus why                      # the catalogue, with the counts computed
majordomus why list --audience solo-builder --severity high
majordomus why show two-agents-one-bug
majordomus why audiences            # each with the moments that name it, derived
majordomus why areas
majordomus why diagnose             # the questionnaire, from the catalogue's own signals
majordomus why diagnose --signal same-subsystem-twice --format json
majordomus why validate             # exit 10 on any error
```

Over HTTP: `GET /api/v1/why`, `/api/v1/why/moment?id=…`, `/api/v1/why/audiences`,
`/api/v1/why/areas`, `/api/v1/why/diagnose?signals=a,b`, `/api/v1/why/validate`; every one
of them is in `docs/generated/openapi.json` and on `/docs/api/` because the registry put it
there. Over MCP: the tools `majordomus_why`, `majordomus_why_moment`,
`majordomus_why_audiences`, `majordomus_why_areas`, `majordomus_why_diagnose` and
`majordomus_why_validate`, the resources `majordomus://why`, `majordomus://why/audiences`
and `majordomus://why/areas`, and every entry as `majordomus://moment/<id>`,
`majordomus://audience/<id>` and `majordomus://area/<id>`.

A diagnosis is a `GET` because it changes nothing: `POST` in this executable means a
capability with an effect on the process (`CapabilityKind::Command`). Its selection is
therefore a comma-separated parameter, which binds identically on the query string, over
MCP and on the command line.

## Adding one

```bash
cp .ai/repo/why/moments/two-agents-one-bug.md .ai/repo/why/moments/<id>.md
$EDITOR .ai/repo/why/moments/<id>.md
majordomus why validate
just derive && git add -A
```

That is the whole procedure. There is no registry to edit, no navigation file, no template
list, no Rust and no schema change.

## What enforces what

<div class="overflow-x-auto" tabindex="0">

| concern | source | enforcement |
|---|---|---|
| front matter shape | `share/schemas/majordomus/{moment,audience,area}/*.v1.schema.json` | the index validates every object at build; an unknown key, an unknown enum value or an unsupported `schema:` version excludes the file and degrades the index |
| the file name is the id | the record's `id` | `why validate` — `filename_mismatch`, naming the file it should be |
| a moment does not claim a route the section owns | `why::RESERVED` | `why validate` — `reserved_identity` |
| one identity per file | the index | two claimants are both excluded; the catalogue adopts the diagnostic rather than reporting itself valid |
| references | the catalogue, the registries, the rule set | `why validate` — `unknown_reference`, with the nearest candidate |
| what a public record owes | `status: stable` | `why validate` — `missing_content` warnings; a `draft` is exempt and says so |
| an audience nobody recognises | the moments | `why validate` — `thin_audience` |
| CLI, HTTP, OpenAPI, MCP | one `capability!` declaration each | `majordomus capabilities validate`, and `generate --check` for the documents |
| the site's pages and data | `site/data/registry/why.json` | `scripts/generate-site-data` refuses a catalogue that does not validate; `scripts/site-check` refuses a page a record did not produce, and a link that does not resolve both ways |
| add-one-file, remove-one-file | — | `test/cases/98_why_catalogue.sh` and `apps/majordomus-cli/tests/why.rs` |
| the queries stay cheap as it grows | — | `apps/majordomus-cli/benches/why.rs` at 10, 100 and 1000 records |

</div>


## What it is not

It is not the use-case catalogue. A moment is the pain, in the reader's words; a use case
(`docs/USE_CASES.md`) is the workflow that answers it, with a scenario the tool executes; a
capability is the mechanism. They cross-link by id and never restate each other's text.

It is not a claims matrix either. What the tool guarantees is `docs/CLAIMS.yaml`, with a
status, an implementation and a test; a moment names claims and does not repeat their
wording.
{% endraw %}
