# CLI specification — `majordomus`

Behaviour of v0.1 as implemented and tested in `test/cases/`. Where implementation and this
document disagree, the document is wrong and changes in the same commit as the fix.

One executable, `bin/majordomus`, portable shell (bash 3.2 and BSD userland are the floor). Subcommands are dispatched to
sourced modules in `lib/`. Every subcommand accepts `--help` and `--repo <path>`
(default: the git toplevel of the current directory).

## Exit-code contract

Exit codes are part of the interface. A caller, including a git hook, must propagate
them. A hook that receives any non-zero code and continues is a contract violation, and
`doctor` scans hook scripts for `|| true` and `|| exit 0` around Majordomus invocations.

| Code | Name | Meaning |
|---|---|---|
| `0` | `OK` | the command succeeded; for checks, nothing was found |
| `2` | `USAGE` | bad arguments, or not inside a git repository |
| `10` | `CONTRACT_UNMET` | a deterministic contract failed: finish contract, policy validation, wiring |
| `11` | `DRIFT_FOUND` | `watch` found at least one drift; informational, never used by a hook to block |
| `12` | `MISSING_ARTIFACT` | a required file or tool is absent |
| `13` | `INTERNAL_ERROR` | Majordomus itself failed; never silently treated as pass |
| `15` | `REFUSED` | the command declined to overwrite, or an override was rejected |

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
`$USER`.

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

**Writes:** one new file `state/handovers/<utc-ts>--<branch-key>--<short-head>--<rand>.md`,
mode `0600`, created atomically (temp file, then hard link; retry with a new random
suffix on collision). Front matter is computed from git and from `current.yaml`; a body
that tries to set identity fields is rejected.

**Never** stages, commits, or modifies any other file except the task record's
`checkpoint_at`. Appends `task.handed_over` to the ledger. Prints the path.

`--close` additionally sets the task's outcome to `handed_over`, so that a new task may
`start` in this checkout; the old record is archived by that `start`. Without
`--close` the task stays active for the next session to continue.

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
read-only, all accepting `--json`; see [`CONTEXT.md`](CONTEXT.md) for the model and
[`SCHEMAS.md`](SCHEMAS.md) for the front matter.

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

- `start [--owner <who>] [--worker <id>]` opens the episode. One open session per
  worktree: a second `start` is refused rather than replacing the first. `--worker` is a
  free-form identity string, recorded only when supplied — an unrecorded worker stays
  unrecorded, because a guessed one is indistinguishable from a recorded one the moment it
  is written down.
- `status` prints the open session with the divergence label of the commit it opened at,
  or reports that there is none. Read-only. Absence is an answer, not a failure.
- `close [--outcome closed|interrupted]` closes the episode into an immutable record under
  `state/sessions/` and removes the open one. An authored summary may arrive on stdin and
  is optional; identity fields in it are refused, as they are in a checkpoint.

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

**Writes:** `state/session-current.yaml`, mode `0600`, written atomically, and one
`session.started` line in the ledger. `session_id`, `repository_id`, `worktree`, `branch`,
`start_head` and `start_working_tree` are computed from git and are never authored.

Nothing under `.ai/local/state/` is tracked, the open session record included: an open
session carries nothing anyone else needs, and a record that arrived from another checkout
would make this one inherit an episode it did not open. A record that names another
worktree is reported and never obeyed, which keeps the defence in place for every way one
can still arrive — a copied working directory, a synced folder.

Exit `15` when a session is already open here, `10` when the record does not parse — a
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
.ai/local/state/sessions/20260904T171402Z--s-20260904153733-fc51--master--3c9ba2f--c0ffee1234567890.md
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

- `--show` prints the newest checkpoint for the active task in this worktree and branch.
- `--list` lists this worktree's checkpoints, newest first, with each one's git label.

Exit `12` with no active task, `15` when the task is no longer active, `10` when the body
carries identity fields or exceeds the cap.

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
satisfying `share/schemas/skill.schema.json` over a Markdown body that is the procedure —
and optionally `examples/*.md`. Nothing registers it. The source class `skill` in
`.ai/repo/knowledge/sources.yaml` discovers it, and that is the same declaration the
Rust executable indexes, so a skill exists for `skills list`, for `doctor`, for the
website and for MCP (`majordomus://skill/<id>`), or for none of them. See
[`SCHEMAS.md`](SCHEMAS.md) for the file contract.

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
  `# Output` sections; refuses two skills claiming one id, a `related` id that names no
  skill, and an example without a level-one heading. Every finding names the file and
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

