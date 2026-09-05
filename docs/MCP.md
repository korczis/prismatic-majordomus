# MCP surface — `majordomus mcp`

What the Rust executable under [`apps/majordomus-cli/`](../apps/majordomus-cli/) serves
to an MCP client, where it comes from, and what it refuses. Behaviour as implemented and
tested; where implementation and this document disagree, the document is wrong and
changes in the same commit as the fix. The developer-facing detail (architecture, every
option, the kind schema) is in the application's own
[`README.md`](../apps/majordomus-cli/README.md).

## What it is

One process, started by the client, speaking the Model Context Protocol over stdio,
serving the repository's AI layer read-only:

```bash
cargo build --manifest-path apps/majordomus-cli/Cargo.toml
apps/majordomus-cli/target/debug/majordomus mcp --inspect    # what would be served, and every diagnostic
apps/majordomus-cli/target/debug/majordomus mcp              # serve until the client goes
```

It is not a daemon: it lives as long as the client's pipe, listens on no port, keeps no
state, needs no database, and writes nothing. It is one projection of the executable's
capability registry ([`CAPABILITIES.md`](CAPABILITIES.md)): the same capabilities are the
HTTP routes of `majordomus serve` and the `capabilities` commands, and every tool and
resource here is derived from a registry entry, none declared in the MCP code.

## What decides what is served

The executable names no repository file except the two conventions the layer itself
documents, `.ai/manifest.yaml` and `sources.yaml` under the `knowledge` section, and reads
how each kind is read from the tool distribution at run time. The rest is data:

| decides | read from |
|---|---|
| where the layer is | the nearest ancestor holding `.ai/manifest.yaml`; `.git` and `.majordomus/` are not markers |
| which sections exist | `sections:` in the manifest |
| which files are sources, of which kind | `.ai/repo/knowledge/sources.yaml`, one pathspec and kind per class, through the git index |
| how a kind is read and which keys it may carry | `share/kinds.yaml` and `share/schemas/<kind>.schema.json` in the distribution, plus a repository's own under `.ai/repo/knowledge/` |
| which tools exist | the executable capabilities with an MCP tool exposure, composed in `apps/majordomus-cli/src/capability/builtin.rs` |

Consequences a repository can rely on:

- a new rule, prompt, profile, milestone, issue, claim or document is served after `git add`
  and a restart, with no change to the executable;
- a new class in `sources.yaml` naming a known kind, or a new kind with its schema under
  `.ai/repo/knowledge/`, is served the same way;
- `.ai/local/` is never served, tracked or not;
- the YAML read is the subset [`SCHEMAS.md`](SCHEMAS.md) defines, so a file the shell tool
  refuses is refused here with the same line named.

## Resources

| URI | content |
|---|---|
| `majordomus://repository` | JSON: root, layer schema, sections, git state, discovery mode, kinds present, every diagnostic |
| `majordomus://<kind>/<identity>` | the file as read, `text/markdown` or `application/yaml` |

Identity is the kind's identity fields joined with `@` — `majordomus.scope-integrity@1`
for a rule, `continue` for a prompt, `implementation` for a profile, `M001` for a
milestone — or, for a kind with no identity fields (policy, document), the
repository-relative path. Every listed resource carries `_meta.majordomus` with the kind,
the identity and the provenance: path, directory, the class that discovered it, the
manifest section it falls under, and its size.

## Tools

| tool | capability | arguments | answers |
|---|---|---|---|
| `majordomus_list` | `objects.list` | `kind?`, `tag?` | the objects, summarised |
| `majordomus_get` | `objects.get` | `uri` | metadata, provenance, media type, content |
| `majordomus_search` | `objects.search` | `query`, `kind?`, `limit?` | case-insensitive substring hits with one snippet line each |
| `majordomus_repository` | `repository.info` | none | the `majordomus://repository` document |
| `majordomus_capabilities` | `capabilities.list` | `kind?`, `exposure?` | every capability with its projections |
| `majordomus_capability` | `capabilities.describe` | `id` | one capability: schemas, provenance, every projection |

All six are read-only and say so in their annotations; each carries the canonical id in
`_meta.majordomus.id` and its `inputSchema` and `outputSchema` from the canonical schemas.
A refused call is a result with `isError: true`; an unknown tool, method or resource is a
protocol error.

## Failure behaviour

| state | what happens |
|---|---|
| no `.ai/manifest.yaml` above the working directory | exit `12`, the start directory named |
| project data under `.majordomus/` and no manifest | exit `12`, naming `majordomus migrate` |
| manifest or `sources.yaml` malformed, unknown key, unsupported schema | exit `10`, the path and the key or line named |
| one file that cannot become an object | excluded; an error diagnostic names its path and a stable code; the index is `degraded` and still serves; `--strict` exits `10` instead |
| `git` unusable | `--discovery vcs` (the default) exits `13` naming `--discovery filesystem`; the git block of `majordomus://repository` reads `unavailable` with the reason |
| no share directory (kinds and schemas) found | exit `12`, every directory tried named, and `--share` / `MAJORDOMUS_SHARE` named as the remedy |
| a kind schema or a schema file invalid, or a repository redefining a distributed kind | exit `10`, both files named |
| client closes its pipe | the process ends with `0` |

Two files of one kind claiming one identity are both excluded and both named, as the
rules contract requires. Nothing is repaired, defaulted or rewritten.

## Not served, on purpose

- **MCP prompts.** The repository's prompt assets render `{{CONTEXT}}` from checkout-local
  state the shell tool owns; served unrendered they would read as finished. They are
  resources (`majordomus://prompt/<name>`), not prompts.
- **The hierarchy of bootstrap files.** Root `README.md`, `AGENTS.md` and the other
  provider files are served as documents with their directory recorded; nothing merges
  or ranks them, because the repository defines no merge semantics. Recorded in
  [`.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md`](../.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md).
- **Any mutation**, any second transport, subscriptions, list-change notifications.
  The HTTP projection is a separate command, `majordomus serve`, on the loopback
  interface; see [`CAPABILITIES.md`](CAPABILITIES.md).

## What proves it

`test/cases/72_rust_mcp.sh` builds the executable and speaks to it over pipes inside a
repository the shell tool's `init` wrote; `76_capabilities_projections.sh` reads one
capability back through every interface. The crate's own suite
(`cargo test --manifest-path apps/majordomus-cli/Cargo.toml`) covers the command line,
root selection, the metadata contract, determinism, the protocol round trip, protocol-only
stdout, non-mutation, and the add–remove–break sequence of external extension. The
claims are in [`CLAIMS.yaml`](CLAIMS.yaml) under `mcp-`.
