# A majordomus:// URI resolves the same way through the MCP resource read, the majordomus_get tool and the HTTP object route, and majordomus://repository answers repository.info as a JSON document tagged builtin

## What it means

A client that learns a URI from `resources/list`, from `majordomus_list`, from a search
hit or from the documentation can read it through whichever interface it has, and gets
the same thing. A file of the layer (`majordomus://rule/...`, `majordomus://prompt/...`)
is the file as read, with its metadata and provenance. `majordomus://repository` is not a
file: it is the `repository.info` query with a resource exposure, and reading it runs the
query. Through `resources/read` the report comes back as JSON text; through
`majordomus_get` or `GET /api/v1/object` it comes back as data under `answer`, with the
capability's id, kind, title and provenance, and the same text under `content`. The answer
carries `source: declarative` or `source: builtin` so that a client can tell which it got
without guessing from the fields. A URI nothing projects is not found on every interface.

## How it works

`apps/majordomus-cli/src/capability/builtin/objects.rs` has one function, `resolve`: the
registry maps the URI to the capability that projects it and to the exposure it matched
(`CapabilityRegistry::by_mcp_uri`); a declarative capability is answered from the index,
an executable one is executed through `Context::execute` with an empty input, so the
executor's counters and cache policy apply as for any call. `objects.get` renders the
result as a `ResourceView` (an `ObjectView`, or an `AnswerView` around the query's answer);
the MCP surface's `read` renders the same result as resource content, so the two cannot
drift apart. A query that refuses passes its refusal through; a query that fails is an
internal error naming the capability; a registry naming an object the index does not hold
is an internal error, never "not found". The registry refuses to build when a query
exposed as a resource has an input with a required property, because a read supplies
none (`tests/registry.rs`).

## How to see it

```bash
just build
B=apps/majordomus-cli/target/debug/majordomus
$B capabilities describe objects.get --format json | jq '.output.name'      # ResourceView
# in an MCP client: call majordomus_get with {"uri": "majordomus://repository"}
#   -> source: builtin, id: repository.info, answer: the report, content: its text
# with a server running (an MCP client open, or `just serve` in another terminal):
curl -s 'http://127.0.0.1:8741/api/v1/object?uri=majordomus://repository' | jq '{source, id, identity, media_type}'
curl -s 'http://127.0.0.1:8741/api/v1/object?uri=majordomus://rule/none@1'   # 404 not_found
```

## What it does not cover

Nothing here makes a new URI: the set of readable URIs is the registry's, one per object
of the index plus every query with a resource exposure, and today that is
`majordomus://repository` alone. `objects.get` executes only what the registry projects
at a URI, never a capability a client names by id; `capabilities.describe` is for that.

## Why it exists

The resource read served `majordomus://repository` while `majordomus_get` and the object
route answered "unknown resource" for it, because the tool looked in the index alone. Two
answers for one URI is the kind of drift the capability model exists to prevent, so the
fix is one resolution both projections call, proved by `test/cases/72_rust_mcp.sh` over
real pipes and by the crate's `tests/objects_get.rs`, `tests/mcp_stdio.rs` and
`tests/http_serve.rs`.
