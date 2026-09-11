+++
title = "Entering the repository"
description = "entering the repository: what converges on its own and what is asked for, the three commands of the shared server, what each standing means, what is deliberately not automatic and why, and what to run when it does not come up"
weight = 27
[extra]
source = "docs/ENTRY.md"
+++

{% raw %}

What happens when a person or an agent enters a Majordomus-enabled repository, what
converges on its own, what is deliberately left to be asked for, and what to do when
something does not come up. `docs/ENTRY_AUDIT.md` is the forensic finding this document
came out of; ADR 0035 decided the agent's door and ADR 0043 the shell's; this is the
operator's page.

The short version: **nothing here is a command anybody has to remember.** Entering the
directory is enough, by either door.

## The sequence

```text
cd <repository>          direnv evaluates .envrc, which makes one call — `majordomus env
                         enter` — and evaluates what it prints: the variables a shell here
                         benefits from on stdout, the banner on stderr, the workflow bridge
                         refreshed when a declaration behind it moved, and the repository's
                         shared server ensured when nothing is serving this checkout. The
                         file itself reads nothing about the repository, builds nothing and
                         starts nothing; the executable does all of it. Nothing is waited
                         for: a server this entry starts becomes ready beside the shell, and
                         `watch_file` over the lease brings its address in when it lands.

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
```

Everything above happens without a command being typed, except the last, which is a
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

`serve ensure` is what the start event runs, and `majordomus env enter` runs the same
function — one convergence, three surfaces, so that an agent and a person cannot reach two
different answers about whether a server is already there. It reads the lease and probes the
server it names, exactly as the election does, and converges: a ready server is reported and nothing
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

<div class="overflow-x-auto" tabindex="0">

| standing | what it says | what to do |
|---|---|---|
| `absent` | no lease: nothing serves this checkout | nothing, unless you want one: `serve ensure` |
| `starting` | a lease without an address, young enough that its owner is still binding | wait; `serve ensure` waits for you |
| `ready` | the server the lease names answers for this checkout, from the file on disk, at this executable's version | nothing |
| `outdated` | it answers, but from another version or from a file replaced since it started | `serve stop`, then `serve ensure` |
| `stale` | the lease names a server that does not answer, or is not a lease at all | nothing: the next start takes it over and says which it was |

</div>


## Entering as a shell

```text
majordomus env enter [--shell SHELL] [--no-banner] [--no-bridge] [--no-runtime] [--wait S]
```

What `.envrc` calls, and the only call it makes. The file carries a `PATH_add`, a
`watch_file` and that line; everything else — whether to ensure, on what port, with what
idle life, when to stay silent, when to refuse — belongs to the executable, where it is
typed, benchmarked and tested.

Three things about it are contract rather than implementation:

- **It never waits.** A cold entry finds that nothing answers, starts a server as a process
  of its own, and returns. Measured on a machine under a load average of 136, a cold entry
  cost 185–217 ms and a warm one 134–218 ms: the same, because starting the runtime is a
  fork and an exec and nothing else. The budgets are `benchmark.budget.enter_ms` and
  `benchmark.budget.enter_cold_ms` in the policy, and `test/cases/190` measures both on
  every run.
- **It never fails.** Every runtime outcome is at most one line on standard error and an
  exit of 0. A non-zero exit here makes direnv report that the whole environment failed and
  leaves a person with a broken shell in a repository that is fine.
- **It is silent when there is nothing to say.** Entering a healthy repository prints
  nothing about the runtime, changes no file under `.ai/local/`, and starts no process. A
  person who enters this directory forty times a day is told nothing forty times.

**The switches.** `session.ensure_server_on_start` in the policy governs both doors — one
question, one answer, so the two cannot drift. `MAJORDOMUS_RUNTIME=off` is the same refusal
for one person's shell, and only that exact value means it: a typo in a profile must not
silently stop a repository coming up. `--no-runtime` is the same thing as an argument.

**Until ADR 0043 this was the other way round**, and the rule read "entry by a shell reports
and does not serve". What that cost was the whole point of the design: a person who opened a
terminal here got a banner and no runtime — no Cockpit, no API, no peer board, so no way to
see that other workers were on the same machine — and the remedy was a command to remember.
The objection ADR 0003 makes is about a process outliving its reason to exist, and the idle
life answers it: a server nobody attaches to ends.

