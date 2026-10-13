+++
title = "A stopped test server accepted one more connection and reset it"
description = "A stopped test server accepted one more connection and reset it"
weight = 2
[extra]
kind = "post-mortem"
slug = "pm-20261013-a-stopped-server-accepted-one-more-connection"
identity = "pm-20261013-a-stopped-server-accepted-one-more-connection"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-a-stopped-server-accepted-one-more-connection.md"
+++
{% raw %}

## A stopped test server accepted one more connection and reset it

### What failed

`rust (3)` failed in `rust-check`: `the_bridge_client_speaks_to_a_real_socket_and_names_every_failure`
panicked at `tests/shared_units.rs:385` with `the shared server at http://127.0.0.1:41551
did not answer: Connection reset by peer (os error 104)`; eight other tests of the target
passed.

### Root cause

The test stops a server and expects the next connection to be refused. The server's accept
loop could take one more connection after the stop was requested and then drop it, which a
client sees as a reset. Which of the two a client got depended on timing: the session that
fixed it measured the failure in two of ten local runs before the fix.

### Fix

`a178e1b2bd` (pull request 1078): a stopped server refuses the next connection.

### Prevention

The test stays and now asserts the refusal. The same session measured two failures in seven
runs of `mcp_shared` on macOS after the fix; that is a separate candidate and is not
answered by this record.
{% endraw %}
