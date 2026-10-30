# The session brief — `majordomus brief`

What surrounds one working session, in one line and in a few, so that a session can report on
itself without being asked. Behaviour as implemented and tested; where implementation and this
document disagree, the document is wrong and changes in the same commit as the fix. The decision
and what it rejected are
[ADR 0136](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md).

## The problem this solves

Every fact a working session needs about its surroundings already has an owner: the episode
(`lifecycle.episodes`) and the task (the checkout's task record), the session's own claims
(the mesh journal), its pull requests, runs and the CI pool (the forge, through the monitor's
shared cache), the workers of every checkout (`peers.list`), the machines (`mesh.cooperation`,
`mesh.state`), the pull-request queue (`majordomus prs brief`), the machine itself (free disk,
swap, builders), the release (`release.currency`) and the machine's derive lock
(`lib/machine_lock.sh`). A session reads none of them unless it remembers to. On 2026-10-10 the
sessions of this repository relayed board state to each other by hand; on the night of
2026-10-28 every blocker a session had to report — a master run failing, a train timed out, a
pull request in conflict, swap at 91 % — was in no brief at all. A session that is not told
goes quiet, and a quiet session cannot be told from a dead one.

## One capability, four surfaces

`brief.session` asks each owner once, under a budget per section, and says per section what it
observed, from where, when, how old that is against the age its owner calls stale, a verdict
word, the command that refreshes it, and what asking cost. It stores nothing, judges nothing of
its own, writes nothing and builds nothing.

| Surface | How it is asked |
|---|---|
| Command line | `majordomus brief` (the lines), `--line` (the one line), `--keep-awake` (the start text, then the line), `--report` (the report owed now), `--since <rfc3339>`, `--format json` |
| HTTP | `GET /api/v1/brief?session=<id>&checkouts=this&since=<rfc3339>` |
| MCP | the tool `majordomus_brief` |
| Cockpit | the "Session brief" card on the overview: the attention rows, then every section with its verdict, age and remedy |

The input is optional throughout. `session` is the provider's own session identity (Claude
Code's session id); the command line takes it, when none is given, from the environment in the
order the shell lifecycle resolves it (`MAJORDOMUS_PROVIDER_SESSION`, then
`CLAUDE_CODE_SESSION_ID`). `since` is the previous report's instant, and the work section then
says what was delivered since. `background` is the provider's own background tasks, passed only
when its engine exposes them. `checkouts` is `repository` (every board, the default) or
`this`; `local` is what a forwarded request says.

### The sections

Reported in this order, in the lines, in the JSON and in the report, and never left out. The
command each section's line names when it is not current, and the live read the report is
given for it, are the declaration's: [the reader page](generated/brief-report.md) lists them, and
a section past its budget names the command that reads that section alone, never the brief.

| Section | Source | Bound | Budget | Says |
|---|---|---|---|---|
| `attention` | derived from the sections below | — | — | every condition the sections measured, as `[word] section: fact — remedy (actor)`, a person's first, then worst first |
| `session` | `lifecycle.episodes`, the checkout's refs and `current.yaml` (files) | read now | 200 ms | the caller (its board peer and provider session), the episode the provider session owns, else the checkout's pointer said as such, the branch and head, and the task only while its outcome is `active` |
| `work` | the mesh journal (memory), refs and canonical worktrees (files), the forge via the monitor cache | 2 min, refreshed in the background past 30 s | 5 s | this session's claims, its branches with their canonical worktrees and tasks, pull requests with mergeStateStatus, trains, the latest run with jobs done/total and failed jobs by name, builds under its client, and what it delivered since the previous report |
| `peers` | `peers.list` (every board's server), asked once at a time per process | read now | 4 s | workers attached, the checkouts asked and the boards read, unread boards, overlaps with this session's claims and with whom, overlaps in all, and up to eight workers with their branch and their whole intent, then `+N more: majordomus peers` |
| `mesh` | the mesh journal (memory) | read now | 200 ms | links connected and failing, claims held and this runtime's share, open reviews naming it, handovers nobody consumed, overlaps with another runtime's claim |
| `integration` | `.ai/local/state/integration/observation.json` and the queue's `summary.json` (files; a linked worktree reads the primary checkout's, said so), `origin/master`, and the `FETCH_HEAD` files naming master with `origin/master`'s reflog for the fetch age | 1 h | 1 s | the queue's observation, its age and the master it saw against `origin/master` now (and by how many commits it moved), the required run on master, the last merge on master's first-parent line and the executor's own last merge |
| `ci` | the forge via the monitor cache, leaseholder only | 2 min, refreshed in the background past 30 s (runners past 60 s) | 5 s | hosted runs in progress and queued, self-hosted runners online, busy and offline by name |
| `machine` | statvfs, `vm.swapusage` or `/proc/meminfo`, one `ps` | read now | 1 s | free disk against `machine.heavy_work.min_free_gib`, swap, builders against `machine.heavy_work.slots` |
| `release` | `release.currency`, asked once at a time per process and reused while `origin/master` and the tags are unchanged (5 min at most) | read now | 3 s | current, owed with the count, overdue, or unread with why (I3070) |
| `derive` | the derive lock (files) | read now | 100 ms | its holder, whether that process runs, how long it has held the lock, the priority marker |
| `answered by` | — | — | 6 s overall | which process composed the brief, how long it took, and every section that ran past its budget, as `unread past its budget` when its owner had not answered when it was collected or `read past its budget: <section> <took> of <budget>` when it had |

Each section carries a state (`read`, `partial`, `unread`, `undeclared`, kept for older
readers) and an observation whose `verdict` is one status word of the design declaration:
`current`, `stale` (its owner answered with a fact older than its bound, or contradicted by a
newer one), `partial`, `unread` or `undeclared`. A stale or unread fact is never shown as
current: the line says `prs stale 17h`, never a bare count, and a section past its budget reads
`[unread] took > 3.0 s` while every other section is read. Every section's line leads with its
status word in brackets and, when that word is not `current`, ends with its remedy
(`[unread] not the leaseholder ... — majordomus serve ensure`), kept whole on one line so that
it can be copied as shown. A fact that was not read is said with the status word, `[unread]`,
and never a question mark: `origin/master is 51b38035a7 (moved, count [unread])` when the
commit count did not answer inside its bound, `session unread` and `disk unread` in the one
line, and a peer that names no branch is `on no branch it named`. A list that was not read is
said so with why (`delivered [unread]: the caller named no previous report (--since)`), never
left empty. The forge is asked by the leaseholder alone; a brief composed anywhere else says
`[unread] not the leaseholder` and names `majordomus serve ensure`, and the integration line
then says `master run [unread] (not the leaseholder)` with the `gh run list` that reads it.

The session's task is the running task of this provider session: this checkout's record when
it names this session or none, else the record in the canonical worktree of one of the
session's own branches, and the line names that worktree. A record another provider session
wrote is said to be that session's, and raises no `session.no_task`. Instants are UTC; the
brief also carries `at_local`, the composing host's clock with its offset and zone, which the
report's header names. Free disk and swap are GiB with one decimal, cut and never rounded up,
in every place they appear.

A process the mesh is not active in (a command line in a checkout whose server is absent)
holds no journal of this session's claims. It reads the board instead: the work section waits,
inside the peers section's budget, for this session's entries on the boards the peers section
read, and lists their announcements as claims in the state `announced`, their `refs/heads/`
scopes as branches, and the processes under the client's pid when that client runs on this
machine. Its source then says `the board's announcements (this process holds no mesh)`.

### The one line

At most 160 columns: the caller and the session's task or episode and branch, the worst
attention entry led by its section (`release owed 1 landed commit(s) past v0.1.0`), then one
token per section in a fixed order (the CI pool, the queue with its age, the machine, the mesh,
the peers, the release, the derive lock). Tokens that do not fit are dropped from the right and
counted as `+N`, so the worst entry is said even when its section's own token is dropped; the
one line is the only form in which anything is ever shortened.

## Who answers

The board and the mesh live in the memory of the server that holds the checkout's lease. A
command-line process has an empty board of its own and no mesh, so a brief composed there
would say "nobody is here" when the truth is "I did not ask". The command line therefore asks
the leaseholder over the HTTP route the registry declares, with `local=true` so that the server
never asks back, and composes the brief itself only when no server answers. It waits 10 s for
that answer: a leaseholder of this generation answers inside its 6 s deadline, and only an older
one, whose brief has no budgets, takes longer. The answer says
which process composed it (`answered_by`: `leaseholder`, `superseded` or `this_process`), why
when it is not the leaseholder, and in how long. An older leaseholder's answer, which carries
no report, is finished by the command line: its attention and report are composed here from its
sections.

A server that lost the lease to a newer one still holds its own MCP sessions, and the
repository's gather cannot see them, because `peers.list` reads a checkout's board from the
server its lease names. Such a server answers an MCP session's brief from its own board, adds
the leaseholder's, marks the peers section `partial` and the line `superseded`, and says that
the peers attached to it are unreachable by everyone else.

## The report

What a session writes about itself on the interval is declared once, in
[`share/brief/report.yaml`](../share/brief/report.yaml) (schema
`majordomus/brief-report/v1`): its sections in order, the rules every line obeys, the
attention rows and the live reads. `brief.session` composes three texts from it with one
function, each naming the contract and its fingerprint and carrying the measured attention
entries verbatim: `keep_awake` (what a session is told at its start), `report.prompt` (what
wakes it on the interval) and `report.reminder` (what rides on a prompt the person sends).
Every carrier relays those texts verbatim; none of them, and not this document, restates the
contract. `majordomus brief --report` prints the report owed now, and
[the reader page](generated/brief-report.md) renders the declaration, attention rows included.

## Keeping a session awake

`session.brief_on_interval` is the switch and `session.brief_interval_minutes` the interval
(20 when absent). The brief carries the interval, so every reader takes it from one place. The
interval is a ceiling, not a schedule: a session is told at most once per interval, and only
when a fact in the brief changed since it was last told (ADR 0136, amended 2026-10-27 and
2026-10-29).

- **At the start of every episode**, whatever the provider, the start event's briefing ends
  with a "Keeping awake" section, which is `majordomus brief --keep-awake` relayed verbatim
  (`lib/capture.sh`, the only carrier frame).
- **In Claude Code**, `capture install` writes the provider mod
  `share/mods/claude-code/majordomus-brief` to `.claude/skills/majordomus-brief`, beside the
  handover mod of ADR 0123 and under the same ownership rules. The mod shows the one-line brief
  as the status line and refreshes it every minute. When the interval has passed and a section
  fingerprint changed, it submits `report.prompt` once the session is idle; a prompt the person
  sends under the same two conditions carries `report.reminder`. The mod never builds the
  executable (`MAJORDOMUS_NO_BUILD=1`), and outside a supervised repository it does nothing.

Neither writes `.claude/settings.json`. The start event's section rides on the SessionStart
shim `capture install` already wires, and a Claude Code session loads a mod from
`.claude/skills/<name>` by itself.

### What counts as a change

The brief carries, per section, a fingerprint of the facts that concern this session, with
every instant, age and cost taken out (`fingerprints` in the JSON), together with the attention
entries about it. Two briefs differ in a section exactly when its fingerprint does, so the mod
compares fingerprints and reads no prose. `queue observed 17 h ago` and `18 h ago` an hour later
are one fact; master moving past the observation is a new one. The peers section is
fingerprinted by who is attached on which branch, the boards not read and the overlaps with
this session's claims, so another worker re-announcing or one more worktree is no change; the
CI pool by its state and its runners online and offline; the machine by its word. A section
read past its budget carries `late` and is never said to have changed, and the over-budget
entry belongs to no section's fingerprint. A threshold the clock crosses still counts when it
changes a verdict, as when a release that has waited too long becomes `overdue`.

The mod passes the fingerprints it was last told (`--previous-json`), and the interval prompt
then opens with `Changed since <instant>: ...` and lists the report sections none of whose
sources changed; the report writes those as their heading and `[unchanged]`. `--since` is the
instant of the last report, which a reminder never moves.

Claude Code adds a framing sentence of its own after any prompt a plugin submits ("This is how
Claude Code surfaces a prompt a plugin submits between turns ..."). That sentence is Claude
Code's, not the mod's, and no mod can change or remove it.

## What it is not

The brief is one composed answer, asked when wanted. It is not a stream and has no follow
mode: following events as they happen is the monitor's (ADR 0133). It reads the forge only
through the monitor's shared, bounded cache, only in the leaseholder, and never once per
session. It never runs `majordomus prs refresh`, which fetches every pull request's head and
writes four files: it names the command. Its only children are `git`, `gh` and `ps`; it never
runs this executable or a compiler (ADR 0224), and it starts no server.

## Where it lives

| Path | What |
|---|---|
| `share/brief/report.yaml` | the report contract, the one declaration of the self-report |
| `share/schemas/majordomus/brief-report/brief-report.v1.schema.json` | its schema |
| `apps/majordomus-cli/src/capability/builtin/brief.rs` | the capability, its sections, their budgets and their composition |
| `apps/majordomus-cli/src/capability/builtin/brief/report.rs` | the contract's compiled copy, the attention conditions and the one render |
| `apps/majordomus-cli/src/capability/builtin/brief/report.yaml` | the compiled copy, written by `majordomus generate` |
| `apps/majordomus-cli/src/capability/builtin/brief/work.rs` | this session's own work |
| `apps/majordomus-cli/src/capability/builtin/brief/ci.rs` | the CI pool |
| `apps/majordomus-cli/src/capability/builtin/brief/machine.rs` | the machine |
| `apps/majordomus-cli/src/capability/builtin/brief/refs.rs` | git refs read from files |
| `apps/majordomus-cli/src/commands/brief.rs` | the command line's rendering |
| `apps/majordomus-cli/tests/brief.rs` | every surface over a real server, and the command line beside it and without it |
| `share/mods/claude-code/majordomus-brief/` | the Claude Code mod and its own test (`claude plugin test`) |
| `test/cases/1065_a_session_briefs_itself.sh` | the install, the switch and the command line's shapes |
| `test/cases/4250_the_brief_reports_only_what_changed.sh` | the mod's own test, and mutations of its comparison and its relay that must fail it, where `claude` is installed |
| `test/cases/4401_the_report_contract_is_declared_once.sh` | no second copy of the contract anywhere in the tree |
| `test/cases/4402_brief_words_are_status_words.sh` | every verdict and attention word is filed once in the design declaration |
| `test/cases/4403_a_stale_source_is_never_shown_as_current.sh` | an old or contradicted observation reads stale with its remedy |
| `test/cases/4404_a_brief_never_truncates_without_a_count.sh` | whole intents in the lines, a counted line |
| `test/cases/4405_a_brief_is_bounded_and_never_builds.sh` | a slow forge reads unread, and no compiler is ever started |
