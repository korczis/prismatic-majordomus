+++
title = "evidence.record"
description = "Reads what the runs already wrote — the suite's TSV report, cargo test's output — stamps each result with the provenance the run itself did not carry (the commit, the tree state, the digest of the test's own source, the time, the origin) and merges it into the ledger. It records; it decides nothing: a case that failed is a case the runner said failed, and a test no report named is left exactly as it was, so recording one case never erases the evidence for the rest. A tree with no commit to name is refused, because an execution with no commit proves nothing."
weight = 27
slug = "evidence-record"
[extra]
id = "evidence.record"
source = "apps/majordomus-cli/src/capability/builtin/evidence.rs"
+++
