# A task record names its checkout, and another checkout is never held to its scope

## What it means

`.majordomus/state/` is tracked by default, so the active task record travels with the branch. A second worktree checking out that branch reads a record it never wrote — and, before this, was held to a scope it never claimed. Its `pre-push` hook ran `finish --check` against someone else's task and refused every file outside that task's paths.

Now the record says which checkout it belongs to, and a reader that is not that checkout reports it and enforces nothing.

## How it works

`start` computes `worktree` from `git rev-parse --show-toplevel` and writes it beside `repository_id`, `branch` and `head`. It is an identity field: computed, never authored.

`mj_task_is_foreign` compares it with the current repository root. When they differ:

- `check` prints one INFO line naming the task and the checkout that owns it, and exits `0`
- `finish --check` does the same, so the `pre-push` gate passes
- `watch` reports it rather than counting it as this checkout's drift
- `finish` refuses outright: it will not write an outcome into another checkout's record
- `start` proceeds — one active task per checkout means a foreign record does not block this one — and warns that the replacement is local to this working copy, and that committing it would replace the other checkout's record on the branch

A record with no `worktree` field, written before it existed, is treated as local. An upgrade that turned every existing installation red would be a worse bug than the one being fixed.

## How to see it

```bash
git worktree add ../second
majordomus --repo ../second check
# INFO task  t-20260903193012-a4f1 — belongs to /abs/path, not this checkout; nothing enforced here

majordomus --repo ../second finish --check; echo $?           # 0
majordomus --repo ../second finish --outcome completed; echo $?   # 15, "finish it there"
```

## What it does not cover

One tracked path still holds one record. Two checkouts working concurrently and both committing `current.yaml` will overwrite each other's record on the branch; `start` warns when it is about to set that up, and nothing prevents it. A repository with several concurrent worktrees should run `majordomus init --gitignore` so each checkout keeps its own untracked record.

It does not attribute commits. A task records the commit it started at, and every file changed since counts as its work regardless of who changed it — see the limitation in [`CONTINUITY.md`](CONTINUITY.md).

## Why it exists

It was found the way these things are found: a second worktree could not push a tested fix, and every one of the failures it reported was about another session's task. "One active task per checkout" had been enforced at `start` since the first version and then never written into the record, so no other reader could apply it. A rule checked once and not represented is a rule that only holds where it was checked.
