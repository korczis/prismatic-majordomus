---
schema: adr/v1
id: adr-0024
kind: adr
title: An orchestrator is a provider only at the bootstrap level, and its worktrees are scratch checkouts
status: accepted
date: 2026-09-09
tags:
  - architecture
  - provider
  - worktree
provenance:
  origin: authored
  derived_from:
    - decision:adr-0009
    - decision:adr-0015
    - decision:adr-0021
    - file:lib/capture.sh
    - file:share/providers/agents.tmpl
---

# 24. An orchestrator is a provider only at the bootstrap level, and its worktrees are scratch checkouts

## Context

The provider table knows three things about a tool that works in this repository: the
bootstrap file it reads, the client configuration that starts the shared MCP server for it,
and the hooks through which it hands Majordomus a prompt before the model sees it (ADR 0009)
and marks where its own episode begins and ends (ADR 0015). Claude Code has all three;
Codex and Gemini have the first two; `agents` and `generic` have a bootstrap and nothing
else. Every provider so far has been an agent: one process, one conversation, one hook
surface.

bb (getbb.app, MIT, `github.com/get-bb/bb`) is the first candidate of another shape. It is
an orchestrator: a server over SQLite, a host daemon and a UI that start *other* agents as
processes — Claude Code through `@anthropic-ai/claude-agent-sdk` with
`settingSources: ["user", "project", "local"]`, Codex through its app server, Cursor,
OpenCode, Grok and Hermes through ACP — and run several of them at once over one
repository. Its own layer is thin: `<workspace>/.bb/AGENTS.md` appended to every thread's
instructions regardless of provider (the repository-root `AGENTS.md` is explicitly not
read), `.bb/skills/`, worktree setup and teardown scripts, and TypeScript plugins whose
agent-facing surface is `contributeInstructions` and `configure`, both synchronous, both
without the prompt text, neither able to run a command. Thread events exist only behind the
server's HTTP and WebSocket API.

Two facts were established before this record was written, because the decision turns on
them and neither could be read off a page:

- Filesystem hooks survive the orchestrator. The Agent SDK documents that hooks in
  `.claude/settings.json` "run automatically in the SDK with no extra configuration" when
  `settingSources` includes `project`, which bb sets. Prompt capture and the episode boundary
  for Claude Code under bb are therefore the same shims, fired by the same events, as under
  the CLI. The SDK's MCP guide says the same of `.mcp.json`: "picked up when the `project`
  setting source is enabled", so the shared server's client configuration reaches a Claude
  Code thread under bb too, with one difference worth knowing — a server loaded from a
  settings file is given two seconds before the first turn, so `bin/majordomus-mcp` building
  the executable on a cold checkout shows `pending` at init and connects afterwards.
- The worktree topology refuses the orchestrator's checkouts. bb places a managed worktree at
  `~/.bb/plugins/environment-git-worktree/host-data/worktrees/<thread-id>/<repo>` on a fresh
  branch. A worktree created there on 2026-09-09 was reported `misplaced` with
  `worktree.path_mismatch`, and the pre-commit guard refused the commit with the remedy
  `majordomus worktree migrate` — a remedy that would move a checkout out from under a
  running bb thread. The scratch roots the topology already tolerates, the temporary
  directory and `.claude/worktrees/`, are a list compiled into `ephemeral_root_of` in
  `apps/majordomus-cli/src/worktree/service.rs`, not data a provider declares.

The question is not whether bb can be supported. It is which of the three levels an
orchestrator occupies, and who declares the paths it creates.

## Decision

An orchestrator is a provider at the bootstrap level only. It gets a row in the provider
table with a bootstrap projection and no client configuration and no hooks — the shape
`codex` and `gemini` already have — and the projection is the one line that points at
`.ai/README.md`, rendered by the same renderer as every other bootstrap
(claim `provider-projections-one-renderer`). For bb that projection is
`<workspace>/.bb/AGENTS.md`. It is not a copy of `AGENTS.md`: bb appends it behind the
instructions of every plugin, and the agent it starts loads its own native bootstrap on
top, so a second statement of any rule would be the duplication the layer forbids.

