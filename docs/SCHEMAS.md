# File schemas — every file Majordomus reads or writes

As implemented in v0.1. Every field listed here is both written by something and read by
something. A field that loses one of those is removed, not kept "for later".

Conventions:
- YAML for anything a person edits. JSON Lines for anything only Majordomus appends to.
- Unknown keys are errors at every level.
- Identity fields (`repository_id`, `branch`, `head`, `working_tree`, `changed_files`)
  are always computed from git at write time. A worker or a person supplying them is an
  error, not an override.
- Timestamps are UTC ISO 8601 with seconds: `2026-09-03T19:30:12Z`.

---

## `.majordomus/policy.yaml`

The one canonical, provider-neutral policy.

```yaml
version: 1                               # schema version; only 1 is valid in v0.1

context:
  always_loaded_budget_lines: 150        # hard cap on the always-loaded projection
  builder_budget_lines: 300              # hard cap on what `majordomus context` prints
  recent_decisions: 5                    # decisions offered to a worker, newest first
  max_list_items: 20                     # cap on any list inside the assembled context
  strategy: minimum-sufficient           # documentation of intent; projected into instructions
  transcript_is_state: false             # projected as a rule; never true in v0.1

profiles:
  default: implementation                # must name a file in profiles/
  checkpoint_interval_default: 15m       # profiles may override

verification:
  required_for_changes: true             # finish requires --verify-command when files changed
  finish_requires:                       # the contract; order is display order
    - scope_respected
    - verification_ran
    - state_updated
    - no_open_blockers
    - note_present

checkpoint:
  max_body_lines: 40                     # a body over this is refused, not truncated
  retention_max_files: 500               # doctor reports when exceeded

handover:
  required_sections: [Objective, Current State, Next Action]
  retention_max_files: 200               # doctor reports when exceeded

ledger:
  retention_max_lines: 5000

enforcement:                             # what doctor reconciles; each must be wired
  - name: doctor-on-commit
    path: majordomus                       # on PATH, repo-relative, absolute, or named in the hook line
    args: [doctor]
    wired_by: git-hook:pre-commit
  - name: finish-on-push
    path: majordomus
    args: [finish, --check]
    wired_by: git-hook:pre-push

projections:
  - provider: claude-code
    target: CLAUDE.md
    mode: region                         # file (default) | region
    always_loaded: true                  # exactly one projection may be always_loaded
  - provider: codex
    target: AGENTS.md
  - provider: gemini
    target: GEMINI.md
  - provider: generic
    target: docs/AI_INSTRUCTIONS.md
```

`mode: file` generates the whole target and owns every byte of it. `mode: region`
generates only the text between two markers and owns nothing else in the file:

```markdown
Whatever the repository already wrote. Majordomus never reads or rewrites this.

<!-- majordomus:begin 7b88abe0f22a -->
... generated from the policy ...
<!-- majordomus:end -->
```

The hash in the begin marker names the policy the region came from; it is not part of
the hashed content, so re-running `update` after an unrelated policy edit is not a hand
edit. `update` replaces an existing region in place and appends one to a file that has
none. Markers that are unclosed, out of order, or repeated are refused (`15`), never
guessed at. `fingerprints.yaml` records `mode` alongside the hash, and for a region the
hash covers the region — an edit outside the markers is the repository's business and is
never reported as drift.

`wired_by` values in v0.1: `git-hook:<name>` (resolved through `core.hooksPath` or
`.git/hooks/`), `ci:<path>` (a file that must exist and contain the invocation),
`manual` (documented, not verified; doctor lists it as unverified, never as wired).
The hook line must not swallow the exit code (`|| true`, `|| exit 0`).

---

## `.majordomus/profiles/<name>.yaml`

Five independent axes. Nothing here names a vendor model.

