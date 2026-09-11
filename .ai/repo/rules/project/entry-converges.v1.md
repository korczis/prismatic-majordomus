---
id: project.entry-converges
version: 1
kind: rule
title: Entering as an agent converges on a server; entering as a shell reports
description: An agent entering the repository is owed a ready shared server before its first attach, and gets one through the provider's start event without a command being typed; a shell entering the repository is told where the server stands and starts nothing; nothing on entry builds the executable; what a worker is told on entry is derived from the same typed reading every other surface uses; and a server no client owns ends by itself.
statement: Entry by an agent converges on a ready shared server through the provider's start event, entry by a shell reports and serves nothing, nothing on entry builds the executable, the briefing has one author, and a server nobody owns has a bounded life.
status: active
class: blocking
depends_on: [project.envrc-is-an-adapter@1, project.shared-server-resilience@1, project.interfaces-are-projections@1, project.the-lease-is-read-once@1]
tags: [entry, mcp, environment, enforcement]

x-majordomus:
  tests: [test/cases/118_entry_converges_by_rule.sh, test/cases/108_entry_converges_on_a_server.sh]
---

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
notice. A `serve ensure` can be added to `.envrc` by whoever reads "entry converges" and
misses that a shell is not a client — which is exactly the change ADR 0003 refuses and
`project.envrc-is-an-adapter` was written against, and which that rule's own word list
never covered: it forbids `git`, `grep`, `curl` and `cargo`, and says nothing about the
tool's own `serve`. A build can be added to either entry path by whoever is tired of
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

**Entry by a shell reports and does not serve.** The file a shell evaluates on entry, and
the adapter it calls, may not run the tool's `serve` or `mcp`, invoke a builder, or
background a process. The reason is not taste: that file runs on every `cd`, on every
machine, in whatever shell somebody happens to use, and ADR 0003 refuses a process without
a client — a shell is not a client. `project.envrc-is-an-adapter` says what the entry point
may read; this says what it may start.

**Nothing on entry builds the executable.** Both entry paths — the shell adapter and the
provider's start event — resolve the executable without building it. A missing one, or one
older than its sources, is named on standard error with the recipe that builds it, and the
entry still exits zero. A server started from stale code would answer with a tree that is
no longer there, which is why the start event names it rather than starting one.

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
- that `session.ensure_server_on_start` is declared in the policy, the skeleton, the allow
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
tree rather than this checkout: a `serve ensure` added to the entry point, a builder added
to either entry path, a backgrounded server, the switch dropped from the schema, a shim
that stops dispatching the start event, a start path that stops ensuring or starts
building, a briefing that asks the server for its own standing, an idle life of zero
and a spawn that drops `--idle` are each planted and each refused with the cause named; the
same fixture unmutated is accepted.
The case also asserts that the gate is declared in `.ai/repo/ci/gates.yaml` and selected by
the classes that can move it, because a gate no plan selects is prose with an exit code.

What the gate deliberately does not decide, and a reviewer does:

- **That entry actually converges.** No script can enter a repository as an agent. That is
  `test/cases/108_entry_converges_on_a_server.sh`, which drives the provider's shim with
  the payload the provider would send, in a repository `init` wrote: a first start ensures
  a ready server and names it in the briefing, a second finds the same one and starts
  nothing, `serve status` agrees, `serve stop` ends it, the switch turned off says so and
  starts nothing, and a missing executable is named while the event still exits zero. The
  gate runs that case unless it is given `--no-suite`.
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