Prompt capture and the episode boundary stay with the agent the orchestrator runs. They are
the agent's hooks, they fire under the orchestrator exactly as they fire under the agent's
own CLI, and Majordomus records the provider that fired them. No adapter line is written
for the orchestrator itself, because it has no event that hands a command the prompt before
the model, and an adapter that polled its server would be a different mechanism wearing
the same name (ADR 0009's argument, unchanged).

The scratch roots a provider creates are data the provider declares, alongside its
bootstrap, and the topology reads that declaration instead of a compiled list. A checkout
under a declared scratch root is a session's scratch checkout with the standing ADR 0021
already gives one: reported, never migrated unasked, never cleaned up by the tool, and the
guard refuses a commit from it with the remedy of continuing in the canonical worktree,
not of moving the checkout. The two roots compiled in today become the first two
declarations; bb's `plugins/environment-git-worktree/host-data/worktrees` under its data
directory is the third.

## Alternatives rejected

**A full adapter over the orchestrator's API.** bb exposes every thread event over HTTP and
WebSocket, so a process could subscribe and write prompt and episode records for every
agent bb runs, ACP ones included. Rejected: it is a daemon of Majordomus's own, polling a
server, where every other capture is a shim the provider itself invokes at the moment the
event happens. The records would be produced by a second mechanism with a second failure
mode, and the ACP agents it would cover have no hook surface of their own to fall back on,
so the coverage would be real and the guarantee would not be.

**Migrating the orchestrator's worktrees into the container.** The topology has a migration
with a fingerprint check; bb's worktrees could be moved to `<repo>-wt/<branch>` the way the
32 siblings were on 2026-09-07. Rejected: bb owns those paths, records them by thread id in
its own database, and removes them when the thread is archived. A moved checkout is one bb
can no longer find, and a removed one is one the topology would report as stale. ADR 0021
already draws this line for `.claude/worktrees/`.

**Refusing managed worktrees for this repository.** bb can run a thread in the project's own
checkout instead. Rejected as the *only* answer: it is a workable operator choice, and the
bootstrap can say so, but a rule that a tool's default mode does not work here is a rule
that will be broken by the next person to install the tool. The topology has to answer
correctly for the path bb actually uses.

**Adding bb's root to the compiled list.** One more `is_inside` in `ephemeral_root_of`.
Rejected: the last hardcoded list in discovery hid nine objects in twenty-two, and the
doctrine since is run-time data over compiled knowledge. Each orchestrator would otherwise
be a code change to a module that has nothing to do with it.

## Consequences

- The provider table gains a row `bb` with bootstrap `.bb/AGENTS.md`, client configuration
  `-`, hooks `-`; `share/providers/bb.tmpl` renders the one-line pointer; the bootstrap
  projection check covers it like `AGENTS.md` and `CLAUDE.md`.
- The provider declaration grows a field for scratch roots, with the two existing roots
  moved into data under the providers that create them (`claude-code` for
  `.claude/worktrees`, the temporary directory as the tool's own) and bb's root declared
  relative to its data directory, whose default is `~/.bb` and whose override is
  `BB_DATA_DIR`. `ephemeral_root_of` reads the declarations; its tests move with it.
- The guard's remedy for a scratch checkout already names `majordomus worktree create`;
  it holds for bb without change. What changes is that a bb thread committing from its
  worktree is refused with that remedy rather than told to migrate. A bb thread that must
  commit works in the project checkout, and `.bb/AGENTS.md` says so in its one line by
  pointing at the rule.
- Prompt capture and the episode boundary are recorded against `claude-code` when Claude
  Code runs under bb, not against `bb`. A reader of the archive sees the agent, not the
  orchestrator, which is the truth of who saw the prompt. Agents bb runs over ACP remain
  uncaptured, and `capture status` says `unsupported` for them as it does today.
- One verification is still owed: a synthetic Claude Code run under an installed bb with
  `capture status` and the session shim, to prove the SDK path fires the shims in this
  repository as the documentation says it does. The other was done before the row was
  accepted: a worktree created at bb's path is reported `ephemeral`, "created by bb", and
  the guard refuses a commit from it with the remedy of continuing in the canonical
  worktree.
- Nothing here is specific to bb. The next orchestrator is a row, a template and a
  declared root.
