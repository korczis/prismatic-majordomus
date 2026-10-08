+++
title = "Planning"
description = "milestones as executable outcome specifications, issues as execution contracts, the dependency graph, derived status, execution waves, evidence, and the projections"
weight = 20
[extra]
source = "docs/PLANNING.md"
+++

{% raw %}

This document explains what the model means. It contains no current figures: what is true
right now is printed by `majordomus plan status` and rendered on the website's roadmap from
the same data. If you want to know how many issues are ready, run the command.

`majordomus start` supervises one task. This layer supervises the thing a task belongs to.

## The problem

A task record says what one session is doing. It does not say why that work exists, what it
depends on, what would prove it finished, or what should be done after it. That knowledge
lived in conversation, and conversation does not survive a session, a provider switch, or a
handover to a person.

Three failures follow. Work gets picked from memory rather than from a dependency order, so
something is started before the thing it needs. "Done" gets asserted rather than shown.
And the plan exists in several places at once — a chat, a README paragraph, a hand-drawn
diagram, a GitHub milestone — which disagree the first time one of them changes.

## The two records

A **milestone** is an executable specification of an outcome. Not a folder for tickets: a
statement of the problem, the outcome that ends it, the current and desired state, what is in
scope and what is deliberately not, the criteria that would make it true, the validation that
would demonstrate it, and the evidence required to accept it. One file under
`.ai/repo/project/milestones/`.

An **issue** is a bounded execution contract. It carries enough for a worker with no
conversation history to execute it: the objective, why it exists, the current and desired
state, the paths it may touch, what it depends on, the acceptance criteria, the validation
commands, and the evidence its completion requires. One file under
`.ai/repo/project/issues/`.

Both are hand-written YAML in the same restricted subset the policy uses, and both are
checked against an allowlist: a key nobody reads is an error, not a comment.

An issue names its milestone. A milestone does not list its issues. One direction means the
two can never disagree about which issues belong to which outcome.

## Status is derived

Neither record has a `status` field. Writing one is an unknown key and fails validation.

An issue records what happened to it — `started_at`, `verified_at`, `completed_at`,
`cancelled`, and its `evidence` — and the status follows:

<div class="overflow-x-auto" tabindex="0">

| status | when |
|---|---|
| `CANCELLED` | `cancelled: true` |
| `DONE` | `completed_at` is set and every `evidence_required` token has evidence |
| `VERIFY` | implementation is claimed complete and the evidence is not yet sufficient |
| `ACTIVE` | `started_at` is set and nothing further is recorded |
| `BLOCKED` | a dependency is not `DONE` |
| `READY` | nothing above applies |

</div>


A stamp counts only when its transition wrote it. `majordomus plan start|verify|done` (and
the `plan.transition` capability) write the stamp together with its seal — `started_event`,
`verified_event`, `completed_event`: the SHA-256 of the event, the issue and the stamp — and
append the `plan_*` event to the checkout's ledger. The ledger is never tracked, so the seal
is the part of the event a clone can check. A stamp without its seal moves no status and is a
`stamp_without_event` failure of `plan validate`; the stamps recorded before seals existed
are named in each record's `unsealed_stamps`, a list that only shrinks
(`project.no-plan-stamp-without-its-event`, ADR 0097). `done` is refused unless the issue is
ACTIVE or VERIFY.

A milestone's status follows from its issues and its own evidence:

<div class="overflow-x-auto" tabindex="0">

| status | when |
|---|---|
| `PLANNED` | it has no issues, or none of them has moved |
| `ACTIVE` | at least one issue is active, verifying or done |
| `BLOCKED` | it is unfinished and no issue is ready |
| `VERIFY` | every issue that is not cancelled is done and the milestone's own evidence is not complete |
| `DONE` | that evidence is complete too |

</div>


A milestone is never `DONE` because a count of closed issues reached its total. The last step
is the milestone's own acceptance, and it is evidence-gated like every other step.

The **active milestone** is derived the same way: the lowest-ordered milestone that is
`ACTIVE`, or failing that the lowest-ordered one that is not finished. Nothing declares it.

## The graph

`depends_on` is a list of issue ids. The graph they form is validated, not trusted. Each of
these is a distinct finding naming the issue that caused it:

