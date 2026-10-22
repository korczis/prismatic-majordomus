+++
title = "A session ends, the work does not"
description = "An AI session's context window fills, gets compacted, or closes with the terminal, usually in the middle of the work. Why Majordomus does not save the conversation, what it writes at each boundary instead, and how six days of an outdated instruction taught it to say how old a record is."
template = "page.html"
[extra]
lang = "en"
related = ["the-bet-on-a-layer/", "everything-followed/", "claimed-before-it-is-built/", "one-branch-one-place/", "many-machines-one-journal/"]
[extra.translations]
cs = "session-skonci-prace-ne/"
+++

A conversation with an AI worker is a bad place to keep anything. Its window fills and is compacted. A person closes the terminal, a machine restarts, or another worker with a different window takes over. Most of the time the work is not finished when that happens, and the next worker has to find out where it stands.

The obvious fix is to save the conversation. [`docs/CONTINUITY.md`](@/docs/continuity.md) explains why this repository does not. A transcript records how someone arrived at a conclusion, every wrong turn included, and it is not state. Operational state lives in files that outlive every conversation. The next worker gets a briefing assembled from those files, not a transcript to read again.

## The provider draws the boundary

The unit is an **episode**, the provider's own conversation boundary: it begins when a client attaches and ends when it detaches. Where a provider fires lifecycle events, those events open and close it ([ADR 0015](@/adrs/adr-0015.md), proposed). Both are idempotent, because a start fires again on a resume or a compaction: it keeps the episode that is open, and an end that finds nothing open writes nothing. The event's reason decides whether the episode closed or was interrupted, and a reason the adapter does not know closes it as interrupted, because calling a cut-short episode complete is the worse of the two mistakes.

When an episode opens, the next worker is handed what the last one left ([ADR 0017](@/adrs/adr-0017.md), proposed). The start event writes a bounded briefing into the context the provider is about to build. It says which episode this is, what the last one left with a label for how far to trust it, and what blocks acceptance, and it quotes the one section of a handover a resuming worker acts on. When there is nothing to hand over, it says so, because silence looks the same as a briefing that failed to run.

Two other moments get a record. A compaction is announced before it happens, which is the only moment anything can be written about a context that is about to be discarded, so a checkpoint is derived there. And when an episode ends, a handover is derived before it closes, whether or not a task is open.

## A checkpoint and a handover

The two records are easy to confuse and expensive to confuse. A **checkpoint** is a short progress note, written often and capped in length, so that the next briefing can quote it whole. A **handover** is a deliberate continuation package, written when a worker stops. It has three required sections, the objective, the current state and the next action, and it is refused when any of them is empty.

What a machine can determine, a worker does not get to state. The repository, worktree, branch, commit and changed files of every record are computed from git when it is written, and a body that contains any of them is refused. A plausible summary with a commit hash nobody verified is worse than no summary, because it looks checkable.

## Six days of an outdated instruction

On 11 September 2026 an audit found that the repository had written no checkpoint and no handover since 5 September. For six days `doctor` and `watch` reported health. Episodes opened and closed normally, and only the two records a worker resumes from had stopped.

A task had been marked as handed over and never replaced. Three guards, each correct against its own contract, asked whether a task was active before writing anything, and each stopped without leaving a record that it had. Meanwhile every new episode was briefed with the newest handover on the branch, its next action quoted word for word, under a label that said the recorded commit was an ancestor of the current one. That was true, and it said nothing about age. Six days of workers were told to continue work that had been finished six days earlier.

[ADR 0052](@/adrs/adr-0052.md) (proposed) removed the premise the three guards shared, that an episode happens inside a task. An episode's lifecycle no longer depends on task state. Every lifecycle event is recorded before any guard runs, so a skipped record leaves evidence that the event arrived. Freshness became a dimension of its own, beside divergence: a record can be `advanced` and `stale` at once, and that combination is the one that caused the outage. A stale record is shown as history, with its age and the reason, and its next action is not quoted as the thing to do now. The manual puts the principle in one line: a record is evidence, never authority.

## Asked before the window fills

Writing a handover takes a turn of its own, and that turn has to come before the window runs out. So a provider mod asks for it in advance ([ADR 0123](@/adrs/adr-0123.md), proposed). Once the window crosses a line, a fixed headroom short of its end or a share of it, whichever comes first, the worker is asked once for a `majordomus handover` with the three sections, told to read the live state first, and told to stop afterwards. It is not asked again in that window, and not at all when it already wrote a handover there. An adopter turns this on with one policy switch, and a missing switch is not consent, because the mod writes into the adopter's own `.claude/` tree.

## A handover moves between machines

Work also moves from one machine to another, and a handover used to reach another machine only through a live mesh link. Now it can be published as a record that carries only portable state ([ADR 0105](@/adrs/adr-0105.md), proposed): the handover itself, with the repository's identity, the device, the episode, the branch, the commit, the changed paths and a fingerprint of the uncommitted change but never its content, the task and its decisions, and the record it continues. The record is identified by its content and signed by the device's mesh key. It travels in a ref of its own, written with git plumbing, so no branch, index or working tree moves and no pull request carries it.

It is refused, not redacted, when it would carry what must not travel: a credential of a known shape, the value of a secret environment variable, a path of somebody's home directory, or a repository path that is absolute or leaves the repository. The refusal names the field and never repeats the value, and the author decides what the record says instead. Every record is admitted, by its schema, its id, its signature, the repository's identity and its portability, before anything reads it. The rule is [project.session-continuity-is-portable](@/rules/project-session-continuity-is-portable-1.md).

## What it costs

Nothing here makes a model call of its own, and none of it summarises a conversation. It costs a few files per episode, a briefing bounded by policy at the start of each, a handover per stop and, where the mod is on, one turn per window. What it buys is that a worker that starts in the middle of somebody else's work is told where that work stands, and how old that statement is. The whole lifecycle, from one episode on one machine to a handover between two of them, is in [`docs/CONTINUITY.md`](@/docs/continuity.md).
