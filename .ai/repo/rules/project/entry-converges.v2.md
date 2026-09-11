---
id: project.entry-converges
version: 2
kind: rule
title: Entering the repository converges on a server, by either door, without anything to remember
description: An agent entering the repository is owed a ready shared server before its first attach and gets one through the provider's start event; a shell entering the repository is owed the same and gets it through one bootstrap command of the executable, which the entry file calls and nothing else; neither entry path builds the executable, ensures from a stale one, waits for readiness or fails; what a worker is told on entry is derived from the same typed reading every other surface uses; entry has a declared time budget; and a server no client owns ends by itself.
statement: Both entry paths converge on one ready shared server through one implementation, neither builds nor ensures from a stale executable nor waits for readiness nor exits non-zero, the entry file starts nothing itself and makes one call, entry stays inside a declared budget, the briefing has one author, and a server nobody owns has a bounded life.
status: active
class: blocking
depends_on: [project.envrc-is-an-adapter@2, project.shared-server-resilience@1, project.interfaces-are-projections@1, project.the-lease-is-read-once@1, project.every-wait-is-bounded@1]
tags: [entry, mcp, environment, enforcement, performance]
---

# What changed in version 2, and why

Version 1 said, in as many words, that **entry by a shell reports and does not serve**, and
the gate refused a `serve` or `mcp` subcommand anywhere in the entry path. ADR 0043
supersedes that clause. The reasoning it replaces ran two things together:

- *ADR 0003 refuses a process without a client, and a shell is not a client.* The objection
  ADR 0003 makes is about a server outliving the reason it exists, and it was already
  answered in time rather than at every instant: a server `ensure` starts is given
  `DEFAULT_IDLE_SECONDS` and ends when no peer has attached for that long. A shell that
  enters a repository, reads its environment and leaves is exactly as much of a client as
  an agent whose session a provider opened and closed. Both are covered by that idle life.
- *A `.envrc` must not grow logic.* True, and unchanged: `project.envrc-is-an-adapter@2`
  keeps every word of its refusal and narrows the file to **one** call. Nothing about that
  changes when the one call it makes happens to start a process, because the deciding —
  whether, on what port, with what idle life, when to stay silent, when to refuse because
  the executable is stale — happens inside the executable.

What the closed half cost was the whole requirement. A person entering this repository in a
terminal got a banner, a `PATH` and a workflow bridge, and no runtime: no Cockpit, no API,
no Swagger, and no peer board — so no way for the other workers on the same machine to see
that they were not alone, which is the failure `project.work-is-claimed-before-it-is-built`
exists to prevent and which this repository has paid for more than once. The remedy was a
command to remember, which is the thing the design is against.

So version 2 keeps every refusal version 1 made about *building*, *waiting*, *failing* and
*second opinions*, moves the "does not serve" clause to "does not serve **itself**", and
adds what the new permission needs to stay honest: one implementation for both doors, one
switch, a refusal to ensure from a stale executable, and a declared time budget.

# Rationale

`docs/ENTRY_AUDIT.md`, measured on 2026-09-10, asked one question — when a person or an
agent enters this repository, what converges on its own — and found that the answer was
"the MCP launcher, and nothing else". A shell entering the repository was told nothing
about a server; a session whose MCP handshake failed was told nothing either; a provider
without a lifecycle adapter never had one. ADR 0035 and the slice that implemented it made
entry converge: the provider's start event calls `serve ensure`, the briefing names where
the server stands, `serve status`/`ensure`/`stop` exist, and the election survives a start
slower than its bind grace.

None of that was a rule, and every piece of it is one line away from being undone by
somebody reasonable. The switch can be added to the policy and never taught to the schema,
where an unknown key is refused at run time and the whole policy stops parsing. The shim
can be unwired from the provider, which nothing but the policy's enforcement list would
notice. A `serve ensure` can be added to `.envrc` by whoever finds
the bootstrap command too indirect — putting a decision back in a shell file that nothing
tests, which `project.envrc-is-an-adapter` was written against and which that rule's own
word list never covered: it forbids `git`, `grep`, `curl` and `cargo`, and says nothing
about the tool's own `serve`. A build can be added to either entry path by whoever is tired of
seeing "the executable is not built", which turns a `cd` into a two-minute compile and a
start event into a server built from a tree the worker has not read yet. A second opinion about where the
server stands can be formed in one `curl`, beside the one the start event was handed, and
will disagree with it the first moment the two are asked at different times.

This repository has a name for the shape of that failure: a report nobody must clear is a
report nobody clears. `docs/ENTRY.md` is prose; `test/cases/108` proves the behaviour once;
neither refuses the change that quietly removes it.

# Required behaviour

