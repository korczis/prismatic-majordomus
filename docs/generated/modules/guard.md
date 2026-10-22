<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `guard` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.7 -->
# Module `guard` — Edit guard

Whether a write to a file of this checkout is covered by a claim: the active task's scope, or a live mesh claim of the writing session. The judgement a provider's pre-tool hook asks before every edit (majordomus guard hook), switched by session.edits_require_claim.

Stability: experimental. Capabilities: 1.

## `guard.edit` — Judge one edit

Whether the given session may write the given path now: allowed when the policy's session.edits_require_claim is off, the path is outside the checkout or its local state, the repository declares no enabled mesh, the active task's scope covers it, or a live mesh claim of the session covers it; refused as unclaimed otherwise, with the remedy. What cannot be read — the policy, the mesh — is let through and named, never refused.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_guard_edit` |
| HTTP | `GET /api/v1/guard/edit` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::guard |
| tags | guard, mesh, coordination |

| input | type | required | description |
|---|---|---|---|
| `path` | string | yes | The path about to be written: absolute, or relative to the repository root. |
| `session` | string or null | no | The writing session's id on the mesh (`claude-code-<session id>` for Claude Code);
without it only the active task's scope can cover the write. |

Output: `EditVerdict`.

