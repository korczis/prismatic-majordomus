+++
title = "CLI specification"
description = "every command: behaviour, reads, writes, exit-code contract, target output"
weight = 11
[extra]
source = "docs/CLI.md"
+++

{% raw %}

Behaviour of v0.1 as implemented and tested in `test/cases/`. Where implementation and this
document disagree, the document is wrong and changes in the same commit as the fix.

One executable, `bin/majordomus`, portable shell (bash 3.2 and BSD userland are the floor). Subcommands are dispatched to
sourced modules in `lib/`. Every subcommand accepts `--help` and `--repo <path>`
(default: the git toplevel of the current directory).

## Exit-code contract

Exit codes are part of the interface. A caller, including a git hook, must propagate
them. A hook that receives any non-zero code and continues is a contract violation, and
`doctor` scans hook scripts for `|| true` and `|| exit 0` around Majordomus invocations.

<div class="overflow-x-auto" tabindex="0">

| Code | Name | Meaning |
|---|---|---|
| `0` | `OK` | the command succeeded; for checks, nothing was found |
| `2` | `USAGE` | bad arguments, or not inside a git repository |
| `10` | `CONTRACT_UNMET` | a deterministic contract failed: finish contract, policy validation, wiring |
| `11` | `DRIFT_FOUND` | `watch` found at least one drift; informational, never used by a hook to block |
| `12` | `MISSING_ARTIFACT` | a required file or tool is absent |
| `13` | `INTERNAL_ERROR` | Majordomus itself failed; never silently treated as pass |
| `15` | `REFUSED` | the command declined to overwrite, or an override was rejected |

</div>


There is no "warn and continue" code. Fail closed on ambiguity: if it is unclear whether
a check passed, it failed.

## Output conventions

- Human output on stdout, one finding per line, machine-greppable:
  `<LEVEL> <category> <subject> — <message>  [reproduce: <command>]`
- Levels: `OK`, `INFO`, `WARN`, `FAIL`, `DRIFT`, `REFUSE`. Only `FAIL` and `DRIFT` affect the
  exit code. `WARN` is advice about work in progress and never blocks.
- `--json` on any read-only command emits one JSON object per finding on stdout.
- Nothing is written to stderr except usage errors and internal errors.
- A finding without a reproduce command is a bug in Majordomus.

---

## `majordomus init`

Create the repository's AI layer, `.ai/`, from the tool's skeleton. It installs nothing:
the tool stays wherever it was run from, no hook and no shell file is touched, and
`.majordomus/` is never created.

**Reads:** the skeleton under `share/skeleton/` and the standard rule package under
`share/standard/majordomus/`.
**Writes:** `.ai/README.md` (the protocol, readable without the tool) and
`.ai/manifest.yaml` (the section registry, `ai-repository/v1`); under `.ai/repo/`, the
tracked half: its `README.md`, `policy.yaml`, `profiles/`, `prompts/`, `rules/` with its
`README.md`, the vendored baseline `rules/vendor/majordomus/` (manifest and rule files,
byte for byte the package the tool ships) and an empty `rules/project/`,
`knowledge/sources.yaml` beside an empty `knowledge/curated/`, `workflows/`, `skills/`,
`adrs/`, `project/`; under `.ai/local/`, the checkout's own half: `state/` seeded with
`decisions.md` and `open-questions.md` and the empty `state/handovers/` and
`state/checkpoints/`, plus `cache/`, `prompts/` and `session-contexts/`; and one
`.ai/local/` line in `.gitignore`, added once. Everything under `.ai/repo/` belongs to
the repository from that moment; a newer tool does not rewrite it.

**Refuses** (`15`) when `.ai/` already exists, unless `--extend`, which adds every file
the skeleton ships and the repository lacks and overwrites nothing; (`15`) when `.ai/`
exists without a manifest, naming the choice between moving it aside and `--extend`; and
(`15`) when project data still lives under `.majordomus/`, the pre-`.ai` layout, naming
`majordomus migrate`.

**Does not** install git hooks. It prints the two lines a hook needs and the command
`majordomus doctor` that will verify they were added.

```
$ majordomus init
created
  .ai/README.md
  .ai/manifest.yaml
  .ai/repo/README.md
  .ai/repo/policy.yaml
  .ai/repo/profiles/
  .ai/repo/prompts/
  .ai/repo/rules/README.md
  .ai/repo/rules/vendor/majordomus/
  .ai/repo/knowledge/
  .ai/repo/workflows/
  .ai/repo/skills/
  .ai/repo/adrs/
  .gitignore:.ai/local/
local state: .ai/local/state/ (ignored by git; this checkout's own)
next: majordomus update      # generate the provider instruction files named in the policy
next: majordomus doctor      # verify nothing is declared that is not wired

$ majordomus init
majordomus: .ai/ already exists in … (use --extend to add what is missing; nothing is overwritten)
$ echo $?
15
```

## `majordomus migrate`

Move a repository's project data from the pre-`.ai` layout under `.majordomus/` into the
portable AI layer under `.ai/`, once and explicitly. No ordinary command migrates: `check`,
`doctor`, `start` and the rest refuse (`12`) a legacy layout and name this command.

**Markers:** legacy is `.ai/repo/policy.yaml`; new is `.ai/manifest.yaml`; a
`.majordomus/bin/majordomus` is a tool installation and never project data.

**Reads:** every file under `.majordomus/`, the skeleton manifest (the destinations come
from it), the tool's templates (to tell an unchanged template from a customised one).
**Writes:** `.ai/README.md`, `.ai/manifest.yaml`; the canonical files moved into
`.ai/repo/` (`git mv` where tracked, so history follows); `.ai/local/state/` moved to
`.ai/local/state/` and taken out of the index; the rest of the layer seeded from the
skeleton without overwriting anything that moved; one `.ai/local/` line in `.gitignore`;
a byte-for-byte copy of the state under `tmp/majordomus-migrate-backup/<utc>/state/`,
and of the legacy `providers/` under `.../providers/`, verified and printed, before
either is touched; a `layout.migrated` ledger line; then `update --force` renders the
projections from the tool's thin bootstraps and `doctor` judges the result, each exit
reported on its own line.

**Behaviour:**
- `--dry-run` prints the whole plan, one line per file with its action and destination,
  and writes nothing. The same table drives the real run.
- `templates/*.md` identical to the tool's own are dropped; a customised one moves to
  `.ai/repo/templates/`. `generated/` is dropped: every projection now carries its own stamp.
- `providers/` — the provider body and the monolithic templates that asked for it — is not
  carried into `.ai/`. The body no longer exists anywhere and `update` renders none, so an
  old template would leave a literal token in a generated file. The directory is copied
  to the backup, removed from the index, and reported in one line: the bootstraps are now
  the tool's thin adapters, a repository override goes under `.ai/repo/providers/<provider>.tmpl`
  in the new format, and the body's rules belong under `.ai/repo/rules/project/` as rule
  objects (`DOCTRINE.md` describes the format).
- A file under `.majordomus/` this version does not know is never deleted. It is reported,
  and `.majordomus/` is removed only when it is empty.
- Refuses (`15`) a `.majordomus/` that holds both `policy.yaml` and `bin/majordomus`,
  and names the safe manual step; refuses (`15`) when a destination under `.ai/` already
  exists, listing each one. Nothing is written in either case.
- A legacy policy that does not parse, or is not version 1, is refused (`10`) before
  anything moves: fix it in place first.
- Idempotent: on a repository already on the `.ai` layout it says so and exits `0`. With
  neither layout present it exits `12` and names `init`.
- The state stops being tracked: its durability is the checkout plus the backup, not the
  branch. Commit the tracked half after reviewing `git status`.

```
$ majordomus migrate --dry-run
migrate: .majordomus/ (pre-.ai layout) -> .ai/ (ai-repository/v1)
  move  .ai/repo/policy.yaml -> .ai/repo/policy.yaml
  move  .ai/repo/profiles/debugging.yaml -> .ai/repo/profiles/debugging.yaml
  state .ai/local/state/current.yaml -> .ai/local/state/current.yaml
  drop  .majordomus/templates/handover.md    (identical to the tool's template)
  ...
dry run: nothing written
```

## `majordomus recover`

Bring this checkout's record stores back to what the contract describes: close episodes
nobody ended, fold duplicate records of one episode into one, and classify the stray files
the stores accumulate. Recurring, idempotent maintenance — unlike `migrate`, which is the
one-time move off the pre-`.ai` layout and refuses to run twice.

**Subjects:** `status` (the default, read-only), `episodes`, `records`, `orphans`, `all`.

**Reads:** `.ai/local/state/sessions-open/`, `.ai/local/state/ledger.jsonl`, the closed
record store, and the checkpoint and handover stores.
**Writes:** a closed record per recovered episode under `.ai/repo/sessions/`, the folded
record where duplicates existed, a `session.recovered` ledger line per action, and the
removal of the open records and stray temps it acted on.

**Behaviour:**
- `--check` prints the plan and the evidence behind it and writes nothing. The same
  measurement drives the real run, so the plan is the action.
- A stray file is judged by its age before its content: a publish temp seconds old belongs
  to a `session close` running now, and a `.mj-stage.XXXXXX` minutes old to a
  `scripts/derive` running now. Both are reported as `live` and neither is touched; one
  whose age cannot be read is skipped and counted.
- An episode is *stranded* when its last sign of life — the later of its own `started_at`
  and the newest ledger line it stamped — is older than `session.stranded_after`.
  `--older-than <duration>` overrides the policy for one run.
- Two guards stand between a live episode and the sweep, and neither is the other's
  backstop. The episode this process resolves to is excluded before any predicate runs; and
  a candidate whose evidence cannot be read as a timestamp is skipped and counted, never
  treated as the oldest in the set (`project.destructive-sweeps-fail-closed`). An episode
  belonging to another worktree is reported and never closed here.
- A recovered record carries what the open record and the episode's own ledger lines prove.
  `commits` and `changed_files` are the explicit empty list and `head` and `working_tree`
  are absent: they describe a close that never happened, and the working tree at recovery
  time belongs to whoever is working now. `outcome` is `interrupted`, which is what it was.
- Duplicate records fold into the oldest by `created_at`, with the union of the lists the
  others prove, and the superseded file names recorded in the survivor's body. Records that
  disagree about the episode's identity or times are reported and nothing is folded.
- Nothing is deleted for being unrecognised. A publish temp past the threshold holding the
  only copy of a record is *published*; one whose episode is already published, or which is empty, is
  removed; one holding content this version cannot classify is left exactly where it is and
  counted. A directory somebody put in the checkpoint store is named, measured and left.
- Idempotent. Running a subject twice takes its actions once, and running it after a crash
  mid-run takes the ones that did not happen: the record is written before the open file is
  removed, so an interruption between them costs nothing.

```
$ majordomus recover episodes --check
episodes — open episodes in .ai/local/state/sessions-open
  s-20260910194205-72ea  key=187c9e1e-…  last seen 2026-09-10T19:42:05Z (ledger, 13h ago)
    stranded: 13h ago, over the 12h threshold; close with outcome interrupted
  s-20260911063532-b41b  key=c93d195f-…  last seen 2026-09-11T08:31:02Z (ledger, 0m ago)
    live: this process is inside it; never a candidate

check: 1 action(s) would be taken and nothing was written (run: majordomus recover all)
```

## `majordomus doctor`

Is the supervisory layer real here? Read-only. Blocking by design: intended to run
from a pre-commit hook and from CI.

**Checks, in order:**

1. `policy.yaml` parses; `version` supported; no unknown keys at any level.
2. Every `profiles/*.yaml` parses; every profile referenced by policy exists; no unknown
   keys.
3. **Enforcement wiring.** For every entry in `policy.enforcement`: `path` resolves (on
   `PATH`, repository-relative, absolute, or as an executable path on the hook line
   itself) and the artifact named by `wired_by` exists, is executable, invokes
   `majordomus <first arg>`, and does not swallow its exit code with `|| true` or
   `|| exit 0`. When the hook is a dispatcher, the invocation is looked for in the hook
   file *and* in every file in its `<hook>.d/` directory; the finding names whichever
   file actually carries it. A subhook that carries the invocation but is not executable
   is reported as not wired, because the dispatcher skips it. `wired_by: manual` is
   reported as unverified, never as wired. Declared-but-not-wired is `10`.
4. Every `projections[].target` exists, its provider has a template, and its entry in
   the content matches the stamp it carries. Missing or unstamped → `12`; mismatch → `10` (hand-edited).
   For `mode: region` the hash is taken over the region alone; absent or malformed
   markers are reported as such.
5. The projection marked `always_loaded: true` is within
   `context.always_loaded_budget_lines`.
6. Every repository-relative path referenced from the always-loaded projection resolves.
7. No hardcoded counts in the always-loaded projection (a digit sequence adjacent to
   words like `agents`, `files`, `apps`, `commands`, `skills`, `rules`).

   Checks 5 to 7 judge generated content only. For a region projection that is the
   region and never the host document, so that every failure `doctor` reports can be
   fixed by editing the policy.