```yaml
name: debugging
description: reproduce, isolate, fix, and prove a defect fixed

capability: strong            # fast | standard | strong | strongest — projections map this
effort: high                  # low | medium | high | xhigh | max; omit to inherit the default
effort_escalation:            # optional; projected as guidance, not enforced in v0.1
  after_blocked_attempts: 2
  to: xhigh
verbosity: concise            # terse | concise | detailed
presentation: engineering     # machine | engineering | summary

context:                      # toggles; projections turn them into loading guidance
  task: true
  current_state: true
  decisions: true
  relevant_files: true
  failing_output: true
  recent_history_depth: 50    # commits; 0 disables
  architecture_notes: false

verification:
  verify_command_required: true
  regression_test_required: true
  decision_record_required: false

checkpoint_interval: 15m

output_contract:              # fields a completion note must carry for this profile
  - outcome
  - changes
  - verification
  - blockers
  - next_action
```

The four shipped profiles:

| | `routine` | `implementation` | `debugging` | `deep-work` |
|---|---|---|---|---|
| capability | fast | standard | strong | strongest |
| effort | low | medium | high | high, escalates to xhigh |
| verbosity | terse | concise | concise | detailed |
| presentation | machine | engineering | engineering | engineering |
| context | task, state | + decisions, files | + failing output, history 50 | + architecture, history 200 |
| verify command | if files changed | required | required | required |
| regression test | no | no | required | no |
| decision record | no | no | no | required |
| checkpoint | 30m | 15m | 15m | 30m |

---

## `share/doctrines.yaml`

Ships with the tool, not with a repository. A repository configures Majordomus in
`.majordomus/policy.yaml`; it does not declare doctrines, because a doctrine is a
statement about how the tool behaves and every user of the tool is entitled to the same
behaviour. See [`DOCTRINE.md`](DOCTRINE.md) for the model.

```yaml
version: 1
doctrines:
  - id: scope_integrity          # stable; used by --rule, by refusal messages, and by the site
    title: Scope integrity
    class: blocking              # blocking | advisory — nothing else parses
    principle: 4                 # 1-10, into the operating principles
    summary: A task touches only the paths it claimed; work found elsewhere is not accepted as done.
    validator: scope             # runs mj_validate_scope; a name with no function is a failure
    category: scope              # the finding category a violation is reported under
    enforced_by: [check, finish, watch]
    exit_code: 10                # from the existing contract; a doctrine invents no code
    policy_key: scope_respected  # finish doctrines only: the name used in verification.finish_requires
    claims: [scope-enforcement, scoped-task]   # ids in docs/CLAIMS.yaml
    test: test/cases/04_start_check.sh
```

| field | required | meaning |
|---|---|---|
| `id` | yes | unique; `[a-z_]+`. The URL slug on the site is the id with `_` replaced by `-` |
| `title`, `summary` | yes | one line each, rendered verbatim |
| `class` | yes | `blocking` stops the command; `advisory` reports and lets it pass. Any other value is a configuration error and fails closed |
| `principle` | yes | index into the ten principles in the provider body |
| `validator` | yes | the suffix of `mj_validate_<validator>`; the function must exist in `lib/` |
| `category` | yes | the finding category, so output stays one vocabulary |
| `enforced_by` | yes | commands that dispatch it; each must exist and call `mj_doctrine_dispatch` |
| `exit_code` | yes | `0` for advisory, otherwise a code from the exit-code contract |
| `policy_key` | no | present only on doctrines a repository can select in `verification.finish_requires` |
| `claims` | no | claim ids this doctrine backs; each must exist in `docs/CLAIMS.yaml` |
| `test` | yes | the case that proves the behaviour; the file must exist |

`majordomus doctor` verifies every one of those constraints against the source, and
additionally that no `mj_validate_*` function exists which no doctrine declares.

## `.majordomus/project/project.yaml`

The canonical project model's root. Present only in a repository that plans through
milestones and issues; the model is opt-in and `doctor` skips it where it is absent.