## What is not automatic, and why

**Nothing on entry builds the executable, and nothing ensures from a stale one.** A missing
executable, or one older than its sources, is one line on standard error naming the recipe
that builds it, and the entry still exits zero — with no runtime. Everything a person sees
here — the banner, the workflow bridge, the completion, the shared server — is a projection
of that one file, so they go together and the line says so. A server started from stale code
would answer with a tree that is no longer there, to the Cockpit, to the API and to every
other worker attached to it, none of whom can see that the process is older than the
checkout it claims to serve. Both doors decide staleness with the same `mj_rust_stale` in
`lib/rust_bin.sh`, so a checkout where they disagree cannot exist.

**No remote network, ever.** The only socket entry opens is to a loopback address a server
of this checkout already published.

**The briefing is still the agent's.** A shell is handed an environment and a banner; the
episode, the task and the last handover's next action are what a provider's start event
composes, because there is no episode without a worker to have one.

## When it does not converge

<div class="overflow-x-auto" tabindex="0">

| what you see | what it means | what to run |
|---|---|---|
| the banner is gone, `just` lists nothing, the completion is silent | the executable is missing or older than its sources; every surface is a projection of it | `just build` |
| `Shared server: not ensured: the executable is not built` in the briefing | the same, seen from the provider's start event | `just build`, then start a new session |
| no address in the environment | no server has published one for this checkout | `majordomus serve status`, then `serve ensure` |
| the server answers, but with things the tree no longer has | the process is older than the code | `majordomus serve status` says `outdated`; `serve stop` then `serve ensure` |
| a linked worktree reports no server while the primary checkout has one | a server serves the checkout it started in; each has its own lease | `serve ensure` in that worktree; `serve status` lists both |
| `direnv: error .envrc is blocked` | direnv approves by path and content, and a new worktree starts unapproved | `majordomus worktree ensure <branch>` carries the primary's approval over |
| `majordomus: nothing was serving this checkout; a server is starting` | the expected line on a first entry; the address arrives a moment later, when `watch_file` sees the lease | nothing |
| that same line on every `cd` | the server is started and then fails | read the log the line names, then `majordomus serve status` |
| `majordomus: the executable is older than the sources`, and no server comes up | entry refuses to start a runtime from stale code | `just build`; the next `cd` ensures one |
| the peers board is empty although others are working | the board is one server's memory, and each checkout has its own server | `majordomus serve status` names every server of the repository and what each holds |

</div>


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
decides it: the entry file starts nothing itself, makes exactly one call, and that call is
the bootstrap command with no `--wait`; the shell's door reads the same switch, honours
`MAJORDOMUS_RUNTIME` and converges through the same function `serve ensure` does; the
adapter refuses to ensure from a stale executable; neither door builds; the switch and the
two entry budgets are each declared in the policy, the skeleton, the allow list and the
schema together; the start shim is wired where the policy says so that `doctor` reconciles
it; the briefing forms no opinion of its own about where the server stands; and a server
nobody owns has a bounded life. That the
lease itself has one reader is `project.the-lease-is-read-once`, decided for every file at
once by `scripts/ci/lease-reader-check`. What no script can
decide — that entry actually converges — is `test/cases/108_entry_converges_on_a_server.sh`
for the agent's door, which drives the provider's own shim, and
`test/cases/190_entry_is_automatic.sh` for the shell's;
`test/cases/191_entry_races.sh` and `test/cases/192_entry_across_checkouts_and_clients.sh`
hold the convergence under real concurrency — ten simultaneous entries, a dead pid in the
lease, a live process that is not ours, an abandoned lease, a port a stranger holds, two
worktrees entering at once, and ten clients on a runtime an entry started.
`test/cases/118_entry_converges_by_rule.sh` proves the
gate by planting each thing the rule forbids and watching it refused.

## Related

`docs/MCP.md` has the shared server's lifecycle and the election in full; `docs/ENVIRONMENT.md`
the snapshot and the adapter; `docs/CONTINUITY.md` the episode, the briefing and the
records; `docs/WORKTREES.md` the branch-to-worktree topology; `docs/ENTRY_AUDIT.md` what was
measured before any of this was built, and what is still owed.
{% endraw %}
