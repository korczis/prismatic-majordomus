---
schema: adr/v1
id: adr-0136
kind: adr
title: Every session briefs itself on an interval, from one capability a provider mod the policy switches keeps in front of it
status: proposed
date: 2026-10-11
tags:
  - session
  - peers
  - provider
provenance:
  origin: authored
related:
  - file:.ai/repo/adrs/0123-the-worker-is-asked-for-a-handover-before-the-window-fills.md
  - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - file:apps/majordomus-cli/src/capability/builtin/brief.rs
  - file:share/mods/claude-code/majordomus-brief/hooks/register.ts
  - file:lib/capture.sh
  - file:docs/BRIEF.md
  - file:.ai/repo/adrs/0207-a-new-mechanism-is-admitted-only-with-the-measured-problem-it-re.md
  - file:.ai/repo/project/issues/I4250.yaml
  - file:.ai/repo/project/issues/I4293.yaml
  - test:test/cases/1065_a_session_briefs_itself.sh
  - test:test/cases/4250_the_brief_reports_only_what_changed.sh
  - file:share/brief/report.yaml
  - file:apps/majordomus-cli/src/capability/builtin/brief/report.rs
  - file:share/schemas/majordomus/brief-report/brief-report.v1.schema.json
  - test:test/cases/4401_the_report_contract_is_declared_once.sh
  - test:test/cases/4402_brief_words_are_status_words.sh
  - test:test/cases/4403_a_stale_source_is_never_shown_as_current.sh
  - test:test/cases/4404_a_brief_never_truncates_without_a_count.sh
  - test:test/cases/4405_a_brief_is_bounded_and_never_builds.sh
---

# 136. Every session briefs itself on an interval, from one capability a provider mod the policy switches keeps in front of it

## Context

The owner ordered on 2026-10-11 that every session working in this repository keeps itself
alive and reports briefly without being asked: its own state, the peers of every board, the
mesh, the integration queue and the derive queue.

Every one of those facts has an owner already. `lifecycle.episodes` and `continuity.state` hold
the episode and the task, `peers.list` gathers the board of every checkout, the mesh runtime
holds the links and the journal, `majordomus prs brief` says the queue, and the derive lock
beside the primary checkout says who derives. A session reads none of them unless it remembers
to, and none of them says anything unasked. On 2026-10-10 sessions relayed board state to each
other by hand. A superseded server's board was missing from every gather, and no answer said
so.

Two further facts shape the decision. `prs brief` was a private function of the command line.
And a command-line process has an empty peer board and no mesh: both live in the memory of the
server holding the checkout's lease, so `peers.list` run from the command line reads its own
empty board and calls it this checkout's.

## Decision

**One capability composes the brief.** `brief.session` is declared once and projected to the
command line (`majordomus brief`), HTTP (`GET /api/v1/brief`), MCP (`majordomus_brief`) and the
Cockpit's overview. It asks each owner once and returns one line and a few, with a state for
every section (`read`, `partial`, `unread`, `undeclared`) and a summary that says what could
not be read. A section is never omitted because its owner did not answer.

**The leaseholder composes it.** A process that does not hold the lease, asked without an MCP
caller, asks the leaseholder over the route the registry declares, with `local=true`. The
server never asks back, so the hop is one deep. It composes the brief itself only when no
server answers, and says so in `answered_by`. A server that lost the lease answers its own MCP
sessions from its own board, adds the leaseholder's, and says that its board is unreachable by
the repository's gather.

**The policy declares the interval and the switch.** `session.brief_interval_minutes` (20 when
absent) is carried by the brief itself, so every reader takes it from one place.
`session.brief_on_interval` is the switch, declared in the schema, the skeleton and the
allow-list. Only `true` turns it on, for the reason ADR 0123 gave its own switch: what it
installs writes into the adopter's `.claude/` tree.

**Two carriers keep it in front of a session.**

- The start event's briefing ends with what the session is told about keeping itself awake:
  report every N minutes from `majordomus brief`, and schedule that report where the client can.
  This reaches every provider whose start event Majordomus captures.
- In Claude Code, `capture install` writes a second provider mod,
  `share/mods/claude-code/majordomus-brief`, to `.claude/skills/majordomus-brief`. It is a row
  of `MJ_CAPTURE_MODS` beside the handover mod and follows the same ownership rules. Only a
  mod can do this job: the status line and a clock are the provider's, inside the
  conversation, and no event fires for either. The mod shows the one line as the status line
  and refreshes it every minute. Once per interval it submits a prompt of its own, queued until
  the session is idle, that carries the brief and asks for a report of two to four lines
  (superseded 2026-10-29: no line count; see the amendment of that date and
  `share/brief/report.yaml`). A prompt the person sends after the interval carries the last
  brief.

## Consequences

- An idle Claude Code session wakes on the interval and reports, and a busy one is reminded
  on its next prompt. The cost is one turn per interval while the session is open, and the
  policy can lengthen the interval or switch it off.
- A command-line brief reports the server's board and mesh, or says it could not. "Nobody is
  here" is no longer printed when the truth is "I did not ask".