8. Retention caps not exceeded on `state/ledger.jsonl` and `state/handovers/`.
9. Environment probes: bash version, `git`, `jq` and `shellcheck` if present. Reported
   as `INFO`. Nothing in `doctor` needs a tool beyond bash, git, and a checksum command.

```
$ majordomus doctor
OK   policy      .ai/repo/policy.yaml — parsed, version 1
OK   profiles    4 files — parsed
FAIL wiring      finish-contract — bin/majordomus is not invoked by .git/hooks/pre-push  [reproduce: grep -n 'majordomus finish' .git/hooks/pre-push]
OK   projection  CLAUDE.md — content matches its stamp
FAIL budget      CLAUDE.md — 212 lines, budget 150  [reproduce: wc -l CLAUDE.md]
OK   links       3 projections — all references resolve
OK   retention   ledger 412 lines, cap 5000
INFO env         bash 3.2.57, git 2.45, yq 4.44, jq 1.7
doctor: 2 failures
$ echo $?
10
```

## `majordomus start <task>`

Begin a scoped task.

**Arguments:** `<task>` one line. `--scope <path>[,<path>...]` required. `--profile
<name>` default from `policy.profiles.default`. `--owner <string>` free-form, default
`$USER`. `--requires <token>[,<token>...]` what the task owes before `completed` is
available (`share/obligations.yaml`). `--issue <id>|none` the issue of the plan this work
serves, or `none` for work the plan does not track, declared as such; recorded as `issue:`
on the task record, and the completion report's `issue` question resolves it against the
plan — a task that names nothing is told it owes one, a task naming an issue the plan does
not hold is refused by that question.

**Reads:** policy, profile, git state.
**Writes:** `state/current.yaml` and one `task.started` line to `state/ledger.jsonl`.

**Behaviour:**
- Refuses (`15`) if `state/current.yaml` exists and its outcome is not terminal. One
  active task per checkout: hand the existing task over (`handover --close`) or finish
  it first. No flag discards an active task.
- Normalises each scope path: strips trailing `/`, canonicalises, refuses paths outside
  the repository. Records the normalised form.
- Reads every other worktree from `git worktree list` and, where one has an active
  task, reports any scope that contains or is contained by this scope. Reported, not
  blocked: that is a coordination fact for the person, not a rule. Git is the registry;
  there is no sidecar file.
- Records `repository_id`, `branch`, `head`, `working_tree` from git. Never from
  arguments.

```
$ majordomus start "fix OAuth callback" --scope lib/auth --profile debugging
started t-20260903-193012-a4f1  profile=debugging  scope=lib/auth
INFO overlap  ../wt-alice-oauth-refresh claims lib/auth/oauth — contained by your scope  [reproduce: majordomus check --overlap]
next: worker reads AGENTS.md; checkpoint every 15m; majordomus check
```

## `majordomus check`

Is the current task consistent with policy, scope, and state? Read-only.

**Checks:**
- `state/current.yaml` exists and parses; else `12`.
- Git divergence label for the record: `exact`, `advanced`, `diverged`,
  `different_context`. `diverged` and `different_context` are findings.
- Touched files (`git status --porcelain` plus `git diff --name-only <base>..HEAD`) are
  within the claimed scope. Any outside → finding.
- Checkpoint age against the profile's `checkpoint_interval` (`WARN`, never `FAIL`).
- Files under `.ai/` and the projection targets are always in scope.
- `--checkpoint` updates `checkpoint_at` and appends `task.checkpoint` to the ledger:
  the one documented write in an otherwise read-only command.
- `state/open-questions.md` has no unresolved entry for this task.
- `--explain` prints the effective merged policy and profile for this task and exits
  `0`.
- `--overlap` prints claim containment against other worktrees.

Exit `0` with no `FAIL` findings, `10` with any, `12` with no active task. Intended for
a worker to run before claiming completion, and for a person to run any time.

```
$ majordomus check
OK   state      t-20260903-193012-a4f1 — exact (head 3f2a9c1)
FAIL scope      config/secrets.example — outside claimed scope (lib/auth)  [reproduce: git status --porcelain; git diff --name-only 3f2a9c1 HEAD]
OK   checkpoint 7m ago, interval 15m
OK   blockers   none open
check: 4 finding(s), 1 failing
```

## `majordomus watch`

What has drifted? Read-only. Never used to block; exit `11` when findings exist so that
scripts can tell "drift" from "healthy" without confusing it with a contract failure.

<div class="overflow-x-auto" tabindex="0">

| Drift | Detected how |
|---|---|
| policy | the policy hash a projection's stamp names is not the policy on disk |
| projection | a projection's content differs from the hash its own stamp names |
| state | `current.yaml` outcome contradicts ledger, or label is `diverged` |
| scope | touched files outside claim (same check as `check`) |
| handover | task is `handed_over` but no handover names it, or that handover lacks a required section |
| verification | `current.yaml` has a terminal outcome with no `task.finished` ledger record for it |
| checkpoint | `current.yaml` older than the profile's checkpoint interval |
| retention | `ledger.jsonl` or `handovers/` over cap |

</div>


```
$ majordomus watch
DRIFT policy      .ai/repo/policy.yaml modified after last update  [reproduce: majordomus update --dry-run]
DRIFT projection  AGENTS.md — content differs from its stamp (hand-edited?)  [reproduce: majordomus update --diff AGENTS.md]
DRIFT checkpoint  t-20260903-193012-a4f1 — last checkpoint 48m ago, interval 15m
watch: 3 findings
```

## `majordomus update`

Regenerate provider projections from policy. Deterministic: same policy, same output,
byte for byte.

**Reads:** policy, profiles, `providers/*.tmpl`.
**Writes:** every `projections[].target`, one `projections.updated` ledger line. Nothing
else: each target carries its own provenance.

**Behaviour:**
- `--dry-run` prints what would change; `--diff <target>` shows the diff for one. For a
  region projection the diff is of the region, not of the host document.
- Refuses (`15`) to overwrite content whose current hash matches neither the stamp it
  carries nor the new output, unless `--force`. A target with no stamp at all was not
  written by `update` and is refused the same way. A hand edit is never silently lost; the refusal
  names the file and the `--diff` command that shows it.
- `mode: region` (see `SCHEMAS.md`) generates only the text between the
  `majordomus:begin` and `majordomus:end` markers. The rest of the target is copied
  through byte for byte, an absent region is appended once, and malformed markers are
  refused (`15`). This is how a repository that already has a hand-written `CLAUDE.md`
  adopts Majordomus without losing it.
- Appends `projections.updated` to the ledger.
- Every file-mode target begins with a stamp naming this command, the policy hash it
  came from and the hash of the content below it; a region-mode target carries the same
  two hashes in its begin marker. That stamp is the provenance `doctor` and `watch`
  compare against, on a fresh clone as much as here.
- The always-loaded projection is checked against the budget after generation; over
  budget is `10` and nothing is written. For a region projection the budget measures the
  generated region, and `doctor` reports the host document's own length as `INFO`.
- The Rust executable renders the same targets from the same inputs, byte for byte:
  `majordomus generate providers` writes them and `majordomus generate --check` (which CI
  runs) exits `10` naming every target that differs from the policy or is missing. `update`
  is the interactive writer with its refusals; `generate --check` is the gate. Test case 93
  proves the two agree in both directions.

## `majordomus handover`

Write an append-only continuation record.

**Record resolution** — choosing which prior record is about the work happening here — is the same rule for handovers, checkpoints and `context`: same worktree and branch, else same branch, else nothing.


**Input:** the authored body on stdin. Required sections are the policy's
`handover.required_sections` as level-one headings, each with non-empty content;
template placeholders in angle brackets count as empty. Optional: `# Completed`,
`# Decisions`, `# Verification`, `# Risks`, `# Open Work`.

`--derive` composes the body instead of reading stdin, from the task record, the ledger,
git, the open questions and the newest checkpoint — each already written, already validated
and already identity-checked. It calls no model and makes no network request, which is what
makes it safe for a provider hook to run: a derived body can be wrong only if a record it
reads is wrong, and every one of those has its own gate. The policy's required sections are
emitted in the policy's order; a required section this generator cannot fill is refused by
name rather than written empty, because an empty section passes the section gate and tells
the next worker nothing. An authored body still says more than a derived one, and `--derive`
exists so that the absence of somebody willing to type is no longer the same thing as the
absence of a record.

**Writes:** one new file `state/handovers/<utc-ts>--<branch-key>--<short-head>--<rand>.md`,
mode `0600`, created atomically (temp file, then hard link; retry with a new random
suffix on collision). Front matter is computed from git and from `current.yaml`; a body
that tries to set identity fields is rejected.

**Never** stages, commits, or modifies any other file except the task record's
`checkpoint_at`. Appends `task.handed_over` to the ledger. Prints the path.

`--close` additionally sets the task's outcome to `handed_over`, so that a new task may
`start` in this checkout; the old record is archived by that `start`. Without
`--close` the task stays active for the next session to continue.

The provider's `SessionEnd` event runs `--derive --close`, or `--derive --no-task` when no
task record exists, so that an episode that ends leaves a continuation record and not only
the envelope of what it produced. The two answer different questions: the session record
indexes the episode, and a handover is what the next worker resumes from. It does not ask
whether the task is active — until ADR 0052 it did, and a repository whose last task had
been handed over stopped writing the one record a future worker resumes from, for six days,
while every health check passed.

**Refuses** (`10`) if a required section is missing or empty, or if the body contains
an identity field. Refuses (`12`) with no active task unless `--no-task`.

**Resolve:** `majordomus handover --resolve` finds the most relevant prior handover for
the current worktree and branch: same worktree and branch first, then same branch, never
a repository-wide fallback. Prints its git-state label and its body. No candidate is a
normal outcome and exits `0` with `No relevant handover.` `--path` prints the path
alone, for scripting; `--no-task` resolves without an active task.

## `majordomus context`

What does whoever works next need to know? Read-only. Assembles durable state into one
briefing and prints it. Nothing is persisted, no model is called, and the output is a
projection: every line is recomputed from the records and git each time it runs.

**Section order is authority order.** Git first, because it is the only thing that cannot
be stale; then the task and the profile that constrains it; then blockers, which change
what may be accepted; then authored records; then event history last, because it is the
weakest evidence about the present.

<div class="overflow-x-auto" tabindex="0">

| Section | Source | Included when |
|---|---|---|
| `GIT` | git | always |
| `TASK` | `state/current.yaml` | a task is active and the profile's `context.task` is not `false` |
| `PROFILE` | `profiles/<name>.yaml` | the task names a profile that exists |
| `CONTEXT DOCUMENTS` | `.ai/**/README.md` (the context contract) | a task is active; the effective chain is listed for each of its scope paths |
| `OPEN QUESTIONS` | `state/open-questions.md` | any unresolved entry names this task |
| `DECISIONS` | `state/decisions.md` | `context.decisions: true` (this task) or `context.architecture_notes: true` (the repository) |
| `LATEST CHECKPOINT` | `state/checkpoints/` | a checkpoint resolves for this task |
| `LATEST COMPATIBLE HANDOVER` | `state/handovers/` | a handover resolves for this worktree and branch |
| `FILES TOUCHED IN SCOPE` | git | `context.relevant_files: true` |
| `RECENT HISTORY` | `state/ledger.jsonl` | `context.recent_history_depth` is above zero |
| `PROMPT` | `prompts/<name>.md` | `--prompt <name>` was given |

</div>


This is the only code that reads a profile's `context` block, which is what makes those
fields state rather than documentation.

**Budget.** `context.builder_budget_lines` in the policy, or `--budget-lines`. When the
assembled text exceeds it, sections are dropped in a fixed order — history, files,
decisions, then the bodies of the checkpoint and the handover, which degrade to a pointer
rather than disappearing. Git, task, profile and blockers are never dropped. Every drop is
named under `EXCLUDED` with its reason, so an under-filled context is debugged from the
exclusion list instead of guessed at. Exit `10` if what cannot be dropped is already over
budget.

**`--for <provider>`** wraps the same body with a header naming the provider and its
always-loaded file. The body does not change: the canonical context is provider-neutral,
and a provider that needed different facts would be a different policy, not a different
rendering.

**`--json`** emits one object: `git`, `task`, `sections[]` (each with `id`, `lines` and
`text`), `excluded[]` (each with `item` and `reason`), and `budget`. The same selection as
the text form, because both are assembled once and rendered twice.

```
$ majordomus context
# Majordomus context — 2026-09-03T19:41:02Z
# a projection of durable state, not a source of truth: validate every line against git

## GIT
repository   /home/dev/app
branch       main
head         3f2a9c1e4b7d8a05c1119f2b6e0d7a3c8e5f1b42
working_tree dirty
task_record  advanced (recorded head 3f2a9c1)

## TASK
id           t-20260903193012-a4f1
task         fix the OAuth callback
profile      debugging
scope        lib/auth

## OPEN QUESTIONS (1 unresolved — every one refuses finish --outcome completed)
- Does the legacy mobile callback still require the old URI form? (2026-09-03)

## EXCLUDED
- history — profile debugging sets context.recent_history_depth: 50

## BUDGET
41 of 300 lines
```

