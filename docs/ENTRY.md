# Entering the repository

What happens when a person or an agent enters a Majordomus-enabled repository, what
converges on its own, what is deliberately left to be asked for, and what to do when
something does not come up. `docs/ENTRY_AUDIT.md` is the forensic finding this document
came out of; ADR 0035 is the decision; this is the operator's page.

## The sequence

```text
cd <repository>          direnv evaluates .envrc, which asks the tool for one snapshot:
                         the variables a shell here benefits from on stdout, the banner on
                         stderr. It reads nothing about the repository itself, builds
                         nothing, starts nothing, and reaches no network beyond one
                         connection attempt to an address a running server already published.

an agent starts          the provider's start event opens the episode, makes sure the
                         repository's shared server is serving this checkout, and writes a
                         briefing the provider adds to the context it is about to build:
                         the episode, the task, what blocks acceptance, the last handover's
                         next action, and one line naming where the server stands.

an MCP client attaches   the client configuration at the root starts the launcher, which
                         builds the executable when it must and elects: the first process
                         serves, every later one bridges to it. This has always converged.

the worker announces     `majordomus_announce` puts one line of intent and the paths it
                         expects to touch on the board every other client can read.

before each mutation     the provider's pre-tool event asks again whether this episode may
                         mutate the repository: is the shared server serving this checkout,
                         and is an episode open? A tool call it refuses is blocked and the
                         reason is handed back to the model. It is off unless a key of the
                         policy turns it on; the section below says what it does and how.
```

Everything above happens without a command being typed, except the announcement, which is a
request the bootstrap makes of the worker and the one a bridge repeats on its behalf when
its server changes underneath it.

## The three commands

```text
majordomus serve status [--checkouts this|repository] [--format json]
                                            where this checkout's server stands, and — unless
                                            `--checkouts this` narrows it — every server of
                                            this repository
majordomus serve ensure [--idle S] [--wait S] [--port P]
                                            a ready server for this checkout, started if
                                            there must be one
majordomus serve stop [--wait S]            end the server this checkout's lease names
```

`serve ensure` is what the start event runs. It reads the lease and probes the server it
names, exactly as the election does, and converges: a ready server is reported and nothing
is started; one that is still binding is waited for; an absent or stale lease gets a server
started as a process of its own, and the call waits until it answers. Run twice, it starts
nothing the second time. Run by three shells at once, the election lets one bind and the
others attach. The whole call is bounded by `--wait`, and a server that did not become
ready in time is reported with the standing it reached and a non-zero exit.

A server `ensure` starts has no client of its own, so it ends when no peer has been
attached for `--idle` seconds. That is how ADR 0003's line — there is no process without a
client — is kept true in time rather than at every instant: an agent's entry is owed a
server before its first attach, and a checkout nobody works in does not keep one.

## What the standings mean

| standing | what it says | what to do |
|---|---|---|
| `absent` | no lease: nothing serves this checkout | nothing, unless you want one: `serve ensure` |
| `starting` | a lease without an address, young enough that its owner is still binding | wait; `serve ensure` waits for you |
| `ready` | the server the lease names answers for this checkout, from the file on disk, at this executable's version | nothing |
| `outdated` | it answers, but from another version or from a file replaced since it started | `serve stop`, then `serve ensure` |
| `stale` | the lease names a server that does not answer, or is not a lease at all | nothing: the next start takes it over and says which it was |

## What is not automatic, and why

**A shell entering the repository is told, not served.** The file a shell evaluates on entry
may not start anything: it runs on every `cd`, on every machine, in whatever shell somebody
happens to use, and a build or a server started there is started at the worst possible
moment. The rule is `project.envrc-is-an-adapter`. An agent's entry is different — a client
is arriving, and ADR 0003's "no process without a client" is satisfied — so the start event
converges and the shell reports.

**Nothing on entry builds the executable.** A missing one, or one older than its sources, is
one line on standard error naming the recipe that builds it, and the entry still exits
zero. Everything a person sees here — the banner, the workflow bridge, the completion, the
shared server — is a projection of that one file, so they go together and the line says so.
A server started from stale code would answer with a tree that is no longer there, which is
why the start event names it rather than starting one.

**The switch.** `session.ensure_server_on_start` in the policy turns the start event's half
off; the briefing then says so instead of naming a server. The rest of the episode is
unchanged.

## Asking again: the guard

Entry asserts readiness once. An episode is hours long, and in those hours a server is
killed by somebody reclaiming a port, dies with the terminal that started it, ends because
no peer attached for its idle life, or goes `outdated` the moment its executable is rebuilt
under it. None of that is visible to the worker: it keeps calling Edit and Write, and the
one line that would have told it otherwise was printed at the start of the episode.

The provider's pre-tool event asks again, before each mutation. It is the only one of the
four provider events that may answer no.

```text
session.guard_before_mutation: false      the switch, in .ai/repo/policy.yaml
.claude/hooks/majordomus-session-guard    the shim the provider runs
majordomus capture guard --provider <p>   what the shim runs, payload on stdin
```

**It is off.** `session.guard_before_mutation` is `false` in this repository's policy and in
the skeleton a new repository is written from, and a policy written before the key existed
reads as false too. That is not a temporary state on the way to something: the hook is
tracked, so it reaches every worktree of the repository at once, and a wrong determination
in it blocks the sessions that would fix it. It is armed per repository, by somebody who has
watched it run. To turn it on, set the key to `true` in `.ai/repo/policy.yaml` — the next
tool call decides again, because the remembered answer is invalidated by the policy file
being newer than it.

