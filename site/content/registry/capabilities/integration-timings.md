+++
title = "integration.timings"
description = "How long CI takes on the open pull requests, measured from the recorded forge observation: for every check, how many of its runs started and completed, how many passed and failed, and its median, 90th percentile (nearest rank) and longest duration in seconds, the slowest median first; for every open head with a measured check, the wall time from its first check's start to its last check's completion and the check that took longest, the longest wait first; and how many runs could not be measured (still running, queued, or recorded before starts were read), which are counted and never measured as zero. A read: it asks the forge for nothing; `majordomus prs refresh` records the observation. `observed: false` with the reason when this checkout has recorded none."
weight = 78
slug = "integration-timings"
[extra]
id = "integration.timings"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