**Context documents.** The same command carries the scoped-context subcommands, all
read-only, all accepting `--json`; see [`CONTEXT.md`](@/docs/context.md) for the model and
[`SCHEMAS.md`](@/docs/schemas.md) for the front matter.

```
majordomus context list                       every document: id, path, scope, composition, providers, status
majordomus context resolve <path> [--provider P] [--audience A]
                                              NN  <id>  <path>  scope=… composition=… providers=…   in effective order
majordomus context explain <path> [...]       resolve, plus one line per document saying why it is in
                                              (ancestor at depth N, tracks, directory) and every filtered
                                              or superseded document with its reason
majordomus context validate                   the whole tree; findings in the usual format; exit 10 on any FAIL
majordomus context affected [--base <ref>|--staged|--worktree]
                                              which documents and scopes a change set touches; a tracked
                                              path that changed is a WARN naming the document, never the exit code
majordomus context check-sync [--base <ref>]  validate, projections up to date, affected review items;
                                              invalid tree FAIL exit 10, hand-edited projection DRIFT exit 11,
                                              absent projection INFO (doctor owns missing), otherwise 0
```

The target of `resolve`, `explain` and the bare briefing is a repository-relative
directory (a file resolves to its own); a path that does not exist, or a symlink or `..`
that escapes the repository, is refused (`15`, `refused-path`). Inside the `.ai/` tree
the result is the ancestor chain admitted by each document's scope; outside it, the root
chain plus every document whose `tracks` matches the target. Order is depth, then
`order`, then path. The briefing gains a `CONTEXT DOCUMENTS` section listing the
effective chain for the active task's scope paths.

The error classes, used verbatim in messages and in `--json`: `invalid-front-matter`,
`unsupported-schema`, `unknown-key`, `duplicate-id`, `broken-reference`, `cycle`,
`illegal-override`, `unknown-provider`, `invalid-manifest`, `stale-projection`,
`out-of-sync`, `refused-path`.

## `majordomus session`

Open, inspect and close one execution episode. A session is the seventh durable record and
the only one that is not task-shaped.

A task is a unit of work: it is scoped, it has a profile, and it can outlive the worker
doing it. A session is one worker's sitting: it claims no paths, gates no acceptance, and
may cross several tasks — while one task may be crossed by several sessions. Neither
contains the other, which is why they are two records rather than one field.

Sessions are optional. A worker that never opens one loses the episode boundary and
nothing else; every other record is written exactly as before.

- `start [--owner <who>] [--worker <id>] [--if-open refuse|keep]` opens the episode. One
  open session per worktree: a second `start` is refused rather than replacing the first,
  unless `--if-open keep` says to keep what is open — which is what a provider hook passes,
  because its start event fires again on a resume and on a compaction. `--worker` is a
  free-form identity string, recorded only when supplied — an unrecorded worker stays
  unrecorded, because a guessed one is indistinguishable from a recorded one the moment it
  is written down. `--provider` and `--provider-session` record which provider's event
  opened the episode and that provider's own session identity; only something running
  inside that provider's hook can supply them, which is what makes `opened_by: hook` in the
  working context a fact rather than a claim.
- `status` prints the open session with the divergence label of the commit it opened at,
  or reports that there is none. Read-only. Absence is an answer, not a failure.
- `close [--outcome closed|interrupted] [--if-none refuse|ignore]` closes the episode into
  an immutable record under the layer's sessions section and removes the open one. An
  authored summary may arrive on stdin and is optional; identity fields in it are refused,
  as they are in a checkpoint. `--if-none ignore` makes "nothing was open" the normal case
  rather than a failure, which is what a provider's end event passes.
- `context [<session-id>]` prints the path of an episode's working context. Read-only, and
  it prints the path rather than the document: the document is local evidence, and a command
  that pours it into a terminal invites it into somebody's context.

**The open freezes the context it was given.** `start` writes
`.ai/local/session-contexts/<stamp>--<session-id>.md` — the front matter of the episode, the
context builder's output verbatim, and a `## Notes` section for the worker — and `close`
appends a `## Close` section naming the outcome and the record. The document is appended to
and never rewritten, so what a worker typed into it survives. The store is local: it names
this machine and it is a snapshot of a projection, so it is never published and never loaded
into a context on its own, and `doctor` refuses a document that carries a conversation. See
`capture session` for the hooks that make the boundary independent of anybody remembering
to draw it.

**The closed record is an envelope of references.** It names the tasks, issues, milestones,
checkpoints, handovers, decisions, questions and evidence of the episode, and copies the
body of none of them. It also carries the commits between the opening and closing commit —
or the single entry `diverged` when the opening commit is no longer an ancestor, because a
list computed across a history that no longer connects is a fiction.

**The lists are derived at close, not accumulated while the session is open.** No other
command knows sessions exist: `checkpoint`, `decision`, `question` and `plan` are
unchanged. The references are read out of the ledger, which is already append-only, already
written only by Majordomus, and already validated.

**Selection is by the session stamp on each ledger line, not by a time range.** Every line
carries the session that wrote it, next to the commit and branch it already carried. A time
range was implemented first and was wrong the first time it ran: the ledger is one file per
repository, two workers were writing to it, and no timestamp separates them, so one
episode's envelope claimed the other's tasks, checkpoints and handovers. A line with no
session belongs to no episode — sessions are optional, and work done outside one is
attributed to nobody rather than to whoever had a session open nearby.

`--outcome` takes `closed` or `interrupted`. Both are self-reported and neither is
verified; `interrupted` exists because "this episode was cut short and its records may be
incomplete" is the one thing about an ended session that changes what somebody does next.

Exit `12` with no open session, `10` when the summary carries identity fields, `15` when
the open record belongs to another checkout.

- `list [--all]` prints closed episodes, newest first, with each one's divergence label.
- `show <session-id>` prints one record whole.
- `latest [--path]` prints the newest record that resolves for this worktree and branch.

All three are read-only, and all three print the record's divergence label — `exact`,
`advanced`, `diverged`, `different_context`. No second vocabulary for staleness is invented,
because a session written before a branch was rewritten, handed to the next worker as though
it still described this history, is exactly what those four words exist to prevent.

Resolution is the rule every other record follows: same repository, same worktree and
branch, then same branch, then nothing. A record from an unrelated worktree is never
offered — borrowed context cannot be recognised as wrong until it has been acted on.
`--all` lifts the rule explicitly and shows each record's branch, because a record from
elsewhere is worth seeing when you asked for everything and is never worth being handed
silently.

**Ordering is by the recorded timestamp, with ledger position breaking a tie inside one
second.** Filesystem modification time is never read: it does not survive a clone and it is
not the time the record asserts, so touching an old record does not make it the newest.
Filename order normally agrees with ledger order, which makes an implementation that fell
through to the filename look correct; `test/cases/62_session_divergence.sh` makes the two
disagree on purpose and fails when the ledger is not what decides.

A malformed record is skipped with a warning on stderr and never silently, and never
fatally: one unreadable file must not cost the whole listing.

**One episode per provider session, not one per worktree.** Two windows of the same
provider open on one checkout are two workers. Folding them into one record stamped one
worker's ledger lines with the other's id and let either window's end event close the
episode for both; measured in this repository on 2026-09-09, seven concurrent sessions
produced one record between them. So an open episode is a file of its own,
`state/sessions-open/<provider session>.yaml`, keyed by the provider session that opened
it. `--if-open keep` keeps *your* episode — which is why it exists: the start event fires
again on a resume and on a compaction — and gives a different provider session an episode
of its own. `close --provider-session <id>` closes that episode and no other, which is what
a provider's end event passes. An episode nobody named is keyed `hand`, and there is at
most one of those: a provider that sends no session identity is indistinguishable from a
person at a terminal, and inventing a distinction there would multiply episodes nobody can
close. A hand-opened episode needs no provider anywhere and behaves exactly as before.

`state/session-current.yaml` is a relative symlink into that store: the pointer to the
episode of this checkout, aimed at the one opened here last and re-aimed by a close at the
one still open when exactly one is. Which episode a given process's commands belong to is
resolved in `mj_session_here_file` (`lib/common.sh`): the provider session named on the
command line, else the provider session this process is running inside when an episode with
that key is open here (`MAJORDOMUS_PROVIDER_SESSION`, or the variable the lifecycle adapter
declares — `CLAUDE_CODE_SESSION_ID` for Claude Code), else the pointer. Two episodes open
and a process that can name neither is the one case nothing can resolve: the pointer names
the one opened last, and `session status` lists the others under `Also open:` rather than
letting a guess pass for a fact.

**Writes:** `state/sessions-open/<provider session>.yaml`, mode `0600`, written atomically,
the pointer beside it, and one `session.started` line in the ledger. `session_id`,
`repository_id`, `worktree`, `branch`, `start_head` and `start_working_tree` are computed
from git and are never authored. `provider` and `provider_session` are recorded only when a
provider hook supplied them, which is also what makes them reportable: `continuity.state`
declared an `OpenSession.provider` for a year that no writer could produce.

Nothing under `.ai/local/state/` is tracked, the open session records included: an open
session carries nothing anyone else needs, and a record that arrived from another checkout
would make this one inherit an episode it did not open. A record that names another
worktree is reported and never obeyed, which keeps the defence in place for every way one
can still arrive — a copied working directory, a synced folder.

Exit `15` when your own session is already open here, `10` when the record does not parse — a
corrupt record fails loudly rather than being read as "no session", because reading it as
absent is exactly what would let a second `start` overwrite it.

```
$ majordomus session status
No open session in this worktree.
next: majordomus session start

$ majordomus session start --worker some-provider/some-model
session s-20260904153733-fc51 opened at 2026-09-04T15:37:33Z (head 9c13909)
next: majordomus plan next; majordomus context; majordomus session close when the episode ends

$ majordomus session start
majordomus: session s-20260904153733-fc51 is open here since 2026-09-04T15:37:33Z; run majordomus session close first

$ majordomus session close <<'EOF'
The extraction boundary and the session schema landed; the compiler's discovery stage is next.
EOF
.ai/repo/sessions/20260904T171402Z--s-20260904153733-fc51--master--3c9ba2f--c0ffee1234567890.md
```

## `majordomus evidence`

Record evidence against the active task: one obligation it declared, discharged, or one
validation gate of the CI model that has reported.

```
majordomus evidence --covers <token> [--type <kind>] (--command <cmd> | --artifact <ref>) [--result <r>] [--json]
majordomus evidence --gate <id> --exit <status> [--command <cmd>] [--result <r>] [--json]
majordomus evidence --run-gates
```

`--run-gates` runs every gate the task's change selects, here, with the commands the CI
model names — `scripts/ci/run-plan` over the affected plan, the same dispatcher CI runs —
and records each gate's exit as it finishes through the `--gate` form (`MJ_GATE_RECORD`).
A gate that fails is recorded as failing, which is the point: the completion report refuses
over it until the gate reports again. The exit status is the dispatcher's, non-zero when a
gate failed.

The two forms record different kinds of fact and are never one invocation. An obligation is
a promise the task made; a gate is what the validation pipeline said about the tree. A line
claiming to be both would be readable as neither, so passing `--covers` and `--gate`
together exits `2`.

A task's `scope` says where a worker may write; its `requires` says what the worker owes
before the outcome `completed` is available. The tokens are declared in
`share/obligations.yaml` — implementation, tests, docs, generated, rules, commit, push,
target, pages, deploy, verify — each naming the command that discharges it and, where the
fact is local, the pathspecs its evidence is taken over.

Six of them are never recorded here. `commit`, `push`, `target` and `pages` name facts the
tool can establish — a clean tree, a remote-tracking ref that reaches the head, a trunk that
reaches it, a published site that serves it — and `deploy` and `verify` name what the
deployed surfaces state at their own addresses, asked by `deploy.verify` against the trunk's
head once the trunk reaches the task's commit (rule `project.deployment-is-verified-live`).
`check` and `finish` settle all six live at HEAD instead of asking a worker to transcribe
them. Recording one has no effect: an established obligation discharges by being true and
refuses by being false. Where the checkout cannot settle it (no remote, no default branch
recorded, a site that never answered, no deployment object to ask) the token falls back to
the recorded line, and the finding says which it was.

`--command` or `--artifact` is required: narrative is not evidence. A token the vocabulary
does not declare exits `2`. A token the active task never promised is accepted when the
change set implies it — the completion report's `obligations` say which, derived from the
token's own inputs and the deployment plan — and is declared on the task record by the
act, so that the closure judges it from then on exactly as one declared at `start`; a
token neither promised nor implied exits `15`, because recording evidence for something
nobody asked for is how a checklist grows entries nobody wanted.

