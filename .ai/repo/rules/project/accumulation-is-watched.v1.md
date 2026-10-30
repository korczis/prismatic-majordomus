---
id: project.accumulation-is-watched
version: 1
kind: rule
title: What accumulates is watched at the episode's events, against budgets the policy declares
description: Worktrees, merged branches and the free disk under them are read against the budgets in the policy's accumulation block at the provider's start, prompt, compaction and end events; a crossed budget is said where a worker is listening, and reduced only on the policy's consent, by a command that re-measures every candidate before it removes one.
statement: A quantity that accumulates MUST have its budget declared under `accumulation:` in the policy and MUST NOT have a default in any reader; a reading that could not be taken MUST be reported as unknown with its reason and MUST NOT count as within budget; the provider's start event MUST say every quantity that is over its budget; nothing MUST be removed unless `accumulation.act` is `reduce`; a reduction MUST re-measure each candidate immediately before removing it, MUST NOT delete a branch name that a worktree has checked out, that the trunk does not reach or that stands at the trunk's own tip, and MUST record what it removed and what it kept.
status: active
class: blocking
depends_on: [project.accumulation-is-measured@2, project.destructive-sweeps-fail-closed@1, project.reclaim-only-what-you-own@1, project.worktree-topology@1, project.no-claim-without-test@1]
tags: [operations, git, worktree, disk, lifecycle, enforcement]

x-majordomus:
  tests: [test/cases/2986_accumulation_is_watched.sh]
---

# Rationale

`project.accumulation-is-measured` gave every accumulating quantity a command that measures
it and one that reduces it. Nothing ran either. On 2026-10-13 this repository had 247
worktrees, 118 merged branches and a volume that had reached zero three times in two days,
and each time a session noticed, ran the command by hand and wrote an alert by hand. A
session's context does not work like that: the provider watches it and fires an event. This
rule gives the other three quantities the same watch. ADR 0150.

# Required behaviour

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

# Failure behaviour

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

# Verification

```
MJ_TEST_JOBS=2 bash test/run.sh 2986_accumulation_is_watched 611_the_reclaim_refuses_what_it_cannot_prove
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib accumulation
```
