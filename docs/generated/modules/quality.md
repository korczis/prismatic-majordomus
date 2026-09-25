<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `quality` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.9.0 -->
# Module `quality` — Public API quality

What this executable's own public surface is held to, measured from its syntax tree: documentation that says more than the signature, an executable example on everything that carries behaviour, a module boundary something exercises, and every command of the command line accounted for against the capability registry. The rules are project.rust-public-api-quality and project.operation-transport-parity; this is the measurement of them.

Stability: behaviorally_verified. Capabilities: 2.

## `quality.report` — Public API quality report

The crate's exported surface measured against the repository's rules: how many items are documented and exampled, how many modules are documented, exampled and behaviourally tested, how the canonical operations stand against the command line, HTTP, OpenAPI and MCP, and one finding per violation carrying a stable code, the rule that requires it, its file and line, why it matters and what to do about it.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_quality` |
| MCP resource | `majordomus://quality` |
| HTTP | `GET /api/v1/quality` |
| CLI | `majordomus quality report` |
| cache | process, 8 entries, 10s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::quality |
| tags | quality, introspection, rust |

| input | type | required | description |
|---|---|---|---|
| `code` | string or null | no | Only findings carrying this code (`RUST_PUBLIC_MISSING_EXAMPLE`). All of them when
absent. A code nothing recognises is refused rather than answered with an empty
list, so a typo cannot read as a clean report. |
| `path` | string or null | no | Only findings whose file path starts with this, repository-relative
(`apps/majordomus-cli/src/web`). All of them when absent. |
| `summary_only` | boolean | no | Answer with the counts and leave the findings out. For a caller that wants the
state of the crate and not the list of what to do about it. |
| `include_baselined` | boolean | no | Include the findings the ratchet already accepts, which are left out by default.

The default is what a gate wants: only what is new. This is what a person paying the
debt down wants, and it is what `--write-baseline` must ask for — a baseline written
from a report that had the baseline applied to it would empty the file, accepting
nothing and failing on everything the next time it ran. |

Output: `QualityAnswer`.

## `quality.rustdoc` — Rustdoc tree integrity

The crate's rustdoc tree — the rustdoc surface's artifact, as the web topology resolves it — judged against the crate's own inventory of exported items: every item that owns a page has it at the route rustdoc gives it, no item page is left without an item, the tree declares it was built from HEAD, the library's index is present and names the crate, its assets are present, every relative link resolves, and no file names the machine it was built on or carries a credential. Answers the verdict (clean, findings, or no_tree when there is nothing to judge), the counts it joined, every exported module with its page, and one typed finding per defect with the file and what to do.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_quality_rustdoc` |
| MCP resource | `majordomus://quality/rustdoc` |
| HTTP | `GET /api/v1/quality/rustdoc` |
| CLI | `majordomus quality rustdoc` |
| cache | process, 8 entries, 10s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::quality |
| tags | quality, introspection, rust, documentation |

| input | type | required | description |
|---|---|---|---|
| `kind` | object | no | Only findings of this kind (`missing-page`, `link`, `stale`). All of them when
absent. The verdict and the counts are never narrowed: a filter that changed the
verdict would be a way of passing a tree with a missing page. |
| `summary_only` | boolean | no | Answer with the verdict, the counts and the module routes, and leave the findings
out. |

Output: `RustdocReport`.