**Writes:** a `task.evidence` line in the ledger, carrying the obligation, how it was taken,
the command or artifact, and the hash of the tracked files the obligation names. Nothing
else is written; the ledger is already append-only, ordered and integrity-checked, and its
envelope already carries the head, the branch and the session.

That hash is the point. Evidence discharges an obligation only while the recomputed hash of
its inputs equals the recorded one, so a change to any file the obligation names takes the
proof away rather than leaving it behind — the same currency question the site's own
`source_hash` asks, asked of a test result. An obligation whose fact is remote (a push, an
integration, a publication, a deployment) has no inputs and is bound instead to the commit
it was taken at, judged `exact | advanced | diverged | different_context` like every other
record here.

`check` and `finish` evaluate every obligation on every run through the doctrine
`majordomus.obligation-closure`, so a stale evidence is visible before someone builds on
it; only an outcome of `completed` is refused. A worker reporting `blocked` is being honest,
and refusing that would teach them to claim `completed` instead.

### Recording a gate

`--gate` names a gate of `.ai/repo/ci/gates.yaml`, this repository's CI model, and `--exit`
the status it reported; `0` is a pass. A gate the model does not declare exits `2`, as does
an `--exit` that is not a number.

```
majordomus evidence --gate rust-check --exit 0 --command 'scripts/rust-check --ci'
```

**Writes:** a `task.gate` line in the ledger, carrying the gate, its exit status and the hash
of the files that select it. Those files are derived from the model rather than declared:
the union of the paths of every class that names the gate, and everything for a gate every
plan selects, because a gate every plan selects is a gate any change can invalidate. Change
one of them and the run stops discharging the gate — committed or not, since the change set
a task is judged over includes its working tree.

`check` and `finish` evaluate every gate the task's own change set selects, through the
doctrine `majordomus.completion-gates`, and report each in one vocabulary:

<div class="overflow-x-auto" tabindex="0">

| status | meaning |
|--------|---------|
| `pass` | it ran over these inputs and exited `0` |
| `fail` | it ran over these inputs and did not |
| `stale` | it ran, and the files that select it have changed since |
| `blocked` | something it cannot run without has not passed |
| `queued` | the plan selects it and no run has ever reported |
| `exempt` | nothing this change did can make it true or false |
| `unknown` | it cannot be judged here at all — no model, no reader |

</div>


Only `fail`, `stale` and `blocked` refuse the outcome `completed`. `queued` is reported by
name, never accepted as a pass and never refused: a verdict that never arrived and a verdict
that said pass are different facts, and on 2026-09-10 this repository's trunk carried three
branch-breaking defects overnight because they looked identical
(`majordomus.never-reported-is-not-green`).

The whole judgement — every gate, the plan that selected it, which obligations the change
implies, and every question of the completion policy (`share/completion.yaml`) with the
source that answered each, folded into the lifecycle stage the task stands at — is one
document, `gates.completion`, read the same way by the command line, the HTTP API, MCP and
the Cockpit ([`COMPLETION.md`](@/docs/completion.md)):

```
majordomus-cli run gates.completion --input '{}' --format json | jq '.output.questions'
```

## `majordomus checkpoint`

Record compact progress inside an active task. Append-only; the body arrives on stdin.

A checkpoint is not a small handover. A handover is a deliberate continuation package
written when a worker stops; a checkpoint is what was true a moment ago, short enough that
the next worker's context can quote it whole. `checkpoint.max_body_lines` in the policy
enforces that difference — a body over the cap is refused with the suggestion to write a
handover instead, rather than truncated.

**Writes:** `state/checkpoints/<ts>--<branch>--<head>--<rand>.md`, mode `0600`, created
atomically with `link`, never staged. Front matter is computed exactly as for a handover; a
body containing identity fields is refused. Also updates `checkpoint_at` on the task record
and appends `task.checkpoint` to the ledger.

An empty body is allowed and writes no file: it updates `checkpoint_at` only, which is what
`check --checkpoint` has always done. The two are the same operation; `checkpoint` is the
one that can also say what was true.

- `--derive` composes the body from git and the ledger instead of reading stdin: the commit,
  the working tree, how many files have changed since the task started, the commits since
  then, and the count of blockers. It stays inside the same cap, and it is what the
  provider's compaction event runs — a compaction discards the conversation while the work
  continues, and the moment it is announced is the only moment anything can be written about
  a context that is about to stop being reachable.
- `--show` prints the newest checkpoint for this worktree and branch — for the active task
  when there is one, and otherwise the newest whatever task it names.
- `--list` lists this worktree's checkpoints, newest first, with each one's git label.

A checkpoint belongs to the **episode**, not to a task. It is written whether or not a task
is open and whatever outcome the last one reached; the record's `task_id` is the task when
there is one and `none` otherwise, and the ledger event omits the field entirely rather than
writing a literal `none` that every reader of that event would collect as a task by that
name. Only the task's own `checkpoint_at` is left alone when the task is over, because that
field is the task's.

This refused with `12` (no active task) and `15` (the task is no longer active) until ADR
0052. Those two refusals are the 2026-09-05 outage: a task marked `handed_over` and never
replaced silenced this repository's progress records for six days, while episodes went on
opening and closing and every health check passed.

Exit `10` when the body carries identity fields or exceeds the cap.

```
$ majordomus checkpoint <<'EOF'
OAuth state mismatch reproduced with the fixture in test/fixtures/callback.json.
Cause is in callback normalisation, not in the comparison.
Next: regression test before touching the implementation.
EOF
.ai/local/state/checkpoints/20260903T194500Z--main--3f2a9c1--8c1d0e4a2b6f9317.md
```

## `majordomus history`

Read the append-only ledger back. Read-only.

Operational reconstruction, not a transcript. It answers what happened, when, for which
task, at which git head, and what was accepted — and nothing about what anyone said.

**Filters:** `--task <id>`, `--event <name>`, `--since <n>m|h|d` or an ISO timestamp,
`--limit <n>` (default 20, newest), `--all`. Output is oldest line first, so a filtered run
reads as a narrative. `--json` emits the matching ledger lines verbatim.

`--validate` reports every line that is not a well-formed event and exits `10` if any is;
`doctor`, `check` and `watch` run the same test, because a ledger the tool cannot parse is
a ledger that cannot be used as evidence.

`--rotate` moves all but the newest `ledger.retention_max_lines` lines into
`ledger.<utc>.jsonl.archived` and appends a `ledger.rotated` event recording how many moved.
It never deletes, refuses to overwrite an existing archive, and does nothing when the ledger
is under the cap.

```
$ majordomus history --task t-20260903193012-a4f1
2026-09-03T19:30:12Z  task.started         t-20260903193012-a4f1  3f2a9c1  profile=debugging scope=lib/auth
2026-09-03T19:45:00Z  task.checkpoint      t-20260903193012-a4f1  3f2a9c1  20260903T194500Z--main--3f2a9c1--8c1d0e4a2b6f9317.md
2026-09-03T19:52:31Z  decision.recorded    t-20260903193012-a4f1  3f2a9c1  Normalise the callback URI before comparing state
2026-09-03T20:14:08Z  task.finished        t-20260903193012-a4f1  b71e0c9  outcome=completed verify_exit=0
```

## `majordomus decision`

Record or read durable decisions. One append-only file: `state/decisions.md`.

`decision add "<what>" --why "<why>"` appends an entry with the task id and git head
computed. `--why` is required: a decision with no recorded reason cannot be reviewed later,
only re-argued. `--rejected` and `--evidence` are optional and default to `-`.

An entry is never edited or deleted. `--supersedes "<text>"` records that a later decision
replaced an earlier one and refuses text that matches no recorded decision, so a
supersession always points at something real.

`decision list [--task <id>] [--limit <n>]` prints entries newest first; `decision show
"<text>"` prints the first entry whose title contains that text, or exits `12`.

The `deep-work` profile sets `verification.decision_record_required: true`; `finish` then
refuses `completed` unless an entry names the task.

```
$ majordomus decision add "Normalise the callback URI before comparing state" \
    --why "the mismatch is a trailing-slash difference, not a forged state parameter" \
    --rejected "relaxing the comparison, which would accept genuinely forged states" \
    --evidence "test/auth/callback_test.exs:41"
recorded: Normalise the callback URI before comparing state
```

## `majordomus question`

Open, resolve and list the questions that block acceptance. One mutable index:
`state/open-questions.md`.

Unresolved questions are explicit state, not prose inside a handover, because
`finish --outcome completed` refuses while any entry names the active task. That gate is
the reason the file has a machine-written line format, and the reason `check`, `doctor` and
`watch` fail on an entry that does not parse: a gate that cannot read an entry is a gate
that can be bypassed by mistyping one.

- `question add "<question>"` appends `- [unresolved] <task id> — <question> (<date>)`.
- `question resolve <n|"<text>"> --answer "<answer>"` rewrites that one line to
  `[resolved <date>]` and appends the answer. `n` is the number `question list` printed.
  `--answer` is required, and an ambiguous selector is refused rather than guessed.
- `question list [--all] [--task <id>]` shows every unresolved entry, because every one of
  them refuses a completed finish here; `--task` narrows to what one task opened and
  `--all` adds the resolved ones. The numbering is what `question resolve <n>` selects.
  Any unresolved question can be resolved, not only one the active task opened: a gate
  nobody can clear is a gate that gets worked around.

Resolving edits the index because an index of what is still open must not accumulate. The
append-only record of every opening and resolution, with its answer, is the ledger.

```
$ majordomus question add "Does the legacy mobile callback still require the old URI form?"
opened for t-20260903193012-a4f1: Does the legacy mobile callback still require the old URI form?
$ majordomus finish --outcome completed --verify-command "mix test"
FAIL blockers   t-20260903193012-a4f1 — unresolved entry in open-questions.md  [reproduce: majordomus question list]
finish: refused, 1 unmet
```

## `majordomus prompt`

List, show and render repository-local prompt assets. Read-only.

An asset is `.ai/repo/prompts/<name>.md`: YAML front matter with `name` (matching the
filename) and a non-empty `description`, then a body. They are small, versioned with the
repository, and provider-neutral — a prompt library is not the goal, and nothing here
invokes a model.

**Rendering substitutes a closed set of tokens and no others.** Inline: `{{TASK}}`,
`{{TASK_ID}}`, `{{PROFILE}}`, `{{SCOPE}}`, `{{OWNER}}`, `{{BRANCH}}`, `{{HEAD}}`,
`{{WORKING_TREE}}`, `{{REPOSITORY}}`, `{{NOW}}`. Alone on a line: `{{OPEN_QUESTIONS}}`,
`{{DECISIONS}}`, `{{CHECKPOINT}}`, `{{HANDOVER}}`, `{{CONTEXT}}`.

There is no templating language: no conditionals, no loops, no includes, no shell. An
unknown token is an error, exactly as an unknown configuration key is, because a prompt
that silently renders `{{TSAK}}` as literal text is worse than one that refuses. `doctor`
and `watch` validate every asset for the same reason.

```
$ majordomus prompt list
continue               resume a task from durable state instead of from someone's memory
debug                  frame a defect so that the fix is proven, not asserted
handover               produce a continuation record body for the current task
review                 review the working diff against the claimed scope and the finish contract
$ majordomus prompt render debug | head -3
Task t-20260903193012-a4f1 on branch main at 3f2a9c1e...: fix the OAuth callback

Scope: lib/auth
```

## `majordomus skills`

The repository's skills: provider-neutral procedures for one bounded kind of work each,
under the layer's skills section. Read-only.

A skill is a directory `.ai/repo/skills/<id>/` holding `SKILL.md` — YAML front matter
satisfying `share/schemas/majordomus/skill/skill.v1.schema.json` over a Markdown body that is the procedure —
and optionally `examples/*.md`. Nothing registers it. The source class `skill` in
`.ai/repo/knowledge/sources.yaml` discovers it, and that is the same declaration the
Rust executable indexes, so a skill exists for `skills list`, for `doctor`, for the
website and for MCP (`majordomus://skill/<id>`), or for none of them. See
[`SCHEMAS.md`](@/docs/schemas.md) for the file contract.

```
majordomus skills list [--json]        every skill: id, status, version, description
majordomus skills show <id> [--json]   the repository-relative path, then the file as written
majordomus skills check [--json]       validate every skill and every reference it makes
```

- `list` prints one line per discovered skill in discovery order, invalid ones included
  (a listing that silently shrank would hide the file that needs fixing). `--json` adds
  the URI, tags, related ids, inputs, outputs, the path, the content hash and the tracked
  examples.
- `show` prints the path on the first line and the file below it; `--json` adds the
  body as a field. An id that is not a skill exits `12` and names `skills list`.
