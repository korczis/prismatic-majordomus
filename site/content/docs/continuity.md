+++
title = "Continuity"
description = "how work survives the session doing it: the durable records, why transcripts are not state, how context is selected and records resolved"
weight = 16
[extra]
source = "docs/CONTINUITY.md"
+++

{% raw %}

A conversation with an AI worker is not a place to keep anything. It ends, it is
compacted, it is replaced by a different worker with a different context window, and
none of that is under your control. The work, meanwhile, has to continue.

Majordomus's answer is that operational state lives in the repository, in files that
outlive every conversation, and that the next worker is given a briefing assembled from
those files rather than a transcript to re-read.

```
Conversation is ephemeral.
Operational state is durable.
Context is selected.
History is reconstructable.
Handovers are explicit.
Completion is independently verified.
```

## The problem, stated precisely

A worker stops. Something must survive, or the next worker begins by reconstructing what
the last one knew — from commit messages, from a diff, from a human's memory of a
conversation they were not part of. That reconstruction is slow, and worse, it is
confidently wrong often enough to matter.

The obvious fix is to save the conversation. That fix is wrong for three reasons.

A transcript is not state. It is a record of how someone arrived at a conclusion,
including every wrong turn, in an order optimised for nothing. What the next worker needs
is the conclusion, the current facts, and the next action.

A transcript is unbounded. It grows with the length of the session rather than with the
size of the work, and the tenth session's context is filled with the first session's
false starts.

A transcript goes stale silently. It records what was true when it was written and says
nothing when the repository moves past it. A worker that trusts a stale narrative makes
decisions against a repository that no longer exists.

So Majordomus stores none of it. What it stores is small, typed, and each piece has one
home.

## Six kinds of durable record, and a seventh that is not task-shaped

<div class="overflow-x-auto" tabindex="0">

| Record | Answers | Mutability | Where |
|---|---|---|---|
| **task** | what is being worked on, under what constraints, within which paths | mutable; one active per checkout | `state/current.yaml` |
| **checkpoint** | what was true a few minutes ago, and what comes next | append-only | `state/checkpoints/` |
| **handover** | the deliberate package for whoever continues | append-only | `state/handovers/` |
| **decision** | what was decided, why, and what was rejected | append-only | `state/decisions.md` |
| **open question** | what is unresolved, and what it blocks | mutable index; the ledger keeps the history | `state/open-questions.md` |
| **history event** | what happened, when, for which task, at which commit | append-only | `state/ledger.jsonl` |

</div>


Prompt assets in `.ai/repo/prompts/` are a seventh thing, but they are not records of
work — they are reusable framings, versioned with the repository.

## The seventh kind: the session

The six above are all task-shaped. Each one hangs off the work, which is right, and it
leaves one question with nowhere to live: *what did one worker do between sitting down and
stopping?*

That question is not the same as any of the six. A task can outlive a worker; a worker can
touch three tasks in an afternoon. Nothing in the six records that two decisions an hour
apart, filed under different tasks, were made by the same person in the same sitting — and
that is exactly the causal thread a later reader is trying to pick up.

<div class="overflow-x-auto" tabindex="0">

| Record | Answers | Mutability | Where |
|---|---|---|---|
| **session** | what one execution episode did, between which commits, producing which records | one open per provider session, then immutable | `local/state/sessions-open/<provider session>.yaml`, pointed at by `local/state/session-current.yaml`, then the layer’s `repo/sessions/` |
| **working context** | what the worker was told when the episode opened, and what it noted while working | appended to, never rewritten | `local/session-contexts/<stamp>--<session-id>.md` |

</div>


The two are not the same record and answer opposite questions. The closed session says what
the episode produced, derived from git and the ledger, and it is shared. The working context
says what the episode was given, frozen from the builder at the open, and it is local: it
names this machine, and re-resolving it later would produce a different document, so no
surface can reproduce it and none publishes it (ADR 0015).

A session opens, may cross several tasks, and closes. `task != session` in both
directions: a task spanning two sessions is named by both, and a session spanning two
tasks names both.

