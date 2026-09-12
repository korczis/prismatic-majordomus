<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `commands` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `commands` — Command graph

Every command this repository offers, from whichever program offers it: the Rust executable, the shell tool that carries the task lifecycle, and the workflows the repository declares for a person to run. Composed from the three declarations that already exist — the clap tree, the shipped command registry and the workflow runner's own dump — never from a list. Each command carries what running it changes, what it needs, where its argument values come from, and every surface that carries it, with the reason when one does not.

Stability: implemented. Capabilities: 3.

## `commands.get` — One command in full

One command by its canonical identity: its arguments with the source of each one's values, what running it changes, what it needs, where it came from, and every surface that carries it — with the reason a machine surface withholds it when one does.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_command` |
| HTTP | `GET /api/v1/command` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commands |
| tags | commands, introspection |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The identity, `executable.worktree.status`. |

Output: `CommandNode`.

## `commands.graph` — The whole command graph

The graph as one document, with its fingerprint and every diagnostic its build found: a duplicate identity, a recipe name two commands would take, an annotation that names a command which no longer exists. Deterministic — two builds over one tree produce the same document — so a client may cache against the fingerprint.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_command_graph` |
| HTTP | `GET /api/v1/commands/graph` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commands |
| tags | commands, introspection, diagnostics |

Input: none.

Output: `CommandGraphReport`.

## `commands.list` — Every command, one line each

The commands this repository offers, filtered by the program that runs them, by what running them changes, or by text. A summary rather than the whole graph: enough to choose a command, and never so much that a client has to read every argument of every command to find one.

| | |
|---|---|
| kind | query |
| stability | implemented |
| MCP tool | `majordomus_commands` |
| MCP resource | `majordomus://commands` |
| HTTP | `GET /api/v1/commands` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commands |
| tags | commands, introspection |

| input | type | required | description |
|---|---|---|---|
| `origin` | string or null | no | Only the commands of one program: `executable`, `tool` or `workflow`. |
| `effect` | string or null | no | Only the commands whose effect is at most this one: `read_only`,
`local_mutation`, `repository_mutation`, `network_mutation`, `destructive`. |
| `search` | string or null | no | Only the commands matching this text in their invocation, summary, tags or identity. |

Output: `CommandIndex`.