- The mod's behaviour is held by its own test against the engine (`claude plugin test`). Case
  1065 proves the half the repository owns: install, idempotence, the switch, and the command
  line's shapes. No case runs the mod inside a live provider session.
- `prs brief` is now read by the capability as well as by the command line. Its sentence is
  the integration section, and the one line keeps its open count and next merge.
- The Cockpit overview renders `brief.session`, attention included. `/ui` is on master and has
  no brief page; a `/ui` page for it is a separate decision (amended 2026-10-29).

## Alternatives rejected

- **A brief composed in each command-line process.** It reads an empty board and no mesh and
  reports them as facts.
- **A stream or follow mode.** A session needs a summary at an interval, not every event. The
  event stream is the monitor's (ADR 0133).
- **A settings hook on UserPromptSubmit that runs `majordomus brief`.** It costs a process on
  every prompt, cannot draw a status line, and cannot wake an idle session. It would also need
  `.claude/settings.json` edited, which is the person's own.
- **A cron in each session, scheduled by the worker alone.** It depends on the worker
  remembering, which is the failure being corrected. It stays as the advice the start event
  gives to providers that have no mod.

## Amended on 2026-10-27: at most once per interval, and only when something changed

On the night of 2026-10-26/27 a session in an adopter repository received about 40 self-reports. They
were identical except for the brief's `at` and its relative ages ("observed 4 d ago"), and each
woke the model for an answer that nothing had changed. That is noise for the person and tokens
spent on nothing. The person also read the submitted text verbatim and took two paragraphs for
unreplaced placeholders: the five-line instruction addressed to the model, and the sentence
Claude Code adds after any prompt a plugin submits ("This is how Claude Code surfaces a prompt a
plugin submits between turns ..."). The first was this mod's text; the second is Claude Code's,
and no mod can change it. Issue I4250 carries the change.

1. **The interval is a ceiling, not a schedule.** The mod tells the session the brief at most
   once per interval, and only when a fact in it changed since the session was last told. That
   covers both the self-report an idle session is woken with and the row a prompt from the
   person carries. What the session was last told starts as the first brief the mod reads,
   which is what the start event's briefing has just said.
2. **A fact is what is left when the clock is taken out.** The mod fingerprints the brief's
   lines section by section. It replaces every RFC 3339 instant (the `at`, the date of an
   observation) and every span of time ("95 s", "4 min", "38 h", "5 d", "21 h 44 min",
   "3 d 2 h", "16s", "36m") with a placeholder. From `answered by` it keeps only which process
   composed the brief, and drops why another did not answer. Everything else counts: the
   branch and head, the episode and task, the peers with what they announced, mesh links and
   claims and what is addressed to the session, the integration lanes and counts, the release
   stance, the derive lock. A threshold the clock crosses still counts when it changes a word,
   as when a release that waited too long becomes `overdue`.
3. **The submitted text is written for the person who reads it.** It opens with what changed,
   in one sentence ("Majordomus brief: since the last report, main moved 1a2b3c4 → 9f8e7d6 and
   a peer attached."), then the brief, then one line marked as addressed to the agent. Its
   first words stay the constant by which the mod's own prompt is never briefed.
4. **No report after a long unchanged stretch.** A forced report after N quiet hours was
   considered and not adopted, so no policy key declares one. The status line refreshes every
   minute and is the liveness signal a person sees, and the board and the monitor say which
   sessions are attached. A report that says nothing changed is the measured problem above,
   not a remedy for one, and ADR 0207 admits no mechanism without the problem it removes.
5. **The start event's text agrees.** `keep_awake` tells every provider's session to read the
   brief on the interval and report only when a fact in it changed. It tells a Claude Code
   session where the mod is loaded not to schedule a loop of its own, because a loop beside
   the mod repeats it unconditionally.

The consequence above that "the cost is one turn per interval while the session is open" now
reads: one turn per interval at most, and none while nothing changes. The mod's own test holds
the rule, including twelve quiet hours with no report and a mutation of the comparison that
fails it. Case 4250 runs that test, and that mutation, where `claude` is installed; case 1065
checks that what `capture install` writes is the change-aware module.

## Amended on 2026-10-27: every word of the line is current

The same day the owner read an adopter repository's line as
`<a finished task> @main | peers 1/<n>b | mesh 0/0 +4 for you | prs <n> open | release ?`
and found four of its words false. Issue I4293 carries the correction.

1. **Only a running task is the session's task.** `current.yaml` keeps the last task after it
   finished or was handed over. That task had finished `partial` fourteen days earlier. The
   session section names a task only while its outcome is `active`, and otherwise says
   `no task active`.
2. **A count the trunk has moved past is marked stale.** The integration section reads the
   last observation of the forge, which the brief never refreshes. When the forge's reported
   head of the base differs from this clone's `origin/<base>`, `prs brief` adds
   `STALE: the forge reported main at …, this clone's is …: majordomus prs refresh`. The line
   then reads `prs 23 open (stale 101m)`, with the observation's age. That comparison is the
   same one `prs status` makes.
3. **An overlap is news only across runtimes.** `+N for you` counts an overlap when exactly
   one of its two claims is this runtime's. Overlaps among one runtime's own claims, such as
   one session's subagents in several worktrees, are not counted. This reverses what the
   first version tested, which counted them. Overlaps between two other runtimes are theirs.
4. **A restart does not take a client off the board.** The board the peers section reads
   understated the repository for hours after the shared server restarted. The bridge's
   heartbeat took the new server's 404 for a pong (ADR 0003, amended the same day).

The 28 boards are the repository's 28 worktrees, each a checkout with a board of its own
(ADR 0044). They are reported and not pruned: deleting a worktree is
`majordomus://skill/assess-before-deleting`'s, not the brief's.

## Amended on 2026-10-29: one report contract, derived attention, and a brief that says how old each fact is

The owner asked, in Czech, on 2026-10-29: "vylepsi 'Periodic self-report' neni detailni jak by
mel, taktez ta zustava spousta placeholders … ui/ux/dx output standards must be high". In
English: the self-report is not detailed enough, too many placeholders remain, and the output
standard must be high.

What was measured that morning:

- The interval prompt asked for a fixed number of lines and named categories rather than
  anything measured.
- The same contract was worded differently in four places: brief.rs `keep_awake`, the mod's
  `selfReport`, docs/BRIEF.md and this ADR.
- `majordomus brief` named another session's episode and a partial task from 10-23.
- It showed a 17 h old integration observation as current, with master three merges further on.
- Peer intents were cut mid-word.
- It said nothing about the machine, the CI pool or the session's own pull requests and runs,
  which were the night's blockers.

Decision, in addition to the above:

- **The report is declared once.** `share/brief/report.yaml` (schema
  `majordomus/brief-report/v1`, validated by
  `share/schemas/majordomus/brief-report/brief-report.v1.schema.json`) declares the report's
  sections in a fixed order, the rules each line obeys, the attention rows and the live reads.
  The executable holds a generated compiled copy, as it does for the design declaration.
- **`brief.session` composes the texts from it.** One function produces `keep_awake`,
  `report.prompt` and `report.reminder`, each naming the contract and its fingerprint.
- **The carriers relay those texts verbatim.** These are the start event (`lib/capture.sh`),
  the Claude Code mod and the Cockpit. None of them, nor docs/BRIEF.md, nor this ADR, restates
  the contract. Case 4401 refuses a second copy.
- **No line count.** The report is bounded by a cap per section and never padded.
- **Attention is derived, not judged.** `SessionBrief.attention` lists every condition the
  sections measured, in the words of the contract's row: a status word filed in
  `share/design/tokens.yaml`, the fact, the remedy command and who must act. Entries a person
  must act on lead the report. An empty list says so in one sentence.
- **Every section says what it observed, when, and from where.** Each carries a source, an age,
  the bound past which it is stale, a verdict word, a remedy and its cost. A stale or unread
  fact is never presented as current.
- **The new sections are leaseholder-composed.** They are the session's own work (claims,
  branches, worktrees, pull requests, trains, runs with jobs done/total, background processes),
  the machine (free disk, swap and builders against the limits `machine.heavy_work` declares,
  I4006) and the CI pool.
- **Each section has a budget.** A section past its budget is reported unread, never waited on.
- **The brief now reads the forge.** It does so only through the monitor's shared, bounded
  cache (ADR 0133), only in the leaseholder, and never per session. This supersedes "It reads
  no forge".
- **The brief still writes nothing and builds nothing.** It never runs `prs refresh`. It names
  the command.
- **The session is the caller's.** The provider session defaults from the environment as the
  shell lifecycle resolves it. A task counts only when its outcome is active.
- **A change is a section fingerprint.** The brief carries, per section, a fingerprint of its
  clock-free facts; the mod compares those and reads no prose. A section's fingerprint is of
  the facts that concern this session: the peers section's is who is attached on which
  branch, the boards not read and the overlaps with this session's claims, never the count of
  checkouts asked or another worker's re-announcement; the CI pool's and the machine's are
  their words and attention entries; a section read past its budget is `late` and is never
  said to have changed.
- **The report opens with what changed.** Item 3 of the 2026-10-27 amendment stands: the mod
  passes the fingerprints of the brief the session was last told (`--previous-json`), and the
  interval prompt opens with the sections that changed since and lists the report sections
  none of whose sources moved, which the report writes as their heading and `[unchanged]`.
  `--since` is the instant of the last report, which a reminder never moves.
- **Where an attention entry goes is the contract's.** Each attention row names the report
  section it is written under (`report_section`), and the prompt carries each entry once,
  grouped under those sections, never a second time inside the fenced lines.

Consequences:

- The interval prompt grows to carry the contract. It is still one turn per interval, and the
  report a person reads is shorter where nothing happened (the unchanged sections are a
  heading and one word) and complete where something did.
- Adding a section means adding an attention row and its condition. A unit test holds the two
  sets equal.
- The 2026-10-11 sentence on the `majordomus ui` surface is superseded: `/ui` is on master and
  has no brief page. The Cockpit overview renders the brief, attention included, and a `/ui`
  page is a separate decision.
