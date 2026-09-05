<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `repository`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `repository` — Repository

The repository this process serves: its layer, its git state, and the state of the index built from it.

Stability: behaviorally_verified. Capabilities: 1.

## `repository.info` — Repository and index state

The repository root, layer sections, git state, discovery mode, kinds present, every diagnostic, and the capability registry counted.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_repository` |
| MCP resource | `majordomus://repository` |
| HTTP | `GET /api/v1/repository` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::repository |
| tags | repository, introspection |

Input: none.

Output: `RepositoryReport`.

