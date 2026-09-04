# A task record names its checkout, and another checkout is never held to its scope

## What it means

Before the `.ai` layout, the state directory was tracked by default, so the active task record travelled with the branch. A second worktree checking out that branch read a record it never wrote — and, before this, was held to a scope it never claimed. Its `pre-push` hook ran `finish --check` against someone else's task and refused every file outside that task's paths.

Now the record says which checkout it belongs to, and a reader that is not that checkout reports it and enforces nothing. The state lives under `.ai/local/`, which is ignored, so a record no longer arrives through git at all; the field remains the defence for every other way one can arrive — a copied working directory, a synced folder.

## How it works

`start` computes `worktree` from `git rev-parse --show-toplevel` and writes it beside `repository_id`, `branch` and `head`. It is an identity field: computed, never authored.

`mj_task_is_foreign` compares it with the current repository root. When they differ:

- `check` prints one INFO line naming the task and the checkout that owns it, and exits `0`
- `finish --check` does the same, so the `pre-push` gate passes
- `watch` reports it rather than counting it as this checkout's drift
- `finish` refuses outright: it will not write an outcome into another checkout's record
- `start` proceeds — one active task per checkout means a foreign record does not block this one — and says that the replacement is local to this working copy and changes nothing where the task is active

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

One checkout still holds one record. Two people working in the same working directory share it, and the second `start` replaces the first; nothing about the field tells two workers in one checkout apart, because to git they are one.

It does not attribute commits. A task records the commit it started at, and every file changed since counts as its work regardless of who changed it — see the limitation in [`CONTINUITY.md`](../CONTINUITY.md).

## Why it exists

It was found the way these things are found: a second worktree could not push a tested fix, and every one of the failures it reported was about another session's task. "One active task per checkout" had been enforced at `start` since the first version and then never written into the record, so no other reader could apply it. A rule checked once and not represented is a rule that only holds where it was checked.
