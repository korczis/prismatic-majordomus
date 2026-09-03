# Prismatic Majordomus

**A lightweight supervisory control layer for AI-assisted work.**

Majordomus holds one canonical policy for how AI workers operate in a repository,
generates the instruction file each tool reads from it, keeps task state outside every
conversation, reports when two workers are about to collide, and refuses to call work
finished until a contract is met. It runs entirely locally, in portable shell, and
never invokes a model.

```
$ majordomus finish --outcome completed --verify-command "make test"
OK   scope         t-20260903193012-a4f1 — 12 touched file(s), all within scope
OK   verification  t-20260903193012-a4f1 — make test — exit 0, 41s
OK   state         t-20260903193012-a4f1 — advanced (head 9b1e2d4)
FAIL blockers      t-20260903193012-a4f1 — unresolved entry in open-questions.md  [reproduce: grep -n 'unresolved' .majordomus/state/open-questions.md]
OK   note          t-20260903193012-a4f1 — 20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md
finish: refused, 1 unmet
```

## The problem

Most AI-assisted work fails for boring reasons. Too much context. The wrong rules for
the tool that happens to be open. Reasoning at maximum by default. Sessions that never
end. Two agents editing the same file. No definition of done, so "done" means the
worker said so.

These are operational-control problems, and the environments this tool was distilled
from had them at scale:

- instruction files for two AI tools in the same repository sharing **0–9 %** of their
  content: not duplicates, disjoint rule sets
- an always-loaded operating contract oscillating between **0 and about 1,100 lines**
  across roughly 200 hand edits, until a hard budget with a failing check pinned it
- **seventeen** independent cases of a check documented as blocking, present on disk,
  executable, and invoked by nothing
- full worktree isolation still producing about **3,200** concurrently modified files
  and **67** duplicated patches across forty worktrees, with nine ever merged
- **10 GB** of session notes standing in for a database, recovered by a hand-written
  runbook

Better prompts do not fix any of this.

## What it does

| | |
|---|---|
| **Policy** | one provider-neutral `policy.yaml`; unknown keys are errors |
| **Projection** | `update` generates `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, or any target you name, deterministically, and fingerprints them; a hand edit is detected and never silently overwritten |
| **State** | one active task per checkout in `current.yaml`, with `branch`, `head`, and `working_tree` computed from git, never authored |
| **Scope** | `start` takes the paths a task may touch; `check` and `finish` fail on files outside them; other worktrees' overlapping claims are reported |
| **Profiles** | four bundles that set capability class, reasoning effort, verbosity, presentation, context toggles, verification, and checkpoint interval independently |
| **Handover** | append-only records with computed front matter and required sections, created atomically, never staged; `--resolve` finds the right one for this worktree and branch and labels how far git has moved since |
| **Finish** | a typed outcome and a contract evaluated line by line; nothing written when any line fails |
| **Doctor** | proves the installation is real: policy parses, every declared enforcement is actually invoked by the hook it names without a swallowed exit code, projections match fingerprints, the always-loaded file is under budget |
| **Watch** | reports drift between policy, projections, state, scope, and git, each finding with the command that reproduces it |

Every row above is backed by a case in [`test/cases/`](test/cases/). The README is not
allowed to say more than the tests do.

## Quick start

```bash
git clone https://github.com/korczis/prismatic-majordomus ~/majordomus
export PATH="$HOME/majordomus/bin:$PATH"

cd your-project
majordomus init            # .majordomus/ with policy, four profiles, templates
majordomus update          # CLAUDE.md, AGENTS.md, GEMINI.md generated from the policy
majordomus doctor          # names the two hook lines you still need to add
```

Add the hook lines, run `doctor` again, commit. Then, per task:

```bash
majordomus start "fix OAuth callback" --scope lib/auth --profile debugging
# ... the AI worker reads the generated instructions and works ...
majordomus check
majordomus finish --outcome completed --verify-command "make test"
```

Requirements: bash 3.2 or newer, git, and `sha256sum` or `shasum`. Nothing else.
Nothing is installed into your project except `.majordomus/` and the files the policy
names.

## Mental model

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

Two principles run through everything. **Git is the authority; everything else is an
aid.** Identity fields on any record are computed from git, and a body that tries to
set them is rejected. **Numbers in prose are computed or forbidden.** `doctor` fails on
a hardcoded count in the always-loaded file.

## Core principles

- Sessions are workers, not memory.
- Load minimum sufficient context.
- Externalise decisions and durable state.
- One worker, one clear scope.
- Escalate capability and reasoning only when justified, and record it.
- Execution depth is not output verbosity.
- Define done before executing.
- Verify outcomes, not activity.
- Parallelism requires isolation.
- Handovers transfer state, not transcripts.

These are projected into every generated instruction file, so every worker reads the
same ten sentences.

## Execution profiles

| profile | capability | effort | verbosity | context | verification | checkpoint |
|---|---|---|---|---|---|---|
| `routine` | fast | low | terse | task, state | verify command if files changed | 30m |
| `implementation` | standard | medium | concise | + decisions, files | verify command | 15m |
| `debugging` | strong | high → xhigh | concise | + failing output, history | verify command, regression test | 15m |
| `deep-work` | strongest | high → xhigh | detailed | + architecture | verify command, decision record | 30m |

Capability classes are not vendor model names; the projection tells the worker to pick
the closest its environment offers. Each axis is a separate field. Add a profile by
adding a file. Full schema: [`docs/SCHEMAS.md`](docs/SCHEMAS.md).

## Example workflow

```
$ majordomus start "fix OAuth callback" --scope lib/auth --profile debugging
started t-20260903193012-a4f1  profile=debugging  scope=lib/auth
INFO overlap     wt-bob — claims lib/auth/oauth — contained by your lib/auth  [reproduce: majordomus check --overlap]
next: worker reads the projected instructions; checkpoint every 15m; majordomus check

