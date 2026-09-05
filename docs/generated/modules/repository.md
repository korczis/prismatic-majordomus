<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the canonical Majordomus capability registry, module `repository`; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.1.0 -->
# Module `repository` — Repository

The repository this process serves: its layer, its git state, the state of the index built from it, and its scope: what a worker reads of it and what it never reads.

Stability: behaviorally_verified. Capabilities: 3.

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

## `repository.scope` — The repository scope

The scope declaration as read, where it came from (the repository's own or the distribution's default), and every tracked file tallied against it: how many are in, how many are out for each reason, and which.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_scope` |
| MCP resource | `majordomus://scope` |
| HTTP | `GET /api/v1/scope` |
| CLI | `majordomus scope` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::repository |
| tags | repository, scope, introspection |

Input: none.

Output: `ScopeReport`.

## `repository.scope_classify` — Judge a path against the scope

Whether a repository-relative path is in or out of the scope, the reason when it is out, and the pattern or limit that decided; an existing file is judged by name, then size, then content.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_scope_classify` |
| HTTP | `GET /api/v1/scope/classify` |
| CLI | `majordomus scope classify` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::repository |
| tags | repository, scope |

| input | type | required | description |
|---|---|---|---|
| `path` | string | yes | Repository-relative, forward slashes; `./` is stripped. An absolute path or a
`..` segment is an invalid input. |

Output: `Classification`.