```yaml
schema_version: 1
name: Prismatic Majordomus
repository: korczis/prismatic-majordomus     # owner/name; the projection target
default_branch: master
```

Nothing derived is stored here. There is no `active_milestone` field: the active milestone
is the lowest-ordered one that is `ACTIVE`, or failing that the lowest-ordered one that is
not finished, computed on every read.

---

## `.majordomus/project/milestones/<ID>.yaml`

One executable specification of an outcome. The filename is the id; a record whose `id`
disagrees with its filename is refused rather than reconciled.

```yaml
id: M000
title: Milestone and DAG driven development
slug: milestone-dag-driven-development
order: 0                              # integer; decides which milestone is active
priority: p0                          # p0 | p1 | p2 | p3
problem: "What is wrong today."       # single-line scalars; the YAML subset has no folding
outcome: "What is true when it ends."
current_state: "Where it stands."
desired_state: "Where it is going."
scope: [...]                          # what this outcome covers
non_scope: [...]                      # what it deliberately does not
acceptance_criteria: [...]            # what would make the outcome true
validation: [...]                     # commands that demonstrate it
evidence_required: [...]              # tokens gating milestone acceptance
risks: [...]
cancelled: false                      # optional
evidence:                             # appended; each entry covers one required token
  - covers: suite
    type: test                        # test | build | ci | artifact | manual
    command: "bash test/run.sh"
    result: "every case passed"
    artifact: "…"                     # optional: a path, URL or hash
    commit: 60f83e3…                  # written by the tool
    recorded_at: 2026-09-04T03:14:00Z # written by the tool
created_at: 2026-09-04
updated_at: 2026-09-04
```

There is no `issues:` list. An issue names its milestone and the relation is read in that
one direction, so the two records cannot disagree about which issues belong to the outcome.
There is no `status:` field; see below.

---

## `.majordomus/project/issues/<ID>.yaml`

One bounded execution contract, carrying enough for a worker with no conversation history.

```yaml
id: I0007
milestone: M000
title: Enforce the canonical model as doctrine
slug: model-doctrine
priority: p1
profile: implementation               # which execution profile suits this work
parallel_safe: true                   # false forces serialisation regardless of the graph
objective: "What this issue produces."
why: "Why it exists."
current_state: "…"
desired_state: "…"
scope: [...]                          # the paths this issue may touch
non_scope: [...]
depends_on: [I0005]                   # issue ids; the edges of the DAG
acceptance_criteria: [...]            # required; an issue without one is a placeholder
validation: [...]                     # required; the commands that demonstrate it
evidence_required: [...]              # tokens that gate completion
risk: "…"
owner: alice                          # optional
completion: "…"                       # optional one-line completion report

# lifecycle markers — written only by `majordomus plan`, never by hand
started_at: 2026-09-04T03:00:00Z
verified_at: 2026-09-04T03:20:00Z
completed_at: 2026-09-04T03:31:00Z
cancelled: false
evidence:                             # same shape as a milestone's
  - covers: doctrine_test
    type: test
    command: "bash test/run.sh 44_model_doctrine"
    result: "1 passed, 0 failed"
    commit: aa90a8b…
    recorded_at: 2026-09-04T03:30:11Z
```

**There is no `status` field on either record.** `BLOCKED`, `READY`, `ACTIVE`, `VERIFY`,
`DONE` and `CANCELLED` are derived by `lib/project.awk` from the lifecycle markers, the
evidence coverage, and the state of the issue's dependencies; a milestone's status is
derived from its issues and its own evidence. Writing `status:` into either file is an
unknown key and fails `majordomus plan validate` and `majordomus doctor`.

Unknown keys are checked against `share/allow/project.txt`, `share/allow/milestone.txt` and
`share/allow/issue.txt`, the same mechanism the policy and the profiles use.

---

## `.majordomus/state/current.yaml`

The one active task. Absent when nothing is active.