$ majordomus check
OK   state       t-20260903193012-a4f1 — advanced (head 3f2a9c1)
FAIL scope       config/secrets.example — outside claimed scope (lib/auth)  [reproduce: git status --porcelain; git diff --name-only 3f2a9c1 HEAD]
OK   checkpoint  t-20260903193012-a4f1 — 7m ago, interval 15m
OK   blockers    t-20260903193012-a4f1 — none open
check: 4 finding(s), 1 failing

$ printf '# Objective\n…\n# Current State\n…\n# Next Action\n…\n' | majordomus handover
.majordomus/state/handovers/20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md

$ majordomus handover --resolve      # next session, same branch
Handover: .majordomus/state/handovers/20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md
Match: same_worktree_same_branch
Git state: advanced
---
# Objective
…
```

A walk-through with real output is in [`examples/minimal/`](examples/minimal/).

## Provider adapters

```
.majordomus/policy.yaml + profiles/ + providers/body.md
         |
         |  majordomus update
         v
CLAUDE.md   AGENTS.md   GEMINI.md   <any target named in the policy>
```

Adapters are templates in `.majordomus/providers/`. They wrap one shared body; they do
not add rules. A rule that exists for one provider and not another is a policy bug. The
body is yours to edit per repository; the policy hash in every generated header changes
when you do.

## Commands

| command | answers | writes | exit |
|---|---|---|---|
| `init` | set up `.majordomus/` here | yes; refuses to overwrite | 0 / 15 |
| `doctor` | is Majordomus itself real and wired here? | no | 0 / 10 / 12 |
| `start <task>` | begin a scoped task under a profile | task record, ledger | 0 / 15 |
| `check` | is the task consistent with policy, scope, state? | no (`--checkpoint` updates one timestamp) | 0 / 10 |
| `watch` | what has drifted? | no | 0 / 11 |
| `update` | regenerate projections from policy | projections, fingerprints | 0 / 10 / 15 |
| `handover` | write a continuation record; `--resolve` finds one | one new file | 0 / 10 / 12 |
| `finish` | evaluate the finish contract | task record, ledger | 0 / 10 / 15 |

Exit codes are a contract: `0` ok, `2` usage, `10` contract unmet, `11` drift found,
`12` missing artifact, `13` internal error, `15` refused. There is no "warn and
continue". Details: [`docs/CLI.md`](docs/CLI.md).

## Customisation

- **Rules workers read:** edit `.majordomus/providers/body.md`, run `update`.
- **Which files are generated:** the `projections` list in the policy.
- **What "done" means:** `verification.finish_requires` in the policy, plus per-profile
  `verification.*` fields.
- **Budget for the always-loaded file:** `context.always_loaded_budget_lines`.
- **What must be wired:** the `enforcement` list; `doctor` reconciles it.
- **A new task class:** a new file in `.majordomus/profiles/`.

Unknown keys anywhere are errors, so a typo fails loudly.

## What this is not

- not a model, and it never invokes one
- not an agent framework, orchestrator, or runtime
- not a prompt library or a memory system
- not a server, daemon, database, queue, MCP surface, or hosted service
- not a slice of any other platform; there is no shared code

## Limitations

- Majordomus is invoked by a person, a git hook, or a worker following its
  instructions. It does not hook the worker's runtime.
- It measures no tokens and no cost. [`docs/ECONOMICS.md`](docs/ECONOMICS.md) says what
  it would take.
- Scope overlap is reported, never blocked, and only across worktrees of one repository
  on one machine.
- The regression-test check in `finish` is a path heuristic (`test/`, `spec/`, `_test.`)
  and says so in its message.
- The YAML subset is deliberately small: maps, lists, lists of maps, inline lists,
  quotes, comments. Anchors, multi-line scalars, and flow maps are rejected.

## Roadmap

| version | adds |
|---|---|
| **0.1** | everything above |
| 0.2 | opt-in runtime adapters: read-size clamp, output condensation, subagent budget, with limits derived from the profile |
| 0.3 | execution telemetry, only from providers that expose it honestly |
| 0.4 | cost per accepted outcome, only on measured data |
| 0.5 | routing recommendations derived from 0.4 |
| 1.0 | shared policy across repositories and workers |

Each step is gated by the previous one being real.

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and the generated [`AGENTS.md`](AGENTS.md).
The best contribution is often a deletion. Run `bash test/run.sh`; every behaviour has
a success and a failure case. This repository supervises itself: `bin/majordomus doctor`
runs in its own pre-commit hook and in CI.

## Origin and licence

Majordomus distils operational patterns learned while building **Prismatic**, a much
broader cognitive and epistemic platform. The relationship is one-way; no code or
vocabulary flows back. The evidence is in
[`docs/EXTRACTION_REPORT.md`](docs/EXTRACTION_REPORT.md). MIT licence.
