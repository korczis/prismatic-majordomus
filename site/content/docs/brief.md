+++
title = "The session brief"
description = "the session brief: `majordomus brief` and `brief.session` — one line and a few about what surrounds a session (its episode and task, the peers of every board, the mesh, the integration queue, the derive lock), who composes it, the policy switch, and the Claude Code mod that keeps a session briefed and awake"
weight = 59
[extra]
source = "docs/BRIEF.md"
+++

{% raw %}

What surrounds one working session, in one line and in a few, so that a session can report on
itself without being asked. Behaviour as implemented and tested; where implementation and this
document disagree, the document is wrong and changes in the same commit as the fix. The decision
and what it rejected are
[ADR 0136](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md).

## The problem this solves

Every fact a working session needs about its surroundings already has an owner: the episode
and the task (`lifecycle.episodes`, `continuity.state`), the workers of every checkout
(`peers.list`), the machines (`mesh.cooperation`, `mesh.state`), the pull-request queue
(`majordomus prs brief`) and the machine's derive lock (`lib/machine_lock.sh`). A session reads
none of them unless it remembers to. On 2026-10-10 the sessions of this repository relayed
board state to each other by hand, and a superseded server's board was missing from every
gather without anyone being told. A session that is not told goes quiet, and a quiet session
cannot be told from a dead one.

## One capability, four surfaces

`brief.session` asks each owner once and says, per section, whether it could be read. It
stores nothing and judges nothing of its own.

<div class="overflow-x-auto" tabindex="0">

| Surface | How it is asked |
|---|---|
| Command line | `majordomus brief` (the lines), `--line` (the one line), `--keep-awake` (the start text, then the line), `--format json` |
| HTTP | `GET /api/v1/brief?session=<id>&checkouts=this` |
| MCP | the tool `majordomus_brief` |
| Cockpit | the "Session brief" card on the overview, beside the four questions |

</div>


The input is optional throughout: `session` is the provider's own session identity (Claude
Code's session id), so the brief names the open episode that session owns, else the one the
pointer resolves to; `checkouts` is `repository` (every board, the default) or `this`; `local`
is what a forwarded request says, and a person never needs it.

### The sections

<div class="overflow-x-auto" tabindex="0">

| Section | Owner | Says |
|---|---|---|
| `session` | `lifecycle.episodes`, `continuity.state` | the episode, the branch and head, the active task, and the calling MCP session's place on the board |
| `peers` | `peers.list` | attached workers across the boards read, every board that could not be read with its reason, overlapping claims, and up to eight workers by name with what they announced |
| `mesh` | the mesh runtime | links connected and failing, claims held and this runtime's share, and what the journal addresses to this runtime: open reviews naming it, handovers nobody consumed, overlaps with its claims |
| `integration` | `majordomus prs brief` | the queue as the last observation of the forge left it, or that this checkout never observed it |
| `derive` | the derive lock | its holder, whether that process runs, how long it has held the lock, and the priority marker; `undeclared` in a repository that ships no lock |

</div>


Each section carries a state: `read`, `partial` (part of what it covers could not be asked;
the summary says which), `unread` (its owner did not answer; the summary says why) or
`undeclared`. A section is never left out because it could not be read.

## Who answers

The board and the mesh live in the memory of the server that holds the checkout's lease. A
command-line process has an empty board of its own and no mesh, so a brief composed there
would say "nobody is here" when the truth is "I did not ask". The command line therefore asks
the leaseholder over the HTTP route the registry declares, with `local=true` so that the server
never asks back, and composes the brief itself only when no server answers. The answer says
which process composed it (`answered_by`: `leaseholder`, `superseded` or `this_process`) and
why when it is not the leaseholder.

A server that lost the lease to a newer one still holds its own MCP sessions, and the
repository's gather cannot see them, because `peers.list` reads a checkout's board from the
server its lease names. Such a server answers an MCP session's brief from its own board, adds
the leaseholder's, marks the peers section `partial` and the line `SUPERSEDED`, and says that
the peers attached to it are unreachable by everyone else.

## Keeping a session awake

`session.brief_on_interval` is the switch and `session.brief_interval_minutes` the interval
(20 when absent). The brief carries the interval, so every reader takes it from one place.

- **At the start of every episode**, whatever the provider, the start event's briefing ends
  with a "Keeping awake" section: every N minutes, without being asked, report in two to four
  lines from `majordomus brief`, never from memory, and schedule that report where the client
  can schedule its own prompts. Then the brief as it stands.
- **In Claude Code**, `capture install` writes the provider mod
  `share/mods/claude-code/majordomus-brief` to `.claude/skills/majordomus-brief`, beside the
  handover mod of ADR 0123 and under the same ownership rules. The mod shows the one-line brief
  as the status line and refreshes it every minute. Once per interval, when the session is
  idle, it submits a prompt of its own that carries the brief and asks for the short report, so
  the session wakes and reports. A prompt the person sends after the interval has passed
  carries the last brief with it. The mod never builds the executable
  (`MAJORDOMUS_NO_BUILD=1`), and outside a supervised repository it does nothing.

Neither writes `.claude/settings.json`. The start event's section rides on the SessionStart
shim `capture install` already wires, and a Claude Code session loads a mod from
`.claude/skills/<name>` by itself.

## What it is not

The brief is one composed answer, asked when wanted. It is not a stream and has no follow
mode: following events as they happen is the monitor's (ADR 0133). It reads no forge and
starts no server; the integration section is what the last observation left.

## Where it lives

<div class="overflow-x-auto" tabindex="0">

| Path | What |
|---|---|
| `apps/majordomus-cli/src/capability/builtin/brief.rs` | the capability, its sections and their composition |
| `apps/majordomus-cli/src/commands/brief.rs` | the command line's rendering |
| `apps/majordomus-cli/tests/brief.rs` | every surface over a real server, and the command line beside it and without it |
| `share/mods/claude-code/majordomus-brief/` | the Claude Code mod and its own test (`claude plugin test`) |
| `test/cases/1065_a_session_briefs_itself.sh` | the install, the switch and the command line's shapes |

</div>

{% endraw %}
