---
schema: adr/v1
id: adr-0043
kind: adr
title: Every client gets an episode, and every provider capability cites its evidence
status: accepted
date: 2026-09-11
tags:
  - architecture
  - session
  - providers
  - mcp
  - governance
related:
  - file:.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md
  - file:.ai/repo/adrs/0024-an-orchestrator-is-a-provider-only-at-the-bootstrap-level-an.md
  - claim:connection-episode
  - claim:provider-capability-evidence
  - rule:project.a-provider-capability-cites-its-evidence
  - file:share/providers.yaml
  - file:apps/majordomus-cli/src/episodes.rs
  - file:lib/capture.sh
  - file:test/cases/200_generic_mcp_episode.sh
provenance:
  origin: authored
---

## Context

Majordomus draws the episode boundary below the model. The argument is in
`lib/capture.sh` and it is a good one: a worker told to open a session opens one when it
remembers to, which is never the episode that mattered — the one that ended in a crash, a
compaction, or somebody closing the window. The provider knows, it fires an event, and
the boundary is drawn there or it is fiction.

Measured on 2026-09-11, that was wired for one provider, and the tool's own account of
the situation was wrong in two directions at once.

**It reported one provider out of six.** `share/providers.yaml` declares `agents`, `bb`,
`claude-code`, `codex`, `gemini` and `generic`. `majordomus capture status` looped over
`mj_capture_providers` — the providers with a line in the *prompt adapter table* — and
printed two rows, both `claude-code`. The five others were not reported as unsupported;
they were not reported at all. The vocabulary had a word for them, `unsupported`, whose
own definition was "no adapter: no documented event hands a command the prompt before the
model runs" — and because the loop was over the adapter table, no provider a person could
ask about could ever reach it. A state that cannot be printed is not a state. Somebody
asking "can Codex capture prompts in this repository" got silence, and silence reads as
no.

**And the silence had become false.** The comment above the adapter table says a provider
absent from it "is unsupported: it has no documented event that hands the person's prompt
to a command before the model runs". That was a true statement about the world when it
was written and is not one now:

| | session boundary | prompt before the model |
|---|---|---|
| Claude Code | `SessionStart`, `SessionEnd`, `PreCompact`, `PostCompact` | `UserPromptSubmit` (`prompt_text`) |
| Codex CLI | `SessionStart`, `SessionEnd`, `PreCompact`, `PostCompact` | `UserPromptSubmit` (`prompt`) |
| Gemini CLI | `SessionStart`, `SessionEnd`, `PreCompress` | `BeforeAgent` (`prompt`), `BeforeModel` |

All three verified against vendor documentation on 2026-09-11; the citations are in
`share/providers.yaml`, one per cell. Two products had shipped hook systems and this tool
went on implying, by omitting them, that they had none. The omission was a statement
about somebody else's product that the tool had no evidence for and had never been asked
to defend, because nothing in the declaration ever asked "where did you verify that".

**The third fact is the one the mandate was about.** Even with adapters for all three,
episode continuity would be a privilege of tools that happen to ship hooks. A client with
none — the `generic` provider, "a worker with no convention of its own" — would have no
boundary at all, and the repository's claim to draw the boundary below the model would
remain a claim about whichever vendors had obliged.

## Decision

**One. A provider capability is declared as data, with a citation per cell.**

`share/providers.yaml` grows a `lifecycle:` block per provider: `episode`
(`hooks` | `connection` | `none`), `prompts` (`hook` | `none`), and an evidence string for
each — a vendor documentation URL, or the statement that no such documentation exists
together with what was searched for it. The words deserialise into enums, so a value
outside the vocabulary fails the read of the file with its path named rather than
degrading to `none`.

Every surface is a projection of that one declaration. `share::ProviderDeclaration`
carries it, `product::ProductProvider` carries it, and from there the command line
(`majordomus product providers`), the HTTP route (`/api/v1/product/providers`), the MCP
tool and resource (`majordomus_providers`, `majordomus://product/providers`), the
generated `docs/generated/providers.{json,yaml}` and the site's product dataset all follow
without a line of their own. Adding a provider or changing a cell is one edit and a
`majordomus generate`.

**Two. The capability is not the wiring, and `capture status` reports both.**

Six words now, not five. `unsupported` is reserved for a provider that *cannot* do the
thing, and the declaration's citation is the reason it prints. `unadapted` is new and is
the honest name for the state Codex and Gemini are in today: the provider documents the
event, this distribution ships no adapter line, and the gap is in the tool. Collapsing the
two — which is what the code did — made this tool say something untrue about somebody
else's product. `unconfigured`, `named`, `wired` and `verified` are unchanged.

