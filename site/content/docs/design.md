+++
title = "Prismatic Majordomus"
description = "the v0.1 specification: problem, thesis, models, boundaries, what is intentionally absent"
weight = 9
[extra]
source = "docs/DESIGN.md"
+++

{% raw %}

A lightweight supervisory control layer for AI-assisted work.

This document is the v0.1 product specification. It was written after studying a
large body of accumulated engineering practice around AI coding workers: instruction
files, session-continuity protocols, policy corpora, enforcement hooks, parallel-work
tooling, audit ledgers, retrospectives, and several abandoned attempts at exactly this
kind of tool. The source material is private and is not reproduced here. What is
reproduced is the shape of what failed, the magnitude of the failures, and the small
number of mechanisms that demonstrably held.

This document describes the design; `CLI.md` and `SCHEMAS.md` describe what v0.1
actually does, and `test/cases/` proves it.

---

## Problem

Teams put capable AI workers to work and then operate them with none of the controls
they would insist on for a human team. The failures are operational, not linguistic,
and they recur regardless of which model or vendor is involved:

- **Instructions rot silently.** In one workspace of roughly twenty repositories, the
  instruction file for one AI tool and the instruction file for another tool in the same
  repository shared between 0 % and 9 % of their content. They were not duplicates. They
  were disjoint rule sets. Which rules applied depended on which worker was hired.
- **Always-loaded context has no ceiling.** One repository's always-loaded operating
  contract oscillated between 0 and about 1,100 lines across roughly 200 hand edits
  before a hard budget with a failing check pinned it under 50.
- **Enforcement is declared, not wired.** Across the material studied, at least
  seventeen independent instances were found of a check that was documented as
  blocking, existed on disk, was executable, and was invoked by nothing. In one case
  34 of 39 pre-commit scripts were orphaned while the policy files naming them read
  "BLOCKING". In another, a catalogue of 546 agents had exactly one entry actually
  registered with the runtime. In a third, a well-engineered guard hook lost its
  registration when unrelated tooling rewrote a settings file; the hook, its tests, and
  the documentation citing it all remained in place.
- **Isolation without coordination defers the collision.** With one git worktree per
  worker fully in force, forty worktrees still produced about 3,200 concurrently
  modified files and 67 clusters of duplicated patches. Only 9 of 80 worktrees ever
  merged. The scope-claim step that would have prevented this was optional, was matched
  by exact string equality, and was enforced by hooks that never ran.
- **Transcripts become the database.** Session notes grew to 10 GB in one repository.
  Recovery of working state after a session ended required a hand-written fifteen to
  thirty minute runbook. Notes had filename dates that disagreed with their content
  dates, and nothing checked either against git.
- **Nothing measures AI spend on AI-assisted development.** The same team that built a
  competent cost profiler for its product's own model calls had zero telemetry on what
  its coding workers consumed. Every "35 % better" and "50 % faster" figure in its
  routing documentation had no benchmark behind it.
- **Supervisory tooling is abandoned within days.** Four separate attempts at a
  control layer were found in one workspace. Each was abandoned not because the idea was
  wrong but because nothing detected when it stopped working, and nothing made the next
  session cheaper than ignoring it.

These are control problems. Better prompts do not fix them.

## Product Thesis

**AI workers should not manage themselves.**

Majordomus is the layer that decides what a worker is told, what it may touch, when it
is done, and whether its claim of being done can be believed. It does not perform the
work and it does not invoke a model. It supervises.

The single most important property, taken directly from the evidence: **a declaration
of enforcement that is not mechanically reconciled against what actually runs is worse
than no declaration.** It produces a policy document, a compliance report, and a false
sense of coverage, and then it decays. Majordomus therefore treats "is this check
actually wired?" as its primary deterministic guarantee, applied to itself first.

## Optional Complexity

Simple things trivial, complex things possible, weird things hackable, in that order. The
core is the small set of concepts [`CONCEPTS.md`](@/docs/concepts.md) names, and it stays closed;
extension adds instances, validators, templates and integrations around those concepts, at
the edges, and never a concept. Mechanisms stay stable and meaning may be data-driven,
without becoming a framework for defining frameworks. The rule, with the review questions
an architectural change must answer and the anti-patterns it refuses, is
`.ai/repo/rules/project/optional-complexity.v1.md`; this document does not restate it.

