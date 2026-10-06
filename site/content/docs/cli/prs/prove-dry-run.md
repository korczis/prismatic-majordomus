+++
title = "majordomus prs prove-dry-run"
description = "Prove the non-mutating cycle moves nothing: snapshot origin's refs, the open pull requests, the audit trail, the lease and the local refs, run refresh, plan, drain --dry-run and cleanup (listing), snapshot again and compare; the refresh's fetched mirrors must equal what origin serves. Exit 10 naming what moved. Takes no flag: there is nothing to turn on"
weight = 93
[extra]
route = "/docs/cli/prs/prove-dry-run/"
command = "majordomus prs prove-dry-run"
source = "apps/majordomus-cli/src/cli.rs"
+++
