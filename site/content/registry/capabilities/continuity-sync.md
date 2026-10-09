+++
title = "continuity.sync"
description = "Fetches the remote's refs/majordomus/continuity, merges it into the local store as the union of both (records are content-addressed, so a name holding different bytes is reported and the local copy kept), and pushes the result, never forced. Reports each line as equal, remote_newer, local_newer, diverged, local_only or remote_only. An unreachable remote changes nothing and leaves what is pending pending."
weight = 19
slug = "continuity-sync"
[extra]
id = "continuity.sync"
source = "apps/majordomus-cli/src/capability/builtin/continuity.rs"
+++