```yaml
id: t-20260903-193012-a4f1          # t-<utc compact>-<4 hex>
task: fix OAuth callback
profile: debugging
owner: alice                          # free-form string
scope:                                # normalised at start; no trailing slashes
  - lib/auth
started_at: 2026-09-03T19:30:12Z
checkpoint_at: 2026-09-03T20:02:41Z   # updated by check --checkpoint and by finish
outcome: active                       # active | completed | partial | blocked | no_match | failed | handed_over

# computed from git at start; refreshed at finish; never authored
repository_id: /abs/path/.git         # git rev-parse --git-common-dir
worktree: /abs/path                   # the checkout this task belongs to
branch: main
head: 3f2a9c1e...                     # full SHA
working_tree: dirty                   # clean | dirty
```

`.majordomus/state/` is tracked by default, so this record travels with the branch. Another
worktree on the same branch reads it and must not be held to a scope it never claimed, so
`worktree` says which checkout the task belongs to. `check`, `finish --check` and `watch`
report a record from another checkout and enforce nothing from it; `finish` refuses to write
to it at all. A record written before this field existed has no opinion and is treated as
local, so upgrading does not turn an existing installation red.

`outcome` and `checkpoint_at` are the only fields a command changes after `start`.
`active` refuses a new `start`; every other outcome lets `start` archive the record to
`state/archive/<id>.yaml` and begin a new task.

---

## `.majordomus/state/decisions.md`

Append-only, dated, one entry per decision. Human-authored. Read by `decision list`,
by `context` (which prints as many entries as `context.recent_decisions` allows, for this
task or for the repository depending on the profile), by `search`, and by a prompt asset
that uses `{{DECISIONS}}`. `check` reads it only to report entries that no task or head can
be attributed to. Projections reference it by path and never inline it.

Append-only. `decision add` writes one entry with `Task` and `Head` computed; the
remaining fields come from its options. `--why` is required, because a decision with no
recorded reason cannot be reviewed later, only re-argued. Absent optional fields are
written as `-` rather than omitted, so every entry has the same shape.

An entry is never edited or deleted. `--supersedes` records that a later decision replaced
an earlier one, and refuses text that matches no recorded decision.

```markdown
## 2026-09-03 — token refresh uses the existing session store
Task: t-20260903193012-a4f1
Head: 9b1e2d4f8c3a5e7b1d0f2a4c6e8b0d3f5a7c9e1b
Why: one source of truth for expiry; a second cache would mean two expiry clocks
Rejected: a separate refresh cache
Evidence: lib/auth/session_store.rb#expiry
Supersedes: -
```

`check`, `doctor` and `watch` report an entry missing `Task`, `Head` or `Why` as a
warning: the file is hand-editable by design, and an entry nothing can attribute is a
decision that no gate will ever find. Text inside an HTML comment is the file's own
template and is not an entry.

## `.majordomus/state/open-questions.md`

Things blocked on a human. `finish --outcome completed` refuses while any entry for the
current task is `unresolved`.

Written by `question add` and rewritten in place by `question resolve`. Resolving edits the
line because an index of what is still open must not accumulate; the append-only record of
every opening and resolution, with its answer, is the ledger.

```markdown
- [unresolved] t-20260903193012-a4f1 — token refresh window: 15 or 60 minutes? (2026-09-03)
- [resolved 2026-09-02] t-20260902120000-b3d1 — keep the legacy callback path — yes, until Q4
```

Exactly two line shapes are valid:

```
- [unresolved] <task id> — <question> (<YYYY-MM-DD>)
- [resolved <YYYY-MM-DD>] <task id> — <question> — <answer>
```

` — ` separates the fields, so a question may not contain it. Any other `- [` line is a
**failure** in `check`, `doctor` and `watch`, not a warning: this file is a gate on
acceptance, and a gate that cannot read an entry is a gate that can be bypassed by
mistyping one.

---