- a dependency on an issue that does not exist
- an issue that depends on itself
- the same dependency named twice
- an issue naming a milestone that does not exist
- a cycle, with every issue trapped in or behind it named
- an issue that is `ACTIVE`, `VERIFY` or `DONE` while a dependency is not `DONE`
- an issue with no acceptance criteria or no validation command — a placeholder

A failing finding makes `majordomus plan validate` and `majordomus doctor` exit non-zero.
`majordomus watch` reports the same violations as drift. Work in progress is reported and
never blocked: an issue that is merely unfinished is not a failure of anything.

## Execution waves

A **wave** is a layer of the graph. Wave zero is every issue with no dependencies; an issue
sits one layer past its deepest dependency. Waves are computed on every read and stored
nowhere, so an execution plan cannot go stale — changing one edge moves every wave that
depends on it, immediately, on every surface.

Sharing a wave is necessary for two issues to run concurrently. It is not sufficient. If
their declared `scope` paths overlap — one equal to, inside, or containing another — the
overlap is reported and they serialise. The direction is deliberate: a false serialisation
costs time, a false parallel costs a conflict discovered after the work is done.

## Evidence

An issue declares `evidence_required` as a list of tokens. `majordomus plan evidence`
attaches one piece of evidence against one token and refuses a token the issue does not
declare. It also refuses without a command or an artifact — narrative is not evidence.

`majordomus plan done` refuses while any token is uncovered. An issue whose `completed_at`
is set but whose evidence is incomplete derives `VERIFY`, never `DONE`. The evidence lives in
the issue's own file, beside the contract it satisfies, with the commit it was recorded at.

What this does not do: rerun the command. The tool records what a worker says a command
produced. The commit hash stored beside it is what makes a false record checkable later.

## What the work is for: intents, coverage, and the two planning records

Above the milestones sits the **intent** (ADR 0070): what must become true, the invariants
that must stay true, and the satisfaction criteria that settle it, each naming the evidence
that decides it. An intent names the milestones that realise it. It stores no status: its
stage follows the plan and its satisfaction follows the evidence ledger.

Beside the stage, every surface carries the intent's **verdict** (ADR 0107), which the criteria
derive alone and the plan never touches: `satisfied` when every criterion is met, `unsatisfied`
when a `test` or `claim` criterion — a kind the ledger or the claim join can settle — is not,
and `unknown` when only `command` or `deployment` criteria are unmet, or none is declared. Its
`reasons` name each unmet criterion with its evidence kind and state. The stage keeps its
meaning, so `satisfied` still requires every milestone DONE; the verdict can read `satisfied`
while a milestone is open, and nothing a finished task does moves it.

A criterion is met only by evidence that is *current*, and current is the evidence module's
own judgement at the working tree (`evidence::freshness`, read through `evidence::current`),
the one the claim report and the evidence pages make: a passing run whose commit is in this
history, that measured a tree that was its commit, whose test still hashes to what ran, and
since which nothing the criterion names has changed — for a test, its source and the source
and implementation of every claim that test proves; for a claim, what the claim names. A
change the criterion does not name, the evidence ledger's own included, leaves it met. A test
that proves no claim naming an implementation names no code under test, so for it any change
since the run but the ledger's is one nothing rules out: such a criterion is met only by a run
at the checkout's own commit. To keep a criterion met across unrelated work, name its test in
a claim of `docs/CLAIMS.yaml` that names the implementation. A pass recorded on a dirty tree,
or one whose inputs moved, is `stale`; a failed run is `failing`; either un-meets a criterion
that was met, and recording a current pass meets it again. Each criterion carries `proof`,
the evidence module's verdict behind its state, so a met criterion still says whether it is
`proven` at this revision or rests on `inputs_unchanged`.

That relates an intent to milestones and to evidence. What relates a *criterion* to the work
meant to make it true is `serves` on the issue (ADR 0073):

```yaml
serves:
  - intent-planning#criteria-are-covered
```

The link is authored on the issue, which is written after the intent, so replanning never
edits an intent. The plan carries `serves` as data and derives no finding from it; the intent
engine does the judging, and `majordomus intent coverage` prints it:

```text
CRITERION                             STRENGTH   ISSUES
intent-planning#criteria-are-covered  covered    I1901
```

