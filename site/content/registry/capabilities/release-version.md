+++
title = "release.version"
description = "The version the crate manifest declares — the one place it is authored — the version the shell tool prints from its projection `share/version.txt`, and whether that projection is current — the question `generate --check` refuses and `scripts/release-version --check` gates on. Then the bump the conventional commits since the last release imply, the version it would produce, and the commits themselves as the evidence for it."
weight = 92
slug = "release-version"
[extra]
id = "release.version"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