- `check` validates every skill against the allow-list generated from the schema (no
  unknown key), `schema: skill/v1`, an integer `version`, a `status` from the closed
  set, an `id` equal to the directory name, non-empty `# Purpose`, `# Procedure` and
  `# Output` sections; refuses two skills claiming one id, two skills whose descriptions
  do not tell them apart, a `related` id that names no skill, and an example without a
  level-one heading. Descriptions are compared folded to lower case, with runs of
  whitespace collapsed and trailing sentence punctuation dropped, so the difference has
  to be in what a description says rather than in how it is typed. Every finding names the file and
  every reason. It ends with the counts of what it examined — skills, examples,
  references — and exits `10` on any failure. A repository with no skills is a `WARN`,
  never a pass over nothing. An absent allow-list (`share/allow/skill.txt`, a distribution
  that was not generated) is `13`, naming `majordomus generate allow`.

`doctor` and `watch` run the same examination through the doctrine
`majordomus.skill-integrity`; `scripts/generate-site-data` reads the same catalogue and
refuses to build the site from a skill that does not validate.

```
$ majordomus skills check
OK   skill       1 skill(s) — every one parses, matches its directory and carries its sections
OK   skill       5 reference(s) — every related id and every example resolves
skills: 1 discovered, 1 valid; examples: 5; references: 5 checked; failures: 0
```

## `majordomus capture`

Record the person's raw prompts from a provider hook — as a JSON record with a Markdown
and a YAML rendering of it — draw the episode boundary from the provider's own lifecycle
hooks, install those hooks, and report what each provider actually does in this repository.

**Why a hook and not an instruction.** A worker cannot be asked to record its own prompts.
It never sees the bytes the person typed, only what the provider assembled from them, and a
record written by a model is missing exactly the prompts that mattered: the first one of a
session, which arrives before any instruction has been read, and every one where the model
was busy doing what it was asked. A line in `AGENTS.md` is a request, not a mechanism, and
no behavioural test can prove a request was honoured. So capture happens below the model,
in the provider's own hook, where running it is the proof that it works.

`capture install` writes a shim per event and the matching entries in
`.claude/settings.json`, and refuses to overwrite any of them: `majordomus-capture` for
`UserPromptSubmit`, `majordomus-session-start` for `SessionStart` and
`majordomus-session-end` for `SessionEnd`. When the configuration exists and does not name
one of them — including a configuration this tool wrote before it knew about the lifecycle
events — the command prints the entries to add, one per missing event, and exits 15 rather
than rewriting a file it did not write. A shim finds the repository from its own location:
the provider substitutes its project directory into the command string textually, so nothing
in the environment names the repository, and the working directory a hook runs in is not
contracted.

`capture prompt` reads one JSON payload on stdin and writes one record. **It never exits
2**, because in `UserPromptSubmit` that exit code rejects the person's prompt, and a broken
archive must never cost someone their input; a payload it cannot read is written to the
archive's own log instead — and **`doctor` fails while that log is non-empty**, because a
capture that failed means prompts were lost. That log is the only thing that can catch the
failure that matters: if the provider renames the field the prompt arrives in, the hook
still runs and the self test still passes, since it sends a payload of the tool's own
making. Read the log and delete it; it is a diagnostic, and unlike a record, nothing is lost
by removing it. The prompt text itself is the raw span from the payload, copied through
still escaped, so no decode and re-encode step can lose a character.

**One prompt is two files under one stem**, `.ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json`
and `.md`, where the slug is the opening of the prompt itself:

```
.ai/local/prompts/
  20260905113342-why-arent-prompts-saved-automatically.json
  20260905113342-why-arent-prompts-saved-automatically.md
  20260905114501-make-it-a-file-per-prompt.json
  20260905114501-make-it-a-file-per-prompt.md
```

**Both formats, always, and they are not alternatives.** The `.json` is the record: the
provider's own spans, still escaped, pretty printed one member per line — what everything
else is derived from and what capture must not lose. A record is read by people too, and a
single forty-kilobyte line is not something any editor or diff shows usefully; the
indentation lies outside the string spans, so the prompt's bytes are untouched by it. The `.md` is that record rendered for a person, in one shape
every rendering has: the fields as YAML front matter, the same fields again as a table, and
the prompt last under `## PROMPT`, decoded and fenced.

````markdown
---
schema: 'majordomus.capture/v1'
ts: '2026-09-05T12:48:55Z'
provider: 'claude-code'
event: 'UserPromptSubmit'
id: 'bb6f8c96'
session: '5b13785c-728f'
source: 'user'
cwd: '~/src/your-repo'
repository: '~/src/your-repo'
branch: 'feature/prompt-capture-markdown'
head: 'a0ccbc9e25d6ec7f6bb754f3515affb9b1cc4014'
record: '20260905124855-make-it-a-file-per-prompt.json'
---

# Prompt — 2026-09-05 12:48:55 UTC

| | |
|---|---|
| **Started** | `2026-09-05T12:48:55Z` |
| **Provider** | `claude-code` · `UserPromptSubmit` |
| **Session** | `5b13785c-728f` |
| **Prompt** | `bb6f8c96` |
| **Source** | `user` |
| **Repository** | `~/src/your-repo` |
| **Branch** | `feature/prompt-capture-markdown` · `a0ccbc9` |
| **Directory** | `~/src/your-repo` |
| **Schema** | `majordomus.capture/v1` · `share/schemas/majordomus/capture/capture.v1.proto` |
| **Record** | `20260905124855-make-it-a-file-per-prompt.json` |

## PROMPT

```
make it a file per prompt
```
````

The fields appear twice because the two readers are different: front matter is what a tool
parses, the table is what a person reads, and one function writes both out of one record in
one pass, so they cannot drift. A row is omitted rather than printed as `null`, because a
row reading `null` tells a person less than an absent row does.

**The schema identifier is the path to the file that describes it.** `<namespace>.<name>/<version>`
is `share/schemas/<vendor>/<name>/<name>.v<n>.proto`, so `majordomus.capture/v1` resolves
to `share/schemas/majordomus/capture/capture.v1.proto` with no registry in between — a reader
holding a record holds the way to read it, and there is nothing that can fall out of step
with the records it claims to describe. That file is protobuf used as a schema language and
not as a wire format: nothing serialises these messages, and it is the one place the record's
fields and the document's sections are defined, so the two projections cannot disagree.
`majordomus update` installs it; `doctor` fails if the identifier names a file that is not
there.

**Some header fields are absent, and absent means not observed.** The hook runs before the
model is invoked, so when a record is written the turn has not happened: `finished_at`,
`duration_ms`, `model`, `effort` and `tokens` are not knowable from it, and a conforming
writer leaves them out rather than writing null — an absent field says "not observed", a null
one would claim it was observed to be nothing. The renderer omits the row rather than
printing a placeholder, so a record the hook wrote shows `Started` and no `Finished`. When
something that can observe the turn fills them in, the rows appear with no change to the
shape. The same holds for the `## CONTEXT` and `## OUTPUT` sections, which the schema
declares and the renderer emits exactly when the record carries the field behind them.

`## OUTPUT` is the model's half of the exchange, which a prompt record may not carry — the
doctrine fails a record naming a response or a transcript. A conforming writer puts it in a
separate after-the-turn record whose `meta.parent` names the prompt record, and the rendering
joins the two.

**The fence is sized to the prompt**, never fixed at three backticks. Prompts quote code,
and a prompt that opens a fence of its own would end the block early and spill the rest of
itself into the document; the renderer measures the longest run of backticks inside and
opens with one longer, which is how the prompt is quoted verbatim without a byte of it
being touched.

The direction between them is what makes the pair safe to enforce: a rendering can be
rebuilt from a record and a record can never be rebuilt from a rendering. So `doctor` fails
on a record with no rendering and names `capture render` as the repair, and fails on a
rendering with no record without offering one, because there is none — that half is reported
for a person to delete. `capture render` walks the archive, writes the renderings that are
missing, touches nothing that already has one, and never writes a record; `--force` rewrites
them all, which is for a change to the rendering itself and not for anything a hook does.

A record is immutable once written, two hooks cannot interleave inside one file, the name
sorts chronologically, and the directory listing is already a readable history. The slug is
a hint and never a faithful rendering — escapes and non-ASCII collapse into hyphens, and
the prompt itself is inside the file with nothing done to it. Records stay idempotent on
the provider's prompt identity, which is a field and not the name: a hook delivered twice
within the day writes once.

**Not every payload is a prompt, and the field it arrives in is not fixed.** The event is
not what its name suggests: Claude Code also fires `UserPromptSubmit` for messages it
injects into the turn — a completed background task, a system reminder — and those are not
the person's prompts. A payload whose declared origin is not a person, or whose text opens
with a marker the provider injects, is skipped: no record, and no log line, because a skip
is normal operation rather than a failure. A payload that says nothing about its origin is
captured, since losing a real prompt is the worse of the two mistakes.

The payload keys are candidates rather than one assumed name — the prompt is read from
`prompt`, then `prompt_text`, then `text`. A payload field is the provider's private shape:
it is versioned on their schedule, renaming one is not a breaking change to them, and a
capture built on a single assumed name loses every prompt the day it moves. When none of
the candidates is present, nothing is written and the log names the keys that *were* there,
so the next name is read off a real payload instead of guessed.

**The archive is evidence, not knowledge.** It lives under the ignored half of the layer,
nothing loads it into a context, no command retrieves from it, and `doctor` fails if
anything under it is tracked by Git. The writer emits a closed set of fields, so the
model's half of the exchange cannot arrive through it.

**A record carries the episode it belongs to.** It used to carry the provider's own session
identifier and nothing else, and turning that into the canonical episode id needed
`state/sessions-open/<provider session>.yaml` — a file that closing the episode deletes. So
the instant an episode ended, nothing could say which episode its prompts had belonged to.
Measured in this repository on 2026-09-11: 974 records, 80 provider sessions, 713 prompts
with no surviving path to an episode.

Every record now carries `episode`, `episode_link`, `repository_id` and `worktree_id`,
written while the mapping is still there, using the identity the rest of the tool computes.
`episode_link` says how the episode was determined, and the distinction is the point:

<div class="overflow-x-auto" tabindex="0">

| value | what it means |
|---|---|
| `open` | the episode was open under that provider session when the prompt arrived — an observation |
| `session-context` | linked afterwards, from the frozen working context that names the provider session |
| `ledger` | linked afterwards, because exactly one closed episode's window could contain that provider session's prompts |
| `orphan` | no episode was open under that provider session; none was invented |
| `unlinked-legacy` | written before records carried an episode, and nothing survives to link it |

</div>


`orphan` and `unlinked-legacy` carry no episode. Nothing attaches a prompt to the episode
nearest it in time: a prompt attributed to the wrong episode is worse than one attributed to
none, because the second is visibly missing and the first is quietly false.

`capture reconcile` applies those three kinds of evidence, in that order, to an archive
written before the field existed, and `--dry-run` says what it would do. It refuses the
moment two episodes could both be the answer, so a repository whose evidence is gone keeps a
count of unlinked records rather than an invented attribution — and `doctor` reports that
count rather than rounding it to zero.

**Credential material never reaches the disk.** People paste keys into prompts, and a
record is the one file here whose content nobody vetted before it was written. The known
credential shapes — provider API keys, GitHub tokens, AWS key ids, Slack and Stripe tokens,
PEM headers, bearer tokens, and an assignment of a long opaque value to something called a
key, a token or a password — are replaced on the way to the record, before the bytes are
anywhere but a variable. Nothing downstream sees the original: not the record, not either
rendering, not the file name, not the log, which prints a payload's key names and never a
value. What was replaced is recorded in `redacted`, so "nothing was found" and "the secret
is gone" are statements a reader can tell apart.

**The archive is mode 0600, directory included.** It was 0644 for its whole life, readable
by every account on the machine — including the file names, which are the openings of the
prompts. `capture render`, which is the archive's repair command, sets the mode over the
whole directory on every run.

**Retention takes the body and keeps the record.** The old rule was that nothing may ever be
removed, on the argument that a prompt is derived from nothing and no other file can
reconstruct it. That argument is about the *record* — that it happened, when, in which
episode, under which head — and it was being applied to the bytes of the text, which is the
part that carries the credentials and whose risk does not decay with its value. The archive
reached 20 MB unbounded.

So the policy declares `prompts.retention_max_days` and `prompts.retention_max_bytes`, and
`majordomus capture prune` applies them: age first, then size, oldest body first. A pruned
record keeps every field, including the provenance of its episode link, and gains a tombstone
saying when the body went, how long it was, and its digest. A record is still never deleted,
`doctor` reports an archive over either bound, and nothing prunes as a side effect of the
hook that was supposed to be keeping them.

**`capture status` reports five distinct states, and never a generic pass:**

<div class="overflow-x-auto" tabindex="0">

| state | what is true |
|---|---|
| `unsupported` | the provider has no documented event that hands a command the prompt before the model runs |
| `unconfigured` | an adapter exists, but this repository does not wire it |
| `named` | the configuration declares the hook, but not the shim this tool wrote |
| `wired` | the shim is in place and executable, but a payload through it produced no record |
| `verified` | a synthetic payload driven through the shim produced a record and its rendering |

</div>


