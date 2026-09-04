# A checkpoint is a capped progress record, and a body over the cap is refused

## What it means

`majordomus checkpoint` writes what is true right now — a few lines, inside an active task — with its git identity computed. It is written often. A handover is written rarely, has required sections, and is the package another worker resumes from. The two are different objects, and the cap is what keeps them different.

## How it works

The body arrives on stdin and is checked against `checkpoint.max_body_lines`. Over the cap it is **refused**, not truncated, and the error says to write a handover instead. Truncating would produce a record that reads as complete and is not.

Front matter is computed by the same helper a handover uses: schema version, timestamp, task, profile, owner, repository id, worktree, branch, head, working-tree state and changed files. A body containing any of those fields is refused — prose does not get to author identity.

The file is created with `link` into `state/checkpoints/`, mode `0600`, so it never overwrites and is never staged. `checkpoint_at` on the task record moves, and a `task.checkpoint` event carrying the record's path is appended to the ledger.

An empty body writes no file and updates `checkpoint_at` only, which is exactly what `check --checkpoint` does. The two are the same operation; `checkpoint` is the one that can also say what was true.

A checkpoint belongs to its task: `--show` resolves the newest for the active task in this worktree and branch, and returns nothing once a new task starts. `--list` still shows every record, because the store is append-only.

## How to see it

```bash
majordomus checkpoint <<'NOTE'
State mismatch reproduced with test/fixtures/callback.json.
The cause is in normalisation, not in the comparison.
Next: regression test before touching the implementation.
NOTE
# .ai/local/state/checkpoints/20260903T194500Z--main--3f2a9c1--8c1d0e4a2b6f9317.md

majordomus checkpoint --show
stat -f %Lp .ai/local/state/checkpoints/*.md   # 600

# the cap refuses rather than truncates
yes line | head -100 | majordomus checkpoint; echo $?   # 10
```

## What it does not cover

Nothing writes a checkpoint for you. Majordomus does not summarise a session, and will not: if you want a model to write the note, have the worker write it and pipe it in.

The cap is a line count, not a judgement. A forty-line checkpoint that says nothing passes.

Retention is reported, not enforced: `checkpoint.retention_max_files` makes `doctor` and `watch` report when the store grows past it. Nothing is deleted automatically.

## Why it exists

Without a short record, the gap between "the task record moved" and "a handover was written" holds everything the worker learned, and that is exactly the interval a session tends to die in. Without the cap, checkpoints grow into reports, reports are too long to quote into a briefing, and the briefing degrades to a pointer — which is where the tool started before either record existed.