## Users

- An individual engineer running one or more AI coding tools against a repository,
  who wants the same rules applied regardless of which tool is open.
- A small team where several people and several AI workers touch one codebase, who
  need to know who is working on what and whether two workers are about to collide.
- An engineering lead who wants to know whether AI work satisfied its contract before
  accepting it, without reading a transcript.

Majordomus is not for organisations that need a hosted control plane, SSO, or
cross-repository governance. Those are expansion paths, not v0.1.

## Non-Goals

Majordomus v0.1 does not:

- invoke, proxy, or wrap any model
- measure actual token spend or cost
- route requests dynamically
- run autonomous agents or orchestrate them
- monitor anything in the background
- provide a server, database, web UI, daemon, queue, or hosted service
- compact, summarise, or store conversation transcripts
- define agents, personas, roles, tiers, or a registry of named AI workers

The last item is deliberate. The material studied showed that a supervisory layer which
adds nouns becomes the thing it was meant to supervise.

## Core Mental Model

```
            Human / Organisation
                    |
                    v
               MAJORDOMUS
        policy · state · verification
                    |
      +-------------+-------------+
      v             v             v
   Worker A      Worker B      Worker C
  (Claude Code)  (Codex)       (Gemini, Cursor, local ...)
      |             |             |
      +-------------+-------------+
                    |
                    v
          Verified, accepted outcomes
```

Majordomus sits between the person and the workers. It holds one canonical policy,
projects it into whatever instruction format each worker reads, keeps durable state
outside every conversation, and refuses to call work finished until the finish
contract is satisfied.

Two supporting principles run through every part of the design:

1. **Git is the authority; everything else is an aid.** Identity fields on any record
   are computed from git, never authored. A worker will hallucinate a branch name.
   `git rev-parse` will not.
2. **Numbers in prose are computed or forbidden.** A count written into an
   always-loaded file is stale within days. Majordomus writes the command that
   produces the number, never the number.

## Responsibilities

### Policy

Maintain one canonical, versioned operating policy for AI work in the repository.
Detect when the policy and its projections have diverged. Detect when the policy
declares an enforcement that does not resolve to something that runs.

### Context

Define the minimum sufficient context for a task as a small set of named toggles,
not as a corpus. Keep always-loaded material under a hard budget with a failing check.
Ensure anything not explicitly scoped is counted as always-loaded, because that is
what it is.

### State

Keep durable task state in a small number of typed files outside any conversation.
Every record carries the git state it was written at, so that staleness is computed
at read time rather than trusted at write time.

### Watch

Deterministically inspect state, policy, and projections for drift. Report only what
can be proven. Every finding carries the command that reproduces it.

### Coordination

Represent who is working on what. Normalise scope claims so that overlap detection is
about paths, not strings. Compare claims against what was actually touched, because
every collision in the evidence was invisible to a declaration-only check.

### Budget

Separate reasoning effort, output verbosity, context strategy, and model capability
as independent axes. Bundle them under named profiles. Record effort as a delta from
the default, never as an absolute. Treat presentation (terse machine output, engineering
detail, executive summary) as a final layer chosen by the profile, not as a property of
the work itself. Do not claim to measure spend until spend is measured.

### Verification

Define what "done" means before work starts, as a checklist the finish command
evaluates. Distinguish evidence a worker produced from evidence Majordomus computed.

### Handover

Transfer durable state, not transcripts. Write handovers append-only, atomically,
without staging them. Resolve them on the next session by scope, never by recency
alone.

### Maintenance

Regenerate provider projections deterministically from canonical policy.
Stamp what was generated so that a hand edit or a stale projection is
detectable in one command.

### Termination

Know when a worker should stop: when the finish contract is met, when the task is
blocked on a human, or when the session has exceeded its declared scope.

## Canonical Policy Model

One file. Small. Human-readable. Provider-neutral.

