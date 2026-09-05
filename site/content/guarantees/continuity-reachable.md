+++
title = "Every continuity store is proven reachable through its own command, not merely present on disk"
description = "majordomus doctor does not check that the continuity directories exist. It runs the commands a worker would run and reports what they do. A store nothing reads is indistinguishable from no store at all, and files on disk are not evidence that anything uses them."
weight = 65
[extra]
claim_id = "continuity-reachable"
status = "guaranteed"
source = "docs/claims/continuity-reachable.md"
+++
{% raw %}

## What it means

`majordomus doctor` does not check that the continuity directories exist. It runs the commands a worker would run and reports what they do. A store nothing reads is indistinguishable from no store at all, and files on disk are not evidence that anything uses them.

## How it works

`mj_doctor_continuity` in `lib/doctor.sh` exercises each path:

- the resolver runs against the handover store and reports its match class and git label, or a clean absence — both healthy; a record skipped as malformed is a failure
- every prompt asset is validated through the same function `prompt render` uses, so a broken asset fails here rather than when someone needs it
- `open-questions.md` and `ledger.jsonl` are parsed with the same validators the gates use, and a line that does not parse is a failure
- the context builder is invoked through `mj_cmd_context` and its output measured against the policy's budget
- the directories each command writes into are reported

`watch` runs the equivalent set as drift, and `check` runs the store validators for the active task. The same code answers the same question in all three, so the three cannot disagree.

## How to see it

```bash
majordomus doctor
# ok  layout     .ai/local/state/checkpoints — present
# ok  records    open-questions.md — every entry parses
# ok  records    ledger.jsonl — every line carries ts and event
# ok  prompt     4 asset(s) — front matter valid, every token known
# ok  resolver   handovers — same_worktree_same_branch, advanced
# ok  context    builder — 106 lines, budget 300

printf -- '---\nname: broken\ndescription: d\n---\n{{NOPE}}\n' > .ai/repo/prompts/broken.md
majordomus doctor; echo $?    # 10
```

## What it does not cover

It proves the commands run and the stores parse. It does not judge whether the records say anything useful — a repository full of empty-but-valid checkpoints passes.

It does not prove a worker ran anything. Reachability is about the tool, not about compliance.

## Why it exists

This is the failure the whole tool was built to catch, turned on itself. The evidence this design came from is full of subsystems that existed on disk, were documented as active, and were invoked by nothing — enforcement declared and never wired, stores written and never read. Adding a continuity subsystem without proving it is reachable would have reproduced exactly the thing being fixed, one directory over.
{% endraw %}
