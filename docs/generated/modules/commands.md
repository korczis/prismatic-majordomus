<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `commands` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.3.1 -->
# Module `commands` — Commands

The canonical command graph: every command this repository can be asked to run, what running each one changes, and the surfaces it reaches. Read from the command line's own declaration, never from a list.

Stability: behaviorally_verified. Capabilities: 2.

## `commands.get` — One command, whole

One command by canonical identity: its arguments and where their values come from, what running it changes, how it runs, the names it has answered to, and every surface spelling of it — including the surfaces it is absent from.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_command` |
| HTTP | `GET /api/v1/commands/get` |
| cache | process, 32 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commands |
| tags | commands, introspection |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The canonical identity, `worktree.create`. |

Output: `CommandNode`.

## `commands.list` — Every command, in one line each

The index of the command graph: identity, summary, effect and the surfaces each command reaches, narrowed by namespace, effect, free text or whether a machine may call it. Small on purpose — the detail of one command is commands.get.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_commands` |
| HTTP | `GET /api/v1/commands` |
| cache | process, 16 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::commands |
| tags | commands, introspection |

| input | type | required | description |
|---|---|---|---|
| `namespace` | string or null | no | Only commands whose identity starts with this namespace (`worktree`). |
| `effect` | string or null | no | Only commands with this effect (`read_only`, `destructive`). |
| `search` | string or null | no | Only commands whose identity or summary contains this text, case-insensitively. |
| `machine_callable` | boolean or null | no | Only commands a machine surface may invoke. |

Output: `CommandIndex`.