### A session is an envelope, not a narrative

The closed record is identity, a temporal boundary, and references. It names the tasks,
issues, milestones, checkpoints, handovers, decisions, questions and evidence of its
episode, and it copies the body of none of them. An authored summary is allowed and is
not authority; the records it points at are.

The temptation is to write the episode down instead — one document with the decisions
quoted, the checkpoint bodies inlined, the git log pasted, the reasoning narrated. That
document is a transcript with better formatting, and it fails the same three ways any
transcript does: it is a record of how somebody arrived somewhere rather than where they
are, it grows with the length of the session rather than the size of the work, and it goes
stale silently while the repository moves past it.

So the session file holds pointers, and the pointers are checked. A reference to a
checkpoint that does not exist is a reported defect, not a broken link nobody notices.

### The envelope is derived, not accumulated

Nothing writes into an open session. `checkpoint`, `decision`, `question` and `plan` are
unchanged and know nothing about sessions. At close, the reference lists are computed from
the ledger.

The ledger is already append-only, already written only by Majordomus, already ordered by
the order the commands ran, and already validated. It knows every fact the envelope needs.
Having each command additionally append to a session file would put a write on the hot path
of commands that today append one line, and would create a second mutable account of
events the ledger already holds — which is the second source of truth this whole design
refuses. The cost is that the ledger is now load-bearing for two purposes, and that is
stated rather than hidden.

What it selects, and why it is not a time range, is worth stating because the obvious
implementation is wrong. Every ledger line now carries the session that wrote it, next to
the commit and branch it already carried. A first version selected every line between the
session's two timestamps instead, and the first time it ran for real it claimed another
worker's tasks, checkpoints and handovers: the ledger is one file per repository, two
workers were writing to it, and no timestamp can separate them. Which episode wrote an
event is something the machine knows when it writes it, so it is recorded then.

A line with no session belongs to no episode. Sessions are optional, and work done outside
one is attributed to nobody rather than to whoever had a session open nearby.

### Sessions are read with the same rules as everything else

Resolution is the two-tier rule: same repository, same worktree, same branch; then same
repository, same branch; then nothing. A session from another branch is never offered.

Divergence uses the same four labels — `exact`, `advanced`, `diverged`,
`different_context` — computed at read time from the commit the session closed at. A
session whose branch was rewritten underneath it is labelled `diverged` wherever it is
printed, and is never quietly presented as current knowledge.

Ordering comes from the recorded timestamp, with ledger line order breaking ties inside
one second. Nothing reads filesystem modification time: it does not survive a clone, and
it is not the time the record asserts.

### What a session is not

It is not a scope. A task claims paths; an episode does not, and giving one paths would
create a second, weaker claim for the coordination check to disagree with.

It is not a unit of acceptance. `finish` evaluates a task against its contract. Closing a
session asserts only that the episode ended.

It is not required. A worker that never opens a session loses the episode boundary and
nothing else; every other record is written exactly as before.

## Checkpoint and handover are different objects

They are easy to conflate and expensive to conflate.

A **checkpoint** is a progress note of an episode. It is written often, it is short by
policy (`checkpoint.max_body_lines`), and its job is to be quotable whole into the next
briefing. "Reproduced the fault with the fixture; the cause is in normalisation, not
comparison; next, write the regression test."

A **handover** is a deliberate continuation package written when a worker stops. It has
required sections, it is refused if any of them is empty, and it is the thing another
worker resumes from. It is written rarely.

**Neither of them belongs to a task.** Both name the task they were written under when
there is one, and both are written when there is not. A task is a unit of intended work
that somebody opens and closes; an episode is a provider's conversation boundary that
begins when a client attaches and ends when it detaches. Making the records depend on the
first means that the ordinary act of finishing a task disables continuity for every episode
after it — which is not a hypothetical. Between 2026-09-05 and 2026-09-11 this repository
wrote neither record, because a task had been marked `handed_over` and three separate
guards each asked, correctly against their own contract, whether a task was `active`. ADR
0052 removed the premise they shared.