**Entry by an agent converges.** The provider's start event opens the episode and makes
sure the repository's shared server is serving this checkout, through the executable's own
`serve ensure` — never by starting a process itself, and never by asking a second time what
`ensure` already decides. The shim that carries the event is declared in the policy's
`enforcement` list, wired by `provider-hook:<provider>:session`, so that `majordomus doctor`
reconciles the wiring rather than a reader remembering to check it. The half is a switch,
`session.ensure_server_on_start`, and a key of the policy is declared in four places or in
none: this repository's policy, the skeleton a new repository is written from, the allow
list, and the schema.

**Entry by a shell converges too, and starts nothing itself.** The file a shell evaluates
on entry makes one call — the executable's bootstrap command, `majordomus env enter` — and
that call ensures the runtime the same way the start event does. The file itself, and the
adapter it calls, may not run the tool's `serve` or `mcp`, invoke a builder, or background
a process: that file runs on every `cd`, on every machine, in whatever shell somebody
happens to use, and a decision taken there is a decision nothing tests.
`project.envrc-is-an-adapter@2` says what the entry point may read and that it makes one
call; this says what that call must and must not do.

**Both doors run one implementation.** The provider's start event and the bootstrap command
converge through the same function, `commands::serve::converge`, which is also what
`serve ensure` renders. Three surfaces, one decision — the claim
`project.interfaces-are-projections` makes of every other fact in this repository — because
two implementations of "is a server already serving this checkout" are two answers waiting
to disagree, at the one moment where disagreeing means two authoritative servers.

**One switch, both doors.** `session.ensure_server_on_start` is this repository's answer to
"does entering here bring the runtime up", and it governs the shell as well as the start
event. A second key for the shell path is refused: a repository that answers one question
in two places is a repository where the two answers drift. `MAJORDOMUS_RUNTIME=off` is the
same refusal for one person's shell rather than for the repository, and only the exact
value `off` means it — a typo in somebody's profile must not silently stop a repository
from coming up.

**Entry never waits for readiness.** A cold entry reads the lease, finds that nothing
answers, starts a server as a process of its own, and returns. The server becomes ready
beside the shell rather than in front of it, and the entry file's `watch_file` over the
lease is what brings the published address into the environment a moment later. The
bootstrap command's `--wait` defaults to zero and the entry file does not pass it.
Everything the call does wait for is bounded (`project.every-wait-is-bounded`).

**Entry never fails.** Every runtime outcome is at most one line on standard error and an
exit of 0. A non-zero exit makes direnv report that the whole environment failed and leaves
a person with a broken shell in a repository that is fine.

**Entry has a declared budget.** `benchmark.budget.enter_ms` and
`benchmark.budget.enter_cold_ms` are policy keys like any other — declared in the policy,
the skeleton, the allow list and the schema — and a case measures both. Entering a
directory is something a person does dozens of times a day and never chose to wait for, and
a budget that lives only in somebody's memory of how fast it used to be is not a budget.

**Nothing on entry builds the executable, and nothing ensures from a stale one.** Both
entry paths resolve the executable without building it. A missing one, or one older than
its sources, is named on standard error with the recipe that builds it, and the entry still
exits zero — and no runtime is ensured from it. A server started from stale code answers
with a tree that is no longer there, to the Cockpit, to the API and to every other worker
attached to it, none of whom can see that the process is older than the checkout it claims
to serve. Both paths decide staleness with the same `mj_rust_stale` in `lib/rust_bin.sh`,
so a checkout where they disagree cannot exist.

**What a worker is told on entry has one author.** The briefing is handed where the server
stands as an argument by the event that ensured it, and forms no opinion of its own —
neither by opening the lease nor by asking the server for its standing. Two readings of one
state are two answers waiting to disagree, and this is the same claim
`project.interfaces-are-projections` makes of every other surface. That the lease itself has
one reader is `project.the-lease-is-read-once`, which decides it for every file in the
repository; this rule depends on it rather than restating it, and adds only the half that
rule cannot see, because asking the server over HTTP opens no file.

**A server nobody owns has a bounded life.** A server `ensure` starts is given an idle
life, declared once as a constant and greater than zero, and ends when no peer has been
attached for that long. That is how ADR 0003's line — there is no process without a client
— stays true in time rather than at every instant.

# Failure behaviour

`scripts/ci/entry-converges`, registered as the `entry-converges` gate in
`.ai/repo/ci/gates.yaml` and selected by every path class that can move either side (the
shell tool and the entry point, the layer, the distribution, the crate), reports one line
per check and exits 10 on any finding. It decides, mechanically:

- that `.envrc` and `bin/majordomus-env` run no builder, no background job, and no `serve`
  or `mcp` subcommand of the tool, matched by shape rather than by a list of today's lines;
