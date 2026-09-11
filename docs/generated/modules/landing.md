<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `landing` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.5.0 -->
# Module `landing` — Landing closure

What is preventing this repository from being completely landed and delivered, right now. Twelve stages of delivery — implementation, tests, documentation, generated projections, commit, push, integration, CI, deployment, publication, project model, workspace cleanup — each answered by the capability that already owns the fact, translated into one status vocabulary, and folded into one verdict. Nothing here measures anything of its own: a stage is a question put to `plan.status`, `health.report`, `artifacts.list`, `worktree.topology`, `gates.completion`, `deploy.check` or `distribution.status`, and a stage whose owner could not answer is reported as unknown rather than as clear.

Stability: behaviorally_verified. Capabilities: 1.

## `landing.closure` — What is preventing this repository from being landed

Every stage of delivery with its verdict, the evidence that decided it, the capability that owns that evidence, the specific things standing in the way and the command that would settle each. `landed` is true only when every stage is answered and clear: a stage nothing answered leaves the repository unlanded, because absence of a refusal is not delivery.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_landing` |
| MCP resource | `majordomus://landing` |
| HTTP | `GET /api/v1/landing` |
| CLI | `majordomus landing` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::landing |
| tags | landing, completion, delivery, introspection |

Input: none.

Output: `LandingClosure`.

