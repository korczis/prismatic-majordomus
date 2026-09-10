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
```

Everything above happens without a command being typed, except the last, which is a
request the bootstrap makes of the worker and the one a bridge repeats on its behalf when
its server changes underneath it.

## The three commands

```text
majordomus serve status [--format json]     where this checkout's server stands, and every
                                            server of this repository
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
majordomus env status                    the checkout: project, version control, toolchains,
                                         the layer, the workflows, the local services
majordomus context                       what the next worker needs to know now
majordomus doctor                        whether Majordomus itself is healthy and wired here
```

## Related

`docs/MCP.md` has the shared server's lifecycle and the election in full; `docs/ENVIRONMENT.md`
the snapshot and the adapter; `docs/CONTINUITY.md` the episode, the briefing and the
records; `docs/WORKTREES.md` the branch-to-worktree topology; `docs/ENTRY_AUDIT.md` what was
measured before any of this was built, and what is still owed.