A repository holds itself to this by declaring an `enforcement` entry with
`wired_by: provider-hook:<provider>`; `doctor` then fails unless the state is `verified`,
and because `doctor` runs on `pre-commit`, a hook that stops capturing stops the commit.
Only Claude Code has an adapter today; every other provider the distribution declares
(`docs/generated/providers.md`) is reported `unsupported` rather than assumed, and no other
surface — the web, the desktop app, another machine — is observable from here at all. An
orchestrator such as bb has no adapter by design: it hands no prompt to a command before the
model, and the agent it runs keeps its own hooks, so a Claude Code thread under bb is
captured as `claude-code` (ADR 0024).

### `capture session` — the episode boundary

**The same argument, applied to the other thing a worker cannot do for itself: say where its
own sitting began and ended.** A model told to open a session opens one when it remembers
to, which is never the episode that mattered — the one that ended in a crash, a compaction,
or somebody closing the window. The provider fires an event at both edges whether or not a
model is in a position to notice, so `SessionStart` opens the episode and `SessionEnd`
closes it into the shared record under `.ai/repo/sessions/` (ADR 0015).

**Both directions are idempotent, because the events are.** `SessionStart` fires again on a
resume and on a compaction, so the start passes `--if-open keep` and the open episode is
kept rather than replaced; `SessionEnd` fires whether or not anything was opened, so the
close passes `--if-none ignore` and writes nothing when there is nothing to close. The
event's reason decides the outcome: one the adapter lists as deliberate closes the episode
as `closed`, and anything else — a crash, a name the table has not seen — closes it as
`interrupted`, because calling a cut-short episode complete is the worse of the two
mistakes.

**Neither hook writes to standard output.** Claude Code adds a `SessionStart` hook's output
to the model's context, and nothing under the local half of the layer may be loaded into a
context implicitly. Diagnostics go to stderr, and `capture session` never exits 2, for the
reason `capture prompt` never does.

**The open freezes the context it was given.** `session start` writes
`.ai/local/session-contexts/<stamp>--<session-id>.md`: front matter carrying
`schema: session-context/v1`, the episode's identity, the provider and the provider's own
session id — the same string the prompt records carry — then the context builder's output
verbatim, then a `## Notes` section for the worker. `session close` appends a `## Close`
section naming the outcome and the record it wrote. The document is appended to and never
rewritten, so what a worker typed into it survives the close; `majordomus session context`
prints its path.

That store is local and stays local. It names this machine, and it is a snapshot of a
projection — re-resolving it later gives a different document — so it is never published,
never indexed, and never loaded into a context on its own. It is also never a transcript:
the derived half is the builder's output, the authored half summarises the work, and a front
matter key naming a message list, a completion or a model's reply is a blocking failure.

The wiring is declared as `wired_by: provider-hook:<provider>:session` and `doctor` proves
it the same way, with one thing left out: the synthetic payload is driven through the end
shim with the mutation disabled, because closing somebody's open episode is not a price a
diagnostic may charge. Everything else on the path runs exactly as it does in a real event.

```
$ majordomus capture install
INFO  capture  .claude/hooks/majordomus-capture  written and made executable
INFO  capture  .claude/hooks/majordomus-session-start  written and made executable
INFO  capture  .claude/hooks/majordomus-session-end  written and made executable
INFO  capture  .claude/settings.json  written with the UserPromptSubmit, SessionStart, SessionEnd hook(s)
$ majordomus capture status
claude-code            verified     .claude/hooks/majordomus-capture is wired, and a synthetic payload through it produced one record and its renderings
claude-code:session    verified     .claude/hooks/majordomus-session-start and .claude/hooks/majordomus-session-end are wired, and a synthetic payload through the end shim reached the command
$ majordomus capture render
0 rendering(s) written into .ai/local/prompts
```


## `majordomus search`

Find durable records without reading all of them. Read-only.

A literal, case-insensitive, fixed-string search across handovers, checkpoints, decisions,
questions, prompt assets and the ledger, in that order — authority order, so the most
reliable evidence appears first. `--kind` restricts it and is repeatable; `--task` narrows
to one task; `--limit` caps each kind.

Deliberately not an index and not an embedding. The corpus is a handful of Markdown files
and one JSONL; a scan is faster than the staleness problem an index would introduce, and
"transparent" is worth more here than "clever". Exit `0` with matches, `12` with none.

```
$ majordomus search "callback" --kind decision --kind checkpoint
checkpoint  .ai/local/state/checkpoints/20260903T194500Z--main--3f2a9c1--8c1d0e4a.md:12  Cause is in callback normalisation, not in the comparison.
decision    .ai/local/state/decisions.md:31  ## 2026-09-03 — Normalise the callback URI before comparing state
search: 2 match(es)
```

## `majordomus knowledge`

A compiler over what this repository already states. Read-only in every subcommand
documented here.

It is not a wiki, not a database, not a memory service, and not a second place to write
things down. Every source it reads is a file somebody already maintains, everything it
produces is derived and regenerable, and none of it outranks the file it came from. A
result is a pointer to canonical knowledge, never a rewritten answer.

**`knowledge` is not `search`.** `search` is a literal scan over the durable operational
records — handovers, checkpoints, decisions, questions, prompts, the ledger — with no
index, because that corpus is a handful of files. `knowledge` covers the repository's
canonical artifacts: the policy, the profiles, the prompts, the milestone and issue
contracts, the claims matrix and the documents. Two corpora, two contracts, and neither
changes the other.

- `sources [--scope shared|operational|all]` lists the curated source classes and the
  files each one discovers, with the class, the scope, the kind, a content hash and the
  repository-relative path.
- `nodes [--scope ...] [--kind <k>]` derives one node per canonical object: its identity,
  its kind, the source it came from and that source's hash. Exits `10` when two objects
  claim one identity.
- `edges [--scope ...] [--type <t>]` derives one edge per stated relationship, with the file
  and the field or line it was observed in.

**No edge without provenance.** Every edge names where the relationship was stated, and an
edge missing any of from, to, type or provenance is refused rather than emitted with a
blank — a blank source reads as "unknown" and is indistinguishable from one nobody recorded.
An edge nobody can trace to a line is not a fact, it is a guess wearing a fact's clothes, and
it is a guess the person best placed to notice it is wrong will never see.

**Nothing is inferred from prose.** The repository already states its relationships
explicitly, in fields somebody maintains, and those are both free and correct. The edge
types are a closed set; an undeclared one is an error rather than a new vocabulary word:

<div class="overflow-x-auto" tabindex="0">

| type | from → to | stated in |
|---|---|---|
| `part_of` | issue → milestone | the issue's `milestone` |
| `depends_on` | issue → issue, milestone → milestone | the record's `depends_on` |
| `declares` | milestone → claim | the milestone's `claims` |
| `specified_by` | claim → document | `docs/CLAIMS.yaml`'s `source` |
| `implemented_by` | claim → implementation | its `implementation` |
| `tested_by` | claim → test, doctrine → test | its `test` |
| `supports` | doctrine → claim | the doctrine's `claims` |
| `references` | document → document | an inline Markdown link |

</div>


Links are the one edge source that is not curated, and they are read conservatively. Fenced
code is dropped first, because a path inside a code sample is an example of a path and not a
reference to one. A target with a scheme, a protocol-relative target and an absolute path are
all skipped; a fragment is trimmed, because `docs/CLI.md#session` refers to `docs/CLI.md`.

**Severity distinguishes what Majordomus owns from what an author wrote.** A declared
relationship pointing at a file the repository does not contain is a `FAIL`: it is a broken
promise. A link in a document pointing at nothing is a `WARN`: a document may deliberately
point outside the repository and the target alone cannot tell the two apart. A relationship
pointing at a real file that this compiler does not model as one node — a container of many
objects, or something outside the curated set — is silent, because a report that is large by
design is a report people stop reading.

`nodes` and `edges` are two views of one derivation, so a defect in either exits `10` on
both. There is no clean node listing over a graph that is broken.

**A node's identity is never its content hash.** It is the object's own canonical id where
it has one — `claim:policy-parse`, `issue:I0801`, `milestone:M003`, `profile:debugging` —
and its repository path where it does not, as in `document:docs/CONTINUITY.md`. A hash says
whether something *changed*; it can never say what something *is*, because then every edit
would delete a node and create a stranger, and every reference to it would point at nothing
without anything saying so. The hash rides along on the node and is what freshness is
measured with.

The consequence is deliberate: editing a document keeps its node and moves its hash;
renaming one is a delete and an add, and disturbs no other node.

**A kind comes from structure, never from prose.** It is decided by the source class the
file was discovered in and by fields the file itself declares. Nothing reads a body looking
for a word that suggests a type: a document that discusses roadmaps and milestones is a
document that discusses them. Where no rule applies the kind is `unknown` and the node is
still emitted, with one finding naming the class — an explicit unknown is information, and a
confident wrong answer is not.

**A title is taken or left empty, never invented.** From the record's own `title` or
`description`, or from a document's first level-one heading. A document with no heading gets
no title rather than its filename, because a filename standing in for a title reads like a
fact and is a guess.

Two stores have no id field of their own. A decision is keyed by its title and a question by
its text — in both cases exactly what the ledger records and what a session envelope
references, so three places name the same thing the same way and nothing maps between them.
Answering a question rewrites the line it lives on, and the identity survives that: the
appended answer is stripped, and only when what remains ends in the entry's opening date, so
a question whose own text contains a dash is left whole rather than cut at a guess.

**Discovery is driven by the repository index, not by a filesystem walk.** A walk returns
build output, vendored trees and editor droppings; it returns them in an order that
differs between two machines; and it can return a file nobody meant to publish. Listing
tracked files instead gives repository truth in a stable order, and an untracked file is
never a source. Operational records are discovered from the state directory Majordomus
itself owns under `.ai/local/`, which is never tracked — and from nowhere else. No hidden
directory is scanned because it happens to exist.

The source classes are declared twice, by two owners: the repository declares its shared
sources in its AI layer, `.ai/repo/knowledge/sources.yaml`, and the tool ships the
operational classes, the records it writes itself, in `share/knowledge-sources.yaml`. The
scope of a class is decided by which file declared it; neither file names the other:

<div class="overflow-x-auto" tabindex="0">

| scope | meaning |
|---|---|
| `shared` | repository knowledge; may be projected to a public surface |
| `operational` | this checkout's own working records; never part of a shared projection |

</div>


The scope is a property of the class, so the publication boundary is decided in one place
rather than at each producer. Every tracked pathspec carries the `:(glob)` prefix, under
which `*` does not cross a directory separator. That is not tidiness: without it,
`docs/*.md` also matches `docs/claims/*.md`, two classes silently overlap, and one file
becomes two nodes. `test/cases/64_knowledge_discovery.sh` fails on that mutation.

A class marked `required` that discovers nothing is reported as a `WARN`. The cost of a
curated list is that a path can be forgotten, and a forgotten path is indistinguishable
from a repository that does not have that file unless something says so.

```
$ majordomus knowledge sources --scope shared
policy      shared      policy     4f2a9c1d8b30  .ai/repo/policy.yaml
profile     shared      profile    a1b2c3d4e5f6  .ai/repo/profiles/debugging.yaml
...
issue       shared      issue      0e5a7b9c3f51  .ai/repo/project/issues/I0801.yaml
claims      shared      claim      7c9e1b3d5f70  docs/CLAIMS.yaml
document    shared      document   9b1e2d4f8c3a  docs/CONTINUITY.md
knowledge sources: 169 file(s) in scope shared
```

## `majordomus rules`

The effective rule set: every active rule vendored under the repository's rules section
plus every active rule the repository wrote, resolved as a dependency graph. Read-only in
every subcommand except `vendor update`.

A rule is a Markdown file with YAML front matter. Its identity is the front matter's `id`
and `version`, never the file name. `docs/DOCTRINE.md` describes the format, the
`x-majordomus` block that binds a rule to a validator, and what is authoritative today.

```
$ majordomus rules list
majordomus.scope-integrity                 v1  blocking  vendor:majordomus enforced by check,finish,watch
majordomus.sessions-are-workers            v1  advisory  vendor:majordomus no validator; see the rule
project.english-only                       v1  blocking  project          proven by scripts/ci/english-only-check
project.clean-room                         v1  blocking  project          review-enforced: the rule is about the provenance of what was written, which no program in this tree can read
```

- `list [--json]` prints the effective set in resolved order: identity, class, provenance
  (`vendor:<name>` or `project`), and how it is enforced. Three modes, which
  [`DOCTRINE.md`](@/docs/doctrine.md) describes: `enforced by <commands>` for a rule the dispatcher
  runs, `proven by <paths>` for one a gate or a case holds, and `review-enforced: <reason>`
  for one that carries a `reviewed_because` saying why no program can express it. A rule
  with no `x-majordomus` block at all says `no validator; see the rule` rather than hiding
  it: the rule is normative for whoever reads it, and nothing checks it by machine.

  This command answers what the rules *are*. What *proves* each one — whether the case it
  names is still in the tree, whether a runner drives it, whether anything ever ran it, and
  whether that run is older than what it is about — is a different question, and the
  executable answers it: `majordomus-cli rules report`, `rules show <id>`,
  `rules proves <test>`.
