+++
title = "integration.timings"
description = "How long CI takes on the open pull requests, measured from the recorded forge observation: for every check, how many of its runs started and completed, how many passed and failed, and its median, 90th percentile (nearest rank) and longest duration in seconds, the slowest median first; for every open head with a measured check, the wall time from its first check's start to its last check's completion and the check that took longest, the longest wait first, with its critical path (the chain of checks back from the last completion, each the one that completed last before the next started, inferred from the times because the observation holds no job graph; the longest check of that chain, its seconds and its share of the wall time to two decimals) and, for every matrix family of checks named `<family> (<values>)`, its shards, slowest shard, slowest and fastest seconds and their spread (slowest over fastest, to two decimals; null when the fastest took 0 s), the most uneven deal first; and how many runs could not be measured (still running, queued, or recorded before starts were read), which are counted and never measured as zero. A read: it asks the forge for nothing; `majordomus prs refresh` records the observation. `observed: false` with the reason when this checkout has recorded none."
weight = 95
slug = "integration-timings"
[extra]
id = "integration.timings"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