A criterion is `covered` when a live issue that requires evidence serves it, `weak` when every
serving issue requires none, `observed` when the recorded gap saw it already true, and
`uncovered` when nothing in the plan will make it true. Every issue also answers why it
exists: `intent` when it serves a criterion, `maintenance` when its milestone is named by no
intent — legitimate operational work, stated as such and never given an invented intent — or
`unexplained` when its milestone realises an intent and it serves nothing.

`majordomus intent validate` refuses (exit 10):

<div class="overflow-x-auto" tabindex="0">

| finding | what it means |
|---|---|
| `criterion_uncovered` | the intent has work, and this criterion has none of it |
| `issue_without_purpose` | the issue sits under an intent's milestone and serves nothing |
| `serves_outside_milestone` | the issue serves a criterion of an intent that does not name its milestone |
| `serves_unknown_intent`, `serves_unknown_criterion`, `malformed_serves` | the link points at nothing |

</div>


and warns on `intent_not_planned` (no live issue under any milestone it names — where every
intent starts), `criterion_weakly_covered`, `duplicate_work` and `milestone_contributes_nothing`.

### The gap and the critique

Planning starts from two records rather than from a prompt, so that what a worker observed and
what a review found survive the session that produced them.

A **gap**, one per intent under `.ai/repo/project/gaps/`, states the observations made at an
`observed_at` commit, each with its source, and answers *every* criterion of the intent with
`satisfied`, `missing`, `conflicting` or `unknown`, citing the observations behind it. A gap
that leaves a criterion out is refused; `unknown` must be written rather than omitted, and is
never read as satisfied. A criterion observed `satisfied` with its observations asks for no
planned work — it reads `observed` — while remaining *met* only by evidence.

A **critique**, one per intent under `.ai/repo/project/critiques/`, is the adversarial pass
over the plan: findings classed by the question asked — `missed_requirement`,
`unproven_assumption`, `insufficient_work`, `unnecessary_work`, `regression_risk`,
`surface_missing`, `delivery_verification` — each `blocking` or not, and each resolved `open`,
`planned` into an issue that serves the intent, or `rejected` with a reason. A dismissal
nobody can read is not a resolution.

Work is held to the review of its plan once it has started: an issue serving an intent that is
`ACTIVE`, `VERIFY` or `DONE` while the intent has no critique is `executing_without_critique`,
and while a blocking finding is open, `executing_with_open_blocker`. Both are failures of
`intent validate` and the `intent-check` gate. Whether the start itself is refused is the
policy's choice (`intent.binding`, under [Binding](#binding-what-a-task-serves-asked-before-it-starts)
below): where it is `required`, the issue never becomes `ACTIVE`.

**A reviewer writes the findings; Majordomus runs the structural half and stamps the review.**
A person or a worker does the observing and the criticising, and the
repository refuses the result when it does not hold together. Nothing here derives a plan from
an intent automatically.

### Opposition: the review is run, and it is of one plan

`majordomus-cli intent oppose <intent>` is the review as something the tool does (ADR 0112),
and the brief a reviewing session works from; `intent_opposition.review` answers the same
value over HTTP and MCP. It carries the plan a reviewer reads — statement, invariants,
non-goals, criteria with the state of their evidence, every live issue serving the intent with
its links, dependencies, scope and required evidence, the gap's conditions — and two kinds of
finding:

<div class="overflow-x-auto" tabindex="0">

| | where it comes from | stored |
|---|---|---|
| `structural` | every finding the coverage, the plan and the gap review derive about this intent now: blocking where that derivation calls it a failure, advisory where a warning | nowhere; derived on every call |
| `recorded` | what a reviewer wrote in the critique, each with its resolution, and optionally its `source` and who resolved it (`resolved_by`) | the critique record |

</div>


One disposition is derived from both and written to no record: `reject` while a structural
finding is blocking or a recorded blocking finding is `open`; `accept_with_required_changes`
when recorded blocking findings are each `planned` into live work or `rejected` with a reason;
`accept` otherwise. The command exits `10` on `reject`. No advisor, model or network is
involved.

`majordomus-cli intent stamp <intent>` records that the review was run: it writes
`reviewed_revision`, `reviewed_at` and `reviewed_with` into the critique — three lines, and
every line of the findings is left as the reviewer wrote it — and appends
`opposition.recorded` to the ledger. The revision is the one the executable derives at that
moment over what a review judges: the statement, the invariants, the non-goals, each criterion
with its evidence kind and reference, each serving issue's own milestone, links, dependencies,
scope and required evidence, and the gap's conditions. A title, an objective, an issue's
status and a file elsewhere in the repository are not in it. An intent with no critique gets a
record with no findings, which needs `--by`; a critique whose own findings do not hold is not
stamped, and a file that is not a readable critique is never replaced.