A checkpoint that grows into a report is refused rather than truncated, with the
suggestion to write a handover instead. The cap is the mechanism that keeps the two
objects distinct.

## Identity comes from reality

`repository_id`, `worktree`, `branch`, `head`, `working_tree` and `changed_files` on every
record are computed from git at write time. A body that contains any of them is refused.

This is not fussiness. A worker asked to summarise its own state will produce a plausible
summary, and a plausible summary that includes a commit hash the worker did not verify is
worse than no summary, because it looks checkable. The fields a machine can determine are
determined by the machine; the fields only a person or a worker can supply are supplied by
them and marked as such.

## Records are read with a divergence label

Nothing is trusted because it exists. On read, the recorded `head` is compared with the
current `HEAD` through `git merge-base`, and the record is labelled:

<div class="overflow-x-auto" tabindex="0">

| Label | Meaning | What to do |
|---|---|---|
| `exact` | written at this commit | trust it |
| `advanced` | git has moved forward since | trust it, expect some of it to be done |
| `diverged` | the recorded commit is not an ancestor | history was rewritten; trust git, not the record |
| `different_context` | another branch | this record is not about your work |

</div>


`context` prints the label. `check` and `watch` report it as a finding. A record is
evidence, never authority.

## Resolution is deterministic, and absence is a valid answer

`handover --resolve`, `checkpoint --show` and `context` all use the same rule:

1. Same repository, same worktree, same branch.
2. Same repository, same branch, when the branch is not detached.
3. Nothing.

There is no third tier. A record from an unrelated worktree or branch is never offered,
because a globally newer record silently becoming your context is worse than having no
context at all — you cannot tell that it is wrong until you have acted on it.

Records written inside the same second would otherwise resolve in an order decided by a
random filename suffix, so the ledger, which is append-only and written in command order,
breaks the tie.

When nothing matches, the answer is "no relevant handover", not the closest thing
available. **Absence is better than incorrect memory.**

## Context is selected, not accumulated

`majordomus context` assembles a briefing in authority order — git, then the task and its
profile, then blockers, then authored records, then event history — and prints it. Nothing
is persisted; it is recomputed each time from the records and git.

What goes in is decided by the profile's `context` block. A `routine` task does not need
the decision history; a `deep-work` task wants the repository's decisions rather than only
its own. Those toggles were configuration that nothing read until the builder existed;
now they are the thing that decides.

When the assembled text exceeds `context.builder_budget_lines`, sections are dropped in a
fixed order: history first, then touched files, then decisions, then the bodies of the
checkpoint and the handover, which degrade to a pointer rather than disappearing. Git, the
task, the profile and open blockers are never dropped.

**Every exclusion is named, with its reason**, under `EXCLUDED`. An under-filled context is
debugged by reading that list, not by guessing what the tool decided to leave out. The
reported line count is the count of the document actually printed, including its own
header and trailer.

## History is for reconstruction, not for reading back a conversation

`state/ledger.jsonl` is append-only and written only by Majordomus. One line per event:
started, checkpoint, decision recorded, question opened, question resolved, handed over,
finished, projections updated, ledger rotated. Each carries a timestamp, the task, and the
commit it happened at.

`majordomus history` reads it back with filters. It answers what happened, when, for which
task, at what git state, what verification ran and what outcome was accepted. It does not
answer what anyone said, because that is not recorded.

Retention is deliberate: the ledger has a line cap, and `history --rotate` moves the
oldest lines into a dated archive file. It never deletes, and it refuses to overwrite an
existing archive.

## Blockers are state, not prose

An unresolved question in a handover paragraph is a note. An entry in
`state/open-questions.md` **refuses `finish --outcome completed`** — any entry, not only one
the active task opened. A question is a person owing an answer, and the work it blocks
outlives the task that asked; a gate that forgot at the task boundary would be a gate a
handover walks past. The store is checkout-local, under the ignored `.ai/local/state/`,
so the questions a gate can see are the ones asked in this working copy; a question is
carried to another checkout by the handover that names it, never by git.