```yaml
# .ai/repo/policy.yaml
version: 1

context:
  always_loaded_budget_lines: 150
  strategy: minimum-sufficient
  transcript_is_state: false

profiles:
  default: implementation

verification:
  required_for_changes: true
  finish_requires:
    - tests_run
    - state_updated
    - no_open_blockers

handover:
  required_sections: [objective, current_state, next_action]

enforcement:
  - name: finish-contract
    kind: command
    path: bin/majordomus
    args: [finish, --check]
    wired_by: git-hook:pre-push
  - name: instruction-budget
    kind: command
    path: bin/majordomus
    args: [doctor]
    wired_by: git-hook:pre-commit

projections:
  - provider: claude-code
    target: CLAUDE.md
  - provider: codex
    target: AGENTS.md
  - provider: gemini
    target: GEMINI.md
```

Design rules for this schema:

- **Every field is both written and read.** The evidence contained schemas with four
  permanently null fields ported from an earlier system. A field nothing reads is
  removed.
- **The `enforcement` block names artifacts by path and names what wires them.** This
  is the representation the source material got right and never checked. Majordomus
  checks it: each entry must exist, be executable, and be reachable from the named
  dispatcher, or `doctor` fails.
- **Unknown keys are errors.** A typo must fail loudly. Two agents in the source
  material re-entered a linted catalogue carrying four invented keys; nothing stopped
  them.
- **No provider capability lives in the canonical layer.** Model names, tool grants,
  and vendor flags belong in projections.

## Session Model

```
majordomus start <task>
      |
      v
  scope declared (paths)  --> claim normalised, overlap reported
      |
      v
  profile selected        --> effort / verbosity / context / verification fixed
      |
      v
  worker executes         (Majordomus is not involved)
      |
      v
majordomus check          --> state consistent? scope respected? blockers?
      |
      +-- incomplete --> continue, or majordomus handover
      |
      v
majordomus finish         --> finish contract evaluated; refuses if unmet
```

A session is bounded by `start` and either `finish` or `handover`. There is no
"still open from Tuesday" state that Majordomus recognises as healthy. `watch`
reports any task record whose last update predates its declared checkpoint interval.

Majordomus does not hook the worker's runtime in v0.1. It is invoked by the person, by
a git hook, or by the worker following its projected instructions. This is a known
limitation and is stated as such.

## Context Model

Three tiers, each with a declared trigger:

<div class="overflow-x-auto">

| Tier | What | Loaded when | Budget |
|---|---|---|---|
| Always | the projected instruction file | every session | hard line budget, failing check |
| Scoped | rules bound to path globs | a matching file is in play | short; must declare `paths` |
| On demand | procedures with a routing description | the description matches the task | none, but must be reachable |

</div>


Two rules from the evidence that are non-obvious and load-bearing:

- **An artifact with no declared trigger is always-loaded context** and is counted
  against the budget. The source linter's sharpest single line was exactly this.
- **A routing description must discriminate.** "Reviews supervision trees, process
  ownership and restart semantics" routes. "An elite agent for high-quality missions"
  does not, and it costs tokens on every decision about whether to load it.

Context composition for a task is a vector of named boolean toggles chosen by the
profile, not a corpus assembled by hand. Majordomus reports what it excluded and why,
because an under-filled context is debugged from the exclusion list.

## Durable State Model

```
.ai/
  README.md               # the protocol, readable without the tool
  manifest.yaml           # the section registry and the format version, ai-repository/v1
  repo/                   # tracked: the canonical context every checkout shares
    policy.yaml
    profiles/
      routine.yaml
      implementation.yaml
      debugging.yaml
      deep-work.yaml
    rules/
      vendor/majordomus/  # the pinned baseline, byte for byte the package the tool ships
      project/            # rules this repository wrote
    prompts/  workflows/  skills/  knowledge/  adrs/  project/
  local/                  # ignored: this checkout's own, never normative, never shared
    state/
      current.yaml        # the one active task, or absent
      decisions.md        # append-only, dated
      open-questions.md   # things blocked on a human
      handovers/          # append-only, one file per handover
      checkpoints/        # append-only, one file per checkpoint
      sessions/           # closed session envelopes
      ledger.jsonl        # append-only events, retention-capped
    cache/  prompts/  session-contexts/
```

Rules:

