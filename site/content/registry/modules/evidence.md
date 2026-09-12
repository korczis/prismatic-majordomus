+++
title = "Evidence"
description = "What actually ran, against which commit, and whether it still proves anything. The claims matrix binds a claim to a test by path; the ledger under .ai/repo/evidence records the latest execution of every test with the commit, the tree state, the digest of the test's own source, the time, the origin and the command that runs it again. Joining the two answers, per claim, whether the repository can honestly call it proven — and distinguishes a run recorded against this very commit from one whose inputs merely have not changed since, because collapsing those two is how a green badge stops meaning anything."
weight = 13
slug = "evidence"
[extra]
id = "evidence"
source = "apps/majordomus-cli/src/capability/builtin/evidence.rs"
+++
