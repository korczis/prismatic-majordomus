+++
title = "An exclusive mesh claim lives by its holder session's progress, not only by its runtime's beat"
description = "An exclusive mesh claim lives by its holder session's progress, not only by its runtime's beat"
weight = 82
[extra]
kind = "rule"
slug = "project-an-exclusive-claim-lives-by-its-holders-progress-1"
identity = "project.an-exclusive-claim-lives-by-its-holders-progress@1"
status = "active"
source = ".ai/repo/rules/project/an-exclusive-claim-lives-by-its-holders-progress.v1.md"
+++
{% raw %}

## Rationale

`project.mesh-cooperation-is-authenticated` lets a claim live while it is unreleased, its
session open and its stream's beat rose within the expiry. A stream's beat proves the runtime
is up and nothing about the worker behind one of its sessions, and on 2026-10-22 that
difference stalled the repository: the executor of seat 5d held the integration lease — an
exclusive claim — for hours and never answered, because its server beat on while the worker
waited on nobody. The owner ordered the same day that no work may stall because a peer waits
on a permission dialog, crashed or hung.

This rule adds the second layer of liveness (I3990, ADR 0067 amendment of 2026-10-23): a
session that does real work — an MCP request other than `ping` and `notifications/*`, an
announcement, a claim, or a `mesh beat` naming the phase a long-running process is in — has a
`session_beat` carrying a ttl written for it; every runtime ages the newest one on its own
clock; and an exclusive claim whose session's beat is older than its ttl (plus the expiry, on
any runtime but the holder's) is suspended: it refuses nobody until the session beats again.
Nothing is closed and nothing is taken over — suspension stops the refusal and says so, and
fencing a resumed holder waits for an epoch (I3174). A session that never beat keeps the
authentication rule's liveness, so a command-line claim and a claim of an older runtime are
judged exactly as before.

It is a rule of its own rather than a version of the authentication rule because it says
when a claim stops excluding, not who may link or how events travel; the two meet only in
`mesh::state::fold`, which applies both.

## Required behaviour

1. A `session_beat` is written only for real work: an MCP request other than `ping` and
   `notifications/*`, an announcement, a claim, or `mesh.beat` with its phase. A `ping`
   never counts, so a client stuck on a dialog behind a live bridge stops beating.
2. A runtime writes a beat only for a session that holds a live exclusive claim, at most once
   a quarter of `cooperation.session_ttl_seconds` (default 600; 0 disables), and only after
   new work.
3. `mesh::state::fold_with` suspends an exclusive claim whose session ever beat with a ttl
   above 0 while that session's newest beat is older than its ttl on the holder's runtime,
   or its ttl and the expiry on any other (`Journal::session_progress`). A suspended claim
   is not live and refuses nobody; the session and its advisory claims are untouched.
4. A suspended claim holds again at its session's next beat, unless another session's
   exclusive claim over its scope was admitted meanwhile: then the holder's runtime releases
   it with the reason `OVERTAKEN` before that beat, so two sessions never both hold one scope.
5. Over MCP, `mesh.beat` beats only for the calling session; naming another is refused
   `not_own`. The command line and plain HTTP name the session they beat for.
6. The beats a runtime writes are bounded: `mesh doctor` reports `session_beats` failing
   when a rolling hour holds more than the limit the ttl allows.

## Failure behaviour

A claim suspended by this rule is shown as `suspended` with its detail by `mesh.state`,
`mesh.peers` and the Cockpit; a beat past the hourly limit is withheld and reported by
`mesh doctor`, never silently dropped.

## Verification

`cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh` (a silent session's
exclusive claim suspended while its runtime beats, a beat within its ttl holding the claim, a
session that never beat keeping the stream rule, an overtaken claim released) and
`bash test/run.sh 3990_a_claim_lapses_when_its_session_stops_progressing` (a served runtime:
an exclusive claim over MCP held by tool calls and suspended by pings alone, an advisory claim
untouched, a claim kept by `mesh beat --phase`, an MCP beat for another session refused).
{% endraw %}