## `.majordomus/state/handovers/<file>.md`

Filename: `<utc-compact>--<branch-key>--<short-head>--<16 hex>.md`, e.g.
`20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md`. `branch-key` is the branch
with any character outside `[A-Za-z0-9._-]` replaced by `-`, or `DETACHED`.

```markdown
---
schema_version: 1
created_at: 2026-09-03T20:14:55Z
task_id: t-20260903-193012-a4f1
profile: debugging
owner: "alice"
repository_id: /abs/path/.git
worktree: /abs/path
branch: main
head: 9b1e2d4f...
working_tree: clean
changed_files:
  - lib/auth/oauth.rb
  - test/auth/oauth_test.rb
---

# Objective
Fix the OAuth callback dropping state on redirect.

# Current State
Root cause found: state param stripped by the proxy rewrite. Fix in oauth.rb applied,
regression test added, suite green.

# Next Action
Decide the refresh window (open question) and then finish.

# Decisions
See decisions.md 2026-09-03.

# Verification
make test — exit 0, 41s, at head 9b1e2d4.
```

Front matter is written by Majordomus. A body containing a line that looks like a
front-matter key is rejected. Mode `0600`. Created atomically. Never staged.

---

## `.majordomus/state/checkpoints/<file>.md`

Filename and front matter are exactly a handover's; only the directory and the body rules
differ. Written by `checkpoint`, mode `0600`, created atomically, never staged.

```markdown
---
schema_version: 1
created_at: 2026-09-03T19:45:00Z
task_id: t-20260903193012-a4f1
profile: debugging
owner: "alice"
repository_id: /abs/path/.git
worktree: /abs/path
branch: main
head: 9b1e2d4f8c3a5e7b1d0f2a4c6e8b0d3f5a7c9e1b
working_tree: dirty
changed_files:
  - lib/auth/oauth.rb
---

OAuth state mismatch reproduced with test/fixtures/callback.json.
The cause is in callback normalisation, not in the comparison.
Next: regression test before touching the implementation.
```

The body is free text, not sections, and is capped at `checkpoint.max_body_lines` lines. A
body over the cap is **refused**, not truncated, with the suggestion to write a handover
instead: the cap is what keeps a checkpoint short enough to quote whole into the next
briefing rather than becoming a second kind of report.

A body containing any identity field is refused, as for a handover. An empty body is
allowed and writes no file — it updates `checkpoint_at` only, which is what
`check --checkpoint` does.

Read by `checkpoint --show` and `--list`, and by `context`, all through the same resolver
as handovers: same worktree and branch, else same branch, else nothing.

---

## `.majordomus/state/session-current.yaml`

The open session in this worktree. One at a time; `session start` refuses while one is
open rather than replacing it. Removed by `session close`, which is the only writer that
removes it.

```yaml
session_id: s-20260904153733-fc51
started_at: 2026-09-04T15:37:33Z
owner: "alice"
worker: "some-provider/some-model"      # optional; recorded only when supplied
# computed from git; never authored
repository_id: /abs/path/.git
worktree: /abs/path
branch: master
start_head: 9b1e2d4f8c3a5e7b1d0f2a4c6e8b0d3f5a7c9e1b
start_working_tree: dirty
```

`session_id` is `s-` followed by the compact UTC timestamp with its `T` and `Z` removed
and four hex characters, which is the shape a task id already has. `worker` is a free-form
string and is the only field a person or a worker supplies beyond `owner`; nothing
validates it, and nothing infers it when it is absent — an unrecorded worker stays
unrecorded rather than becoming a plausible guess.

Unknown keys are an error, as in every other Majordomus YAML file.

This file is **not tracked**. The other state files are, because something outside the
checkout reads them: a task record carries the scope claim other worktrees compare
against, the question store is scoped to a branch by version control, and the append-only
records have to travel. An open session carries none of that, so committing one would only
make every checkout on the branch inherit an episode it did not open. Closed sessions are
durable records and are tracked.

