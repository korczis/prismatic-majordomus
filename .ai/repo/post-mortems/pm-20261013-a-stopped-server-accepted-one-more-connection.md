---
schema: post-mortem/v1
id: pm-20261013-a-stopped-server-accepted-one-more-connection
kind: post-mortem
title: A stopped test server accepted one more connection and reset it
description: A connection reset in tests/shared_units.rs after a server was stopped is this race, not the client under test.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: "38350251134"
    commit: 9e50ce2af3
    jobs: ["rust (3)"]
    step: "rust-check"
cause:
  class: race
  summary: The HTTP server kept accepting for a moment after it was told to stop, so a client that connected in that window was reset instead of refused.
components:
  - file:apps/majordomus-cli/src/http/server.rs
  - file:apps/majordomus-cli/tests/shared_units.rs
signals:
  - id: test
    text: "the_bridge_client_speaks_to_a_real_socket_and_names_every_failure"
  - id: panic
    text: "tests/shared_units.rs:385"
  - id: reset
    text: "did not answer: Connection reset by peer"
fixed_by: ["commit:a178e1b2bd", "pr:1078"]
prevention: ["file:apps/majordomus-cli/tests/shared_units.rs"]
tags: [rust, race, http]
---

# A stopped test server accepted one more connection and reset it

## What failed

`rust (3)` failed in `rust-check`: `the_bridge_client_speaks_to_a_real_socket_and_names_every_failure`
panicked at `tests/shared_units.rs:385` with `the shared server at http://127.0.0.1:41551
did not answer: Connection reset by peer (os error 104)`; eight other tests of the target
passed.

## Root cause

The test stops a server and expects the next connection to be refused. The server's accept
loop could take one more connection after the stop was requested and then drop it, which a
client sees as a reset. Which of the two a client got depended on timing: the session that
fixed it measured the failure in two of ten local runs before the fix.

## Fix

`a178e1b2bd` (pull request 1078): a stopped server refuses the next connection.

## Prevention

The test stays and now asserts the refusal. The same session measured two failures in seven
runs of `mcp_shared` on macOS after the fix; that is a separate candidate and is not
answered by this record.