Record the person's raw prompts from a provider hook, install that hook, and report what
each provider actually does in this repository.

**Why a hook and not an instruction.** A worker cannot be asked to record its own prompts.
It never sees the bytes the person typed, only what the provider assembled from them, and a
record written by a model is missing exactly the prompts that mattered: the first one of a
session, which arrives before any instruction has been read, and every one where the model
was busy doing what it was asked. A line in `AGENTS.md` is a request, not a mechanism, and
no behavioural test can prove a request was honoured. So capture happens below the model,
in the provider's own hook, where running it is the proof that it works.

`capture install` writes two things and refuses to overwrite either: a shim at
`.claude/hooks/majordomus-capture`, and the hook entry in `.claude/settings.json`. When the
configuration exists and names something else, the command prints the entry to add and
exits 15 rather than rewriting a file it did not write. The shim finds the repository from
its own location: the provider substitutes its project directory into the command string
textually, so nothing in the environment names the repository, and the working directory a
hook runs in is not contracted.

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

**One prompt is one file**, `.ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json`, where the slug
is the opening of the prompt itself:

```
.ai/local/prompts/
  20260905113342-why-arent-prompts-saved-automatically.json
  20260905114501-make-it-a-file-per-prompt.json
```

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

**Nothing deletes a record.** The ledger, the checkpoints and the handovers rotate under a
policy cap because each restates state that is still available elsewhere; a prompt is not —
it existed once, was derived from nothing, and no other file can reconstruct it. So there is
no cap to configure, the archive grows, `doctor` reports how many records and how many
kibibytes it holds, and a person who wants it smaller deletes files themselves rather than
discovering that the hook meant to keep their prompts had been discarding them.

**`capture status` reports five distinct states, and never a generic pass:**

| state | what is true |
|---|---|
| `unsupported` | the provider has no documented event that hands a command the prompt before the model runs |
| `unconfigured` | an adapter exists, but this repository does not wire it |
| `named` | the configuration declares the hook, but not the shim this tool wrote |
| `wired` | the shim is in place and executable, but a payload through it produced no record |
| `verified` | a synthetic payload driven through the shim produced a record |

A repository holds itself to this by declaring an `enforcement` entry with
`wired_by: provider-hook:<provider>`; `doctor` then fails unless the state is `verified`,
and because `doctor` runs on `pre-commit`, a hook that stops capturing stops the commit.
Only Claude Code has an adapter today. Codex and Gemini are reported `unsupported` rather
than assumed, and no other surface — the web, the desktop app, another machine — is
observable from here at all.

```
$ majordomus capture install
INFO  capture  .claude/hooks/majordomus-capture  written and made executable
INFO  capture  .claude/settings.json  written with the UserPromptSubmit hook
$ majordomus capture status
claude-code    verified     .claude/hooks/majordomus-capture is wired, and a synthetic payload through it produced one record
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

| scope | meaning |
|---|---|
| `shared` | repository knowledge; may be projected to a public surface |
| `operational` | this checkout's own working records; never part of a shared projection |

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
majordomus.sessions-are-workers            v1  advisory  vendor:majordomus not machine-enforced
project.english-only                       v1  blocking  project          not machine-enforced
```

- `list [--json]` prints the effective set in resolved order: identity, class, provenance
  (`vendor:<name>` or `project`), and whether the tool enforces it. A rule without an
  `x-majordomus` block is normative for whoever reads it and enforced by nobody, and the
  listing says `not machine-enforced` rather than hiding it.
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

| exit | meaning |
|---|---|
| 0 | the set resolves; the vendored baseline is current |
| 2 | unknown subcommand or option; `show` without an id |
| 10 | the set does not resolve, or the vendored copy fails its manifest |
| 11 | `vendor status`: the executable ships a different package than the one vendored |
| 12 | no rule with that id; nothing vendored yet; no rules section in this layout |
| 13 | the executable ships no standard rule package: a broken install, not a repository fault |
| 15 | `vendor update` refused over a hand-edited vendor directory (`--force` overrides) |

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
[`DOCTRINE.md`](DOCTRINE.md).

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
[`docs/PLANNING.md`](PLANNING.md) for the semantics.

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
- `--write-baseline` writes the run as `.ai/repo/benchmarks/baseline.json`, schema
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
