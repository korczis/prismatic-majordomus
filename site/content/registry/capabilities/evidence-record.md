+++
title = "evidence.record"
description = "Reads what the runs already wrote — the suite's TSV report, cargo test's output, a coverage summary — carries into each result what that run measured about its own checkout (the commit and the tree, from the measurement `evidence stamp` wrote), with the digest of the test's own source, the time and the origin, and merges it into the ledger. It records; it decides nothing: a case that failed is a case the runner said failed, and a test no report named is left exactly as it was, so recording one case never erases the evidence for the rest. It refuses a tree with no commit, a measurement of another commit or of another report, a local measurement that names no report, and a CI report without a measurement; a local report recorded without one carries the tree unknown, which never reads proven. Every recording returns a run record (its commit, the tree each report's run measured, its totals and what was absent, dropped or unknown, and a coverage summary bound to its commit), and --ledger local writes only the ignored local ledger. A crate binary that ran no test, or only a filtered subset, is recorded as a skip, and what no claim can name yet (the crate's own unit tests, its doctests) is listed as dropped."
weight = 44
slug = "evidence-record"
[extra]
id = "evidence.record"
source = "apps/majordomus-cli/src/capability/builtin/evidence.rs"
+++
