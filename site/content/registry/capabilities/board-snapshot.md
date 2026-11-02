+++
title = "board.snapshot"
description = "Every document of the delivery board by id: meta (the commit, the lanes, the issue counts, the release), milestones in roadmap order, intents, the pull request queue from the last recorded observation, and the open issues of each lane as issues-<lane>, split into issues-<lane>-2 and onwards so that no document serializes past 250000 bytes. The digest is a SHA-256 over the canonical JSON of the documents with meta.syncedAt and meta.syncedBy left out, so equal state is an equal digest. A milestone that is not DONE and is in no lane of share/board.yaml is refused by name, never filed under other. An unobserved queue is said in prs.unavailable and the diagnostics, never refused. Mesh seats, live work, CI runners and blockers are the sync step's and are not here. Reads only; nothing is cached."
weight = 3
slug = "board-snapshot"
[extra]
id = "board.snapshot"
source = "apps/majordomus-cli/src/capability/builtin/board.rs"
+++
