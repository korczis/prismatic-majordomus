+++
title = "Episodes"
description = "The execution episodes this shared server holds: one per client that asked for one, opened when the client attaches, kept alive by its own traffic, detached rather than closed when the connection goes, and closed on a deliberate detach, on shutdown, or by the reaper when nothing came back. The episode boundary for a client with no provider hooks of its own (ADR 0043)."
weight = 11
slug = "episodes"
[extra]
id = "episodes"
source = "apps/majordomus-cli/src/capability/builtin/episodes.rs"
+++
