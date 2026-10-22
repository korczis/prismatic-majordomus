<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `autonomy` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.6 -->
# Module `autonomy` — Autonomy

How much a coding-agent session in this checkout may do without a person's approval, read from the facts: the client's settings layers, the evidence for hard isolation, the person's own trust in the checkout, and what the permission layer actually cost. Fails closed; reads only; never grants itself anything.

Stability: experimental. Capabilities: 4.

## `autonomy.approvals` — Whether this machine holds the owner's standing approvals

The owner's standing approvals, declared once in .ai/repo/autonomy/ (kind autonomy-declaration), against this machine's Claude Code user settings: each approval held or not (its rule is an exact entry of autoMode.allow), whether $defaults stays first, how many declared environment lines and settings are held, how many autoMode.allow entries are not declared (counted, never quoted), and the verdict — full, partial, none, undeclared or unreadable — with the command that writes what is missing. Reads only; `majordomus autonomy apply` is the person's act that writes.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_autonomy_approvals` |
| HTTP | `GET /api/v1/autonomy/approvals` |
| CLI | `majordomus autonomy approvals` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::autonomy |
| tags | autonomy, permissions, fleet |

Input: none.

Output: `ApprovalParity`.

## `autonomy.explain` — Judge one shell command

What a static permission rule may do with one shell command under a profile: allow (it runs unasked), defer (the client's own classifier or prompt decides) or require_approval (a person approves it under every profile), with its risk class, the reason and the table entry that matched. A compound command — chaining, a pipe, a redirection, a substitution, a leading assignment, find -exec — never earns an allow; history rewrites, destructive removal, privilege and external effects require approval even inside isolation.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_autonomy_explain` |
| HTTP | `GET /api/v1/autonomy/explain` |
| CLI | `majordomus autonomy explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::autonomy |
| tags | autonomy, permissions, security |

| input | type | required | description |
|---|---|---|---|
| `command` | string | yes | The shell command, as a session would run it. |
| `profile` | object | no | The profile to judge it under; the checkout's own, as `autonomy.status` derives it,
when absent. |

Output: `CommandDecision`.

## `autonomy.friction` — What the permission layer cost

Counted from Claude Code's own transcripts of this repository's checkouts (the primary and every <repository>-wt/ worktree) over the last since_days days: tool calls, and every refusal by kind (classifier_denial, hook_block, safety_check, user_rejection), by the classifier's stated reason and by tool, with the rate per thousand calls. Counts only: no command, path or payload leaves a transcript. A prompt the person approved leaves no trace and is not counted.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_autonomy_friction` |
| HTTP | `GET /api/v1/autonomy/friction` |
| CLI | `majordomus autonomy friction` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::autonomy |
| tags | autonomy, permissions, measurement |

| input | type | required | description |
|---|---|---|---|
| `since_days` | integer or null | no | How many days back to count, 1 to 90; 7 when absent. |

Output: `Friction`.

## `autonomy.status` — The autonomy this checkout supports

The Claude Code settings layers (managed, local, project, user: presence, default mode, rule counts, hooks, never rule text), the isolation evidence (container markers, the client's sandbox, credential variables by name and credential files), the person's trust-dialog acceptance of the checkout, and the profile those facts support — conservative, balanced, trusted or isolated — with its recommended permission mode, the reason for it and the fact that refused every stronger one. isolated (bypassPermissions) needs a container marker and no credential in reach; trusted (auto) needs the person's acceptance, which the repository cannot check in; an unreadable layer or a managed default mode is a ceiling.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_autonomy` |
| HTTP | `GET /api/v1/autonomy` |
| CLI | `majordomus autonomy status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::autonomy |
| tags | autonomy, permissions, security |

Input: none.

Output: `AutonomyStatus`.

