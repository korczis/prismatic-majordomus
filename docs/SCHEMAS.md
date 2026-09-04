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
branch: main
head: 3f2a9c1e...                     # full SHA
working_tree: dirty                   # clean | dirty
```

`outcome` and `checkpoint_at` are the only fields a command changes after `start`.
`active` refuses a new `start`; every other outcome lets `start` archive the record to
`state/archive/<id>.yaml` and begin a new task.

---

## `.majordomus/state/decisions.md`

Append-only, dated, one entry per decision. Human-authored. Read by `check --explain`
(printed as context) and by projections (referenced by path, never inlined).

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
`doctor` reports when over cap, and `ledger --rotate` moves the oldest lines to
`ledger.<utc>.jsonl.archived` (never deletes).

Common envelope:

```json
{"ts":"2026-09-03T19:30:12Z","event":"task.started","task_id":"t-20260903-193012-a4f1","head":"3f2a9c1e...","branch":"main","by":"majordomus/0.1.0"}
```

Events and their extra fields:

| event | extra fields |
|---|---|
| `bootstrap` | `reason` |
| `task.started` | `profile`, `scope[]`, `owner` |
| `task.checkpoint` | `checkpoint_path` when a body was written; absent when only `checkpoint_at` moved |
| `task.finished` | `outcome`, `contract` (object of line → `pass`/`fail`/`skipped`), `verify` (`command`, `exit`, `seconds`) or null, `checkpoints` (count) |
| `task.handed_over` | `handover_path`, `closed` (true with `--close`) |
| `decision.recorded` | `decision` (the entry's title) |
| `question.opened` | `question` |
| `question.resolved` | `question`, `answer` |
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

## `.majordomus/generated/fingerprints.yaml`

```yaml
policy_sha256: 8e1d...      # hash of policy.yaml + all profiles concatenated
generated_at: 2026-09-03T18:40:00Z
targets:
  - target: CLAUDE.md
    sha256: 4b2f...
    lines: 61
  - target: AGENTS.md
    sha256: 9a01...
    lines: 118
```

`doctor` fails on any target whose current hash differs (hand-edited) and `watch`
reports policy drift when `policy_sha256` no longer matches the policy on disk.

---

## Projection header

Every generated file begins:

```markdown
<!-- generated by `majordomus update` from .majordomus/policy.yaml (8e1d...) — do not edit; edit the policy and regenerate -->
```

The header is part of the fingerprinted content.

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
