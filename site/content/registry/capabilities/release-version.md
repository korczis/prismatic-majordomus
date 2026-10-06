+++
title = "release.version"
description = "The version the crate manifest declares — the one place it is authored — the version the shell tool prints from its projection `share/version.txt`, and whether that projection is current — the question `generate --check` refuses and `scripts/release-version --check` gates on. Then the next version, which is the public contract's answer: the version `release analyze` requires against the last release — the declared version when it already satisfies the contract, otherwise the smallest one it allows, and the smallest release above the last when the contract requires none over it — with `decided_by` naming who answered. It is the one release selection `release bump` also raises to when no target is named, so the report and the writer cannot disagree. When the contract cannot be measured — no published baseline, a version that is not three numbers, or any other error that makes the plan unsound, the plan `release bump` refuses to write from — the baseline is refused, not guessed: `next` is absent, `decided_by` is `undecided` and `contract_unreadable` carries every error, one per line. The bump the conventional commits since the last release imply, the version it would produce and the commits themselves are carried beside it as evidence, and never answer `next` in the contract's place."
weight = 145
slug = "release-version"
[extra]
id = "release.version"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
