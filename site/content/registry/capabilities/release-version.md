+++
title = "release.version"
description = "The version the crate manifest declares — the one place it is authored — the version the shell tool prints from its projection `share/version.txt`, and whether that projection is current — the question `generate --check` refuses and `scripts/release-version --check` gates on. Then the next version, which is the public contract's answer: the version `release analyze` requires against the last release — the declared version when it already satisfies the contract, otherwise the smallest one it allows, and the smallest release above the last when the contract requires none over it — with `decided_by` naming who answered. The bump the conventional commits since the last release imply, the version it would produce and the commits themselves are carried beside it as evidence; they answer `next` only when the contract cannot be measured, and `decided_by` and `contract_unreadable` then say so."
weight = 126
slug = "release-version"
[extra]
id = "release.version"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
