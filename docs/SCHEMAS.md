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
    always_loaded: true                  # exactly one projection may be always_loaded
  - provider: codex
    target: AGENTS.md
  - provider: gemini
    target: GEMINI.md
  - provider: generic
    target: docs/AI_INSTRUCTIONS.md
```

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

```markdown
## 2026-09-03 — token refresh uses the existing session store
Task: t-20260903-193012-a4f1
Decided: reuse SessionStore rather than a new cache; one source of truth for expiry.
Rejected: separate refresh cache (two expiry clocks).
Evidence: lib/auth/session_store.rb#expiry
```

## `.majordomus/state/open-questions.md`

Things blocked on a human. `finish --outcome completed` refuses while any entry for the
current task is `unresolved`.

```markdown
- [unresolved] t-20260903-193012-a4f1 — token refresh window: 15 min or 60? needs product decision (2026-09-03)
- [resolved 2026-09-02] t-20260902-... — keep legacy callback path — yes, until Q4
```

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
| `task.checkpoint` | — |
| `task.finished` | `outcome`, `contract` (object of line → `pass`/`fail`/`skipped`), `verify` (`command`, `exit`, `seconds`) or null |
| `task.handed_over` | `handover_path`, `closed` (true with `--close`) |
| `projections.updated` | `policy_sha256`, `targets` (count) |

`doctor`, `check` (without `--checkpoint`), and `watch` write nothing, the ledger
included.

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
