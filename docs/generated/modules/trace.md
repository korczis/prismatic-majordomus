<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `trace` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `trace` — Traceability

Which branches and commits realised an issue, and which issue and milestone a commit served — derived from git and from the canonical project model on every call, stored nowhere. A branch names an issue when one of its path components is an issue id; a commit belongs to the issue whose branches hold it; a commit no such branch holds is reported as unattributed rather than left out, because work with no execution contract is what a traceability report exists to make visible. Pull requests are a GitHub fact and this executable makes no network call: `scripts/traceability` reads them and joins them to this answer over the branch name.

Stability: behaviorally_verified. Capabilities: 3.

## `trace.commit` — What contract this commit served

One commit with the issue and milestone it served, or the fact that none can be found. Attributed when exactly one issue's branches hold it, ambiguous when branches naming two issues do, and unattributed when no branch naming an issue holds it at all — which is either work committed without an execution contract or a branch deleted after its merge, and the answer says so rather than guessing between them.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_trace_commit` |
| HTTP | `GET /api/v1/trace/commit` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::trace |
| tags | trace, git, plan, commit, provenance |

| input | type | required | description |
|---|---|---|---|
| `commit` | string | yes | Anything git resolves to a commit: a full or abbreviated object name, a ref, `HEAD`,
`master~3`. |

Output: `CommitAttribution`.

## `trace.issue` — What realised this issue

One issue with the branches that name it — local, and remote-tracking where only the remote still has the branch — and, for each, the commits it holds that the trunk did not: measured against the trunk while the branch is open, and against the first parent of the merge commit that brought it in once it is merged. A branch that reached the trunk without a merge commit of its own says so and claims nothing, because its commits cannot be told from the trunk's. The milestone comes from the canonical issue record, which is the one edge here that git does not hold, and `declared` says whether the project model has this id at all — a repository with no plan still gets the branches, and a typo still cannot read as work nobody did.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_trace_issue` |
| HTTP | `GET /api/v1/trace/issue` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::trace |
| tags | trace, git, plan, issue, provenance |

| input | type | required | description |
|---|---|---|---|
| `issue` | string | yes | The issue id, as the project model spells it (`I1305`). An id the model does not
declare is answered with `declared: false` rather than refused, and rather than
answered with a bare empty trace: a typo that read as "nothing realised this issue"
is the one answer this capability must never give. |

Output: `IssueTrace`.

## `trace.report` — The whole work graph above the branch

Every issue at least one branch names with its branches and commits, every declared issue no branch names, and the newest stretch of the trunk with each commit attributed to the issue whose branches hold it or reported as unattributed. The tallies count both sides, so the proportion of the trunk that no execution contract accounts for is a number rather than an impression. Read from git on every call: the history changes outside this process.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_traceability` |
| MCP resource | `majordomus://traceability` |
| HTTP | `GET /api/v1/trace` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::trace |
| tags | trace, git, plan, provenance, introspection |

| input | type | required | description |
|---|---|---|---|
| `limit` | integer or null | no | How many trunk commits to attribute, newest first. Default: 50. Anything above 2000
is read as 2000 — a traceability report is a reading, not a history export. |

Output: `TraceReport`.

