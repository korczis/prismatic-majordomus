+++
title = "machine.sweep"
description = "Walks every cargo cache home of this machine ($CARGO_HOME, ~/.cargo and each ~/.asdf/installs/rust/*, or the homes named): every directory under build-private/, every shared build directory's llvm-cov-target, and every incremental session. Each gets a verdict: unmeasurable when its age cannot be read, recent when written within the idle threshold (120 minutes, 30 for sessions), in_use when a running process names it (any coverage run, for a coverage target; every path when the process table cannot be read), held when its cargo lock cannot be taken, and otherwise reclaimable. A check by default; asked to reclaim, it asks each reclaimable directory again, renames it out of cargo's reach and removes it. Below the floor (30 GiB free by default) everything is swept at 30 minutes. Answers with the free space before and after, every candidate and the bytes reclaimed."
weight = 103
slug = "machine-sweep"
[extra]
id = "machine.sweep"
source = "apps/majordomus-cli/src/capability/builtin/machine.rs"
+++
