+++
title = "evidence.stamp"
description = "The one measurement a runner takes at the end of its run, for evidence record --provenance: the commit; the tree, ignoring the evidence ledger and the run's own untracked outputs, never a tracked change; the producer; its toolchain; the recorder's version; the report's digest; the host; the CI run. It writes nothing. It is offered only on the command line because it reads a path its caller names."
weight = 38
slug = "evidence-stamp"
[extra]
id = "evidence.stamp"
source = "apps/majordomus-cli/src/capability/builtin/evidence.rs"
+++