- that `.envrc` makes exactly one call to the tool, that the call is the bootstrap command,
  and that it passes no `--wait`, so that entry cannot become a wait for readiness;
- that the shell entry path reads `session.ensure_server_on_start` and honours
  `MAJORDOMUS_RUNTIME`, and that the adapter turns the runtime off when the executable is
  stale rather than ensuring from it;
- that the bootstrap command converges through the same function `serve ensure` does, rather
  than through a second implementation;
- that `session.ensure_server_on_start`, `benchmark.budget.enter_ms` and
  `benchmark.budget.enter_cold_ms` are each declared in the policy, the skeleton, the allow
  list and the schema;
- that the start shim dispatches `capture session --event start`, that the policy's
  enforcement list declares a `provider-hook:<provider>:session` entry so `doctor`
  reconciles it, and that the start path runs `serve ensure`, reads the switch, consults
  the staleness check and runs no builder;
- that the briefing composer does not ask the server for its own standing (the lease half is
  `scripts/ci/lease-reader-check`'s, for the whole repository, and is not read twice here);
- that the idle life is one constant greater than zero and that the spawned server is given
  it.

No validator of this rule lives in `lib/`. Which file a shell evaluates on entry, which
provider fires the start event, and where this repository declares its policy, its skeleton
and its schema are facts about *this* repository, and a validator in `lib/` would teach the
shipped executable one repository's layout; `.ai/repo/rules/project/README.md` is where that
distinction is written down.

# Verification

`test/cases/118_entry_converges_by_rule.sh` proves the gate by mutation, against a fixture
tree rather than this checkout: a `serve ensure` added to the entry point, a second call to
the tool added to it, a `--wait` added to the bootstrap call, a builder added to either
entry path, a backgrounded server, a switch or a budget dropped from the schema, a shim
that stops dispatching the start event, a start path that stops ensuring or starts
building, a shell entry path that stops reading the switch, an adapter that ensures from a
stale executable, a briefing that asks the server for its own standing, an idle life of
zero and a spawn that drops `--idle` are each planted and each refused with the cause named;
the same fixture unmutated is accepted.
The case also asserts that the gate is declared in `.ai/repo/ci/gates.yaml` and selected by
the classes that can move it, because a gate no plan selects is prose with an exit code.

What the gate deliberately does not decide, and a reviewer does:

- **That entry actually converges.** No script can enter a repository as an agent, or as a
  shell. The agent's door is `test/cases/108_entry_converges_on_a_server.sh`, which drives
  the provider's shim with the payload the provider would send, in a repository `init`
  wrote: a first start ensures a ready server and names it in the briefing, a second finds
  the same one and starts nothing, `serve status` agrees, `serve stop` ends it, the switch
  turned off says so and starts nothing, and a missing executable is named while the event
  still exits zero. The shell's door is `test/cases/190_entry_is_automatic.sh`: a cold entry
  brings a runtime up and returns without waiting for it, a warm entry starts nothing,
  rewrites nothing and stays inside `benchmark.budget.enter_ms`, repeated entries on a
  healthy runtime change no file at all, the switch and `MAJORDOMUS_RUNTIME=off` each stop
  it, and a stale executable is named rather than served from. That both doors converge on
  *one* server under concurrency is `test/cases/191_entry_races.sh`, which runs real
  concurrent processes: ten simultaneous entries, a lease with a dead pid, an occupied port,
  a server that crashes during startup, and two worktrees at once. The gate runs case 108
  unless it is given `--no-suite`.
- **That the briefing says something useful.** The gate can see that the composer forms no
  second opinion; it cannot see whether the line it was handed is worth reading.
- **That a new entry path was declared here at all.** A provider whose lifecycle adapter
  lands without an enforcement entry, or an entry point this rule does not name, is a
  reviewer's finding: the gate reads the paths it knows and cannot notice a path nobody
  told it about. The related invariant — that a provider is data and not code — is
  `project.providers-are-data`.
- **That the lease has one reader.** `project.the-lease-is-read-once` and
  `scripts/ci/lease-reader-check` decide that, and `test/cases/110_lease_reader.sh` proves
  it; the three shell parsers ADR 0035 recorded as stage-09 debt were paid there. This rule
  depends on it and checks none of it again.
- **The election's own promises** under a slow start, a stale lease or a storm. Those are
  `project.shared-server-resilience`, `apps/majordomus-cli/tests/mcp_shared.rs`,
  `tests/serve_lifecycle.rs` and `test/cases/90_mcp_shared_server.sh`; this rule is about
  entry reaching that machinery, not about the machinery.

`docs/ENTRY.md` is the operator's page for all of it, and ADR 0035 the decision.