<div class="overflow-x-auto" tabindex="0">

| the critique is | `intent validate` | the binding |
|---|---|---|
| stamped against the plan as it stands | — | binds |
| stamped against another plan | `critique_stale` | refused, `critique_stale` |
| not stamped | `critique_not_stamped` | binds, unless opposition is required: `opposition_not_executed` |
| current, and a structural finding is blocking | that finding | refused, `plan_rejected` |

</div>


The two findings of `intent validate` are warnings, and failures where the policy says
`intent.opposition: required`; there a `planned` or `rejected` resolution that names no
resolver fails too (`resolution_names_no_resolver`). A stamp is evidence that the command ran
and that the stamp was not edited carelessly afterwards. It is a hash of public content and
proves nothing against an author determined to forge it; what makes the review executed is
that the structural half is derived again at every ask.

### Preflight: may this work proceed, and what is it held to

`majordomus intent preflight --issue <id>` (or `--path <p>`, repeated) is the one join a session
or a transition consumes before work begins; the `intents.preflight` capability answers the
same value over HTTP and MCP. An issue is followed through the criteria it declares in `serves`
— never through its milestone alone — and its links are judged by the coverage `intent
validate` reports from, so the two cannot disagree about which link is broken. The verdict is
one of three:

<div class="overflow-x-auto" tabindex="0">

| verdict | when | exit |
|---|---|---|
| `serves` | every link holds, and each intent served has a critique with no blocking finding open | `0` |
| `maintenance` | the issue serves nothing under a milestone no live intent names, where validation allows it | `0` |
| `refused` | any cause below | `10` |

</div>


<div class="overflow-x-auto" tabindex="0">

| cause | what it means |
|---|---|
| `unknown_issue` | the issue named is not in the plan |
| `no_issue_covers_paths` | no open issue's scope covers any of the paths |
| `issue_serves_nothing` | the issue's milestone realises a live intent and the issue serves none of its criteria |
| `serves_another_intent` | it serves a criterion of an intent that does not name its milestone |
| `serves_unknown_criterion` | it serves an intent or a criterion that does not exist, or a malformed reference |
| `intent_not_critiqued` | an intent it serves has no critique |
| `open_blocking_finding` | the critique of an intent it serves has a blocking finding still `open` |

</div>


With paths, every open issue whose scope covers one is judged and listed with its own
verdict; the answer is the worst of them, so one served issue never vouches for another that
serves nothing. Served work beside maintenance answers `serves`.

The answer carries the intents the work serves and no other: each with its statement, the
served criteria with the live state of their evidence, its invariants, non-goals and
governance, its critique with the blocking findings still open, and its recorded gap bounded
to the served criteria.

### Binding: what a task serves, asked before it starts

The preflight is a question. The binding is who asks it (ADR 0111): `majordomus start`,
`majordomus context`, `majordomus handover` and `majordomus plan start` each ask the
`intents.binding` capability — `majordomus-cli intent binding`, `GET /api/v1/intents/binding`,
`majordomus_intent_binding` — and decide nothing themselves. A task names the work it
executes:

<div class="overflow-x-auto" tabindex="0">

| a task started with | is bound by |
|---|---|
| `--issue <id>` | the criteria that issue serves |
| `--intent <id>` | the open issues serving that intent |
| `--exempt <class> --because "<reason>"` | nothing: a class the policy declares under `intent.exemptions`, and why |
| a scope alone | the open issues whose scope covers a path, when the policy asks at all |

</div>


The standing is `bound`, `maintenance` (the issues sit under milestones no live intent names),
`exempt` or `refused`. A refusal carries one of the preflight's causes, or one only a binding
can find:

<div class="overflow-x-auto" tabindex="0">

| cause | what it means |
|---|---|
| `nothing_named` | no issue, intent, path or exemption was given |
| `unknown_intent`, `intent_retired` | the intent named does not exist, or is cancelled or superseded |
| `intent_has_no_open_work` | no open issue serves the intent named |
| `issue_outside_intent` | the issue named serves no criterion of the intent named beside it |
| `ambiguous_intent` | paths alone reached more than one intent; name the issue |
| `unknown_exemption`, `exemption_without_reason`, `exemption_names_work` | the class is not declared, no reason was given, or an exemption was given beside named work |