- `show <id>` prints one rule, front matter and body, with the repository-relative path it
  was read from as the first line. An id outside the effective set exits 12.
- `vendor status` compares the vendored baseline with the package the running executable
  ships. It prints both revisions, then the manifest integrity of the vendored copy, then
  whether the two packages are the same.
- `vendor diff` is the reviewable difference between the two, as a unified diff of the two
  directories. It exits 0 whether or not they differ; `vendor status` carries the exit code.
- `vendor update [--force]` replaces the vendored baseline with the executable's package.
  The write is atomic: the new package is staged beside the target and swapped in. It never
  touches `rules/project/`.

**Resolution fails closed.** A missing dependency, a dependency on a deprecated rule, a
cycle, one `id@version` claimed by two files, a project rule in the vendor namespace,
malformed or incomplete front matter, an unknown front-matter key, or an `x-majordomus`
block that names no validator, no enforcing command or no test — each stops `list`, `show`
and every command that reads the set, with exit 10 and the reason. Nothing is applied
partially. The order is deterministic: two runs agree, and every dependency is listed before
the rule that depends on it.

**The repository's vendored copy is authoritative.** A newer executable reports a newer
baseline through `vendor status` and `vendor diff`; it never applies one. `update`,
`doctor` and `check` leave the vendored directory alone. The baseline changes only when
`vendor update` is asked for.

**A hand edit under `vendor/` is detected.** The package manifest names every rule file
with its hash. A file whose hash no longer matches, a listed file that is absent, or a file
present beside the manifest that it does not list, is reported by `vendor status` and
refused by `vendor update` until `--force`.

<div class="overflow-x-auto" tabindex="0">

| exit | meaning |
|---|---|
| 0 | the set resolves; the vendored baseline is current |
| 2 | unknown subcommand or option; `show` without an id |
| 10 | the set does not resolve, or the vendored copy fails its manifest |
| 11 | `vendor status`: the executable ships a different package than the one vendored |
| 12 | no rule with that id; nothing vendored yet; no rules section in this layout |
| 13 | the executable ships no standard rule package: a broken install, not a repository fault |
| 15 | `vendor update` refused over a hand-edited vendor directory (`--force` overrides) |

</div>


`test/cases/67_rule_dag.sh` proves each refusal by mutation, and proves that a newer
distribution's package is not applied until asked, that `vendor update` leaves
`rules/project/` byte for byte what it was, and that the resolved order is the same across
runs.

## `majordomus finish`

Evaluate the finish contract. Refuse if unmet.

**Reads:** policy, profile, `current.yaml`, git, ledger.
**Writes:** on success, `current.yaml` outcome set to the given value, one
`task.finished` ledger line carrying the evaluated checklist and the verification
result, and a copy of `--note` under `state/completed/<id>.md` when given.

**Arguments:** `--outcome completed|partial|blocked|no_match|failed` required.
`--verify-command "<cmd>"` runs the project's own verification in the repository root
and records its exit code, duration, and command. `--note <file>` supplies the
completion note; otherwise the newest handover naming this task is used. `--check`
evaluates scope and state without writing and exits `0` when no task is active or the
task is already finished, so a pre-push hook never blocks a repository with nothing to
enforce.

Profile requirements are also evaluated for `completed`: `regression_test_required`
passes when a touched path looks like a test (`test/`, `spec/`, `_test.`, `.spec.`);
`decision_record_required` passes when `decisions.md` contains `Task: <id>`. The
regression check is deliberately crude and says so in its message.

**Contract for `completed`:**

```
scope respected        touched files within claimed paths
verification ran       --verify-command exited 0, recorded
state updated          current.yaml is at HEAD or advanced, not diverged
no open blockers       open-questions.md has no unresolved entry for this task
note present           newest handover or completion note has required sections
```

Every line of the contract is evaluated and printed, pass or fail, so that a refusal
says exactly what is missing. `partial` and `blocked` require a note with `# Next
Action`; `no_match` and `failed` require `# Reason`; all four skip the verification
line, and `blocked` skips the blockers line. Nothing is written when any line fails.
Finishing an already finished task is refused (`15`).

```
$ majordomus finish --outcome completed --verify-command "make test"
OK   scope         12 files, all within lib/auth
OK   verification  make test — exit 0, 41s
OK   state         exact (head 9b1e2d4)
FAIL blockers      open-questions.md: "token refresh window — needs product decision" unresolved  [reproduce: grep -n 'unresolved' .ai/local/state/open-questions.md]
OK   note          handover 20260903T201455Z--main--9b1e2d4--c0ffee.md
finish: refused, 1 unmet
blocking doctrines:
- majordomus.blocker-resolution
$ echo $?
10
```

The contract is not a list inside `finish`. It is the set of doctrines whose
`enforced_by` names `finish`, selected by this repository's
`verification.finish_requires`. A requirement in the policy that no doctrine defines is
reported and refuses, rather than being ignored. See
[`DOCTRINE.md`](@/docs/doctrine.md).

---

## `majordomus doctrine`

Report what rules are enforced here, by what, and whether they are wired. Read-only.

**Reads:** the repository's effective rule set (`.ai/repo/rules/vendor/majordomus/` and
`.ai/repo/rules/project/`, resolved as `majordomus rules list` resolves it), `lib/`,
`docs/CLAIMS.yaml`.
**Writes:** nothing.

```
majordomus doctrine [status]     derived counts
majordomus doctrine list         id, class, validator, enforcing commands
majordomus doctrine show <id>    the full record for one doctrine, and the rule file it lives in
```

**Behaviour:**
- `status` prints how many doctrines are declared, how many block, how many are advisory,
  how many name a validator that does not exist, and how many name a test file that does
  not exist. Every number is derived on the spot; none is stored.
- `list` prints one line per doctrine.
- `show <id>` prints the record, including which claims it backs, the tests that prove it,
  the rules it depends on, the rule file it lives in, and which file defines its
  validator. An unknown id exits `12`. A doctrine's id is its rule id,
  `majordomus.<name>` for the baseline.
- A rule set that does not resolve exits `10` with the reason; nothing is listed
  partially.
- Exits `0` when no validator and no test file is missing, `10` otherwise. Whether the
  enforcement is actually *reached* is a stronger question, and `majordomus doctor`
  answers it.

## `majordomus plan`

Read the canonical project model, and move one issue through its lifecycle. A milestone is
an executable specification of an outcome; an issue is a bounded execution contract; the
dependency graph between the issues decides what may be executed next. See
[`docs/PLANNING.md`](@/docs/planning.md) for the semantics.

**Reads:** `.ai/repo/project/project.yaml`, `.ai/repo/project/milestones/*.yaml`,
`.ai/repo/project/issues/*.yaml`, `share/allow/{project,milestone,issue}.txt`.
**Writes:** one lifecycle field in one issue file, and one ledger event — `start`, `verify`,
`evidence` and `done` only. Every other subcommand writes nothing.

```
majordomus plan validate         schemas, references, the DAG, status consistency
majordomus plan status           milestone progress and the next executable issue
majordomus plan list             one line per issue: id, status, wave, milestone, title
majordomus plan show <id>        the full record of one milestone or issue
majordomus plan ready            issues whose dependencies are all satisfied
majordomus plan blocked          issues waiting on a dependency, and on which one
majordomus plan waves            topological execution waves, derived from the graph
majordomus plan graph            the dependency DAG as Mermaid
majordomus plan next             the one issue a worker should take now
majordomus plan body <id>        the provider-neutral projection body for one record
majordomus plan start <id>       record that execution began
majordomus plan verify <id>      record that implementation is complete, evidence pending
majordomus plan evidence <id>    attach one piece of evidence
majordomus plan done <id>        record completion
```

**Options:** `--json` on the read subcommands a surface consumes; `--milestone <id>` to
restrict `list`, `ready`, `blocked`, `waves` and `graph`; `--covers`, `--type`, `--command`,
`--result` and `--artifact` on `evidence`.

**Behaviour:**
- No status is stored anywhere. `BLOCKED`, `READY`, `ACTIVE`, `VERIFY`, `DONE` and
  `CANCELLED` are derived from what an issue records about itself and from the state of its
  dependencies, by `lib/project.awk`. Writing a `status:` field is an unknown key.
- `validate` reports unknown keys, unknown and self dependencies, duplicate ids, cycles,
  issues executing ahead of a dependency, and issues with no acceptance criteria. It exits
  `10` when any finding is a failure, `0` when only warnings remain.
- `start` refuses (`15`) an issue that is not `READY`, naming what it waits on. `verify`
  refuses an issue that is not `ACTIVE`.
- `evidence` refuses (`15`) a token the issue does not declare, and refuses (`2`) without a
  `--command` or an `--artifact`: narrative is not evidence. It appends to the issue's own
  file with the commit and the timestamp.
- `done` refuses (`10`) while any declared evidence token is uncovered, and refuses (`15`)
  while a dependency is not `DONE`. Every writing subcommand prints the next ready issue
  after the graph has been recomputed.
- Exits `12` when the record named does not exist, or when the repository has no canonical
  project model at all — the model is opt-in, and `doctor` skips it rather than failing
  where it is absent.

The same model is projected to GitHub by `scripts/github-sync` and to the website by
`scripts/generate-site-data`. Neither re-derives a status; both read this engine.

## `majordomus adr`

The repository's architecture decisions as objects: what was decided, why, what it cost,
and where the record came from. Under the layer's `adrs` section.

A decision is a file `.ai/repo/adrs/<NNNN>-<slug>.md` — YAML front matter satisfying
`share/schemas/majordomus/adr/adr.v1.schema.json` over a body carrying `# Context`, `# Decision` and
`# Consequences`. Nothing registers it: the source class `adr` in
`.ai/repo/knowledge/sources.yaml` discovers it over the tracked tree, which is the same
declaration the Rust executable indexes. See [`SCHEMAS.md`](@/docs/schemas.md) for the contract.

```
majordomus adr list [--status <status>] [--json]      every decision: id, status, date, title
majordomus adr show <id> [--json]                     the path, then the file as written
majordomus adr next [--json]                          the next free identity, and every source surveyed
majordomus adr propose "<title>" [--from <ref>]...    write a new decision, status proposed
                       [--tag <tag>]... [--supersedes <id>]
majordomus adr check [--json]                         validate every decision and every reference
majordomus adr affected [--base <ref>|--staged|--worktree] [--json]   the decisions a change set touches
```

- `list` prints one line per discovered decision, invalid ones included; `--status`
  filters to one of `proposed`, `accepted`, `superseded`, `rejected`.
- `show` prints the repository-relative path on the first line and the file below it. An
  id that is not a decision exits `12` and names `adr list`.
- `next` answers what the next free identity is, and says what it looked at to decide. Four
  sources, each reporting how many identities it claimed: this working tree, every sibling
  worktree as it stands on disk (authored and not yet committed is a real claim), every ref —
  branches, tags, remote-tracking refs and every linked worktree's `HEAD`, in one
  `git log --all --no-renames --diff-filter=A` — and the peer board of the shared server,
  which is the only source that knows about an identity a session has decided to take and has
  not written anywhere. A source that could not be reached is reported as unreachable and
  never dropped silently from the denominator.
  Allocation is **monotonic**: one above the highest identity anything has ever claimed,
  never into a hole. A hole means an identity was taken and withdrawn, or taken and not yet
  written, and nothing can tell either from a number nobody ever used — `0034` of this
  repository is on no branch, in no tag and in no working tree, and `ADR 0035` cites it. Holes
  are therefore listed as spent, with whatever cites them named as evidence for the reader.
  The shell tool has no peer identity of its own, so a session that announced an allocation
  and then asks is stepped over its own claim; the output says when the high-water mark came
  from a board claim. `propose` allocates through the same survey.
- `propose` writes `status: proposed` and nothing else. **There is no way to write
  `accepted`**: `--status` exits `15` naming the reason, because a tool that can write
  `accepted` turns its own inference into repository truth and a later reader cannot tell
  which decisions a person actually made. The identity is allocated under a lock over the
  section directory, so two worktrees proposing at the same moment get two identities.
  A `--from` reference is `<type>:<value>` with the type one of `decision`, `session`,
  `commit`, `issue`, `file`, `test`; a `file:` or `test:` path that does not exist exits
  `2`. Referenced evidence makes the record `provenance.origin: extracted`, and an
  extracted record with no evidence is refused as an assertion. `--supersedes` writes both
  halves of the relation, so the chain is walkable from either end.
- `related` is the other half of a record's references, and it is authored rather than
  written by `propose`: `rule:<id>`, `claim:<id>`, `file:<path>`, `test:<path>` — what the
  decision put in force, as against `provenance.derived_from`, which is where it came from.
  Each is validated where its type says the target lives, and the extractor turns it into a
  graph edge (`declares`, `supports`, `references`, `tested_by`), so the reverse direction —
  which decision put this rule in force — is `knowledge edges`, not a second list somebody
  keeps in step.
