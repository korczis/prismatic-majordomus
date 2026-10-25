---
schema: adr/v1
id: adr-0224
kind: adr
title: No ordinary command builds; a missing or stale executable fails fast or answers through a release of the same generation, and only an explicit build builds
status: accepted
date: 2026-10-22
tags:
  - launcher
  - build
  - enforcement
supersedes:
  - adr-0172
related:
  - file:.ai/repo/adrs/0172-a-client-always-gets-a-server-and-a-session-starts-by-joining.md
  - file:.ai/repo/project/intents/every-capability-has-one-owner-and-the-cli-never-builds.yaml
  - file:bin/majordomus-cli
  - file:lib/rust_bin.sh
  - file:.ai/repo/ci/gates.yaml
  - test:test/cases/3177_a_client_always_gets_a_server.sh
  - test:test/cases/3221_a_bare_session_start_joins_the_cluster.sh
provenance:
  origin: authored
---

# 222. No ordinary command builds; a missing or stale executable fails fast or answers through a release of the same generation, and only an explicit build builds

## Context

On 2026-10-22 a pre-push on a loaded machine sat forty minutes in one gate and then timed out at
ninety, all of it queued behind the shared build lock, with the tree's executable already built:
nine gates of `.ai/repo/ci/gates.yaml` run `cargo run --manifest-path apps/majordomus-cli/Cargo.toml`
and ignore `MAJORDOMUS_BIN`. A critique of the same day counted at least eight entry paths that can
start a cargo build or pass silently on a missing or stale executable: `bin/majordomus` handing words
to `bin/majordomus-cli`, `lib/plan.sh`, every just recipe through the bridge, the git hooks (a stale
executable is rebuilt there), `bin/majordomus-mcp`, `test/lib.sh` with a raw `cargo build` outside the
disk floor and lock, the edit guard failing open, and those gates. ADR 0172 decided that a stale
executable which can be rebuilt is rebuilt, and that the installed-release fallback serves `mcp`
only; I3180 planned a background build on entering a checkout. The owner, the same day, after the
architecture review recorded as intent every-capability-has-one-owner-and-the-cli-never-builds:
"ADR 0172 is replaced, I3180 is cancelled".

## Decision

1. **An ordinary command never starts a build.** A missing or stale executable makes every command
   that does not declare a toolchain requirement either exit 12 naming the explicit build, or answer
   through a fallback (decision 2). Building is the explicit build command and the derive, nothing
   else; a gate, a hook, a recipe, a test harness and the MCP launcher call the tree's executable
   and never `cargo run`.
2. **A fallback is a release of the same generation.** The installed release answers only when its
   build generation matches the tree (rule project.a-tool-that-cannot-match-the-tree-refuses); a
   version string is not a build. With a matching release every command may use it, not `mcp`
   alone, because it answers what the tree's own executable would.
3. **Every caller declares its outcome.** Each caller class — hook gate, provider hook, mcp, entry,
   person — declares refuse, skip with a recorded line, or verified fallback for a missing or stale
   executable, and none passes silently.
4. **Entering a checkout never builds** (rules entry-converges and envrc-is-an-adapter): I3180 is
   cancelled.
5. **Carried forward from ADR 0172, unchanged:** a client always gets a server when one can be
   given; a bare session start is handed the repository's own first message
   (`.ai/repo/prompts/session-start.md`, `share/shell/session-start.sh`) with its opt-outs; and
   autonomy is bounded in that prompt, not assumed.

## Consequences

- A push stops compiling in its gates; a loaded machine queues one build per tree, not one per gate.
- A contributor whose executable is stale is told the one command to run instead of waiting on a
  hidden build; a matching installed release keeps every client working.
- I3957 carries the first slice: the launcher, `lib/rust_bin.sh`, `test/lib.sh`, the just bridge and
  the gates, proved by a case that counts cargo invocations across every path.
