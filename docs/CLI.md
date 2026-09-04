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

Set up `.majordomus/` in the repository.

**Reads:** nothing.
**Writes:** `.majordomus/policy.yaml`, `.majordomus/profiles/*.yaml`,
`.majordomus/templates/*`, `.majordomus/providers/*.tmpl`, empty `.majordomus/state/`
and `.majordomus/generated/`. Appends `.majordomus/state/` entries to `.gitignore` only
with `--gitignore`; default is to track state.

**Refuses** (`15`) if `.majordomus/` already exists, unless `--force`, which rewrites
policy, profiles, templates, and provider templates and still never touches `state/`.

**Does not** install git hooks. It prints the two lines a hook needs and the command
`majordomus doctor` that will verify they were added.

```
$ majordomus init
created .majordomus/policy.yaml
created .majordomus/profiles/{routine,implementation,debugging,deep-work}.yaml
created .majordomus/templates/, .majordomus/providers/
next: majordomus update      # generate CLAUDE.md, AGENTS.md, GEMINI.md
next: majordomus doctor      # verify nothing is declared that is not wired
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
   `generated/fingerprints.yaml` matches. Missing → `12`; mismatch → `10` (hand-edited).
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
OK   policy      .majordomus/policy.yaml — parsed, version 1
OK   profiles    4 files — parsed
FAIL wiring      finish-contract — bin/majordomus is not invoked by .git/hooks/pre-push  [reproduce: grep -n 'majordomus finish' .git/hooks/pre-push]
OK   projection  CLAUDE.md — fingerprint matches
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
- Files under `.majordomus/` and the projection targets are always in scope.
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
| policy | `policy.yaml` newer than any fingerprint |
| projection | projection hash differs from fingerprint |
| state | `current.yaml` outcome contradicts ledger, or label is `diverged` |
| scope | touched files outside claim (same check as `check`) |
| handover | task is `handed_over` but no handover names it, or that handover lacks a required section |
| verification | `current.yaml` has a terminal outcome with no `task.finished` ledger record for it |
| checkpoint | `current.yaml` older than the profile's checkpoint interval |
| retention | `ledger.jsonl` or `handovers/` over cap |

```
$ majordomus watch
DRIFT policy      .majordomus/policy.yaml modified after last update  [reproduce: majordomus update --dry-run]
DRIFT projection  AGENTS.md — hash differs from fingerprint (hand-edited?)  [reproduce: majordomus update --diff AGENTS.md]
DRIFT checkpoint  t-20260903-193012-a4f1 — last checkpoint 48m ago, interval 15m
watch: 3 findings
```

## `majordomus update`

Regenerate provider projections from policy. Deterministic: same policy, same output,
byte for byte.

**Reads:** policy, profiles, `providers/*.tmpl`.
**Writes:** every `projections[].target`, `generated/fingerprints.yaml`, one
`projections.updated` ledger line.

**Behaviour:**
- `--dry-run` prints what would change; `--diff <target>` shows the diff for one. For a
  region projection the diff is of the region, not of the host document.
- Refuses (`15`) to overwrite content whose current hash matches neither its fingerprint
  nor the new output, unless `--force`. A hand edit is never silently lost; the refusal
  names the file and the `--diff` command that shows it.
- `mode: region` (see `SCHEMAS.md`) generates only the text between the
  `majordomus:begin` and `majordomus:end` markers. The rest of the target is copied
  through byte for byte, an absent region is appended once, and malformed markers are
  refused (`15`). This is how a repository that already has a hand-written `CLAUDE.md`
  adopts Majordomus without losing it.
- Appends `projections.updated` to the ledger.
- Every generated file begins with a header naming this command and the policy hash it
  came from.
- The always-loaded projection is checked against the budget after generation; over
  budget is `10` and nothing is written. For a region projection the budget measures the
  generated region, and `doctor` reports the host document's own length as `INFO`.

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
.majordomus/state/checkpoints/20260903T194500Z--main--3f2a9c1--8c1d0e4a2b6f9317.md
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

An asset is `.majordomus/prompts/<name>.md`: YAML front matter with `name` (matching the
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
checkpoint  .majordomus/state/checkpoints/20260903T194500Z--main--3f2a9c1--8c1d0e4a.md:12  Cause is in callback normalisation, not in the comparison.
decision    .majordomus/state/decisions.md:31  ## 2026-09-03 — Normalise the callback URI before comparing state
search: 2 match(es)
```

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
FAIL blockers      open-questions.md: "token refresh window — needs product decision" unresolved  [reproduce: grep -n 'unresolved' .majordomus/state/open-questions.md]
OK   note          handover 20260903T201455Z--main--9b1e2d4--c0ffee.md
finish: refused, 1 unmet
blocking doctrines:
- blocker_resolution
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

**Reads:** `share/doctrines.yaml` (shipped with the tool), `lib/`, `docs/CLAIMS.yaml`.
**Writes:** nothing.

```
majordomus doctrine [status]     derived counts
majordomus doctrine list         id, class, validator, enforcing commands
majordomus doctrine show <id>    the full record for one doctrine
```

**Behaviour:**
- `status` prints how many doctrines are declared, how many block, how many are advisory,
  how many name a validator that does not exist, and how many name a test file that does
  not exist. Every number is derived on the spot; none is stored.
- `list` prints one line per doctrine.
- `show <id>` prints the record, including which claims it backs, the test that proves it,
  and which file defines its validator. An unknown id exits `12`.
- Exits `0` when no validator and no test file is missing, `10` otherwise. Whether the
  enforcement is actually *reached* is a stronger question, and `majordomus doctor`
  answers it.

## `majordomus plan`

Read the canonical project model, and move one issue through its lifecycle. A milestone is
an executable specification of an outcome; an issue is a bounded execution contract; the
dependency graph between the issues decides what may be executed next. See
[`docs/PLANNING.md`](PLANNING.md) for the semantics.

**Reads:** `.majordomus/project/project.yaml`, `.majordomus/project/milestones/*.yaml`,
`.majordomus/project/issues/*.yaml`, `share/allow/{project,milestone,issue}.txt`.
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

## `majordomus version`

Print the version and exit. `--version` is accepted as a synonym, and `version` works
without an installation: it never reads `.majordomus/`.

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