</div>


What a refusal costs is the policy's `intent.binding`:

<div class="overflow-x-auto" tabindex="0">

| value | `start` | `plan start` |
|---|---|---|
| absent or `off` | asks only when given `--issue`, `--intent` or `--exempt`; a refused or unreadable answer to that question does not start | unchanged |
| `advisory` | always asks; a refused binding is reported and the task starts | unchanged |
| `required` | a refused or unreadable binding does not start | an issue whose binding is refused does not become ACTIVE, in both engines |

</div>


A binding that cannot be read — no built executable, no `jq` — is *unknown*, and unknown is
never a pass where an answer was required.

The task record and a handover keep what the worker named and two pins, and nothing of the
intent itself: `plan_revision` moves when the intent's statement, invariants, non-goals or
criteria, a link, or the critique is edited; `evidence_standing` moves when a served
criterion's evidence changes state. `majordomus context` prints an INTENT section read from
the binding on every call, and `majordomus handover --resolve` prints `Intent: unchanged`,
`evidence_moved`, `plan_changed` or `unknown`. The issue a task named is a `declared` link in
the realization below.

### Realization: who is making it true, and whether reality agrees

An intent belongs to the project, not to the session working on it. Which work realises it is
therefore never written into a task, a session or a handover; it is joined on every read from
the records the lifecycle already keeps (ADR 0075): the ledger's tasks, each with every episode
that worked on it, the provider of each episode and the handovers between them; the closed
session records; and the claims on the peer board.

```bash
majordomus intent realization                 # every intent, its unmet criteria, the work realising it
majordomus intent realization --intent <id>   # one intent and the work linked to it
majordomus intent explain <id>                # why it stands where it stands, sentence by sentence
```

Every link from work to an issue says how it is known, and a join keeps the strongest per issue:

<div class="overflow-x-auto" tabindex="0">

| provenance | how the link was read |
|------------|-----------------------|
| `declared` | the claim or the task's own words cite the issue |
| `observed` | a plan transition in the work's episode, or a closed session record's `issues` |
| `derived`  | the branch name carries the issue id |
| `inferred` | only when nothing stronger exists: an open issue's scope overlaps the work's |

</div>


So a task a Claude Code episode starts and hands over, and a Codex episode resumes, is one unit
of work with two episodes and both providers, and the intent it serves lists both. The episode's
`session.started` ledger line names its provider, so the lineage survives the episode's end.

Closed work does not outrank evidence. When every milestone of an intent is DONE and a
criterion's latest recorded run is failing, or stale (no longer current, as above), the
realization reports `closed_work_contradicted` naming the criterion, and exits 10; the
`intent-realization` gate runs it. An intent that was satisfied and regresses lands exactly
there: its stage falls back to `verifying`, and nothing about the intent was written for it to.
`closed_work_unproven` (every milestone DONE, a criterion never evidenced) and
`criterion_closed_unmet` (every issue serving a criterion DONE, the criterion unmet) are
warnings, as is live work that serves no intent (`work_serves_no_intent`). Where the stage and
the verdict disagree, two more warnings name it once per intent: `evidence_ahead_of_plan` (the
verdict is `satisfied` while the stage is `planned` or `executing`) and
`closed_work_not_satisfied` (every milestone DONE, the verdict `unsatisfied` or `unknown`, with
the criteria holding it back).

In the Cockpit, `/cockpit/intents` lists every intent with its stage and the work realising it,
and `/cockpit/intents/<id>` shows one: each criterion with its evidence state, linked to the test
object it names and to the issues serving it, and each unit of work with the provenance of its
link. Both pages render the capabilities above and decide nothing themselves.

`test/cases/388_an_intent_is_realised_across_providers_and_held_to_reality.sh` is the loop end
to end: declared, realised across two providers and a handover, closed while one case fails,
fixed, satisfied, broken again and repaired — with the intent file byte-identical throughout.

### What intents do not do yet

Each of these is a `planned` claim in [`CLAIMS.yaml`](@/guarantees/_index.md), and the homepage lists them
from there:

- `intent-closes-github-milestones` — the GitHub projection closes a milestone from its derived
  plan status alone and reads no intent.