Only *completed* is refused. `blocked`, `partial`, `no_match` and `failed` are honest
statements that the work did not finish, and refusing them would buy a green gate by forcing
a mislabelled outcome.

That gate is why the file has a machine-written line format, and why `check`, `doctor` and
`watch` fail on an entry that does not parse: a gate that cannot read an entry is a gate
that can be bypassed by mistyping one.

## Nothing here calls a model

Majordomus stores, validates, resolves, projects and verifies. It does not summarise, it
does not decide what a checkpoint should say, and it makes no network call. If you want a
model to write your checkpoint, have the worker write it and pipe it in. The tool is
deterministic infrastructure and stays that way.

## Who runs the lifecycle

The start event also converges on the shared server. With `session.ensure_server_on_start`
(on by default) it runs `majordomus serve ensure`: a server is started as a process of its
own when none answers this checkout, never built, and the briefing carries one line naming
where it stands — `Shared server: ready http://127.0.0.1:8741 pid 123`, or `not ensured:
the executable is not built (run \`just build\`)`. Discovery, loading and now the server are
three things a worker no longer has to remember; `docs/MCP.md` has the lifecycle.

Nothing above requires a worker to remember any of it.

Where a provider announces the boundaries of a sitting, its own hooks run the lifecycle.
The start event opens the episode, freezes the context the builder resolved at that moment,
and writes a briefing to standard output — which the provider adds to the context it is
about to build. That is the one moment at which a continuation record reaches a worker
without the worker asking for it, and it is the difference between a record that is written
automatically and one that is also read.

<pre class="mermaid">
flowchart TD
  ss["provider fires SessionStart"]
  opens["session opens (or the open one is kept)"]
  frozen["the working context is frozen"]
  brief["the briefing goes to stdout: the episode, the resolved&lt;br&gt;handover with its divergence label AND its freshness,&lt;br&gt;the blockers, and — only when the record is still&lt;br&gt;current — the one section to act on"]
  work["work happens"]
  pc["provider fires PreCompact"]
  cp1["a derived checkpoint, because the conversation&lt;br&gt;is about to stop holding it"]
  se["provider fires SessionEnd"]
  cp2["a derived checkpoint, for the same reason: the close&lt;br&gt;is the other moment at which what the episode knows&lt;br&gt;stops being reachable"]
  ho["a derived handover"]
  env["the episode closes into its envelope of references"]

  ss --&gt; opens
  ss --&gt; frozen
  ss --&gt; brief
  ss --&gt; work
  work --&gt; pc
  pc --&gt; cp1
  pc --&gt; se
  se --&gt; cp2
  se --&gt; ho
  se --&gt; env
</pre>


Every one of those arrows fires whether or not a task is open and whatever outcome the last
task reached, and each event writes a `provider.event.received` line to the ledger *before*
any of them is decided. A hook must return success to its provider — blocking somebody's
work is the worse failure — but returning success is not the same as claiming the work
happened, and without the receipt an event that never fired and an event that fired and did
nothing are the same observation afterwards.

## Divergence is not freshness

Every record carries two independent judgements and they answer different questions.

**Divergence** — `exact`, `advanced`, `diverged`, `different_context` — says where the
record's commit sits relative to this checkout's HEAD. **Freshness** — `fresh`, `aging`,
`stale`, `unknown`, `invalid` — says how old it is, against `session.freshness` in the
policy, which is the only place either threshold is written.

A record is `advanced` on the day it is written and still `advanced` a month later:
`advanced` means its commit is an ancestor of HEAD, which sounds like agreement and says
nothing whatever about time. A record that is both `advanced` and `stale` is the exact
shape of the 2026-09-05 outage, and it is the reason the two are reported side by side.

A stale record is history. The briefing still shows it — knowing what the last worker was
doing is worth having — but it is labelled as historical, its age and the reason are
stated, and its `Next Action` is deliberately **not** quoted as the thing to do now. An
instruction that was correct six days ago, handed over without qualification, is worse than
no instruction: the worker cannot tell it is wrong until after acting on it.

