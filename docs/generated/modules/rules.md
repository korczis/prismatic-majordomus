<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `rules` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `rules` — Rules

What this repository refuses, and what it can actually show for each refusal. A rule declares a class — blocking, where a gate refuses the work, or advisory, where a reader is expected to have read it — and an enforcement block naming either the validator the dispatcher calls or the behavioural cases that prove it. This module joins that declaration to the tree and to the ledger of recorded runs, and reports, per rule, whether what it names is there, whether a runner owns it, whether it was ever run, whether the run passed, and whether the run is older than what it is about. A blocking rule whose proof is missing, dangling or failing is a finding, because a rule nobody can prove is a rule nobody enforces.

Stability: behaviorally_verified. Capabilities: 3.

## `rules.proves` — What this test proves

One test — by identity or by the path a rule names it with — with every rule that names it and the rules that would be left with no proof at all if it were deleted. This is the reverse of rules.show and reads the same derivation, so the two directions cannot disagree. It is the question that could not be asked while the relation ran one way only: before deleting or renaming a case, what stops being enforced.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_rule_proves` |
| HTTP | `GET /api/v1/rules/proves` |
| CLI | `majordomus rules proves` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::rules |
| tags | rules, governance, tests, verification |

| input | type | required | description |
|---|---|---|---|
| `test` | string | yes | `test/cases/07_scope.sh`, or `suite:07_scope`, or `crate:product`. |

Output: `TestSubjects`.

## `rules.report` — Every rule against the proof there is for it

The whole rule corpus joined to the tree and the ledger: per rule, its class, its enforcement mode, the validator and the cases it names, whether each is in the tree, the execution behind each, the CI gates that run them, and the sentence explaining how the state was derived. The tallies count the whole corpus even when the answer is filtered, and the findings name every rule whose declared class the proof does not support. Read fresh on every call: the ledger is a file that changes outside this process.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_rules` |
| MCP resource | `majordomus://rules` |
| HTTP | `GET /api/v1/rules` |
| CLI | `majordomus rules report` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::rules |
| tags | rules, governance, verification, evidence, gates |

| input | type | required | description |
|---|---|---|---|
| `state` | object | no | Only rules in this proof state (`proven`, `inputs_unchanged`, `stale`, `failing`,
`not_run`, `unrunnable`, `dangling`, `unproven`). Absent: every rule. |
| `class` | object | no | Only rules of this class (`blocking`, `advisory`). Absent: every rule. |
| `namespace` | string or null | no | Only rules of this namespace — `project` for this repository's own, `majordomus` for
the vendored package. Absent: both. |
| `findings_only` | boolean | no | Only the rules whose declared class the proof does not support, with the findings.
The tallies still count the whole corpus, so a filtered answer never misreports how
much of it was examined. |

Output: `RulesReport`.

## `rules.show` — What proves this rule

One rule with its full proof: the validator and the cases it names, the execution behind each, the gates that run them, the rules it depends on and the rules that depend on it — each with the state it is in, because a rule is proven only as far as its dependencies are — and, when it is not proven, the one sentence saying what would have to be true for it to be. A rule this repository does not declare is a not-found rather than an empty answer, because a typo that read as 'this rule has no proof' is the one answer this capability must never give.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_rule` |
| HTTP | `GET /api/v1/rules/rule` |
| CLI | `majordomus rules show` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::rules |
| tags | rules, governance, verification, evidence |

| input | type | required | description |
|---|---|---|---|
| `rule` | string | yes | The rule id, with or without its version: `project.scope-is-declared`, or the
identity the index holds. |

Output: `RuleDetail`.