- `intent-command-deployment-evidence` — a criterion settled by a `command` or a `deployment`
  resolves and reads `not_derivable`; the ledger records runs of tests and claims only, so such
  a criterion is never met.

## Traceability: what realised an issue, and what an issue realised

The model reaches as far as a branch on its own: a branch path component equal to an issue id
(`feature/I1305-traceability` → `I1305`) is the one edge the topology already reads and the
pre-commit guard already enforces. Everything above it — the commits, the pull requests — is
**derived on every read and stored nowhere**. No canonical record under
`.ai/repo/project/` names a branch, a commit, a pull request or a check run, and none may:
git and GitHub already hold those facts, and a record repeating one is a second truth that
starts rotting the moment history is rewritten.

Two systems hold the two halves, and the boundary between them is the same boundary
`scripts/github-sync` respects — the executable, `bin/`, `lib/`, `share/` and `test/` make no
network call, and `test/cases/08_no_forbidden_constructs.sh` proves it:

<div class="overflow-x-auto" tabindex="0">

| half | source | where it lives |
|---|---|---|
| branches and commits | `git for-each-ref`, `git log` | the `trace` capability module of the Rust executable |
| pull requests | the GitHub API, through `gh` | `scripts/traceability` |

</div>


<pre class="mermaid">
flowchart LR
  issue["issue"]
  subgraph git["derived from git"]
    branch["branch"] --&gt;|contains| commit["commit"]
  end
  subgraph github["derived from GitHub"]
    pr["pull request"]
  end
  issue --&gt;|names| branch
  pr --&gt;|"head branch of"| branch
  milestone["milestone"] --&gt;|"the one edge git does not hold:&lt;br&gt;the canonical issue record declares it"| issue
</pre>


A branch's commits are the commits it holds that the trunk did not: measured against the
trunk while the branch is open, and against the first parent of the merge commit that brought
it in once it is merged. Read backwards, a commit belongs to the issue whose branches hold it.

Three answers are states rather than failures, and each is reported by name rather than
silently dropped:

- **absorbed** — the branch reached the trunk with no merge commit of its own (fast-forwarded,
  or rebased onto it). Its commits cannot be told from the trunk's, so none are claimed and
  the trace says it is incomplete.
- **unattributed** — no branch naming an issue holds the commit, and no pull request's head
  branch names one. That is either work committed with no execution contract or a branch
  deleted after its merge, and the answer says so rather than choosing between them. Work
  with no contract is the thing a traceability report exists to make visible; omitting it
  would defeat the report.
- **ambiguous** — branches naming two different issues hold the same commit. The branch-name
  edge cannot decide, so it does not.

```text
scripts/traceability                 every issue, and the trunk commits nothing accounts for
scripts/traceability --issue I1305   its branches, its commits and its pull requests
scripts/traceability --commit <rev>  the issue and milestone it served, or unattributed
scripts/traceability --pull 42       the same, from the pull request's side
scripts/traceability --no-github     the git half alone: no token, no network
scripts/traceability --strict        exit 10 when something has no contract; the shape of a gate
```

The git half is `majordomus_trace_issue`, `majordomus_trace_commit` and
`majordomus_traceability` on MCP and `/api/v1/trace`, `/api/v1/trace/issue`,
`/api/v1/trace/commit` on HTTP; the script is a client of them rather than a second
implementation. `MJ_GH_FIXTURE_PULLS` reads the pull requests from a file in the shape the
live read produces, so the join is provable with no network and no token — the same seam
`scripts/github-sync` uses, and what `test/cases/98_traceability.sh` exercises.

## Projections

The canonical files are the only source. Everything else is generated from them by one
engine — `lib/project.awk`, loaded by `lib/project.sh` — so no two surfaces can hold
different opinions about what is ready:

<div class="overflow-x-auto" tabindex="0">

| surface | how |
|---|---|
| the command line | `majordomus plan` |
| the Mermaid DAG | `majordomus plan graph` |
| GitHub milestones and issues | `scripts/github-sync`, proved current by `scripts/ci/github-check` |
| the website's roadmap, milestone, issue and DAG pages | `scripts/generate-site-data` |
| the documentation | this file explains the semantics; the figures are generated |

</div>