The episode belongs to the provider session that opened it, and not to the checkout. Two
windows of the same provider open on one worktree are two workers: each start event opens
or keeps *its own* episode, each end event closes its own and no other, and each worker's
ledger lines are stamped with its own episode id. An open episode is
`local/state/sessions-open/<provider session>.yaml`; `local/state/session-current.yaml`
points at the episode of this checkout, and a worker that can name its provider session
resolves its own instead of following the pointer. Before this, `--if-open keep` returned
the already-open episode whatever the event said: seven concurrent sessions here on
2026-09-09 produced one record between them, stamped with whichever id happened to be first,
and closed by whichever window was shut first.

The briefing is bounded by `session.briefing_budget_lines` and carries references, labels
and one section — never a conversation. Each of the three behaviours is a switch in the
policy, and `session.briefing_on_start: false` restores the older silence.

This is the one route by which anything under `local/` reaches a model's context without
being asked for. It is narrow on purpose, and the other half of that rule is unconditional:
nothing under `local/` is ever published by a generator or served on a public surface.
`.ai/repo/adrs/0017-an-episode-that-opens-is-handed-what-the-last-one-left.md` records why.

A worker with no such provider loses none of the model and all of the automation: every
command below is the same, and running them is again a matter of remembering.

## Reading it back from somewhere other than a terminal

`continuity.state` in the Rust executable reads this same local half and reports it over
MCP (`majordomus_continuity`), over HTTP (`GET /api/v1/continuity`) and in the Cockpit's
Continuity page: the open episode, the active task, the handover and checkpoint that
resolve here with their divergence labels, and the blockers. It uses the same two tiers and
the same four labels, and it never writes — the lifecycle has one writer, and a second
account of events the ledger already holds is what this design refuses.

It is served and never published. Those records name this machine, so no generated document
and no site page carries one, and a test proves it.

### One episode, and then the rest of them

`continuity.state` answers about **one** episode: the one `state/session-current.yaml`
resolves to. That is the right answer for a briefing — a worker wants its own episode, not a
census — and it is the wrong answer for anybody asking whether the subsystem is working,
because the pointer is a symlink that the most recent start event re-aims. On 2026-09-11
this repository held five open episodes in one checkout and no surface the tool has could
name more than one of them. An episode nobody can see is an episode that never closes.

So there is a second reading, `lifecycle.*`, which reads the **store** rather than the
pointer and answers the operator's questions instead of the worker's:

<div class="overflow-x-auto" tabindex="0">

| Capability | Answers | Where the answer comes from |
|---|---|---|
| `lifecycle.episodes` | every open episode, its provider session, where it stands (`current`, `open`, `foreign`, `stranded`), the tasks it touched, and what the ledger last saw it do | `state/sessions-open/*.yaml`, `state/session-current.yaml`, `state/ledger.jsonl` |
| `lifecycle.recovery` | episodes that can no longer close themselves, temporary files a killed close left in the tracked section, whether the pointer has been migrated, and whether every episode the ledger saw start is accounted for | the same store, plus the sessions section the manifest names |
| `lifecycle.runtime` | the commit this process is answering about, against the commit the repository is on right now | the served index's git state, and `git` read on the call |
| `lifecycle.providers` | which lifecycle events each provider's adapter declares, whether it can archive prompts, and which of this repository's enforcement entries are wired to its hook | `share/providers.yaml` and the policy's `enforcement` |
| `lifecycle.closed` | the tracked records a clone receives: how many, how many on this branch, and the newest twenty | the object index, kind `session` |

</div>


Each is exposed over MCP and over `GET /api/v1/lifecycle/<name>`, and the Cockpit's
Continuity page is their projection. None of them writes: the lifecycle has one writer, and
a second account of events the ledger already holds is what this design refuses.

Two things are deliberately **not** among them.