The defence stays in place regardless: a session record from another worktree is reported and is not treated as this checkout's
open session, exactly as a foreign task record is.

---

## `.majordomus/state/sessions/<file>.md`

The immutable record of a closed session. Filename:
`<utc-compact>--<session-id>--<branch-key>--<short-head>--<16 hex>.md`, e.g.
`20260904T171402Z--s-20260904153733-fc51--master--3c9ba2f--c0ffee1234567890.md`.

The grammar is the handover's with the session id inserted after the timestamp, and it
carries the same four properties. The leading UTC timestamp makes lexicographic order
chronological order. `branch-key` is branch-safe by construction. The short head says
which history the record was written against. The sixteen random hex characters plus an
atomic hard-link publish make it collision-safe: a name already in use is retried, never
overwritten. Nothing reads filesystem modification time, which does not survive a clone
and is not the time the record asserts.

```markdown
---
schema_version: 1
created_at: 2026-09-04T17:14:02Z
task_id: t-20260904153733-fc51
profile: deep-work
owner: "alice"
repository_id: /abs/path/.git
worktree: /abs/path
branch: master
head: 3c9ba2f1d0e5a7b9c3f5e7a9b1d3f5e7a9b1d3f5
working_tree: dirty
changed_files:
  - lib/session.sh
session_id: s-20260904153733-fc51
started_at: 2026-09-04T15:37:33Z
closed_at: 2026-09-04T17:14:02Z
outcome: closed
worker: "some-provider/some-model"
start_head: 9b1e2d4f8c3a5e7b1d0f2a4c6e8b0d3f5a7c9e1b
start_working_tree: dirty
commits:
  - 2c4dc6f
  - 3c9ba2f
tasks:
  - t-20260904153733-fc51
issues:
  - I0801
milestones:
  - M003
checkpoints:
  - .majordomus/state/checkpoints/20260904T161122Z--master--2c4dc6f--a1b2c3d4e5f60718.md
handovers: []
decisions:
  - "2026-09-04 — The session envelope is derived from the ledger, not accumulated"
questions: []
evidence:
  - I0801:boundary_rewritten
---

Optional. Free text, written by whoever closed the session, or absent.
```

`created_at`, `head` and `working_tree` describe the close, so the record reads back
through the same resolver and the same divergence label as a handover: `head` is the
commit the session ended at, and the label compares it with the current one. `start_head`
and `start_working_tree` describe the open.

`commits` is the list of commits between `start_head` and `head`, shortest form. When the
start commit is not an ancestor of the end commit — a rebase during the episode — the
value is the single entry `diverged` rather than a list computed across a history that no
longer connects.

The reference lists carry the identity each kind actually has, and nothing invents one:

| Field | Identity used | Why that one |
|---|---|---|
| `tasks` | task id | It exists and is stable. |
| `issues`, `milestones` | canonical id | Same. |
| `checkpoints`, `handovers` | repository-relative path | The files are immutable, so the path is the identity. |
| `decisions` | the dated title | `decisions.md` has no id field; the dated title is the heading it already uses, and it is what the ledger records. |
| `questions` | the question text | `open-questions.md` has no id field either; the text is what its line format keys on. |
| `evidence` | `<issue>:<token>` | The pair an evidence record is attached to. |

Two of those are weaker identities than the rest, and that is recorded rather than
papered over: a decision or a question is referenced by text, so editing that text breaks
the reference, and session validation reports it as a dangling reference rather than
silently resolving to nothing.

**The lists are derived, not accumulated.** They are computed at close time by reading
`ledger.jsonl`, and by nothing else. No other command writes to the session record;
`checkpoint`, `decision`, `question` and `plan` are unchanged and know nothing about
sessions.