- `check` validates every record against the allow-list generated from the schema (no
  unknown key), `schema: adr/v1`, the closed status set, an `id` whose number equals the
  file-name prefix, the required body sections; and across the set: duplicate identities,
  a `superseded` record with no `superseded_by`, one-sided supersession, and a reference
  that resolves to nothing. It exits `10` on any failure.
- `check` also asks the question no single tree can answer about itself: for every identity
  this tree **adds** relative to the base (`origin/master`, `origin/main`, `master`, `main`,
  in that order; `MJ_ADR_BASE` overrides and is then the only candidate), does another ref
  carry a *different* document at that identity? A different file name is a competing claim
  and is refused, naming both documents, the refs that carry the other one, and
  `majordomus adr next`; the same file name on two refs is an edit and is resolved by
  merging. The verdict always states its subject — how many identities were added and what
  they were measured against — and when the base does not resolve it says the other refs were
  **not** surveyed rather than exiting clean with nothing said. `doctor` runs the same
  examination, and `doctor` is the pre-commit hook, so a collision is refused on the branch
  rather than at the merge where `generate --strict` would exclude every claimant and name
  only the identity. `ADR 0053` is the reasoning.

- `affected` reads a change set — the working tree against `HEAD` by default, `--staged`
  for the index, `--base <ref>` for `<ref>..HEAD` plus the working tree — and names every
  decision whose own file changed, or whose `related` names a path the change touches (a
  reference to a directory covers everything below it). Every item is a `WARN` review note
  and the exit code stays `0`: whether a decision still holds after the code it governs
  moved is the judgement a tool may not make. It is the reverse question the graph answers
  — what decided this file? — asked from the change set instead of from a record.

`doctor` and `watch` run the same examination through the doctrine
`majordomus.adr-integrity`. The threshold for recording a decision at all is the rule
`majordomus.decision-threshold`, which a reviewer decides: the tool validates records and
reports what a change reaches, and never claims to know that a diff embodied a decision.

```
$ majordomus adr propose "The registry is the one canonical declaration" --from file:docs/CAPABILITIES.md
proposed: adr-0011  The registry is the one canonical declaration
.ai/repo/adrs/0011-the-registry-is-the-one-canonical-declaration.md
status is "proposed"; accepting it is a person editing that field.

$ majordomus adr check
examined 11 decision(s) in .ai/repo/adrs/
every identity unique, every status known, every reference resolves
```

## `majordomus usecase`

The executable use cases of the repository: the Markdown objects under the manifest's
`use-cases` section, each a task somebody performs with the tool, the commands, rules,
claims, responsibilities and applications it names, and a scenario that proves it against
`bin/majordomus`. `docs/USE_CASES.md` is the contract; this section is the command.

```
majordomus usecase list [--json]
majordomus usecase show <id>
majordomus usecase validate [--json]
majordomus usecase run [<id>...] [--json] [--out <dir>] [--keep]
majordomus usecase coverage [--json] [--check]
majordomus usecase impact [--base <ref>] [--json]
majordomus usecase scaffold [--missing] [--for command:<name>] [--dry-run]
```

`list` prints every use case with its category, status, whether it has a scenario and the
commands it names; `show` prints one file and the result of its last run. `validate`
resolves every reference (a command against the dispatch table, a doctrine against the
registry, a claim against `docs/CLAIMS.yaml`, a responsibility against
`docs/RESPONSIBILITIES.yaml`, an application both ways, a category against
`taxonomy.yaml`, a setup script and a stdin body against the fixtures, an MCP tool
against the executable's registry), refuses a key the schema does not declare, an id that
is not the file name, a duplicate, a body without its sections, a scenario step running a
command the use case does not list, and an active use case targeting a guarantee with no
scenario. `doctor` applies the same validation under `majordomus.catalogue-integrity`.

`run` executes each scenario in a disposable repository prepared by its setup script:
every step is one real invocation with argv from the file, its exit code and output are
asserted, and the evidence, normalised (paths, timestamps, ids, hashes, durations), is
written to `.ai/local/evidence/use-cases/<id>.json`; `--out` copies it elsewhere,
`--keep` leaves the repository for inspection, and the event `use_cases.ran` is
appended to the ledger. A use case without a scenario is reported as described, not run.

`coverage` tallies every public command of the registry, every guaranteed claim with a
responsibility and every MCP tool the executable projects against the active use cases
that name it, run it, and have passing evidence; the policy's `use_cases.coverage` says,
per class, whether a gap is `required`, `advisory` or `off`; `--check` exits 10 on a
required gap, as `doctor`, `check` and `finish` do under `majordomus.use-case-coverage`.
`impact` maps the files changed since `--base` (the upstream by default) and in the work
tree to the commands, rules, use cases, scenarios and behavioural cases they reach, and
names the run to do next. `scaffold` writes a draft use case for a gap from what the
registry, the command's fixture and the claims already know; a draft validates and runs
and never counts.

Exit codes: 0; 2 for a usage error; 10 when validation, a scenario or a required coverage
gap fails; 12 when the section, a use case or the registry is absent; 13 when a scenario
repository cannot be created.

## `majordomus bench`

Time every public command of the registry, cold and warm, and compare with the baseline.

The targets are the public commands of `share/commands.yaml` and nothing else: a command
added to the registry is a target from that moment, and the harness never times itself.
Each target runs in a disposable repository. When the tool's own suite is present, the
scenario is the first scenario of the command's fixture under `test/fixtures/commands/`,
so what is timed is what the site demonstrates and the suite executes; otherwise it is the
bare command in an installed repository.

A read-only command is run once cold, then `benchmark.warmup` times unsampled, then
`benchmark.samples` times warm, all in one repository. A command that mutates state gets a
fresh repository per sample, so every sample of it is cold and it has no warm
distribution. A sample is the wall-clock time of the child process alone; setup is never
inside the clock. Every distribution is recorded with its count, minimum, p50, p90, p95,
p99, maximum, mean and standard deviation; nothing is averaged away.

- `bench` times every target; `bench <command>...` only those.
- `--list` prints the targets with their class and scenario, `--format json` as data.
- `--samples <n>`, `--warmup <n>` override the policy for this run only; `--mode cold|warm|both`
  selects the distributions recorded.
- `--format json` prints the run as one document with schema `majordomus/benchmark-result/v1`:
  the run id, the commit and whether the tree was dirty, the platform, the profile and one
  result per target and mode.
- Every run is written under `.ai/local/benchmarks/` (`runs/<run-id>.json`, `latest.json`,
  one line per run in `history.jsonl`) unless `--no-save`. That is local evidence: ignored
  by git, never a baseline.
- `--write-baseline` writes the run under `.ai/repo/benchmarks/rust/`, as
  `baseline.<platform>.json` — one baseline per platform, since a duration compared
  across machines compares the machines — with schema
  `majordomus/benchmark-baseline/v1`, and prints the old and new p50/p95/p99 per target. It
  refuses a dirty tree without `--force`, because a baseline records a commit.
- `--check` compares the run with the baseline under `benchmark.regression`: for each target
  and mode, a p50, p95 or p99 more than the threshold fraction over the baseline is a
  `FAIL` naming the metric, both values and the threshold, and the command exits `10`. No
  baseline exits `12`; a baseline with another schema is not comparable and exits `15`.

`MJ_TIMING=1` on any command prints the phases and work counters of that run on stderr,
which is how a slow target is taken apart.

```
$ majordomus bench --list
command      class                      scenario
init         generated-output-mutating  fixture fresh
doctor       read-only                  fixture not-wired
...

$ majordomus bench doctor version
command      mode  status       n     p50     p95     p99     max  scenario
doctor       cold  ok           1    2711    2711    2711    2711  fixture not-wired
doctor       warm  ok          10    2640    2790    2790    2790  fixture not-wired
version      cold  ok           1      41      41      41      41  fixture prints
version      warm  ok          10      38      45      45      45  fixture prints

slowest by warm p95 (cold where warm does not apply):
  doctor       p95 2790 ms
  version      p95 45 ms

run b-20260905T031200Z-9f1c saved as .ai/local/benchmarks/runs/b-20260905T031200Z-9f1c.json
```

Exit `2` on a usage error, `12` when a named target is not a public command of the
registry, `13` when a target did not run cleanly (its row says `setup-failed` or the exit
code it produced), `10`, `12` and `15` from `--check` as above.

## `majordomus archive`

Take a snapshot of the tracked tree that can be read, or checked, somewhere else.

A repository leaves the machine more often than it looks: to a language model that will
read all of it at once, to a reviewer who cannot clone, to an auditor who has to run the
gates on a copy. Those were being done by hand, differently each time, and the hand-made
version got two things wrong that only surface at the far end.

**What travels is the git index and only the git index.** Nothing untracked is ever
archived — not the build output, not the caches, and not `.ai/local/`, which is this
checkout's own state and is never shared. That is one rule instead of a list of
exclusions, and it is what makes the command safe to point at a repository nobody has
read.

**What is left out beyond that is declared, not coded.** `share/archive.yaml` holds the
profiles; a repository may add or replace one in an `archive.yaml` of its own under `.ai/repo/`. A profile that
drops the derived paths reads `.gitattributes` — every projection in this repository is
already marked `merge=derived` there, for a different reason — rather than keeping a
second list of generated paths, which is the duplication
`.ai/repo/rules/project/commands-are-projections.v1.md` refuses.

**Modes are carried explicitly.** `zip -X` strips the extra fields that hold the Unix
mode, and the unpacked tree then has no executable in it: every script fails to run, and
at the far end that is indistinguishable from the repository being broken. So the mode of
each entry is read from the index — the authority, not the working tree — written into
`_ARCHIVE/MANIFEST.txt`, and `_ARCHIVE/restore.sh` inside the archive puts the modes back
and creates a local git index, because the checks here enumerate the repository with
`git ls-files` and without an index they examine nothing and report that nothing is wrong.

Every archive is read back before the command returns: the entry count must match what
was staged, and if executables went in and none came out, that is a failure rather than a
surprise for the recipient.

**Reads:** `share/archive.yaml`, an `archive.yaml` under `.ai/repo/` when the repository has one,
`.gitattributes`, and the git index.
**Writes:** `tmp/archives/` — or wherever `--out` says.

**The profiles shipped:**

<div class="overflow-x-auto" tabindex="0">

| Profile | For |
|---|---|
| `context` | A model that will read the repository. Every tracked source, without the projections generated from it. |
| `audit` | Running the repository's own checks on a copy. Every tracked file, nothing dropped, modes and a git index restored. |
| `governance` | The operating contract alone: `.ai/`, `docs/`, `AGENTS.md`, `CLAUDE.md`, and no code. |

</div>


```
$ majordomus archive --dry-run
archive: profile context — 2299 of 2757 tracked file(s), 12919 KB of content
         left out: 10 (binary)
         left out: 443 (derived)
         left out: 5 (excluded)
         nothing written (--dry-run)

$ majordomus archive
OK   archive     prismatic-majordomus-context-20260911.zip — 2303 entr(ies) read back, 76 of them executable
archive: profile context — 2299 of 2757 tracked file(s), 12919 KB of content
         wrote tmp/archives/prismatic-majordomus-context-20260911.zip (5044 KB)
```

**Behaviour:**
- `--list` prints the profiles with what each one is for, and marks the default.
- `--dry-run` reports the selection and the reason every file was left out, and writes
  nothing.
- `--out` overrides the destination; `--format zip|tar.gz` overrides the container.
- An existing output file is refused with `15` unless `--force` is given, so an archive
  someone is uploading cannot be replaced underneath them.
- A path the index names and the working tree no longer has is counted and reported, not
  silently absent.
- `--json` emits one object with the counts, the reasons and the path.

Exit `2` on a usage error, `12` when there is no such profile or `zip` is not on `PATH`,
`10` when a profile selects none of the tracked files, `15` when the output exists, `13`
when the container could not be written or does not read back as what went in.

## `majordomus version`

Print the version and exit. `--version` is accepted as a synonym, and `version` works
without an installation: it never reads `.ai/`.

**Reads:** nothing.
**Writes:** nothing.

**Behaviour:**
- Prints `majordomus <version>` on stdout and exits `0`.
- The same string is the single source of the version everywhere else, including the
  public site's footer.

## Hook integration (target)

`init` prints these; `doctor` verifies they are present and executable:

```sh
# .git/hooks/pre-commit  (or the file named by core.hooksPath)
majordomus doctor || exit $?

# .git/hooks/pre-push
majordomus finish --check || exit $?
```

Either line may live in a subhook of a dispatching hook instead — `doctor` looks in
`<hook>.d/` as well and names the file it found. A repository that predates Majordomus
needs no bootstrap flag: `finish --check` passes while no task is active, so the hooks
are inert until the first `start`.
{% endraw %}
