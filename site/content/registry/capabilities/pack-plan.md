+++
title = "pack.plan"
description = "What a profile of share/archive.yaml would pack: the files carried and their bytes and o200k_base tokens, every file left out by reason (worktree, link, artifact, binary, derived, excluded; derived counted, the rest listed), the shards it is cut into within the profile's token budget and file count, and every finding that refuses the build — a file over the budget, too many shards, a machine path or credential in a selected file, an empty selection, a profile without limits. Reads git's index and blobs only; writes nothing."
weight = 123
slug = "pack-plan"
[extra]
id = "pack.plan"
source = "apps/majordomus-cli/src/capability/builtin/pack.rs"
+++
