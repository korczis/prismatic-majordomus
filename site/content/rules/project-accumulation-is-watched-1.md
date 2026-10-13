+++
title = "What accumulates is watched at the episode's events, against budgets the policy declares"
description = "What accumulates is watched at the episode's events, against budgets the policy declares"
weight = 75
[extra]
kind = "rule"
slug = "project-accumulation-is-watched-1"
identity = "project.accumulation-is-watched@1"
status = "active"
source = ".ai/repo/rules/project/accumulation-is-watched.v1.md"
+++
{% raw %}

## Rationale

`project.accumulation-is-measured` gave every accumulating quantity a command that measures
it and one that reduces it. Nothing ran either. On 2026-10-13 this repository had 247
worktrees, 118 merged branches and a volume that had reached zero three times in two days,
and each time a session noticed, ran the command by hand and wrote an alert by hand. A
session's context does not work like that: the provider watches it and fires an event. This
rule gives the other three quantities the same watch. ADR 0150.

## Required behaviour

1. **Budgets are declared.** `accumulation.worktrees_max`, `accumulation.merged_branches_max`
   and `accumulation.disk_free_min_mb` are read from the policy. A quantity with no declared
   budget is measured and reported as `unbudgeted`.
2. **Unknown is not ok.** A reading that could not be taken carries its reason and is
   counted as not measured. It never makes a verdict pass.
3. **Said at the start.** The provider's start event puts every quantity that is over its
   budget into the briefing, with what the policy allows. Nothing is printed when every
   budget holds or when `accumulation.watch` is not `true`.
4. **The disk is said on a prompt.** The prompt event reads the free disk with one `df` and
   says when it is under its floor, at most once per `accumulation.cooldown_minutes`.
5. **Consent is the policy's.** `accumulation.act: report` removes nothing and names the
   command. Only `reduce` lets an event start `majordomus accumulation --reduce --if-due`.
6. **Proved at the moment of removal.** A worktree goes only when its branch has landed, has
   an upstream it is not ahead of, has no process inside and no uncommitted work. A branch
   name goes only when no worktree has it, the trunk reaches its head, it does not stand at
   the trunk's tip, and the ref still points where it was read.
7. **Recorded.** A reduction records when it ran, why, and what it removed, deleted and
   kept; `accumulation.status` reports the last one.
8. **Told to the mesh, when announced.** With `accumulation.announce_on_mesh: true` the
   start event makes an advisory claim of the session `accumulation-watch` naming the
   machine and what is over budget, and closes that session when every budget holds again.
9. **Switchable for a process tree.** `MAJORDOMUS_ACCUMULATION=off` silences the watch. The
   test harness sets it.

## Failure behaviour

`test/cases/2986_accumulation_is_watched.sh` fails when a quantity is not measured or is
judged against anything but the policy's budget; when a quantity with no budget is judged;
when the start event's briefing does not name a crossed budget, or speaks with the watch
off or every budget held; when `act: report` removes a worktree or a branch name; when
`act: reduce` leaves a landed worktree or name, or removes unlanded work, a name at the
trunk's tip or a branch that reached no remote; when a reduction leaves no record or runs
inside its cooldown; when a compaction event starts none; or when the prompt event repeats
itself. The unit tests of `apps/majordomus-cli/src/accumulation.rs`,
`commands/accumulation.rs`, `commands/worktree.rs` and `worktree/state.rs` fail on the
verdict, the consent, the cooldown and each refusal.

## Verification

```
MJ_TEST_JOBS=2 bash test/run.sh 2986_accumulation_is_watched 611_the_reclaim_refuses_what_it_cannot_prove
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib accumulation
```
{% endraw %}