- **Two halves, one boundary.** `repo/` is the whole shared contract; a fresh clone
  contains it and nothing else. `local/` is operational state: it never travels through
  git, is never loaded into a model's context implicitly, and outranks nothing. `doctor`
  proves the boundary: the manifest resolves, every section it names exists, `local/`
  is ignored and nothing under it is tracked.
- **Identity is computed.** `repository_id`, `branch`, `head`, `working_tree`, and
  `changed_files` on any record come from git at write time. The projected instructions
  forbid the worker from authoring them.
- **State is read with a divergence label.** On read, Majordomus compares the recorded
  `head` with the current `HEAD` via merge-base and labels the record `exact`,
  `advanced`, `diverged`, or `different_context`. Stale state is detected and named,
  never silently trusted.
- **No status enum in v0.1 unless it is validated.** The source material contained four
  mutually inconsistent, unenforced status vocabularies. Where a lifecycle state is
  recorded, its values are closed and checked.
- **Append-only stores have a retention cap.** One repository accumulated 10 GB of
  metrics snapshots that nothing ever read. `ledger.jsonl` and `handovers/` declare a
  cap and `doctor` reports when it is exceeded.
- **Records are never staged or committed by Majordomus.** The writer creates one new
  file atomically under `local/` and prints its path. A record reaches another checkout
  only by being carried there — a handover names it, git does not move it.
- **Transcripts are never stored.** A handover has required sections (`objective`,
  `current_state`, `next_action`) and is refused at write time if any is empty.

## Parallel Work Model

Scope: **one isolated working tree per concurrent autonomous writer.** With one agent,
or with agents that run strictly sequentially, plain branches are correct and the
coordination machinery is overhead. The README says this rather than overselling.

Mechanism:

- `start` takes a scope: a list of paths. Claims are normalised at claim time: trailing
  slashes stripped, containment tested in both directions. One matching function is
  used by every consumer.
- The claim is not optional. If a step can be skipped it will be; the evidence showed
  10 of 18 active worktrees with empty claims. The claim is part of `start`.
- Overlap is reported on **touched files**, computed from `git status` and
  `git log <base>..HEAD --name-only` in each worktree, not on declarations alone.
- A tiny JSON sidecar records `id`, `path`, `branch`, `base_commit`, `created_at`,
  `owner`, `status`, `claimed_paths`. Eight fields. Nothing that was null in every live
  record of the source system.
- `git worktree list` is the truth. The sidecar is rebuildable from git in one command.
- Majordomus emits the merge commands. It does not own the merge. The source
  implementation silently discarded unpushed commits on the target branch.
- `owner` is a free-form string. A closed enum was already violated in the live
  registry that defined it.

## Verification Model

"Done" is a contract evaluated by `finish`, not a word a worker types.

```
done =
    scope respected            (touched files within claimed paths)
  + verification command ran   (exit 0, recorded with timestamp and command)
  + state updated              (current.yaml reflects completion)
  + no open blockers           (open-questions.md has no unresolved entry for this task)
  + handover or completion note present with required sections
```

Every completion note and every ledger record carries a **typed outcome** from a closed,
validated vocabulary:

```
completed   objective satisfied and the finish contract met
partial     some of the objective delivered; remainder named in next_action
blocked     cannot proceed without a human decision; recorded in open-questions
no_match    the work was done and the thing sought does not exist
failed      the work could not be done; the reason is recorded
```

