<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `postmortems` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.23.0 -->
# Module `postmortems` — Post-mortems

What turned the trunk's CI red and why: one record per cause under .ai/repo/post-mortems/, naming the runs it answers, the cause as a class and a sentence, the components it lives in, the signals a failing log shows, what fixed it and what now prevents it. The records are listed, judged against the rules their schema cannot say, compared with the trunk's failed runs to find the run nobody has answered, and searched for the signals of a new failure. Nothing here changes the repository; the two operations that ask the forge are the command line's alone.

Stability: behaviorally_verified. Capabilities: 5.

## `postmortems.check` — Judge the post-mortems against the rules their schema cannot say

The rules that relate two fields of a record, a record to the tree and one record to another: a draft carries the cause class `undetermined` and a later status does not; an analysed or resolved record names a component and a signal; `fixed_by` is present exactly when the record is resolved; a component or a prevention that names a path (`file:`, `test:`) names one in the tree (a resolved record may name a component its fix removed); no two records share an identity. Whether a reference to an object of the layer resolves is the graph's question, answered for every kind by the one check of unresolved relations, and is not asked again here. Each finding names the record, a stable code and one sentence. Reads the index and the tree; asks no network.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_postmortems_check` |
| HTTP | `GET /api/v1/post-mortems/check` |
| CLI | `majordomus postmortem check` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::postmortems |
| tags | post-mortem, ci, verification |

Input: none.

Output: `PostMortemCheck`.

## `postmortems.diagnose` — Which post-mortems a failure looks like

Searches the text of a failure — a failing log, a job's summary, an excerpt — for the signals of every record, ignoring case and matching each signal as written. Every record with at least one signal in the text is returned with its status, cause, the signals found, its components, what fixed it, what now prevents it and what else it concerned, the record with the most signals first and among equals the newest. No weighting and no closeness: a record is named because its own words are in the log. Reads the index and nothing else.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_postmortems_diagnose` |
| HTTP | `GET /api/v1/post-mortems/diagnose` |
| CLI | `majordomus postmortem diagnose` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::postmortems |
| tags | post-mortem, ci, diagnosis |

| input | type | required | description |
|---|---|---|---|
| `text` | string | yes | The failing log, a job's summary, or any excerpt of either. |

Output: `PostMortemDiagnosis`.

## `postmortems.draft` — Draft a post-mortem for one failed run

Asks the forge, through its own command-line tool, for one run and its failed jobs, and returns the text of a `draft` record for it: the front matter the schema asks for, with the cause class `undetermined`, and the four required sections, each saying that nothing is established yet. The identity is `pm-`, the day of the run, the workflow and the run's identifier. The text is returned, with the path a record of that identity lives at and the records that already name the run; nothing is written. When `runs` is given the run is looked up there and the forge is not asked. Exit 0 when drafted, 12 when the forge could not be asked.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| CLI | `majordomus postmortem draft` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::postmortems |
| tags | post-mortem, ci, forge |

| input | type | required | description |
|---|---|---|---|
| `run` | string | yes | The forge's identifier of the run: digits. |
| `runs` | array or null | no | Runs in the shape `gh run list --json` prints them. When given, the run is looked up
among them and the forge is not asked. |
| `jobs` | array | no | The names of the jobs that failed, for a run given in `runs`. |

Output: `PostMortemDraft`.

## `postmortems.list` — Every post-mortem, newest first

Every record under .ai/repo/post-mortems/ with its status, day, cause class and summary, the runs it answers, its components, what fixed and what prevents it, what else of the layer it concerns, how many signals it carries and its path, with the counts by status and by cause class. A file indexed as a post-mortem whose front matter does not read as one is listed apart, with the reason. Reads the index and nothing else.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_postmortems` |
| MCP resource | `majordomus://post-mortems` |
| HTTP | `GET /api/v1/post-mortems` |
| CLI | `majordomus postmortem list` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::postmortems |
| tags | post-mortem, ci, evidence |

Input: none.

Output: `PostMortemList`.

## `postmortems.owed` — The trunk's failed runs no post-mortem answers

Asks the forge, through its own command-line tool and with the caller's credential, for the newest runs of the trunk (branch `master`, 60 runs, unless named), and returns every run that ended in `failure` or `timed_out` since the rule came into force and that no record names in `runs[].id`, each with its page, its age and whether it is still within its grace (24 hours unless named). A cancelled, skipped or green run owes nothing. When the forge cannot be asked the answer is `observed: false` with the reason and owes nothing: an unobserved forge is said, never guessed. When `runs` is given the forge is not asked. Exit 0 when nothing is owed past its grace, 10 when something is.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| CLI | `majordomus postmortem owed` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::postmortems |
| tags | post-mortem, ci, forge |

| input | type | required | description |
|---|---|---|---|
| `branch` | string or null | no | The branch whose runs are asked about; `master` when absent. |
| `limit` | integer or null | no | How many of its newest runs the forge is asked for; 60 when absent, 500 at most. |
| `grace_hours` | integer or null | no | How many hours after a run finished its record may still be missing; 24 when absent. |
| `since` | string or null | no | The day (`YYYY-MM-DD`) or moment (RFC 3339) before which no run owes anything;
`2026-10-13` when absent. |
| `workflows` | array | no | Only these workflows, by the name the forge reports; every workflow when empty. |
| `now` | string or null | no | The moment ages are measured from, RFC 3339; the clock when absent. |
| `runs` | array or null | no | The runs, in the shape `gh run list --json` prints them. When given, the forge is
not asked. |

Output: `PostMortemOwed`.