**What it decides, in order.** A tool that does not mutate the repository is let through. A
remembered answer that is still good is honoured. Otherwise `serve status --checkouts this`
says where the server stands and the episode store says whether an episode is open for this
provider session; a server that is not ready is given exactly one chance through the same
`serve ensure` the start event runs. Only then is the tool call refused.

**What a refusal looks like to the agent.** The tool does not run. The provider hands the
model what the guard wrote on standard error, which names the standing, the remedy and the
switch:

```text
capture guard: refusing to mutate this repository: the shared server for this checkout is
'stale' and `serve ensure` did not fix it.
  Nothing that reads this repository through the server — the peer board, the index, the
  Cockpit — is answering, so a mutation made now is made blind.
  Run: majordomus serve status --checkouts this   then: majordomus serve stop && majordomus serve ensure
  The switch is session.guard_before_mutation in .ai/repo/policy.yaml.
```

**What it never refuses.** An executable that is not built or is older than its sources, a
payload that does not parse, a policy that does not load, a probe that does not answer:
each is reported on standard error, where the provider records it against the tool call,
and each lets the tool run. Standard error is fed back to the model on a refusal and
recorded in the transcript otherwise, so an undecidable state is visible without
interrupting anyone.
Refusing on a state the guard cannot decide would mean that a checkout nobody has built is
a checkout nobody can edit, which is a worse failure than the one it prevents — and it is
the same choice entry already makes when it names a missing executable rather than building
one.

**What it costs.** The guard runs in front of every Edit, Write and Bash, so the answer is
remembered in the checkout's local state and read without resolving the layout, parsing the
policy or starting a process. Measured on this repository on 2026-09-11, thirty runs each:
81 ms for the remembered answer, against 50 ms for `majordomus version` — the floor of the
shell tool — and 1.6 s for the reading it stands in front of. A `pass` is worth 60 seconds; an `off`
does not age, because the switch is not a fact about a running server. A refusal is never
remembered: between two edits a server can be started.

## When it does not converge

| what you see | what it means | what to run |
|---|---|---|
| the banner is gone, `just` lists nothing, the completion is silent | the executable is missing or older than its sources; every surface is a projection of it | `just build` |
| `Shared server: not ensured: the executable is not built` in the briefing | the same, seen from the provider's start event | `just build`, then start a new session |
| no address in the environment | no server has published one for this checkout | `majordomus serve status`, then `serve ensure` |
| the server answers, but with things the tree no longer has | the process is older than the code | `majordomus serve status` says `outdated`; `serve stop` then `serve ensure` |
| a linked worktree reports no server while the primary checkout has one | a server serves the checkout it started in; each has its own lease | `serve ensure` in that worktree; `serve status` lists both |
| `direnv: error .envrc is blocked` | direnv approves by path and content, and a new worktree starts unapproved | `majordomus worktree ensure <branch>` carries the primary's approval over |
| the peers board is empty although others are working | the board is one server's memory, and each checkout has its own server | `majordomus serve status` names every server of the repository and what each holds |

## Where each fact lives

Nothing above is written down twice. The lease under the checkout's local state is the one
place a running server's address is true, and one typed reader parses it. The port the
server asks for first is declared once in the command line's own declaration. The routes
belong to the surfaces that own them, and the environment snapshot renders them. What each
command does is the command graph's, and this page names commands rather than restating
them.

```text
majordomus serve status --format json    the server, its lease and every checkout's
majordomus serve status --checkouts this this checkout's server alone, without reading or
                                         probing any other checkout
majordomus env status                    the checkout: project, version control, toolchains,
                                         the layer, the workflows, the local services
majordomus context                       what the next worker needs to know now
majordomus doctor                        whether Majordomus itself is healthy and wired here
```

## What holds it shut

`project.entry-converges` is the rule, and `scripts/ci/entry-converges` the gate that
decides it: entry by a shell starts nothing and builds nothing, the switch is declared in
the policy, the skeleton, the allow list and the schema together, the start shim is wired
where the policy says so that `doctor` reconciles it, the briefing forms no opinion of its
own about where the server stands, and a server nobody owns has a bounded life. That the
lease itself has one reader is `project.the-lease-is-read-once`, decided for every file at
once by `scripts/ci/lease-reader-check`. What no script can
decide — that entry actually converges — is `test/cases/108_entry_converges_on_a_server.sh`,
which drives the provider's own shim; `test/cases/118_entry_converges_by_rule.sh` proves the
gate by planting each thing the rule forbids and watching it refused.

`project.the-guard-refuses-an-unready-episode` is the rule for asking again, and
`scripts/ci/guard-refuses-unready` the gate: the pre-tool event is declared in the provider
table with its shim, its matcher and the payload keys naming the tool; the wired
configuration declares that event with a matcher covering the mutating tools and names the
shim; the shim delegates to `capture guard` and passes on the refusal alone rather than
handing the process over; `session.guard_before_mutation` is declared in the policy, the
skeleton, the allow list and the schema together; and the decision reads `serve status`,
asks `serve ensure` once, remembers its answer and names what it cannot decide.
`test/cases/124_the_guard_refuses_an_unready_episode.sh` proves both directions — a ready
episode passes, an unopened one is refused, a killed server is healed rather than refused,
a malformed payload costs nobody a tool call, and with the switch off nothing is refused at
all — and plants each thing the rule forbids in a fixture to watch the gate refuse it.

## Related

`docs/MCP.md` has the shared server's lifecycle and the election in full; `docs/ENVIRONMENT.md`
the snapshot and the adapter; `docs/CONTINUITY.md` the episode, the briefing and the
records; `docs/WORKTREES.md` the branch-to-worktree topology; `docs/ENTRY_AUDIT.md` what was
measured before any of this was built, and what is still owed.
