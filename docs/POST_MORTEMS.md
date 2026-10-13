# Post-mortems — every failure of the trunk's CI is answered by a record

When a workflow run fails on the trunk, somebody reads the log, finds the cause and fixes
it. A post-mortem is that finding kept as an object of the layer: a Markdown record under
[`.ai/repo/post-mortems/`](../.ai/repo/post-mortems/) that names the runs it answers, the
root cause, the components the cause lives in and the text the failure shows. The rule is
`project.a-ci-failure-has-a-post-mortem`
([`.ai/repo/rules/project/a-ci-failure-has-a-post-mortem.v1.md`](../.ai/repo/rules/project/a-ci-failure-has-a-post-mortem.v1.md)),
the decision ADR 0156. Behaviour as implemented and tested; where this document and the
executable disagree, the document is wrong and changes in the same commit.

The commands and their executable examples are in the generated reference
([`generated/cli.md`](generated/cli.md), under `majordomus postmortem`); the capabilities,
their routes and their benchmark cases in
[`generated/capabilities.md`](generated/capabilities.md), module `postmortems`. Neither is
restated here.

## The record

The schema is `post-mortem/v1`
([`share/schemas/majordomus/post-mortem/post-mortem.v1.proto`](../share/schemas/majordomus/post-mortem/post-mortem.v1.proto)).
The front matter is closed: an unknown key is refused.

| field | what it holds |
|---|---|
| `id` | `pm-`, the day the failure was first seen, a slug; the file name without `.md` |
| `status` | `draft`, `analysed` or `resolved` |
| `runs` | every run the record answers: workflow, run id, commit, failed jobs, the step |
| `cause` | a class (`defect`, `test`, `race`, `gate`, `environment`, `dependency`, `undetermined`) and one sentence |
| `components` | where the cause lives, as references of the layer: `file:<path>`, `test:<path>`, `rule:<identity>` |
| `signals` | text a failing log shows, each with a name |
| `fixed_by` | `commit:` and `pr:` references, on a resolved record |
| `prevention` | references to what guards against a return: `test:`, `file:`, `rule:`, `issue:` |
| `related` | what else of the layer the failure concerns: `issue:`, `milestone:`, `adr:`, `intent:`, `claim:`, `rule:` |

The body has four required sections — What failed, Root cause, Fix, Prevention — and an
optional Timeline.

What the schema cannot say, `majordomus postmortem check` judges: a draft claims no cause,
an analysed or resolved record names at least one component and one signal, a resolved
record and no other names what fixed it, and a path or a test a record names is in the
tree. A sentence nobody verified is written as not established; a draft is a complete
answer to "is this failure known", and a guess is not.

## A record is a node of the graph

`components`, `prevention` and `related` are written the way every reference of the layer
is, `<kind>:<identity>`, and the relation table of the executable (`graph::RELATIONS`)
names what each asserts. So nothing about a post-mortem's links is specific to it: the
entity page of a record lists the rule, the issue, the files and the cases it names, and
the page of each of those lists the record as an incoming relation, on the Cockpit and on
the published site alike. A reference that resolves to nothing is refused by the same check
that refuses it for any other kind. `fixed_by` names commits and pull requests, which the
layer does not hold; a commit reaches its record the other way, through the issue it
carries in its `Issue:` trailer and the record's `related`.

## From a red run to a record

1. `majordomus postmortem owed` asks the forge for the trunk's failed and timed-out runs
   and lists those no record names. It exits 10 while one of them is older than the grace
   it prints, and says `observed: false` with the reason when the forge cannot be asked.
2. `majordomus postmortem diagnose` is given the failure's text and names every record
   whose signals occur in it. A match is the record to extend with the new run; no match
   means the failure is new.
3. `majordomus postmortem draft` prints a draft for one run: its workflow, commit, failed
   jobs and step, the status `draft` and the four sections saying nothing is established
   yet. It writes no file.
4. The record is written under `.ai/repo/post-mortems/`, by the session that fixes the
   failure or by one that only reads it, and lands like any change. `scripts/derive`
   publishes its page.

## Where it appears

| surface | what it shows | how it gets there |
|---|---|---|
| command line | `majordomus postmortem` | the module's capabilities |
| HTTP and MCP | the same capabilities, and each record as `majordomus://post-mortem/<id>` | the registry and the index |
| UI | the screen Post-mortems | a reading of `postmortems.list` |
| Cockpit | the records among the objects | the index |
| published site | one page per record, under the Post-mortems section | `site/data/publication.toml` |
| CI | the gate `post-mortems` of the structure job | `.ai/repo/ci/gates.yaml` |

No surface keeps a list of records of its own.

## Hooks

The offline judgement runs wherever the structure job runs. The forge half needs the forge,
so it belongs to the hooks that have it; each is an issue of milestone
`a-failure-is-answered-by-a-record` and is described here when it is on the trunk:

- the trunk's workflow, which on a failed run prints the draft and records what is owed;
- a session's start, where the briefing names the runs that owe a record;
- the gate output, where a failing gate is followed by the records `diagnose` matched.