The alternative — every command appending its reference to the open session file as it
runs — was rejected twice over: it puts a write on the hot path of commands that
currently only append one ledger line, and it creates a second mutable store of facts the
ledger already holds, which is the thing this record exists to avoid being.

**Selection is by the session stamp on each line, not by a time range.** Every ledger line
carries a `session` field naming the episode that wrote it, alongside the `head` and
`branch` it already carried; all three are computed, none is authored. A time range was
tried first and was wrong: the ledger is one file per repository, and the first real run
of it collected another worker's tasks, checkpoints and handovers, because nothing in a
timestamp tells two concurrent workers apart.

A line carrying no `session` belongs to no episode, and no envelope claims it. That is the
correct answer rather than a gap — sessions are optional, and work done outside one is
attributed to nobody instead of to whoever happened to have a session open nearby.

The cost is that the ledger becomes load-bearing for a second purpose. It is append-only,
written only by Majordomus, and already validated by `ledger_integrity`, which is what
makes it a safe thing to derive from. Ledger line order is preserved, so two events
written inside the same second need no tiebreak at all.

Front matter is written by Majordomus. A body containing a line that looks like a
front-matter key is rejected, as for a handover. Mode `0600`. Created atomically. Never
overwritten, never edited after the fact: a closed session is superseded by later
information, not corrected.

---

## `.majordomus/prompts/<name>.md`

A reusable framing, versioned with the repository. Front matter is authored, not computed —
these are not records of work.

```markdown
---
name: debug
description: frame a defect so that the fix is proven, not asserted
profile: debugging
---
Task {{TASK_ID}} on branch {{BRANCH}} at {{HEAD}}: {{TASK}}

Open questions that block acceptance:
{{OPEN_QUESTIONS}}
```

| Key | Required | Meaning |
|---|---|---|
| `name` | yes | must equal the filename without `.md` |
| `description` | yes | non-empty; shown by `prompt list` |
| `profile` | no | the profile this framing suits; documentation only, nothing selects on it |

Unknown keys are errors, as everywhere else.

**Tokens are a closed set.** Inline, substituted anywhere in a line: `{{TASK}}`,
`{{TASK_ID}}`, `{{PROFILE}}`, `{{SCOPE}}`, `{{OWNER}}`, `{{BRANCH}}`, `{{HEAD}}`,
`{{WORKING_TREE}}`, `{{REPOSITORY}}`, `{{NOW}}`. Block, valid only alone on a line:
`{{OPEN_QUESTIONS}}`, `{{DECISIONS}}`, `{{CHECKPOINT}}`, `{{HANDOVER}}`, `{{CONTEXT}}`.

Any other `{{...}}`, or a block token used inline, is an **error**. There is no templating
language: no conditionals, no loops, no includes, no shell. A prompt that silently renders
`{{TSAK}}` as literal text is worse than one that refuses. `doctor` and `watch` validate
every asset for the same reason.

An asset whose body contains `{{CONTEXT}}` is excluded from `context --prompt` rather than
rendered, and the exclusion is named: the result would be the same context nested inside
itself, which the budget then pays for twice.

---

## `.majordomus/state/ledger.jsonl`

Append-only. Written only by Majordomus. One JSON object per line. Retention-capped;
`doctor` reports when over cap, and `history --rotate` moves the oldest lines to
`ledger.<utc>.jsonl.archived` (never deletes).

One line is one **history event**: what happened, when, for which task, at which head, and
in which session. `majordomus history` reads them back.

Common envelope:

```json
{"ts":"2026-09-03T19:30:12Z","event":"task.started","head":"3f2a9c1e...","branch":"main","by":"majordomus/0.1.0","session":"s-20260903193010-a4f1","task_id":"t-20260903193012-b7c2"}
```

`session` is present when a session is open in this worktree and absent otherwise. Like
`head` and `branch` it is computed, never authored. It is what a closed session's
reference lists are selected by, and it exists because the alternative does not work: the
ledger is one file per repository, so a selection by time range cannot tell two concurrent
workers apart, and the first real run of one claimed another worker's records. A line with
no `session` belongs to no episode, which is the honest answer for work done outside one.

