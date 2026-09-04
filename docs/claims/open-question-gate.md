# An unresolved question blocks acceptance, and an unparseable entry is a failure

## What it means

Something unresolved is state, not a paragraph in a handover that a reader may or may not notice. An entry in `state/open-questions.md` naming the active task refuses `majordomus finish --outcome completed`.

## How it works

`question add` appends `- [unresolved] <task id> — <question> (<date>)`. `question resolve` rewrites that one line to `[resolved <date>]` and appends the answer; `--answer` is required, because resolving without recording the answer loses the only thing worth keeping. An ambiguous selector is refused rather than guessed, and a question may not contain ` — `, which separates the fields.

Resolving edits the line, because an index of what is still open must not accumulate. The append-only record of every opening and resolution, with its answer, is the ledger.

`finish --outcome completed` refuses while any unresolved entry names the task; `--outcome blocked` expects them and says so.

**An entry that does not parse is a failure** in `check`, `doctor` and `watch` — not a warning. A gate that cannot read an entry is a gate that can be bypassed by mistyping one, and a silent pass on a malformed blocker is worse than no blocker at all, because someone believes it is there.

## How to see it

```bash
majordomus question add "Does the legacy mobile callback still require the old URI form?"
majordomus finish --outcome completed --verify-command "make test"
# FAIL blockers  t-20260903193012-a4f1 — unresolved entry in open-questions.md
# finish: refused, 1 unmet

majordomus question resolve 1 --answer "no, it was retired in 4.2"
majordomus finish --outcome completed --verify-command "make test"   # accepted

printf -- '- [unresolved] a line with no separator\n' >> .ai/local/state/open-questions.md
majordomus check; echo $?    # 10, naming the line
```

## What it does not cover

Nothing routes a question to a person. There is no assignee, no notification and no due date; the question sits in the repository until someone answers it.

Nothing validates the answer. `--answer "yes"` satisfies the gate.

Only the active task's entries block. Another task's unresolved question is visible with `--all` and blocks nothing here.

## Why it exists

The two failure modes are symmetrical and both common. Work is accepted as complete while a question that would have changed it is still open, because the question lived in prose nobody re-read. Or work stalls on a question nobody knew existed, for the same reason.