**No clock and no thresholds.** Nothing in `lifecycle.*` decides that a record is old. Age
is `session.freshness` in the policy and `continuity.state`'s to judge; a second engine for
it here would be the second source of truth that makes the numbers disagree. (`session.freshness`
and the `fresh | aging | stale | unknown | invalid` labels are ADR 0052's and arrive with it;
until they do, a record carries its divergence label and its recorded timestamp, and a reader
does the arithmetic. The whole point of ADR 0052 is that a reader should not have to.)

**No guess about attachment.** Whether the provider that opened an episode is still attached
to its conversation is not a fact of this repository. The episode file records no process,
the peer board records no episode, and a client that exits without firing its end event
leaves a file identical to one a live worker is using. What `lifecycle.episodes` reports is
what it can observe — where the file is, whose worktree it names, whether the pointer
follows it, and when the ledger last saw it write — and `stranded` is reserved for the one
case the repository can actually establish: the worktree the episode opened in is gone from
disk, so nothing can compose its record.

### The whole path, once

Everything above is one path, and it is worth seeing whole. Every box is a thing that
exists; the labels on the arrows are what has to be true for the next box to be reached.

<pre class="mermaid">
flowchart TD
  cmd["a person runs a command&lt;br&gt;or opens an editor"]
  conv["a provider starts a conversation&lt;br&gt;(Claude Code: SessionStart)"]
  entry["ENTRY&lt;br&gt;bin/majordomus, .envrc, or the hook shim"]
  runtime["RUNTIME ENSURED&lt;br&gt;serve ensure: one shared server per checkout,&lt;br&gt;started if none answers, never built"]
  client["CLIENT ATTACHED&lt;br&gt;MCP over stdio or HTTP; the peer board is what&lt;br&gt;the other workers can see"]
  episode["EPISODE OPENED&lt;br&gt;keyed by the provider session, not by the checkout:&lt;br&gt;state/sessions-open/&amp;lt;provider session&amp;gt;.yaml and the pointer&lt;br&gt;aimed at it. --if-open keep returns this provider&lt;br&gt;session's episode and no other."]
  fresh["FRESHNESS VALIDATED&lt;br&gt;the resolved handover and checkpoint carry two independent&lt;br&gt;labels: divergence (where their commit sits) and freshness&lt;br&gt;(how old they are). A stale record is shown as history;&lt;br&gt;its Next Action is not quoted as the thing to do now.&lt;br&gt;[ADR 0052]"]
  context["CONTEXT PROJECTED&lt;br&gt;the briefing, within session.briefing_budget_lines, frozen&lt;br&gt;into local/session-contexts/ as the working context —&lt;br&gt;what this episode was told, never rewritten"]
  events["WORK AND EVENTS&lt;br&gt;every command appends one ledger line, stamped with the&lt;br&gt;episode that wrote it. A line with no episode belongs to&lt;br&gt;none: work outside a session is attributed to nobody&lt;br&gt;rather than to whoever was open nearby."]
  checkpoints["CHECKPOINTS&lt;br&gt;written by hand, and derived on PreCompact — the moment&lt;br&gt;the conversation stops being a place anything is kept.&lt;br&gt;An artefact of the episode: no task required."]
  closing["CLOSE AND HANDOVER&lt;br&gt;SessionEnd closes the episode and, when work is still open,&lt;br&gt;writes the continuation record. The envelope's reference&lt;br&gt;lists are computed from the ledger at close, never&lt;br&gt;accumulated during the episode."]
  record[".ai/repo/sessions/&amp;lt;stamp&amp;gt;--&amp;lt;episode&amp;gt;--&amp;lt;branch&amp;gt;--&amp;lt;head&amp;gt;--&amp;lt;digest&amp;gt;.md&lt;br&gt;TRACKED. The only half of this that survives a clone:&lt;br&gt;everything under .ai/local/ names this machine and&lt;br&gt;travels nowhere."]
  resume["A FUTURE RESUME&lt;br&gt;locally, the next episode's briefing resolves the records of&lt;br&gt;this worktree and branch; elsewhere, a clone receives the&lt;br&gt;closed records and reads them as history, because a record&lt;br&gt;is evidence and never authority."]
  watching["and, beside the path, watching it:&lt;br&gt;lifecycle.episodes · lifecycle.recovery · lifecycle.runtime&lt;br&gt;lifecycle.providers · lifecycle.closed · continuity.state&lt;br&gt;projected together on the Cockpit's /cockpit/continuity"]

  cmd &amp; conv --&gt; entry
  entry --&gt; runtime --&gt; client --&gt; episode --&gt; fresh --&gt; context
  context --&gt; events --&gt; checkpoints --&gt; closing --&gt; record --&gt; resume
