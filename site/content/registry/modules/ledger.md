+++
title = "Ledger"
description = "The checkout's append-only record of what happened, and its one writer. An event is validated against share/events.yaml, wrapped in the envelope every line carries — the time, the event, the commit, the branch, the writer and the episode this process resolves to — and appended under the exclusive lock every writer of the file takes."
weight = 18
slug = "ledger"
[extra]
id = "ledger"
source = "apps/majordomus-cli/src/capability/builtin/ledger.rs"
+++