Events and their extra fields:

| event | extra fields |
|---|---|
| `bootstrap` | `reason` |
| `task.started` | `profile`, `scope[]`, `owner` |
| `task.checkpoint` | `checkpoint_path` when a body was written; absent when only `checkpoint_at` moved |
| `task.finished` | `outcome`, `contract` (object of doctrine id → `pass`/`fail`/`skipped`), `verify` (`command`, `exit`, `seconds`) or null, `checkpoints` (count) |
| `task.handed_over` | `handover_path`, `closed` (true with `--close`) |
| `decision.recorded` | `decision` (the entry's title) |
| `question.opened` | `question` |
| `question.resolved` | `question`, `answer` |
| `session.started` | `owner`, `worker` when one was supplied |
| `session.closed` | `outcome`, `session_path` |
| `ledger.rotated` | `archived` (lines moved), `kept`, `archive` (path) |
| `projections.updated` | `policy_sha256`, `targets` (count) |

`doctor`, `check` (without `--checkpoint`), `watch`, `context`, `history`, `search`, and
`prompt` write nothing, the ledger included.

Every line must be a JSON object carrying `ts` and `event`. A line that is not is a
**failure** in `history --validate`, `check`, `doctor` and `watch`: a ledger the tool
cannot parse is a ledger that cannot be used as evidence. The reader skips such a line
rather than crashing, so a corrupted ledger is still readable while it is being reported.

Because `created_at` has second resolution, two records written inside one second would
resolve in an order decided by their random filename suffixes. The ledger is append-only
and written in the order the commands ran, which makes it the tiebreak the resolver uses —
the one portable monotonic ordering available without sub-second timestamps.

`history --rotate` moves all but the newest `ledger.retention_max_lines` lines into
`ledger.<utc-compact>.jsonl.archived` and appends a `ledger.rotated` event. It never
deletes and refuses to overwrite an existing archive.

---

## Projection stamp

Every generated target describes itself; there is no provenance file beside it, so a
fresh clone carries the same evidence as the checkout that generated it.

A file-mode target begins:

```markdown
<!-- generated by `majordomus update` from .majordomus/policy.yaml (policy 8e1d2c3b4a5f, content 4b2f9a01c7d3e6f8) — do not edit; edit the policy and regenerate -->
```

`policy` is the hash of the policy and every profile, concatenated, at generation time.
`content` is the hash of everything below the stamp line. A region-mode target carries
the same two values in its begin marker:

```markdown
<!-- majordomus:begin 8e1d2c3b4a5f 4b2f9a01c7d3e6f8 -->
...generated region...
<!-- majordomus:end -->
```

and `content` then covers the region body only, never the host document around it.

`doctor` fails on any target whose content differs from the hash its stamp names
(hand-edited) and on a target with no stamp at all (not written by `update`). `watch`
reports policy drift when a stamp names a policy hash that is no longer the policy on
disk, and `update` refuses to overwrite a target whose content matches neither its stamp
nor the new output.

---

## Worktree coordination

There is no sidecar registry. `start` and `check --overlap` read `git worktree list`
and each worktree's own `.majordomus/state/current.yaml`. Git is the authority; nothing
can rot.

---

## Completion note (when no handover is written)

`finish` accepts `--note <file>`. For `completed` it needs the handover's required
sections; for `partial`/`blocked` a `# Next Action`; for `no_match`/`failed` a
`# Reason`. The profile's `output_contract` fields may lead as a YAML block; v0.1 records
the note but does not validate that block. On success the note is copied to
`state/completed/<id>.md`.

```markdown
---
outcome: completed
changes: 2
verification: make test — exit 0
blockers: []
next_action: none
---
# Objective
...
# Current State
...
# Next Action
none
```
