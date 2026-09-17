<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `skills` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `skills` — Skills

Every skill the layer defines as a proven capability: whether a test names it and the evidence ledger holds a current passing run of that test, whether the site projects its page, whether a doctrine and a CI gate hold it, and whether a workflow, prompt, profile, provider template, recipe or CI workflow invokes it. All four are derived on every read and none is authored; an active skill no test names or nothing invokes is an orphan.

Stability: behaviorally_verified. Capabilities: 3.

## `skills.explain` — One skill, with everything derived about it

One skill in full: its authored identity, status and provenance marker, each test that names it with the state of its latest recorded run and the command that reproduces it, its page, its doctrine and gates, every invocation by path and line, its standing and the findings about it. The file itself stays at `majordomus://skill/<id>`.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_skill_explain` |
| HTTP | `GET /api/v1/skills/explain` |
| CLI | `majordomus skills explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::skills |
| tags | skill, evidence |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The skill's id, which is also its directory name. |

Output: `SkillStatus`.

## `skills.status` — Every skill, with its derived standing

Every skill the index holds, each with the tests that name it and the evidence state of their latest runs, its page projection, the doctrine and gates that hold it, every invocation that references it, and the standing those derive: proven, partial, orphan, invalid, or not_required for a draft or deprecated skill.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_skills` |
| MCP resource | `majordomus://skills` |
| HTTP | `GET /api/v1/skills` |
| CLI | `majordomus skills status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::skills |
| tags | skill, evidence |

Input: none.

Output: `SkillStatusList`.

## `skills.verify` — What the skills owe and do not have

Every finding over the skills: a file that breaks the skill contract, an active skill no test names or nothing invokes, a test whose latest run failed, a test or an invocation naming a skill that does not exist — each a failure — and evidence that is not current, a missing page or a missing gate, each a warning.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_skills_verify` |
| HTTP | `GET /api/v1/skills/verify` |
| CLI | `majordomus skills verify` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::skills |
| tags | skill, evidence, validation |

Input: none.

Output: `SkillVerification`.

