+++
title = "monitor.stream"
description = "A page of signals after a cursor, oldest first, or without one the newest of the last `since` seconds: each with its time, source, kind, severity, machine, session, actor, pull request, branch, issue, paths, a one-line summary and whether it is for the reader. Re-projections and undecided steps are counted as `suppressed`; a source that cannot be read is listed with its reason, never answered as empty. The page's `cursor` is the next page's `after`, and nothing on this page comes back. Reads only."
weight = 118
slug = "monitor-stream"
[extra]
id = "monitor.stream"
source = "apps/majordomus-cli/src/capability/builtin/monitor.rs"
+++