</pre>


One box in that path is marked, because it is the newest and the one this repository most
recently did without: **freshness validated** is a step, not a property of the record.
Without it a handover that was `advanced` — a true statement about git topology, and
identically true on the day it was written and a month later — was quoted into every new
episode for six days after the work it described was finished. The decision, the thresholds
and the labels are ADR 0052's, "The
session lifecycle is the episode's, not the task's"; every other box in the path is behaviour this tree already has. And the **tracked** box is where the local
half stops: a clone receives the closed records and nothing else, which is why no generated
document and no site page may carry a briefing, a working context or an open episode.

## Where the lifecycle puts each piece

<pre class="mermaid">
flowchart TD
  st["majordomus start #quot;&amp;lt;task&amp;gt;#quot; --scope &amp;lt;paths&amp;gt; [--profile &amp;lt;name&amp;gt;]&lt;br&gt;names any prior record for this branch"]
  ctx["majordomus context&lt;br&gt;the briefing, within budget"]
  work["work happens (Majordomus is not involved)"]
  cp["majordomus checkpoint&lt;br&gt;progress, often, short"]
  dec["majordomus decision add&lt;br&gt;what was decided and why"]
  qq["majordomus question add&lt;br&gt;what is unresolved, and it now blocks"]
  ck["majordomus check&lt;br&gt;scope, state, blockers, store integrity"]
  ho["majordomus handover"]
  nxt["the next session runs context"]
  fin["majordomus finish --outcome &amp;lt;...&amp;gt; --verify-command #quot;&amp;lt;cmd&amp;gt;#quot;"]
  hist["majordomus history&lt;br&gt;the lifecycle, reconstructable"]

  st --&gt; ctx --&gt; work
  work --&gt; cp
  work --&gt; dec
  work --&gt; qq
  work --&gt; ck
  ck --&gt;|"not finished"| ho --&gt; nxt
  ck --&gt; fin --&gt; hist
</pre>


## What this does not do

It does not know who wrote which commit. A task records the commit it started at, and
`check` treats every file changed since as the task's work. When another session commits
to the same branch in the same checkout, that session's files are reported outside your
scope. The report is correct about the files and wrong about the author, and Majordomus
has no way to tell the difference. Close the task with a handover and start a new one at
the current commit.

**A question blocks its whole branch, not the task that asked.** That is deliberate, and it
is wrong in one case: a question about work that was abandoned, left unanswered, refuses
completion of every later task on that branch until somebody writes an answer. The direction
was chosen because it fails loudly — `question list` names it and `question resolve` clears
it in one command — where the alternatives fail silently. See `M001` in
`.ai/repo/project/` for the alternatives that were rejected and the case each gets wrong.

It does not measure tokens, context savings, or cost. The budget is lines, because lines
are what it can count. See [`ECONOMICS.md`](@/docs/economics.md).

It does not rank search results, embed anything, or maintain an index. `search` is a
literal grep across the record kinds in authority order. The corpus is a handful of files;
an index would be a second source of truth that has to be kept in step with the first.

It does not resolve across worktrees or branches, and it never will silently.

## Related

[`CONCEPTS.md`](@/docs/concepts.md) for the vocabulary · [`CLI.md`](@/docs/cli-specification.md) for every command ·
[`SCHEMAS.md`](@/docs/schemas.md) for the file formats · [`DESIGN.md`](@/docs/design.md) for why the
models are shaped this way.
{% endraw %}
