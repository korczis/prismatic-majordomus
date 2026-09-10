---
id: project.the-guard-refuses-an-unready-episode
version: 1
kind: rule
title: An agent does not silently keep mutating a repository whose readiness cannot be established
description: Readiness is asserted again before every mutation and not only on entry — the provider's pre-tool event asks whether the shared server is serving this checkout and an episode is open, refuses the tool call when it is not, and says out loud and lets through everything it cannot decide; the event is declared as provider data, the switch is declared wherever a key of the policy must be, and the answer is remembered so the question costs about what starting the tool costs.
statement: A mutation of this repository is preceded by a readiness question that can answer no; a state the guard cannot decide is named on standard error and never refused; the event that carries it is provider data and the switch that arms it is a key of the policy.
status: active
class: blocking
depends_on: [project.entry-converges@1, project.providers-are-data@1, project.shared-server-resilience@1]
tags: [entry, enforcement, mcp, providers]
---

# Rationale

`project.entry-converges` made entering this repository as an agent converge on a ready
shared server: the provider's start event opens the episode, `serve ensure` makes sure a
server is serving this checkout, and the briefing names where it stands. That is one
assertion, at one moment, and nothing asserted it again.

An episode is hours long. In those hours a server is killed by somebody reclaiming a port,
dies with the terminal that started it, ends because no peer attached for its idle life, or
goes `outdated` the moment its executable is rebuilt under it. None of that is visible to
the worker. It keeps calling Edit and Write; the peer board it would have read is not
answering, the index it would have searched is not there, and the one line that would have
told it so was printed at the start of the episode and is now thousands of tokens behind
the context window. The mandate this rule closes put it plainly: an agent must not silently
continue repository mutation when required readiness cannot be established. Before this
rule, `.claude/settings.json` wired four provider events and none of them ran before a
tool.

The obvious implementation is worse than the problem. A hook that can refuse a tool call is
tracked, and a tracked hook reaches every worktree of this repository at once, including
the sessions that would fix it — a wrong determination there is a fleet that cannot edit
its way out. So the shape of the thing is as much of this rule as the existence of it: what
it refuses is bounded, what it cannot decide is never a refusal, it is off unless a policy
key turns it on, and its cost is bounded because it runs in front of every edit rather than
once.

The word is the repository's own. `project.worktree-topology` already calls the pre-commit
check that refuses a branch committed from the wrong worktree a guard; this is the same
kind of thing one event earlier, and it is called the same thing.

# Required behaviour

**The pre-tool event exists and is provider data.** The provider's row in
`MJ_CAPTURE_LIFECYCLE` declares the event, its shim, the matcher naming the tools that
mutate the repository, and the payload keys naming the tool that is about to run — in
columns, exactly as the compaction event was added as a later column. Nothing about the
guard is written a second time in code: the configuration writer and the guard read the one
matcher, so a matcher that drifts cannot leave them disagreeing about what is guarded. A
provider without such an event has no guard, and that is not a failure of this repository.

**It is wired where the reconciliation can see it.** `capture install` writes the shim and
names the event in the provider's configuration; the configuration this repository ships
declares a `PreToolUse` entry whose matcher covers the mutating tools and whose command is
the shim. `capture status` and `majordomus doctor` reconcile it through the lifecycle
aspect that already reconciles the other three — the enforcement entry wired by
`provider-hook:<provider>:session` — and not through a second mechanism. A guard that is
declared and unwired must read as unwired.

**The shim can only ever say yes or no.** It runs the command rather than handing the
process over to it, passes on exactly one non-zero code — the refusal — and exits 0 for
everything else, including anything the command can die of. Every other exit code a shim
invents is a tool call somebody cannot make.

**The decision refuses only what it knows.** In order: a tool that does not mutate the
repository is not refused; a remembered answer that is still good is honoured; otherwise
the canonical reading — `serve status --checkouts this`, which opens one lease, probes one
server and enumerates no other checkout — decides where the server stands, and the episode
store decides whether an episode is open for this provider session. A server that is not
ready is given exactly one chance to become ready, through the same `serve ensure` the
start event runs, bounded. Only then is the tool call refused, with the standing, the
remedy and the switch named on standard error.