GitHub is a projection and a place to talk, never the source. A canonical change updates the
generated region of an issue body; a person editing that region is reported as drift and not
overwritten; a person's comments and any text outside the region are never touched. Nothing
is read back: closing an issue on GitHub does not complete it here.

A record is found on GitHub by an identity it carries in its own body, written beside the
hash as `<!-- majordomus:record I0001 -->`. A GitHub number is GitHub's to assign and a
title is a person's to edit, so neither can be the key: matching on a title means a rename
orphans the record and the next `--apply` creates a second one for work that already exists.

Every finding about a body is one of six states, from two independent questions — has a
person rewritten the region since it was posted, and has the canonical record moved since?

<div class="overflow-x-auto" tabindex="0">

| state | meaning | `--apply` |
|---|---|---|
| `insync` | intact and current | nothing |
| `behind` | intact, the canonical record has moved | rewrites |
| `edited` | a person rewrote the region | refuses without `--force` |
| `conflict` | a person rewrote it and the record has moved | refuses without `--force` |
| `adopt` | no identity marker; matched by title, this once | writes one |
| `missing` | no counterpart on GitHub | creates |

</div>


and, of a remote record rather than a canonical one, `unmanaged`: an issue claiming a
canonical id this repository does not have.

Applying is a deliberate act; agreement is a gate. `scripts/ci/github-check` reads the
remote on every change that can move either side, refuses the first six states outright,
and ratchets `missing` and `adopt` against `.ai/repo/ci/github-drift-baseline.txt`, which
may fall and may never rise. It exists because the detector was written, never called, and
the projection decayed to a tenth of the model over five days with every build green
(`project.github-projection-gated@1`).

The network calls live in `scripts/github-sync`, outside the tool. `bin/`, `lib/`, `share/`
and `test/` contain no network client, and `test/cases/08_no_forbidden_constructs.sh` proves
it.

## Working through the model

```bash
majordomus plan status                  # where the outcome stands
majordomus plan next                    # the one issue to take now
majordomus plan show I0007              # the whole contract, no chat history needed
majordomus plan start I0007             # refused unless it is READY
# ... execute, inside the scope the issue declares ...
majordomus plan verify I0007            # implementation complete, evidence pending
majordomus plan evidence I0007 --covers doctrine_test --type test \
  --command "bash test/run.sh 44_model_doctrine" --result "1 passed"
majordomus plan done I0007              # refused while a required token is uncovered
majordomus plan next                    # recomputed, not chosen
```

A worker takes the issue the graph offers. Picking something else needs a reason: it is a
blocker, it invalidates an assumption the milestone rests on, or a person reprioritised.

## Replanning

The plan is expected to change. Add an issue, remove one, split one, merge two, move an edge,
narrow a scope, change a validation — all of it is editing the canonical files, and all of it
is re-validated on the next read. Two constraints survive every edit: the graph stays acyclic
and resolvable, and evidence already recorded is not deleted to make a story tidier.

Replanning is not optional when an issue discovers a dependency nobody predicted, an
acceptance criterion turns out to be untestable, an implementation path is disproven, or an
issue becomes unnecessary. Leaving the graph describing a plan that reality has left behind
is the failure this layer exists to prevent.

## What was rejected

**A `status` field with validation.** Two sources of truth and a rule to reconcile them. The
graph is the only thing that can be right, so it is the only thing that is stored.

**A milestone that lists its issues.** A second edge, in the opposite direction, that can
disagree with the first.

**A separate evidence store.** The contract and the proof that it was met belong in one
record; splitting them lets one be read without the other.

**A mapping file from canonical id to GitHub number.** State that has to be kept in step with
two systems. The id prefixes the title instead, and matching is a string comparison.

**Estimates, velocity, burndown.** None of them changes what a worker should do next, which
is the only question this layer answers.

**`PLANNED` as an issue status.** It would only ever mean `READY` or `BLOCKED` with a
different word on it.

## See also

- [`docs/DOGFOODING.md`](@/docs/dogfooding.md) — why this repository uses the model on itself
- [`docs/SCHEMAS.md`](@/docs/schemas.md) — the fields of each file
- [`docs/CLI.md`](@/docs/cli-specification.md) — `majordomus plan`, subcommand by subcommand
- [`docs/DOCTRINE.md`](@/docs/doctrine.md) — how `majordomus.project-integrity` and `majordomus.dag-integrity` are enforced
{% endraw %}
