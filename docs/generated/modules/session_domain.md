<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `session_domain` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Module `session_domain` — The session domain

The typed session domain: the lifecycle state machine an episode moves through, declared once in the executable and projected here rather than redrawn per surface, and the identities this checkout records — the repository, the checkout, the episode and the provider's own session — each with the canonical value and each store's own spelling of it, because the stores spell six things as three words and nothing said so.

Stability: behaviorally_verified. Capabilities: 2.

## `session_domain.identity` — What identifies this checkout, and how each store spells it

The repository, the checkout, the open episode and the provider session, each with the canonical value this executable computes and each store's own spelling beside it. The stores record the repository sometimes as an absolute .git path and sometimes as a remote URL, the worktree as a path and as a digest, and the session as both an episode id and a provider UUID; this is the surface that says so, so that a reader never compares two spellings of different things and concludes they disagree.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_session_identity` |
| MCP resource | `majordomus://session/identity` |
| HTTP | `GET /api/v1/session/identity` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::session_domain |
| tags | session, identity, continuity |

Input: none.

Output: `IdentityReport`.

## `session_domain.machine` — The episode lifecycle, as data

Every state an episode can be in, whether it is terminal and what it may move to; every transition — open, resume, checkpoint, detach, close, recover — with the states it runs between and whether it moves the state at all. Derived from the types, so a diagram that disagrees with the code cannot exist. What the machine deliberately does not depend on is stated rather than left to be inferred: task state, which is ADR 0052.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_session_machine` |
| MCP resource | `majordomus://session/machine` |
| HTTP | `GET /api/v1/session/machine` |
| cache | process, 1 entries, 600s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::session_domain |
| tags | session, continuity, lifecycle |

Input: none.

Output: `MachineReport`.