**What it cannot decide it says out loud and lets through.** An executable that is not
built or is older than its sources, a payload that does not parse, a policy that does not
load, a probe that does not answer: each is reported on standard error, where the provider
records it against the tool call, and each exits 0. It is reported and not fed back: the
provider hands standard error to the model on a refusal and to the transcript otherwise,
which is the right asymmetry — a state the guard could not read is a fact about this
machine, and interrupting a worker with it would be the second failure. This is the boundary, and it is
deliberate. Refusing on an undecidable state would mean that a checkout where nobody has
run `just build` cannot be edited, which is a worse failure than the one the guard
prevents, and it contradicts what entering this repository already does — it names a
missing executable rather than building one. It is not the silent fallback the repository
forbids elsewhere, because it is not silent: saying nothing here is the defect, not the
exit code.

**The answer is remembered, and the memory costs about what starting the tool costs.** The
guard runs in front of every Edit, Write and Bash, so the reading it stands in front of may
not be paid every time. The remembered answer lives in the checkout's local state and is
read without resolving the layout, parsing the policy or starting a process. Measured on
2026-09-11, thirty runs each: 81 ms for the remembered answer, against 50 ms for
`majordomus version` — this tool's floor, a process start and one library — and 1.6 s for
the reading it stands in front of. A `pass` ages out; an `off` does not age, because the switch is
not a fact about a running server. Both are invalidated by the policy file being newer than
the memory, so moving the switch takes effect on the next tool call and not when something
expires. A refusal is never remembered: between two edits a server can be started, and
asking again costs one reading.

**The switch is a key of the policy, declared in four places or in none.**
`session.guard_before_mutation`, in this repository's policy, in the skeleton a new
repository is written from, in the allow list and in the schema — a key the schema does not
know is refused at run time (`project.unknown-keys-are-errors`), and one the skeleton lacks
exists only here. It is `false` in both policies, and a policy written before the key
existed reads as false too: the guard is armed per repository, deliberately, by somebody
who has watched it run.

# Failure behaviour

`scripts/ci/guard-refuses-unready`, registered as the `guard-refuses-unready` gate in
`.ai/repo/ci/gates.yaml` and selected by the same classes that select `entry-converges`
plus the one that owns the provider's configuration, reports one line per check and exits
10 on any finding. It decides, mechanically:

- that the provider's lifecycle row declares the pre-tool event, its shim and a matcher,
  and that the event table has a row for it — the guard removed from the adapter table is
  the change this gate exists to catch;
- that the wired configuration declares that event, with a matcher naming the mutating
  tools, and names the shim;
- that the shim exists, is executable, delegates to `capture guard`, and passes on the
  refusal alone rather than exec-ing the tool;
- that `session.guard_before_mutation` is declared in the policy, the skeleton, the allow
  list and the schema, and that the guard reads it before refusing anything;
- that the decision is what the rule says it is: it reads `serve status`, it asks
  `serve ensure` once, it remembers a verdict, and it names an undecidable state rather
  than refusing on it.

No validator of this rule lives in `lib/`. Which provider fires a pre-tool event, and where
this repository declares its policy, its skeleton and its schema, are facts about *this*
repository; a validator in `lib/` would teach the shipped executable one repository's
layout. `.ai/repo/rules/project/README.md` is where that distinction is written down.

# Verification

`test/cases/124_the_guard_refuses_an_unready_episode.sh` proves the behaviour, in a
repository `init` wrote, driving the real shim with the payload shape the provider sends.
It proves both directions with the same weight, because the switch is off and the off path
is therefore the one every session in this repository actually runs:

- with the switch off, every tool — mutating or not — is let through, with no server in the
  checkout at all, and seven malformed payloads cost nobody a tool call;
- with the switch on, a ready server and an open episode pass; an episode that was never
  opened is refused with exit 2 and the cause named; a tool that mutates nothing is never
  refused and never even decided;
- a server killed under the episode, lease and all, is *healed* rather than refused — the
  case asserts which by comparing the process the lease named before and after;
- a server that `ensure` cannot fix is refused, with the standing and the switch named;
- an executable that is not there is named on standard error and exits 0;
- the warm path is proved by counting: the executable is replaced by a spy that records
  every call, and five remembered answers add no line to its log;
- moving the switch takes effect on the next call, proved by establishing a `pass` and
  asserting the next call decided again.

`test/cases/118_entry_converges_by_rule.sh` is the model for proving a gate by mutation;
this rule's gate is proved the same way inside case 124's own fixture assertions and by the
gate's self-check on this tree. What no script can decide — whether the guard's refusal is
the right call in a repository somebody is actually working in — is why the switch exists
and why it is off.

`docs/ENTRY.md` is the operator's page: the sequence, what a refusal looks like to the
agent, how to turn it on, and what it costs.
