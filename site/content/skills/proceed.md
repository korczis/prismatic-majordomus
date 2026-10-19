+++
title = "Proceed with the next admissible work"
description = "Ask the repository what this session proceeds with, take that one piece of work exactly once, carry it to a verified finish and hand it on, without choosing it by hand or duplicating anyone's."
weight = 7
[extra]
id = "proceed"
status = "active"
version = 1
source = ".ai/repo/skills/proceed/SKILL.md"
+++
{% raw %}

## Purpose

A session that is told only to proceed should not have to know what to work on. The plan,
the board and the mesh already know what is ready, what is held and by whom; this skill asks
them through one command and acts on the answer. The choice and its reasons belong to
`majordomus proceed plan`, the take to `majordomus proceed take`: a change to how work is
chosen or taken is a change to those, and never to this file.

It is not a scheduler, a second claim ledger or a landing seat: it chooses nothing itself,
holds nothing itself, and never merges, tags or releases.

## When to use

When a session is asked to proceed, to continue, or to take the next piece of work, and no
issue is named. Said again in the same session it resumes what the session took; with
nothing changed it creates nothing.

## Procedure

### 1. Ask for the decision

Run `majordomus proceed plan`. Read the selection, why it was selected, every passed-over
issue with the hold that refused it, and every source with its state.

- Exit 10: nothing is admissible. Report the reason and the waiting counts, and stop.
- Exit 11: the decision is provisional, because a source named in it was not read. Report
  that source and its reason, and stop; a provisional decision is never taken.

### 2. Take it

Run `majordomus proceed take`. It answers with the issue, the claim, the worktree and the
task, and a step word for each: `created`, `found`, `reclaimed` or `released`. Announce the
issue and the scope it printed on the board (`peers.announce`). A refusal (exit 15) names
what holds the work; report it as it is and stop.

Continue in the worktree it printed. Every later step happens there.

### 3. Do the work

Follow `majordomus://skill/implement` for the selected issue, within the scope the take
printed, until `majordomus check` agrees and `majordomus finish` accepts the task.

### 4. Hand it on

Push the branch and hand its head to the landing seat. Run `majordomus proceed take` once
more: a finished task ends the take and releases its claim. Then ask for the decision again
if the session is to continue.

## Output

The report follows `majordomus://skill/report-verification-state`. It contains:

- the decision and its reason, as `proceed plan` printed them;
- the take's step words;
- the commits and the pushed head;
- what was handed to whom.

When nothing was taken, the report is the exit code, the reason, and the source or the
hold that decided it.
{% endraw %}
