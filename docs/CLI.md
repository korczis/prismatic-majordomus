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
  active task per checkout. `--replace` requires the existing task to be handed over or
  finished first; it never discards state.
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
| staleness | `current.yaml` older than the profile's checkpoint interval |
| retention | `ledger.jsonl` or `handovers/` over cap |

```
$ majordomus watch
DRIFT policy      .majordomus/policy.yaml modified after last update  [reproduce: majordomus update --dry-run]
DRIFT projection  AGENTS.md — hash differs from fingerprint (hand-edited?)  [reproduce: majordomus update --diff AGENTS.md]
DRIFT staleness   t-20260903-193012-a4f1 — last checkpoint 48m ago, interval 15m
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
normal outcome and exits `0` with `No relevant handover.`

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
finish: refused, 1 of 5 unmet
$ echo $?
10
```

---

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
