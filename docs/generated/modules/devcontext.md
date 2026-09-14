<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `devcontext` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `devcontext` — Development context

The context a development session should be given, derived from the repository rather than composed by hand: given an issue, a milestone, an intent or a set of paths, the canonical objects that bear on the work, why each one is in the answer, what was left out and why, what collapsed into what, and the whole cost against a budget. The selection is structured — every entry keeps its identifier, its provenance, the selector that reached it and its confidence — because rendering a prompt is a projection of the selection and not the selection itself.

Stability: behaviorally_verified. Capabilities: 3.

## `devcontext.compile` — Compile the context for a piece of work

The objects a session working on this issue, milestone, intent or set of paths should be given: each with its canonical identifier, the index's own provenance, every discovery path that reached it with the reason and the confidence, its version, its tier and its cost in estimated tokens — followed by everything reached and not given with the reason for each, everything that collapsed into one entry, every pair that does not agree, and the per-tier spend against the budget. Deterministic for a given tree and request; the index fingerprint it was compiled from is in the answer.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_devcontext` |
| HTTP | `GET /api/v1/devcontext` |
| CLI | `majordomus devcontext compile` |
| cache | process, 16 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::devcontext |
| tags | context, session, planning, introspection |

| input | type | required | description |
|---|---|---|---|
| `issue` | string or null | no | An issue id (`I0301`) or its canonical identifier. Its milestone, its dependencies,
its declared scope and the decisions over that scope follow from it. |
| `milestone` | string or null | no | A milestone id or slug, or its canonical identifier. |
| `intent` | string or null | no | What the session is trying to do, in words. The only input the compiler infers
from, and every entry it produces says so and carries a confidence below one. |
| `paths` | array | no | Repository-relative paths the work touches, added to whatever the seeds declare. |
| `uris` | array | no | Canonical identifiers to seed with directly, for a request about something that is
neither an issue nor a milestone. |
| `budget_tokens` | integer or null | no | The ceiling in estimated tokens; [`DEFAULT_BUDGET_TOKENS`] when absent. |
| `max_depth` | integer or null | no | How far from a seed the walk goes; [`DEFAULT_MAX_DEPTH`] when absent. |
| `floor` | number or null | no | Relevance below which an entry is reported rather than given; [`DEFAULT_FLOOR`]
when absent. |
| `all_blocking_rules` | boolean or null | no | Every blocking rule of the layer, not only the ones the work reaches. Honest and
usually over budget: the answer then says `over_budget` rather than dropping what it
may not drop. |

Output: `CompiledContext`.

## `devcontext.explain` — Why one thing is or is not in a compiled context

One canonical identifier judged under a request: whether it was selected, reached and excluded, folded into another identifier, held by the index and never reached, or unknown — with the entry, the exclusion or the collapse itself, and the budget the judgement was made under so that `excluded for budget` can be acted on without a second call.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_devcontext_explain` |
| HTTP | `GET /api/v1/devcontext/explain` |
| CLI | `majordomus devcontext explain` |
| cache | process, 8 entries |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::devcontext |
| tags | context, provenance, introspection |

| input | type | required | description |
|---|---|---|---|
| `uri` | string | yes | The canonical identifier to explain. |
| `issue` | string or null | no | An issue id (`I0301`) or its canonical identifier. Its milestone, its dependencies,
its declared scope and the decisions over that scope follow from it. |
| `milestone` | string or null | no | A milestone id or slug, or its canonical identifier. |
| `intent` | string or null | no | What the session is trying to do, in words. The only input the compiler infers
from, and every entry it produces says so and carries a confidence below one. |
| `paths` | array | no | Repository-relative paths the work touches, added to whatever the seeds declare. |
| `uris` | array | no | Canonical identifiers to seed with directly, for a request about something that is
neither an issue nor a milestone. |
| `budget_tokens` | integer or null | no | The ceiling in estimated tokens; [`DEFAULT_BUDGET_TOKENS`] when absent. |
| `max_depth` | integer or null | no | How far from a seed the walk goes; [`DEFAULT_MAX_DEPTH`] when absent. |
| `floor` | number or null | no | Relevance below which an entry is reported rather than given; [`DEFAULT_FLOOR`]
when absent. |
| `all_blocking_rules` | boolean or null | no | Every blocking rule of the layer, not only the ones the work reaches. Honest and
usually over budget: the answer then says `over_budget` rather than dropping what it
may not drop. |

Output: `Explanation`.

## `devcontext.policy` — The compiler's own rules

What the compiler decides and how: the tiers in the order the budget spends in with the kinds that land in each, every edge kind the composed graph declares with the relevance multiplier the compiler follows it by in each direction — or the reason it refuses to follow it at all — the selectors with which of them infer rather than read, and the defaults for the budget, the depth and the relevance floor. Read this rather than inferring the policy from an answer.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_devcontext_policy` |
| MCP resource | `majordomus://devcontext/policy` |
| HTTP | `GET /api/v1/devcontext/policy` |
| CLI | `majordomus devcontext policy` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::devcontext |
| tags | context, introspection |

Input: none.

Output: `CompilerPolicy`.