`capture status` loops over the union of the declared providers and the adapter tables, so
every provider is reported, with its declared capability in one column and this checkout's
state in another.

**Three. A client with no hooks gets its episode from the connection.**

For `episode: connection`, the MCP connection *is* the boundary:

```
initialize → peer attaches  (no episode: a reader is not a worker)
episodes.attach(external_id) → the episode opens, or the client's own is resumed
  … every message the client sends is that episode's heartbeat …
connection lost           → detached, not closed
episodes.attach again     → resumed, same episode, new peer
episodes.detach           → closed, deliberately
no reconnect in 15 min    → closed by the reaper, as interrupted
server stops              → closed, shutdown
```

`apps/majordomus-cli/src/episodes.rs` holds the board. It does not write a session
record: it runs `majordomus capture session --provider generic --event start|end`, the
same command a provider hook's shim runs, with the same payload shape, through the same
reader. `lib/capture.sh` admits a provider whose declared episode source is `connection`
without an adapter line, because a provider that fires no event has no vendor payload
shape to adapt to and nothing to install.

**Four. A peer is not an episode, and an episode is keyed by an external identity.**

A peer id (`p1`, `p2`, …) is this process's name for this connection. It is handed out in
attachment order, reused by the next server, and is not a worker identity — on 2026-09-09
a session in this repository announced, was re-established under a new id, and was
invisible to eight others for three hours because everybody treated it as one. So an
episode is keyed by an identity the *client* supplies and can reproduce, the peer holding
it is a mutable field, and the invariants are: a connection holds zero or one episode;
`initialize` alone holds none; a reconnecting client re-associates with its own episode by
name; and an episode outlives its connection for a bounded grace, because a dropped socket
is the most ordinary thing that happens to work in progress and is not the end of it.

**Five. What is not claimed.**

Raw prompt capture stays provider-specific. An MCP server is handed `initialize`, tool
calls and notifications; the person's prompt is never among them, in any version of the
protocol. `generic.lifecycle.prompts` is `none` with that reasoning written into its
evidence field, and there is deliberately no `connection` value under `prompts` for any
provider to reach for.

## Alternatives rejected

**Open an episode on `initialize`.** It would make "every client gets an episode" literally
true and it is wrong twice. A client that opens the server to read one rule is not a
worker and should leave no record; and `test/cases/90_mcp_shared_server.sh` asserts that
serving changes the repository not at all, which is a guarantee worth more than the
convenience. Attach is one call and the client that wants an episode is the client that
knows it wants one.

**Key the episode by the peer id.** The reason this is wrong is written above and was paid
for in hours rather than argued.

**Close the episode when the connection drops.** It would publish a record saying the work
ended every time a socket did. The reattach grace is fifteen minutes against the
transport's ninety-second idle timeout, and they are two clocks because they answer two
questions: is this socket alive, and is this work over.

**Write the session record from the Rust executable.** A second writer of the record the
provider hooks already write is the repeated semantic definition ADR 0004 forbids. The
board runs the one command that already does it.

**Ship adapters for Codex and Gemini in this change.** They are real and they are a
different piece of work: each needs a writer for a configuration format this tool does not
write today (a TOML `[hooks]` table, a JSON `hooks` block), and each carries caveats an
adapter must encode — Gemini does not await `SessionEnd`, its `PreCompress` cannot block,
Codex's `SessionEnd.reason` is a constant. Declaring the capability with its citation and
reporting `unadapted` names the gap accurately today; implementing it while guessing at it
would not.

## Consequences

* `capture status` reports twelve rows, not two, and every one carries the evidence behind
  it. Two of them say `unadapted`, which is a standing, visible gap in this distribution
  rather than a silence.
* `majordomus product providers` and every projection of it gain the capability matrix.
  Nothing had to be edited surface by surface, which is the point.
* A generic MCP client — anything that can be pointed at `bin/majordomus-mcp` — gets an
  episode, a working context, a briefing and a closed record, with no vendor cooperation.
* The new blocking invariant is `project.a-provider-capability-cites-its-evidence`: a
  declared capability without a citation is refused. Its validator is
  `scripts/provider-evidence-check` and `test/cases/200_generic_mcp_episode.sh` drives the
  whole lifecycle over the real transport.
* Two clocks now govern a client's disappearance, and a reader of the logs will see a
  connection reaped long before the episode it was carrying. That is intended and is
  written down here because it will otherwise look like a leak.