`no_match` and `failed` look alike in a transcript ("we found nothing" versus "we could
not search"). Operationally they are different facts, and a supervisor that cannot tell
them apart cannot decide whether to retry, escalate, or accept. Prose never substitutes
for the typed field.

Distinctions the evidence made necessary:

- **Guaranteed versus observed.** A check is either deterministic and blocking, or it
  is a report with a reproduce command. A rule's class, `blocking` or `advisory`, is
  read at dispatch time and routes the finding; there is no third class, no severity
  ladder and no override, because every graded tier in the source material decayed into
  decoration within months. An advisory rule is one the tool reports and does not stop
  on, and it says so; it never describes itself as enforcement.
- **A trailer proves authorship of a string, never that a gate ran.** Majordomus never
  treats a commit-message marker as evidence. Evidence is a record in `ledger.jsonl`
  written by the gate itself.
- **Exit codes are a contract.** `0` pass; distinct non-zero codes for contract unmet,
  missing artifact, internal error, and refused override. A hook that receives any
  non-zero code and continues is a contract violation, and `doctor` scans hook scripts
  for `|| true` and `|| exit 0` around Majordomus invocations.
- **Fail closed on ambiguity.** If it is unclear whether a check passed, it failed.
- **Existing dirt is ratcheted, new dirt is blocked.** For any countable metric,
  a baseline file holds one integer. Increase fails; decrease or flat passes; a missing
  baseline warns and passes, because failing with no baseline trains a team to bypass.
  Single-instance catastrophic classes (secrets, remote code execution) are never
  ratcheted; they are hard-zero.

## Provider Projection Model

```
.ai/repo/policy.yaml
         |
         |  majordomus update
         v
   +-----+-----+-------------+
   |           |             |
CLAUDE.md   AGENTS.md    GEMINI.md   (+ .cursor/rules, generic)
```

- Projections are generated, not hand-edited. Each carries a header stating that it
  is generated and naming the command that regenerates it.
- every projection carries a stamp: the policy hash it was rendered from and the hash
  of its own content, in its first line or its region's begin marker; `doctor` fails on
  a projection whose content differs from its stamp and `watch` reports a projection
  whose stamped policy hash is not the policy on disk (stale).
- A projection is a thin bootstrap, not a rulebook. It tells the worker where the
  repository's context lives — `README.md`, then `.ai/README.md` and its discovery
  protocol — names the default profile and the task-lifecycle workflow, and carries no
  rule of its own. The rules are read from `.ai/repo/rules/` by every worker alike, so a
  rule that exists for one provider and not another cannot happen; `doctor` fails a
  projection that carries a rule corpus of its own (a profile table, rule bullets, a
  rules, lifecycle or finish-contract heading), and a `README.md` that does not name
  `AGENTS.md`.
- Adapters translate that bootstrap into a provider's format and nothing more. The
  templates ship with the tool; a repository that needs a different adapter overrides
  one under `.ai/repo/providers/`, in the same thin shape. The generic adapter produces
  a plain Markdown file for providers with no known convention.

## CLI Model

Per-command behaviour, the exit-code contract, and target output are specified in
[`CLI.md`](@/docs/cli-specification.md); file formats in [`SCHEMAS.md`](@/docs/schemas.md). This section states the
shape.

One executable, `majordomus`, portable POSIX shell in v0.1, with a small set of
semantically distinct subcommands:

<div class="overflow-x-auto">

| Command | Answers | Writes |
|---|---|---|
| `init` | create the `.ai/` layer in this repository | yes, refuses to overwrite; `--extend` adds what is missing |
| `migrate` | move pre-`.ai` project data from `.majordomus/` into `.ai/`, previewed, with a verified backup | git moves, the backup, the index |
| `rules` | the effective rule set, resolved: the vendored baseline plus the repository's own | only `vendor update` |
| `doctor` | is Majordomus itself healthy and actually wired here? | no |
| `start <task>` | begin a scoped task with a profile | `state/current.yaml`, claim |
| `check` | is the current task consistent with policy, scope, and state? `--explain` prints the effective merged policy and profile | no |
| `watch` | what has drifted: state, policy, projections, retention? | no |
| `update` | regenerate projections from policy | projections |
| `handover` | write an append-only continuation record | one new file |
| `finish` | evaluate the finish contract; refuse if unmet | `state/current.yaml`, ledger |

</div>


Rules:

- Every command has documented behaviour, safe defaults, an actionable failure
  message, and behavioural tests in a disposable repository.
- `doctor`, `check`, and `watch` are read-only and side-effect free.
- No command performs recursive deletion, silent overwrite, network access,
  or evaluation of generated text.
- `doctor` runs against the Majordomus installation itself before anything else.
  The 500-line anti-sprawl linter in the source material decayed in nineteen days
  because nothing ran it against its own surface.

## Watch / Doctor Model

**`doctor`** answers "is the supervisory layer real here?" Deterministic checks, all
blocking:

1. `policy.yaml` parses; no unknown keys; version supported.
2. Every `enforcement` entry: path exists, is executable, and is reachable from the
   named `wired_by` dispatcher (the git hook file exists, is executable, and invokes
   the path). Declared-but-not-wired is the default end state; this check is the
   product.
3. Every `projections` target exists, carries a stamp, and matches it.
4. Always-loaded projection is within `always_loaded_budget_lines`.
5. Every repository-relative path referenced from a projection resolves.
6. No hardcoded counts in the always-loaded projection.
7. Retention caps on `ledger.jsonl` and `handovers/` not exceeded.
8. Environment probes recorded: shell version, availability of `git`, `jq`, `yq` or
   equivalents; the report says which optional checks were skipped for lack of a tool.

**`watch`** answers "what has drifted in the work?" Deterministic, reported with
reproduce commands, non-blocking by design because they concern work in progress:

<div class="overflow-x-auto">

| Drift | Detected how |
|---|---|
| Policy drift | a projection's stamp names a policy hash that is not the policy on disk |
| Projection drift | a projection's content differs from the hash its stamp names |
| State drift | `current.yaml` status contradicts ledger, or `head` label is `diverged` |
| Scope drift | touched files outside claimed paths |
| Handover drift | most recent handover lacks a required section, or is older than `current.yaml` |
| Verification drift | `current.yaml` marked complete with no ledger record from `finish` |
| Staleness | `current.yaml` older than the profile's checkpoint interval |
| Retention | append-only stores over cap |

</div>


Every finding is emitted as `<category> <path-or-id> <reproduce-command>`. A finding
without a reproduce command is a bug in Majordomus.

## Security and Privacy

- Local-first. No network calls. No telemetry. No credential collection.
- No `eval`, no execution of text that came from a worker or a model.
- Every write is to a path under `.ai/` or to a declared projection target.
  Path arguments are canonicalised and refused if they escape the repository root.
- Overwrite requires an explicit flag; the default is refusal with a message naming the
  existing file.
- Handover files are created with mode `0600`.
- Authorisation inputs to any override are derived by Majordomus or corroborated
  against a real git object. Ambient environment variables never lift a rule. The source
  material's override validator was defeated by exactly that class of input and its
  postmortem is the basis for this rule.
- Bootstrap into an existing repository uses a self-disabling hatch: it is honoured only
  while the ledger does not yet exist, records the event, and is dead thereafter.

## Clean Extraction Boundary

Prismatic Majordomus carries the Prismatic name. It carries nothing else from the
Prismatic Platform.

Excluded absolutely: domain intelligence, investigation workflows, OSINT adapters,
inference machinery, semantic and domain graph machinery, agent societies, internal
component names, internal paths, hostnames, customer or case material, audit contents,
secrets, and the pillar and doctrine vocabulary of the source environment.

One line in that list used to read "graph semantics" without qualification, and it was
too blunt to apply. It forbade two different things at once: a system that infers what
artefacts mean, and a table of the references those artefacts already state. The first
stays out. The second is what `knowledge` is. The boundary is therefore stated as a test
a reader can apply to a proposal:

> Majordomus does not import Prismatic Platform domain, inference, intelligence or
> semantic graph machinery. It may maintain a generic deterministic reference and
> provenance graph derived from its own repository's artifacts, in which every edge
> names the file it was observed in and no edge is inferred from prose.

The test is provenance. An edge that can name the line that states it is a reference; an
edge that exists because something looked related is a semantic claim, and semantic
claims are outside the boundary regardless of how they are computed. Similarity,
embedding, clustering and automatic taxonomy are excluded by the same sentence, because
none of them can name a line.

Direction of dependency is one-way. Prismatic Platform may adopt Majordomus. Majordomus
never imports from, links to, or requires Prismatic Platform. There is no shared code.

What did cross the boundary, by abstraction and re-derivation only:

- the observation that enforcement declarations are never reconciled
- git-derived identity and read-time divergence labelling for state records
- append-only, atomic, never-staged handovers with required sections
- the always-loaded budget with a failing check and pointer integrity
- separation of model, effort, verbosity, and context as independent axes
- claim normalisation and touched-file overlap reporting
- the count-ratchet with the missing-baseline rule and the hard-zero exception
- the exit-code contract and the ban on "warn and continue"
- the self-disabling bootstrap hatch
- the rule that numbers in prose are computed or forbidden

A second pass over the same material, made when the session and knowledge layers were
designed, added these:

- identity derived from a stable source fact, and content hashes used only to detect
  change — never the reverse
- provenance as a required property of every edge, so that a relation nobody can trace
  to a line cannot be constructed at all
- classification from structure and declared metadata, never from prose, with `unknown`
  as a first-class answer rather than a plausible default
- gating only on defects the run itself caused, and reporting everything else, because a
  permanently red report is one nobody reads
- distinguishing an absent artefact, which is a legitimate first run, from a corrupt one,
  which must fail loudly rather than be treated as absent
- an ordered, de-duplicated generated artefact, so that what a rebuild changed is visible
  in a diff
- projecting a high-volume, low-durability record kind as one index rather than one node
  each

The full second-pass ledger, including what was refused and why, is
[`EXTRACTION_REPORT.md`](@/docs/extraction-report.md) section 11.

## MVP

v0.1 ships exactly:

- `policy.yaml` with the schema above and a validator that rejects unknown keys
- four profiles: `routine`, `implementation`, `debugging`, `deep-work`, each setting
  effort, verbosity, context toggles, verification requirements, and the output
  contract (which fields a completion note must carry) independently
- state templates: current task, decisions, open questions, handover, completion note
- the subcommands `majordomus --help` lists, in portable shell, with behavioural tests
- `doctor` dispatching every doctrine the effective rule set declares, including enforcement wiring reconciliation and the vendored package's integrity
- `watch` with the drift table above
- `update` producing projections for Claude Code, Codex, Gemini, and generic Markdown,
  each stamped with its provenance
- a README that lists, in equal prominence, what v0.1 guarantees, what it only
  observes, and what it refuses to do

Success criterion: a developer who has never seen the project reads the README, runs
`init`, `start`, `check`, `handover`, and `finish` in a scratch repository, and
understands where state lives and what "done" means, in about ten minutes.

## Deferred Features

Deferred, with the reason:

- **Runtime hooks into worker tools** (pre-tool and post-tool clamping of read size,
  output size, subagent count). The mechanism is proven and valuable, but it is
  provider-specific and the one implementation studied was rolled back within days
  because its limits were global constants rather than task-derived. v0.2, as opt-in
  adapters with profile-derived limits.
- **Token and cost measurement.** Nothing in v0.1 can measure it honestly. Any
  `estimated_` field is labelled as such and excluded from enforcement.
- **Routing recommendations.** Require measurement first.
- **Dependency edges between tasks.** No real use case in the evidence justified a
  graph. Deferred until one does.
- **Multi-repository or organisational policy.** Out of scope for a local-first tool.
- **Any daemon, scheduler, or background monitor.** An MCP surface was deferred here
  with them and later added as something else: a read-only stdio process the client
  spawns and that dies with it, in `apps/majordomus-cli/`, decided in
  `.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md` and described in `MCP.md`.

## Commercial Expansion Paths

Stated for orientation, not commitment:

<div class="overflow-x-auto">

| Tier | Adds |
|---|---|
| Community | everything in this document, open source |
| Pro | runtime adapters with profile-derived limits; richer projection set |
| Team | shared policy across repositories; overlap reporting across workers and machines |
| Enterprise | audit export, approval workflows, budget enforcement once spend is measured |
| Advisory | workflow audit of an existing AI-assisted engineering practice, before and after |

</div>


The durable asset is not the policy files. It is the eventual loop from policy, through
observed executions and outcomes, back to policy refinement. v0.1 builds the policy and
state half of that loop honestly and leaves the measurement half explicitly unbuilt.

---

## Intentionally Absent

Published so that readers can see what was refused rather than assume it was forgotten:

- no agents, personas, roles, tiers, or registries of named workers
- no minimum fan-out or minimum agent count
- no advisory tier that describes itself as enforcement
- no counts written into any file a worker loads
- no unbounded append-only store
- no status vocabulary that is not validated
- no rename accepted as a fix
- no finding without a reproduce command
- no self-report trusted without an independent check
{% endraw %}
